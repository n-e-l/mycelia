use std::ops::Div;
use std::process::exit;
use ash::vk;
use ash::vk::{BufferUsageFlags, DescriptorBufferInfo, DeviceSize, Image, ImageView, PushConstantRange, ShaderStageFlags, WriteDescriptorSet};
use bytemuck::{Pod, Zeroable};
use cen::graphics::pipeline_store::{PipelineConfig, PipelineKey};
use cen::graphics::Renderer;
use cen::graphics::renderer::RenderComponent;
use cen::vulkan::{Buffer, CommandBuffer, DescriptorSetLayout};
use cen::vulkan::PipelineErr::ShaderCompilation;
use glam::{IVec3, IVec4, Vec3, Vec4};
use gpu_allocator::MemoryLocation;
use petgraph::matrix_graph::Nullable;
use rand::{random, Rng, SeedableRng};
use log::error;
use rand::rngs::StdRng;
use crate::world::World;

#[derive(Debug)]
#[derive(Copy, Clone)]
#[repr(C, packed)]
struct Node {
    position: Vec3,
    edge_id: i32,
    velocity: Vec3,
    density: f32,
}

#[derive(Debug)]
#[derive(Copy, Clone)]
struct Edge {
    node0: u32,
    node1: u32,
}

struct Pipeline {
    descriptorsetlayout: DescriptorSetLayout,
    pipeline: PipelineKey,
}

pub struct PhysicsComponent {
    node_count: usize,
    edge_count: usize,
    node_buffer_a: Option<Buffer>,
    node_buffer_b: Option<Buffer>,
    edge_buffer: Option<Buffer>,
    descriptorsetlayout: Option<DescriptorSetLayout>,
    physics_pipeline: Option<Pipeline>,
    edge_pipeline: Option<Pipeline>,
    repulsion: f32,
    pub edge_attraction: f32,
    pub running: bool,
    pub step: bool,
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
            running: true,
            step: false,
            node_count: 10000,
            edge_count: 9000,
            repulsion: 1.2,
            edge_attraction: 2.0,
            node_buffer_a: None,
            node_buffer_b: None,
            edge_buffer: None,
            physics_pipeline: None,
            edge_pipeline: None,
            descriptorsetlayout: None,
        }
    }

    pub fn update_weights(&mut self, world: &World) {
        let (_, node_mem_a, _) = unsafe { self.node_buffer_a.as_mut().unwrap().mapped().align_to_mut::<Node>() };
        for (i, node) in world.nodes().iter().enumerate() {
            node_mem_a[i].density = node.level;
        }
        let (_, node_mem_b, _) = unsafe { self.node_buffer_b.as_mut().unwrap().mapped().align_to_mut::<Node>() };
        for (i, node) in world.nodes().iter().enumerate() {
            node_mem_b[i].density = node.level;
        }
    }

    pub fn set_nodes(&mut self, world: &World) {
        self.node_count = world.node_count();
        self.edge_count = world.edge_count();

        let (_, node_mem_a, _) = unsafe { self.node_buffer_a.as_mut().unwrap().mapped().align_to_mut::<Node>() };
        for (i, node) in world.nodes().iter().enumerate() {
            node_mem_a[i] = Node {
                density: node.level as f32,
                position: node_mem_a[i].position,
                edge_id: 0,
                velocity: Vec3::ZERO,
            };
        }
        let (_, node_mem_b, _) = unsafe { self.node_buffer_b.as_mut().unwrap().mapped().align_to_mut::<Node>() };
        for (i, node) in world.nodes().iter().enumerate() {
            node_mem_b[i] = Node {
                density: node.level as f32,
                position: node_mem_b[i].position,
                edge_id: 0,
                velocity: Vec3::ZERO,
            };
        }

        let mut edges = vec![];
        for (i, edge) in world.edges().iter().enumerate() {
            edges.push(Edge {
                node0: edge.source().index() as u32,
                node1: edge.target().index() as u32,
            });
        }

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

        let (_, edge_mem, _) = unsafe { self.edge_buffer.as_mut().unwrap().mapped().align_to_mut::<Edge>() };
        for (i,e) in edges.iter().enumerate() {
            edge_mem[i] = *e;
        }

        // Update nodes
        edges.iter().enumerate().rev().for_each(|(i, edge)| {
            node_mem_a[edge.node0 as usize].edge_id = (i as u32 + 1) as i32;
            node_mem_b[edge.node0 as usize].edge_id = (i as u32 + 1) as i32;
        });
    }

    pub fn node_buffer(&self) -> DescriptorBufferInfo {
        self.node_buffer_a.as_ref().unwrap().binding()
    }

    fn load_pipeline(renderer: &mut Renderer, path: &str, layout: DescriptorSetLayout, push_constant_range: PushConstantRange) -> PipelineKey {
        match renderer.pipeline_store().insert(PipelineConfig {
            shader_path: path.into(),
            descriptor_set_layouts: vec![
                layout,
            ],
            push_constant_ranges: vec![
                push_constant_range
            ],
            macros: Default::default(),
        }) {
            Ok(x) => x,
            Err(ShaderCompilation(x)) => {
                error!("Failed to create pipeline\n{}", x);
                exit(1);
            },
        }
    }

    pub fn edge_buffer(&self) -> DescriptorBufferInfo {
        self.edge_buffer.as_ref().unwrap().binding()
    }

    pub fn node_count(&mut self) -> &mut usize {
        &mut self.node_count
    }

    pub fn edge_count(&self) -> usize {
        self.edge_count * 2
    }

    pub fn repulsion(&mut self) -> &mut f32 {
        &mut self.repulsion
    }

    fn create_buffers(&mut self, renderer: &mut Renderer) {

        let mut rng = StdRng::seed_from_u64(3243451135u64);

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
                position: Vec3::new(rng.gen::<f32>(), rng.gen::<f32>(), rng.gen::<f32>()) * 0.2 - 0.1,
                edge_id: 0,
                velocity: Vec3::ZERO,
                density: 0.,
                // position: Vec3::new(1., 1., 1.) * i as f32 / self.node_count as f32 * 0.2 - 0.1,
            };
        }

        self.node_buffer_a = Some(node_buffer_a);
        self.node_buffer_b = Some(node_buffer_b);

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
                node0: edges[(rng.gen::<u32>() % edges.len() as u32) as usize].node1,
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
            //node.position = Vec4::ZERO;
            node.position = Vec3::new(rng.gen::<f32>() - 0.5, rng.gen::<f32>() - 0.5, rng.gen::<f32>() - 0.5);
        });

        // Update nodes
        edges.iter().enumerate().rev().for_each(|(i, edge)| {
            node_mem[edge.node0 as usize].edge_id = (i as u32 + 1) as i32;
            node_mem[edge.node0 as usize].position = Vec3::new(rng.gen::<f32>() - 0.5, rng.gen::<f32>() - 0.5, rng.gen::<f32>() - 0.5);
        });

        // Copy buffer a into the backbuffer
        let (_, node_mem_b, _) = unsafe { self.node_buffer_b.as_mut().unwrap().mapped().align_to_mut::<Node>() };
        node_mem.iter().enumerate().for_each(|(i, n)| {
            node_mem_b[i] = node_mem[i];
        });

        self.edge_buffer = Some(edge_buffer);
    }

    fn create_edge_pipeline(&mut self, renderer: &mut Renderer) {
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
        let pipeline = Self::load_pipeline(renderer, "shaders/physics_edges.comp", descriptorset.clone(), push_constant_range);

        self.edge_pipeline = Some(Pipeline{
            pipeline,
            descriptorsetlayout: descriptorset.clone(),
        })
    }

    fn create_physics_pipeline(&mut self, renderer: &mut Renderer) {
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
            vk::DescriptorSetLayoutBinding::default()
                .binding(3)
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
        let pipeline = Self::load_pipeline(renderer, "shaders/physics.comp", descriptorset.clone(), push_constant_range);

        self.physics_pipeline = Some(Pipeline {
            pipeline,
            descriptorsetlayout: descriptorset
        });
    }
}

impl RenderComponent for PhysicsComponent {
    fn initialize(&mut self, renderer: &mut Renderer) {
        self.create_buffers(renderer);
        self.create_physics_pipeline(renderer);
        self.create_edge_pipeline(renderer);
    }

    fn render(&mut self, renderer: &mut Renderer, command_buffer: &mut CommandBuffer, swapchain_image: &Image, swapchain_image_view: &ImageView) {

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

        let edge_buffer_bindings = [self.edge_buffer.as_ref().unwrap().binding()];
        let edge_buffer_write_descriptor_set = WriteDescriptorSet::default()
            .dst_binding(2)
            .dst_array_element(0)
            .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
            .buffer_info(&edge_buffer_bindings);

        // Edge physics
        {
            let compute = renderer.pipeline_store().get(self.edge_pipeline.as_ref().unwrap().pipeline).unwrap();

            command_buffer.bind_pipeline(&compute);

            // Reads from buffer b and writes to buffer a
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

            let dispatches = self.node_count.div_ceil(128);
            command_buffer.dispatch(dispatches as u32, 1, 1 );

            command_buffer.buffer_barrier(
                vk::PipelineStageFlags::COMPUTE_SHADER,
                vk::PipelineStageFlags::COMPUTE_SHADER,
                vk::AccessFlags::SHADER_WRITE,
                vk::AccessFlags::SHADER_READ,
                vk::DependencyFlags::default(),
                self.node_buffer_a.as_ref().unwrap().size,
                0,
                self.node_buffer_a.as_ref().unwrap()
            );
        }

        // Node physics
        {
            let compute = renderer.pipeline_store().get(self.physics_pipeline.as_ref().unwrap().pipeline).unwrap();

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

            let dispatches = self.node_count.div_ceil(128);
            command_buffer.dispatch(dispatches as u32, 1, 1 );
        }
    }
}