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

#[derive(Debug)]
#[derive(Copy, Clone)]
struct Ordering {
    node_id: i32,
    cell_id: i32,
    offset1: i32,
    offset2: i32,
}

#[derive(Debug)]
#[derive(Copy, Clone)]
struct Lookup {
    ordering_id: u32,
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
    order_buffer: Option<Buffer>,
    lookup_buffer: Option<Buffer>,
    descriptorsetlayout: Option<DescriptorSetLayout>,
    physics_pipeline: Option<Pipeline>,
    edge_pipeline: Option<Pipeline>,
    lookup_pipeline: Option<Pipeline>,
    sort_pipeline: Option<Pipeline>,
    repulsion: f32,
    pub edge_attraction: f32,
    pub running: bool,
    pub step: bool,
    pub kill: bool
}

#[derive(Pod, Zeroable)]
#[repr(C, packed)]
#[derive(Copy)]
#[derive(Clone)]
struct PushConstants {
    nodes: u32,
    repulsion: f32
}

#[derive(Pod, Zeroable)]
#[repr(C, packed)]
#[derive(Copy)]
#[derive(Clone)]
struct BitonicPushConstants {
    node_count: u32,
    group_width: u32,
    group_heigth: u32,
    step_index: u32,
}

impl PhysicsComponent {
    pub(crate) fn new() -> Self {
        Self {
            running: true,
            step: false,
            kill: false,
            node_count: 40100,
            edge_count: 1,
            repulsion: 0.2,
            edge_attraction: 0.2,
            node_buffer_a: None,
            node_buffer_b: None,
            edge_buffer: None,
            order_buffer: None,
            lookup_buffer: None,
            physics_pipeline: None,
            edge_pipeline: None,
            sort_pipeline: None,
            lookup_pipeline: None,
            descriptorsetlayout: None,
        }
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

        let mut lookup_buffer = Buffer::new(
            &renderer.device,
            &mut renderer.allocator,
            MemoryLocation::CpuToGpu,
            (size_of::<Lookup>() * 100000) as DeviceSize,
            BufferUsageFlags::STORAGE_BUFFER | BufferUsageFlags::TRANSFER_DST
        );

        let mut ordering_buffer = Buffer::new(
            &renderer.device,
            &mut renderer.allocator,
            MemoryLocation::CpuToGpu,
            (size_of::<Ordering>() * self.node_count) as DeviceSize,
            BufferUsageFlags::STORAGE_BUFFER
        );

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

        self.lookup_buffer = Some(lookup_buffer);
        self.order_buffer = Some(ordering_buffer);
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

    fn create_lookup_pipeline(&mut self, renderer: &mut Renderer) {
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
            .size(size_of::<BitonicPushConstants>() as u32);

        // Pipeline
        let pipeline = Self::load_pipeline(renderer, "shaders/lookup_index.comp", descriptorset.clone(), push_constant_range);

        self.lookup_pipeline = Some(Pipeline{
            pipeline,
            descriptorsetlayout: descriptorset.clone(),
        })
    }

    fn create_sort_pipeline(&mut self, renderer: &mut Renderer) {
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
            .size(size_of::<BitonicPushConstants>() as u32);

        // Pipeline
        let pipeline = Self::load_pipeline(renderer, "shaders/bitonic_merge.comp", descriptorset.clone(), push_constant_range);

        self.sort_pipeline = Some(Pipeline{
            pipeline,
            descriptorsetlayout: descriptorset.clone(),
        })
    }
}

impl RenderComponent for PhysicsComponent {
    fn initialize(&mut self, renderer: &mut Renderer) {
        self.create_buffers(renderer);
        self.create_physics_pipeline(renderer);
        self.create_edge_pipeline(renderer);
        self.create_lookup_pipeline(renderer);
        self.create_sort_pipeline(renderer);
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

        let buffer_bindings_ordering = [self.order_buffer.as_ref().unwrap().binding()];
        let buffer_write_descriptor_set_ordering = WriteDescriptorSet::default()
            .dst_binding(2)
            .dst_array_element(0)
            .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
            .buffer_info(&buffer_bindings_ordering);

        let buffer_bindings_lookup = [self.lookup_buffer.as_ref().unwrap().binding()];
        let buffer_write_descriptor_set_lookup = WriteDescriptorSet::default()
            .dst_binding(3)
            .dst_array_element(0)
            .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
            .buffer_info(&buffer_bindings_lookup);


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

        // Node sorting
        {
            let compute = renderer.pipeline_store().get(self.sort_pipeline.as_ref().unwrap().pipeline).unwrap();
            command_buffer.bind_pipeline(&compute);

            command_buffer.bind_push_descriptor(
                &compute,
                0,
                &[buffer_write_descriptor_set_a, buffer_write_descriptor_set_ordering, buffer_write_descriptor_set_lookup]
            );

            let num_stages = (self.node_count as f32).log2().ceil() as usize;
            let dispatches = 2_u32.pow(num_stages as u32).div(2).div_ceil(128);
            for stage_index in 0..num_stages {
                for step_index in 0..(stage_index+1) {

                    let group_width = 1 << (stage_index - step_index);
                    let bitonic_push = BitonicPushConstants {
                        node_count: self.node_count as u32,
                        group_width,
                        group_heigth: group_width * 2 - 1,
                        step_index: step_index as u32,
                    };
                    command_buffer.push_constants(&compute, ShaderStageFlags::COMPUTE, 0, bytemuck::bytes_of(&bitonic_push));
                    command_buffer.dispatch(dispatches as u32, 1, 1);

                    command_buffer.buffer_barrier(
                        vk::PipelineStageFlags::COMPUTE_SHADER,
                        vk::PipelineStageFlags::COMPUTE_SHADER,
                        vk::AccessFlags::SHADER_WRITE,
                        vk::AccessFlags::SHADER_READ,
                        vk::DependencyFlags::default(),
                        self.order_buffer.as_ref().unwrap().size,
                        0,
                        self.order_buffer.as_ref().unwrap()
                    );
                }
            }
        }

        // Lookup buffer
        {
            // Fill with zeros
            command_buffer.fill_buffer(
                self.lookup_buffer.as_ref().unwrap(),
                0,
                self.lookup_buffer.as_ref().unwrap().size,
                9999999
            );

            command_buffer.buffer_barrier(
                vk::PipelineStageFlags::TRANSFER,
                vk::PipelineStageFlags::COMPUTE_SHADER,
                vk::AccessFlags::TRANSFER_WRITE,
                vk::AccessFlags::SHADER_READ,
                vk::DependencyFlags::default(),
                self.lookup_buffer.as_ref().unwrap().size,
                0,
                self.lookup_buffer.as_ref().unwrap()
            );

            // Calculate the lookups
            let compute = renderer.pipeline_store().get(self.lookup_pipeline.as_ref().unwrap().pipeline).unwrap();
            command_buffer.bind_pipeline(&compute);

            command_buffer.bind_push_descriptor(
                &compute,
                0,
                &[buffer_write_descriptor_set_a, buffer_write_descriptor_set_ordering, buffer_write_descriptor_set_lookup]
            );

            let bitonic_push = BitonicPushConstants {
                node_count: self.node_count as u32,
                group_width: 0,
                group_heigth: 0,
                step_index: 0,
            };
            command_buffer.push_constants(&compute, ShaderStageFlags::COMPUTE, 0, bytemuck::bytes_of(&bitonic_push));

            let dispatches = self.node_count.div_ceil(128);
            command_buffer.dispatch(dispatches as u32, 1, 1);
        }

        // Node physics
        {
            let compute = renderer.pipeline_store().get(self.physics_pipeline.as_ref().unwrap().pipeline).unwrap();

            command_buffer.bind_pipeline(&compute);

            command_buffer.bind_push_descriptor(
                &compute,
                0,
                &[buffer_write_descriptor_set_a, buffer_write_descriptor_set_b, buffer_write_descriptor_set_ordering, buffer_write_descriptor_set_lookup]
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


            unsafe {
                static mut N: i32 = 0;
                let dispatches = self.node_count.div_ceil(128);
                if !self.kill {
                    if self.running || self.step {
                        self.step = false;
                        command_buffer.dispatch(dispatches as u32, 1, 1 );
                    }
                } else {
                    N = N + 1;
                }

                if self.kill && N == 2 {
                    // Test: Print the nodes
                    let (_, mapping, _) = unsafe { self.node_buffer_a.as_mut().unwrap().mapped().align_to::<Node>() };
                    for i in 0..self.node_count {
                        println!("{}: {:?}", i, mapping[i]);
                    }
                    // Test: Print the ordering lookup
                    let (_, mapping, _) = unsafe { self.order_buffer.as_mut().unwrap().mapped().align_to::<Ordering>() };
                    for i in 0..self.node_count {
                        // println!("{}: {:?}", i, mapping[i]);
                    }
                    println!("---------");
                    // Test: Print the lookup lookup
                    // let mut sectors = Vec::new();
                    let (_, mapping, _) = unsafe { self.lookup_buffer.as_mut().unwrap().mapped().align_to::<u32>() };
                    // for i in 0..1000 {
                    //     println!("{}: {:?}", i, mapping[i]);
                    //     if !sectors.contains(&mapping[i]) {
                    //         sectors.push(mapping[i]);
                    //     }
                    // }
                    // println!("sectors: {:?}", sectors);
                    println!("---------");
                    exit(0);
                }
            }
        }
    }
}