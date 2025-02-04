use ash::vk;
use ash::vk::{BufferUsageFlags, DescriptorBufferInfo, DeviceSize, Image, ImageView, PushConstantRange};
use cen::graphics::pipeline_store::{PipelineConfig, PipelineKey};
use cen::graphics::Renderer;
use cen::graphics::renderer::RenderComponent;
use cen::vulkan::{Buffer, CommandBuffer, DescriptorSetLayout};
use glam::Vec3;
use gpu_allocator::MemoryLocation;
use rand::random;

struct Node {
    position: Vec3,
    val: u32
}

pub struct PhysicsComponent {
    node_count: usize,
    node_buffer: Option<Buffer>,
    descriptorsetlayout: Option<DescriptorSetLayout>,
    pipeline: Option<PipelineKey>
}

struct PushConstants {
    nodes: usize,
    repulsion: f32
}

impl PhysicsComponent {
    pub(crate) fn new() -> Self {
        Self {
            node_count: 10000,
            node_buffer: None,
            pipeline: None,
            descriptorsetlayout: None,
        }
    }

    pub fn node_buffer(&self) -> DescriptorBufferInfo {
        self.node_buffer.as_ref().unwrap().binding()
    }

    pub fn node_count(&self) -> usize {
        self.node_count
    }
}

impl RenderComponent for PhysicsComponent {
    fn initialize(&mut self, renderer: &mut Renderer) {
        let mut node_buffer = Buffer::new(
            &renderer.device,
            &mut renderer.allocator,
            MemoryLocation::CpuToGpu,
            (size_of::<Node>() * self.node_count) as DeviceSize,
            BufferUsageFlags::STORAGE_BUFFER
        );

        // Copy start positions to node buffer
        let (_, node_mem, _) = unsafe { node_buffer.mapped().align_to_mut::<Node>() };
        for i in 0..self.node_count {
            node_mem[i] = Node {
                position: Vec3::new(random::<f32>(), random::<f32>(), random::<f32>()) * 0.2 - 0.1,
                // position: Vec3::new(1., 1., 1.) * i as f32 / self.node_count as f32 * 0.2 - 0.1,
                val: 0,
            };
        }

        // Layout
        let layout_bindings = &[
            vk::DescriptorSetLayoutBinding::default()
                .binding(0)
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
            shader_path: "shaders/physics.comp".into(),
            descriptor_set_layouts: vec![
                descriptorset.clone(),
            ],
            push_constant_ranges: vec![
                push_constant_range
            ],
            macros: Default::default(),
        }).expect("Failed to create pipeline");

        self.node_buffer = Some(node_buffer);
        self.pipeline = Some(pipeline);
        self.descriptorsetlayout = Some(descriptorset);
    }

    fn render(&mut self, renderer: &mut Renderer, command_buffer: &mut CommandBuffer, swapchain_image: &Image, swapchain_image_view: &ImageView) {
    }
}