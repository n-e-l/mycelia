use ash::vk;
use ash::vk::{BufferUsageFlags, DescriptorBufferInfo, DeviceSize, Image, ImageView, PushConstantRange, ShaderStageFlags, WriteDescriptorSet};
use bytemuck::{Pod, Zeroable};
use cen::graphics::pipeline_store::{PipelineConfig, PipelineKey};
use cen::graphics::Renderer;
use cen::graphics::renderer::RenderComponent;
use cen::vulkan::{Buffer, CommandBuffer, DescriptorSetLayout};
use glam::Vec3;
use gpu_allocator::MemoryLocation;
use petgraph::matrix_graph::Nullable;
use rand::random;

#[derive(Debug)]
#[derive(Copy, Clone)]
struct Node {
    position: Vec3,
    edge_index: u32
}

#[derive(Debug)]
#[derive(Copy, Clone)]
struct Edge {
    node0: u32,
    node1: u32,
}

struct Order {
    position: Vec3,
    edge_index: u32,
}

struct EdgePipeline {
    edge_buffer: Buffer,
    descriptorsetlayout: DescriptorSetLayout,
    pipeline: PipelineKey,
}

pub struct PhysicsComponent {
    node_count: usize,
    edge_count: usize,
    node_buffer_a: Option<Buffer>,
    node_buffer_b: Option<Buffer>,
    order_buffer: Option<Buffer>,
    descriptorsetlayout: Option<DescriptorSetLayout>,
    pipeline: Option<PipelineKey>,
    edge_pipeline: Option<EdgePipeline>,
    repulsion: f32,
    pub edge_attraction: f32,
}

#[derive(Pod, Zeroable)]
#[repr(C, packed)]
#[derive(Copy)]
#[derive(Clone)]
struct PushConstants {
    nodes: u32,
    repulsion: f32
}

impl PhysicsComponent {
    pub(crate) fn new() -> Self {
        Self {
            node_count: 8000,
            edge_count: 6000,
            repulsion: 0.2,
            edge_attraction: 0.2,
            node_buffer_a: None,
            node_buffer_b: None,
            order_buffer: None,
            pipeline: None,
            edge_pipeline: None,
            descriptorsetlayout: None,
        }
    }

    pub fn node_buffer(&self) -> DescriptorBufferInfo {
        self.node_buffer_a.as_ref().unwrap().binding()
    }

    pub fn edge_buffer(&self) -> DescriptorBufferInfo {
        self.edge_pipeline.as_ref().unwrap().edge_buffer.binding()
    }

    pub fn node_count(&self) -> usize {
        self.node_count
    }

    pub fn edge_count(&self) -> usize {
        self.edge_count * 2
    }

    pub fn repulsion(&mut self) -> &mut f32 {
        &mut self.repulsion
    }

    fn create_edge_pipeline(&mut self, renderer: &mut Renderer) {
        let mut edge_buffer = Buffer::new(
            &renderer.device,
            &mut renderer.allocator,
            MemoryLocation::CpuToGpu,
            (size_of::<Edge>() * self.edge_count * 2) as DeviceSize,
            BufferUsageFlags::STORAGE_BUFFER
        );

        // Copy edges
        let mut edges = vec![Edge {node0: 0, node1: 1}];
        for i in 0..self.edge_count {
            edges.push(Edge {
                node0: edges[(random::<u32>() % edges.len() as u32) as usize].node1,
                node1: edges.len() as u32 - 1,
            });
        };

        // Add the reverse edges as well
        let mut reverse_edges = edges.clone().iter().map(|edge| {
            Edge {
                node0: edge.node1,
                node1: edge.node0
            }
        }).collect::<Vec<Edge>>();
        edges.append(&mut reverse_edges);

        // Sort by starting node
        edges.sort_by(|a, b| a.node0.cmp(&b.node0));

        let (_, edge_mem, _) = unsafe { edge_buffer.mapped().align_to_mut::<Edge>() };
        for i in 0..(self.edge_count * 2) {
            edge_mem[i] = edges[i];
        }

        // Set node positions to zero
        let (_, node_mem, _) = unsafe { self.node_buffer_a.as_mut().unwrap().mapped().align_to_mut::<Node>() };
        node_mem.iter_mut().enumerate().rev().for_each(|(i, node)| {
            node.position = Vec3::ZERO;
        });

        // Update nodes
        edges.iter().enumerate().rev().for_each(|(i, edge)| {
           node_mem[edge.node0 as usize].edge_index = i as u32 + 1;
            node_mem[edge.node0 as usize].position = Vec3::new(random::<f32>() - 0.5, random::<f32>() - 0.5, random::<f32>() - 0.5);
        });

        // Copy buffer a into the backbuffer
        let (_, node_mem_b, _) = unsafe { self.node_buffer_b.as_mut().unwrap().mapped().align_to_mut::<Node>() };
        node_mem.iter().enumerate().for_each(|(i, n)| {
            node_mem_b[i] = node_mem[i];
        });

        for n in node_mem {
            println!("{:?}", n);
        }

        for i in edge_mem {
            println!("{:?}", i);
        }

        // Layout
        let layout_bindings = &[
            vk::DescriptorSetLayoutBinding::default()
                .binding(0)
                .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
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
            shader_path: "shaders/physics_edges.comp".into(),
            descriptor_set_layouts: vec![
                descriptorset.clone(),
            ],
            push_constant_ranges: vec![
                push_constant_range
            ],
            macros: Default::default(),
        }).expect("Failed to create pipeline");

        self.edge_pipeline = Some(EdgePipeline{
            pipeline,
            edge_buffer,
            descriptorsetlayout: descriptorset.clone(),
        })
    }
}

impl RenderComponent for PhysicsComponent {
    fn initialize(&mut self, renderer: &mut Renderer) {

        let mut node_buffer_a = Buffer::new(
            &renderer.device,
            &mut renderer.allocator,
            MemoryLocation::CpuToGpu,
            (size_of::<Node>() * self.node_count) as DeviceSize,
            BufferUsageFlags::STORAGE_BUFFER
        );

        let mut node_buffer_b = Buffer::new(
            &renderer.device,
            &mut renderer.allocator,
            MemoryLocation::CpuToGpu,
            (size_of::<Node>() * self.node_count) as DeviceSize,
            BufferUsageFlags::STORAGE_BUFFER
        );

        // Copy start positions to node buffer
        let (_, node_mem, _) = unsafe { node_buffer_a.mapped().align_to_mut::<Node>() };
        for i in 0..self.node_count {
            node_mem[i] = Node {
                position: Vec3::new(random::<f32>(), random::<f32>(), random::<f32>()) * 0.2 - 0.1,
                // position: Vec3::new(1., 1., 1.) * i as f32 / self.node_count as f32 * 0.2 - 0.1,
                edge_index: 0,
            };
        }

        self.node_buffer_a = Some(node_buffer_a);
        self.node_buffer_b = Some(node_buffer_b);
        self.create_edge_pipeline(renderer);

        // Layout
        let layout_bindings = &[
            vk::DescriptorSetLayoutBinding::default()
                .binding(0)
                .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
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
            shader_path: "shaders/physics.comp".into(),
            descriptor_set_layouts: vec![
                descriptorset.clone(),
            ],
            push_constant_ranges: vec![
                push_constant_range
            ],
            macros: Default::default(),
        }).expect("Failed to create pipeline");

        self.pipeline = Some(pipeline);
        self.descriptorsetlayout = Some(descriptorset);
    }

    fn render(&mut self, renderer: &mut Renderer, command_buffer: &mut CommandBuffer, swapchain_image: &Image, swapchain_image_view: &ImageView) {

        // Edge pull
        let compute = renderer.pipeline_store().get(self.edge_pipeline.as_ref().unwrap().pipeline).unwrap();

        command_buffer.bind_pipeline(&compute);

        let buffer_bindings_a = [self.node_buffer_a.as_ref().unwrap().binding()];
        let buffer_write_descriptor_set_a = WriteDescriptorSet::default()
            .dst_binding(0)
            .dst_array_element(0)
            .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
            .buffer_info(&buffer_bindings_a);

        let buffer_bindings_b = [self.node_buffer_b.as_ref().unwrap().binding()];
        let buffer_write_descriptor_set_b = WriteDescriptorSet::default()
            .dst_binding(1)
            .dst_array_element(0)
            .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
            .buffer_info(&buffer_bindings_b);

        let edge_buffer_bindings = [self.edge_pipeline.as_ref().unwrap().edge_buffer.binding()];
        let edge_buffer_write_descriptor_set = WriteDescriptorSet::default()
            .dst_binding(2)
            .dst_array_element(0)
            .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
            .buffer_info(&edge_buffer_bindings);

        command_buffer.bind_push_descriptor(
            &compute,
            0,
            &[buffer_write_descriptor_set_a, buffer_write_descriptor_set_b, edge_buffer_write_descriptor_set]
        );

        let push_constants = PushConstants {
            nodes: self.node_count as u32,
            repulsion: self.edge_attraction,
        };
        command_buffer.push_constants(
            &compute,
            ShaderStageFlags::COMPUTE,
            0,
            bytemuck::bytes_of(&push_constants)
        );

        let dispatches = self.node_count.div_ceil(16);
        command_buffer.dispatch(dispatches as u32, 1, 1 );

        // Node positioning
        let compute = renderer.pipeline_store().get(self.pipeline.unwrap()).unwrap();

        command_buffer.bind_pipeline(&compute);

        command_buffer.bind_push_descriptor(
            &compute,
            0,
            &[buffer_write_descriptor_set_a, buffer_write_descriptor_set_b]
        );

        let push_constants = PushConstants {
            nodes: self.node_count as u32,
            repulsion: self.repulsion,
        };
        command_buffer.push_constants(
            &compute,
            ShaderStageFlags::COMPUTE,
            0,
            bytemuck::bytes_of(&push_constants)
        );

        let dispatches = self.node_count.div_ceil(16);
        command_buffer.dispatch(dispatches as u32, 1, 1 );
    }
}