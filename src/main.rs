use std::sync::{Arc, Mutex};
use ash::vk;
use ash::vk::{DeviceSize, WriteDescriptorSet};
use cen::app::App;
use cen::app::app::AppConfig;
use cen::app::gui::GuiComponent;
use cen::graphics::pipeline_store::{PipelineConfig, PipelineKey};
use cen::graphics::Renderer;
use cen::graphics::renderer::RenderComponent;
use cen::vulkan::{Buffer, CommandBuffer, DescriptorSetLayout, Image};
use dotenv::dotenv;
use egui::{Slider, TextWrapMode};
use glam::{IVec4, Vec4};
use gpu_allocator::MemoryLocation;
use graph::Graph;
use crate::database::Database;

mod database;
mod graph;

struct Application {
    database: Database,
    graph: Graph,
    concepts: Vec<String>,
    messages: Vec<String>,
    image: Option<Image>,
    buffer: Option<Buffer>,
    descriptorset: Option<DescriptorSetLayout>,
    pipeline: Option<PipelineKey>
}

impl Application {

    async fn new() -> Application {
        let database = Database::new().await;
        let concepts = database.get_concepts().await;
        let messages = database.get_messages().await;

        Self {
            database,
            graph: Graph::new(),
            concepts,
            messages,
            image: None,
            buffer: None,
            descriptorset: None,
            pipeline: None
        }
    }
}

impl RenderComponent for Application {
    fn initialize(&mut self, renderer: &mut Renderer) {
        // Image
        let image = Image::new(
            &renderer.device,
            &mut renderer.allocator,
            renderer.swapchain.get_extent().width,
            renderer.swapchain.get_extent().height,
            vk::ImageUsageFlags::STORAGE | vk::ImageUsageFlags::TRANSFER_SRC | vk::ImageUsageFlags::TRANSFER_DST
        );

        let positions = 1024;
        let mut buffer = Buffer::new(
            &renderer.device,
            &mut renderer.allocator,
            MemoryLocation::CpuToGpu,
            (positions * 4 * 8) as DeviceSize,
            vk::BufferUsageFlags::STORAGE_BUFFER
        );

        // Transition image
        let mut image_command_buffer = CommandBuffer::new(&renderer.device, &renderer.command_pool);
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

        // Pipeline
        let pipeline = renderer.pipeline_store().insert(PipelineConfig {
            shader_path: "shaders/graph.comp".into(),
            descriptor_set_layouts: vec![
                descriptorset.clone(),
            ],
            push_constant_ranges: vec![],
            macros: Default::default(),
        }).expect("Failed to create pipeline");

        self.image = Some(image);
        self.descriptorset = Some(descriptorset);
        self.pipeline = Some(pipeline);
        self.buffer = Some(buffer);
    }

    fn render(&mut self, renderer: &mut Renderer, command_buffer: &mut CommandBuffer, swapchain_image: &vk::Image, _: &vk::ImageView) {

        self.graph.update();
        let positions = self.graph.get_positions();

        let (_, ivert_mem, _) = unsafe { self.buffer.as_mut().unwrap().mapped().align_to_mut::<IVec4>() };
        ivert_mem[0] = IVec4::new(positions.len() as i32, 0, 0, 0);
        let (_, vert_mem, _) = unsafe { self.buffer.as_mut().unwrap().mapped().align_to_mut::<Vec4>() };
        for i in 0..positions.len() {
            vert_mem[i+1] = Vec4::new(positions[i].x, positions[i].y, positions[i].z, 0.0);
        }

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

        command_buffer.bind_pipeline(&compute);

        let bindings = [self.image.as_ref().unwrap().binding(vk::ImageLayout::GENERAL)];
        let write_descriptor_set = WriteDescriptorSet::default()
            .dst_binding(0)
            .dst_array_element(0)
            .descriptor_type(vk::DescriptorType::STORAGE_IMAGE)
            .image_info(&bindings);

        let buffer_bindings = [self.buffer.as_ref().unwrap().binding()];
        let buffer_write_descriptor_set = WriteDescriptorSet::default()
            .dst_binding(1)
            .dst_array_element(0)
            .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
            .buffer_info(&buffer_bindings);

        command_buffer.bind_push_descriptor(
            &compute,
            0,
            &[write_descriptor_set, buffer_write_descriptor_set]
        );

        command_buffer.dispatch(500, 500, 1 );

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

impl GuiComponent for Application {
    fn gui(&mut self, context: &egui::Context) {

        egui::Window::new("Nodes")
            .resizable(true)
            .title_bar(true)
            .show(context, |ui| unsafe {
                if ui.button("Reset").clicked() {
                    self.graph.reset();
                }

                static mut COUNT: usize = 0;
                if ui.add(Slider::new(&mut COUNT, 0..=100).text("count")).changed() {
                    self.graph.set_count(COUNT);
                }
            }
            );

        egui::Window::new("Messages")
            .resizable(true)
            .title_bar(true)
            .show(context, |ui| {
                use egui_extras::{Column, TableBuilder};

                ui.style_mut().wrap_mode = Some(TextWrapMode::Wrap);

                let available_height = ui.available_height();
                let mut table = TableBuilder::new(ui)
                    .striped(true)
                    .resizable(true)
                    .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                    .column(Column::auto())
                    .column(
                        Column::remainder()
                            .at_least(40.0)
                            .clip(false)
                            .resizable(true),
                    )
                    .min_scrolled_height(0.0)
                    .max_scroll_height(available_height);

                table
                    .header(15.0, move |mut header| {
                        header.col(|ui| {
                            egui::Sides::new().show(
                                ui,
                                |ui| {
                                    ui.strong("Row");
                                },
                                |ui| {
                                },
                            );
                        });
                        header.col(|ui| {
                            ui.strong("Message");
                        });
                    })
                    .body(|mut body| {
                        for row_index in 0..self.messages.len() {
                            body.row(30.0, |mut row| {
                                row.col(|ui| {
                                    ui.label(row_index.to_string());
                                });
                                row.col(|ui| {
                                    ui.label(self.messages[row_index].to_string());
                                });
                            });
                        }
                    });
            }
            );

        egui::Window::new("Concepts")
            .resizable(true)
            .title_bar(true)
            .show(context, |ui| {
                use egui_extras::{Column, TableBuilder};

                let available_height = ui.available_height();
                let mut table = TableBuilder::new(ui)
                    .striped(true)
                    .resizable(true)
                    .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                    .column(Column::auto())
                    .column(
                        Column::remainder()
                            .at_least(40.0)
                            .clip(true)
                            .resizable(true),
                    )
                    .min_scrolled_height(0.0)
                    .max_scroll_height(available_height);

                table
                    .header(15.0, move |mut header| {
                        header.col(|ui| {
                            egui::Sides::new().show(
                                ui,
                                |ui| {
                                    ui.strong("Row");
                                },
                                |ui| {
                                },
                            );
                        });
                        header.col(|ui| {
                            ui.strong("Concept");
                        });
                    })
                    .body(|mut body| {
                        for row_index in 0..self.concepts.len() {
                            body.row(30.0, |mut row| {
                                row.col(|ui| {
                                    ui.label(row_index.to_string());
                                });
                                row.col(|ui| {
                                    ui.label(self.concepts[row_index].to_string());
                                });
                            });
                        }
                });
            }
        );
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize .env environment variables
    dotenv().ok();

    let application = Arc::new(Mutex::new(Application::new().await));
    App::run(
        AppConfig::default()
            .log_fps(true)
            .vsync(true),
        application.clone(),
        Some(application.clone())
    );

    Ok(())
}
