use std::ops::Mul;
use ash::vk;
use ash::vk::{DescriptorBufferInfo, DeviceSize, PushConstantRange, ShaderStageFlags, WriteDescriptorSet};
use bytemuck::{Pod, Zeroable};
use cen::graphics::pipeline_store::{PipelineConfig, PipelineKey};
use cen::graphics::Renderer;
use cen::graphics::renderer::RenderComponent;
use cen::vulkan::{Buffer, CommandBuffer, DescriptorSetLayout, Image};
use egui::debug_text::print;
use glam::{IVec4, Mat4, Vec3, Vec4};
use gpu_allocator::MemoryLocation;

pub struct GraphRenderer {
    image: Option<Image>,
    descriptorset: Option<DescriptorSetLayout>,
    pipeline: Option<PipelineKey>,
    edge_pipeline: Option<PipelineKey>,
    transform: Option<Mat4>,
    buffer_info: Option<DescriptorBufferInfo>,
    edge_buffer_info: Option<DescriptorBufferInfo>,
    edge_descriptorset: Option<DescriptorSetLayout>,
    node_count: Option<u32>,
    edge_count: Option<u32>,
}

#[derive(Copy)]
#[derive(Clone)]
pub struct RenderNode {
    pub p: Vec3,
    pub v: u32
}

#[derive(Pod, Zeroable)]
#[repr(C, packed)]
#[derive(Copy)]
#[derive(Clone)]
struct PushConstants {
    transform: Mat4,
    nodes: u32,
}

impl GraphRenderer {
    pub fn new() -> GraphRenderer {
        GraphRenderer {
            node_count: None,
            edge_count: None,
            buffer_info: None,
            edge_buffer_info: None,
            image: None,
            descriptorset: None,
            pipeline: None,
            edge_pipeline: None,
            edge_descriptorset: None,
            transform: None,
        }
    }

    pub fn transform(&mut self, transform: Mat4) {
        self.transform = Some(transform);
    }

    pub fn graph_data(&mut self, node_count: usize, buffer_info: DescriptorBufferInfo, edge_count: usize, edge_buffer_info: DescriptorBufferInfo) {

        self.node_count = Some(node_count as u32);
        self.buffer_info = Some(buffer_info);
        self.edge_count = Some(edge_count as u32);
        self.edge_buffer_info = Some(edge_buffer_info);
    }
}

impl RenderComponent for GraphRenderer {
    fn initialize(&mut self, renderer: &mut Renderer) {

        // Image
        let image = Image::new(
            &renderer.device,
            &mut renderer.allocator,
            renderer.swapchain.get_extent().width,
            renderer.swapchain.get_extent().height,
            vk::ImageUsageFlags::STORAGE | vk::ImageUsageFlags::TRANSFER_SRC | vk::ImageUsageFlags::TRANSFER_DST
        );

        // Transition image
        let mut image_command_buffer = CommandBuffer::new(&renderer.device, &renderer.command_pool, false);
        image_command_buffer.begin();
        {
            renderer.transition_image(&image_command_buffer, image.handle(), vk::ImageLayout::UNDEFINED, vk::ImageLayout::GENERAL, vk::PipelineStageFlags::TOP_OF_PIPE, vk::PipelineStageFlags::BOTTOM_OF_PIPE, vk::AccessFlags::empty(), vk::AccessFlags::empty());
        }
        image_command_buffer.end();
        renderer.device.submit_single_time_command(renderer.queue, &image_command_buffer);

        // Layout
        let layout_bindings = &[
            vk::DescriptorSetLayoutBinding::default()
                .binding(0)
                .descriptor_type(vk::DescriptorType::STORAGE_IMAGE)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::COMPUTE ),
            vk::DescriptorSetLayoutBinding::default()
                .binding(1)
                .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::COMPUTE ),
        ];
        let descriptorset = DescriptorSetLayout::new_push_descriptor(
            &renderer.device,
            layout_bindings
        );

        let push_constant_range = PushConstantRange::default()
            .offset(0)
            .stage_flags(vk::ShaderStageFlags::COMPUTE)
            .size(size_of::<PushConstants>() as u32);

        // Pipeline
        let pipeline = renderer.pipeline_store().insert(PipelineConfig {
            shader_path: "shaders/graph.comp".into(),
            descriptor_set_layouts: vec![
                descriptorset.clone(),
            ],
            push_constant_ranges: vec![
                push_constant_range
            ],
            macros: Default::default(),
        }).expect("Failed to create pipeline");

        // Layout
        let layout_bindings = &[
            vk::DescriptorSetLayoutBinding::default()
                .binding(0)
                .descriptor_type(vk::DescriptorType::STORAGE_IMAGE)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::COMPUTE ),
            vk::DescriptorSetLayoutBinding::default()
                .binding(1)
                .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::COMPUTE ),
            vk::DescriptorSetLayoutBinding::default()
                .binding(2)
                .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::COMPUTE ),
        ];
        let edge_descriptorset = DescriptorSetLayout::new_push_descriptor(
            &renderer.device,
            layout_bindings
        );

        // Pipeline
        let edge_pipeline = renderer.pipeline_store().insert(PipelineConfig {
            shader_path: "shaders/edges.comp".into(),
            descriptor_set_layouts: vec![
                edge_descriptorset.clone(),
            ],
            push_constant_ranges: vec![
                push_constant_range.clone()
            ],
            macros: Default::default(),
        }).expect("Failed to create pipeline");

        self.image = Some(image);
        self.descriptorset = Some(descriptorset);
        self.pipeline = Some(pipeline);
        self.edge_pipeline = Some(edge_pipeline);
        self.edge_descriptorset = Some(edge_descriptorset);
    }

    fn render(&mut self, renderer: &mut Renderer, command_buffer: &mut CommandBuffer, swapchain_image: &vk::Image, _: &vk::ImageView) {

        // Clear render image
        unsafe {
            renderer.device.handle().cmd_clear_color_image(
                command_buffer.handle(),
                *self.image.as_ref().unwrap().handle(),
                vk::ImageLayout::GENERAL,
                &vk::ClearColorValue {
                    float32: [0.0, 0.0, 0.0, 1.0]
                },
                &[vk::ImageSubresourceRange {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    base_mip_level: 0,
                    level_count: 1,
                    base_array_layer: 0,
                    layer_count: 1,
                }]
            );
        }

        // Render
        let compute = renderer.pipeline_store().get(self.pipeline.unwrap()).unwrap();

        // Create push constant
        let push_constants = if let Some(transform) = self.transform {
            PushConstants {
                transform,
                nodes: self.node_count.unwrap()
            }
        } else {
            panic!("No transform provided");
        };

        command_buffer.bind_pipeline(&compute);

        let image_bindings = [self.image.as_ref().unwrap().binding(vk::ImageLayout::GENERAL)];
        let image_write_descriptor_set = WriteDescriptorSet::default()
            .dst_binding(0)
            .dst_array_element(0)
            .descriptor_type(vk::DescriptorType::STORAGE_IMAGE)
            .image_info(&image_bindings);

        let buffer_bindings = [self.buffer_info.unwrap()];
        let buffer_write_descriptor_set = WriteDescriptorSet::default()
            .dst_binding(1)
            .dst_array_element(0)
            .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
            .buffer_info(&buffer_bindings);

        command_buffer.bind_push_descriptor(
            &compute,
            0,
            &[image_write_descriptor_set, buffer_write_descriptor_set]
        );

        command_buffer.push_constants(
            &compute,
            ShaderStageFlags::COMPUTE,
            0,
            bytemuck::bytes_of(&push_constants)
        );

        let dispatches = self.node_count.unwrap().div_ceil(16);
        command_buffer.dispatch(dispatches, 1, 1 );

        // Render edges
        let compute = renderer.pipeline_store().get(self.edge_pipeline.unwrap()).unwrap();

        command_buffer.bind_pipeline(&compute);

        let edge_buffer_bindings = [self.edge_buffer_info.unwrap()];
        let edge_buffer_write_descriptor_set = WriteDescriptorSet::default()
            .dst_binding(2)
            .dst_array_element(0)
            .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
            .buffer_info(&edge_buffer_bindings);

        command_buffer.bind_push_descriptor(
            &compute,
            0,
            &[image_write_descriptor_set, buffer_write_descriptor_set, edge_buffer_write_descriptor_set]
        );

        let push_constants = if let Some(transform) = self.transform {
            PushConstants {
                transform,
                nodes: self.edge_count.unwrap()
            }
        } else {
            panic!("No transform provided");
        };

        command_buffer.push_constants(
            &compute,
            ShaderStageFlags::COMPUTE,
            0,
            bytemuck::bytes_of(&push_constants)
        );

        let dispatches = self.edge_count.unwrap().div_ceil(16);
        command_buffer.dispatch(dispatches, 1, 1 );

        // Transition the render to a source
        renderer.transition_image(
            &command_buffer,
            &self.image.as_ref().unwrap().handle(),
            vk::ImageLayout::GENERAL,
            vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
            vk::PipelineStageFlags::COMPUTE_SHADER,
            vk::PipelineStageFlags::TRANSFER,
            vk::AccessFlags::SHADER_WRITE,
            vk::AccessFlags::TRANSFER_READ
        );

        // Transition the swapchain image
        renderer.transition_image(
            &command_buffer,
            &swapchain_image,
            vk::ImageLayout::PRESENT_SRC_KHR,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            vk::PipelineStageFlags::TOP_OF_PIPE,
            vk::PipelineStageFlags::TRANSFER,
            vk::AccessFlags::NONE,
            vk::AccessFlags::TRANSFER_WRITE
        );

        // Copy to the swapchain
        unsafe {
            renderer.device.handle().cmd_clear_color_image(
                command_buffer.handle(),
                *swapchain_image,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                &vk::ClearColorValue {
                    float32: [0.0, 0.0, 0.0, 1.0]
                },
                &[vk::ImageSubresourceRange {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    base_mip_level: 0,
                    level_count: 1,
                    base_array_layer: 0,
                    layer_count: 1,
                }]
            );

            // Use a blit, as a copy doesn't synchronize properly to the swapchain on MoltenVK
            renderer.device.handle().cmd_blit_image(
                command_buffer.handle(),
                *self.image.as_ref().unwrap().handle(),
                vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                *swapchain_image,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                &[vk::ImageBlit::default()
                    .src_offsets([
                        vk::Offset3D::default(),
                        vk::Offset3D::default().x(self.image.as_ref().unwrap().width as i32).y(self.image.as_ref().unwrap().height as i32).z(1)
                    ])
                    .dst_offsets([
                        vk::Offset3D::default(),
                        vk::Offset3D::default().x(self.image.as_ref().unwrap().width as i32).y(self.image.as_ref().unwrap().height as i32).z(1)
                    ])
                    .src_subresource(
                        vk::ImageSubresourceLayers::default()
                            .aspect_mask(vk::ImageAspectFlags::COLOR)
                            .base_array_layer(0)
                            .layer_count(1)
                            .mip_level(0)
                    )
                    .dst_subresource(
                        vk::ImageSubresourceLayers::default()
                            .aspect_mask(vk::ImageAspectFlags::COLOR)
                            .base_array_layer(0)
                            .layer_count(1)
                            .mip_level(0)
                    )
                ],
                vk::Filter::NEAREST,
            );
        }

        // Transfer back to default states
        renderer.transition_image(
            &command_buffer,
            &swapchain_image,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            vk::ImageLayout::PRESENT_SRC_KHR,
            vk::PipelineStageFlags::TRANSFER,
            vk::PipelineStageFlags::BOTTOM_OF_PIPE,
            vk::AccessFlags::TRANSFER_WRITE,
            vk::AccessFlags::NONE
        );

        // Transition the render image back
        renderer.transition_image(
            &command_buffer,
            &self.image.as_ref().unwrap().handle(),
            vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
            vk::ImageLayout::GENERAL,
            vk::PipelineStageFlags::TRANSFER,
            vk::PipelineStageFlags::BOTTOM_OF_PIPE,
            vk::AccessFlags::TRANSFER_WRITE,
            vk::AccessFlags::NONE
        );
    }
}
