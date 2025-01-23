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
use glam::{IVec4, Vec3, Vec4};
use gpu_allocator::MemoryLocation;
use graph::Graph;
use crate::database::{Concept, Database, Relation};

mod database;
mod graph;

struct Application {
    database: Database,
    graph: Arc<Mutex<Graph>>,
    concepts: Vec<Concept>,
    relations: Vec<Relation>,
    messages: Vec<String>,
    image: Option<Image>,
    buffer: Option<Buffer>,
    edge_buffer: Option<Buffer>,
    descriptorset: Option<DescriptorSetLayout>,
    pipeline: Option<PipelineKey>,
    edge_pipeline: Option<PipelineKey>,
}

impl Application {

    async fn reload_graph(&mut self) {

        // tokio::spawn(async {
            self.concepts = self.database.get_concepts().await;
            self.messages = self.database.get_messages().await;
            self.relations = self.database.get_relations().await;
            let mut lock = self.graph.lock().unwrap();
            lock.reset();

            for i in 0..self.concepts.len() {
                lock.add_node();
            }
            for i in 0..self.messages.len() {
                lock.add_node();
            }
            for r in &self.relations {
                lock.add_edge(r.a, r.b);
            }
        // });
    }

    async fn new() -> Application {
        let database = Database::new().await;
        let concepts = database.get_concepts().await;
        let messages = database.get_messages().await;
        let relations = database.get_relations().await;

        let mut graph = Graph::new();
        for i in 0..concepts.len() {
            graph.add_node();
        }
        for i in 0..messages.len() {
            graph.add_node();
        }
        for r in &relations {
            graph.add_edge(r.a, r.b);
        }

        Self {
            database,
            graph: Arc::new(Mutex::new(graph)),
            concepts,
            relations,
            messages,
            image: None,
            buffer: None,
            edge_buffer: None,
            descriptorset: None,
            pipeline: None,
            edge_pipeline: None,
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

        let positions = 1024 * 4;
        let buffer = Buffer::new(
            &renderer.device,
            &mut renderer.allocator,
            MemoryLocation::CpuToGpu,
            (positions * 4 * 8) as DeviceSize,
            vk::BufferUsageFlags::STORAGE_BUFFER
        );

        let edges = 1024 * 4;
        let edge_buffer = Buffer::new(
            &renderer.device,
            &mut renderer.allocator,
            MemoryLocation::CpuToGpu,
            (edges * 4 * 8) as DeviceSize,
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

        // Pipeline
        let edge_pipeline = renderer.pipeline_store().insert(PipelineConfig {
            shader_path: "shaders/edges.comp".into(),
            descriptor_set_layouts: vec![
                descriptorset.clone(),
            ],
            push_constant_ranges: vec![],
            macros: Default::default(),
        }).expect("Failed to create pipeline");

        self.image = Some(image);
        self.descriptorset = Some(descriptorset);
        self.pipeline = Some(pipeline);
        self.edge_pipeline = Some(edge_pipeline);
        self.buffer = Some(buffer);
        self.edge_buffer = Some(edge_buffer);
    }

    fn render(&mut self, renderer: &mut Renderer, command_buffer: &mut CommandBuffer, swapchain_image: &vk::Image, _: &vk::ImageView) {

        let mut lock = self.graph.lock().unwrap();
        lock.update();
        let positions = lock.get_positions();
        let edges = lock.get_edges().iter().map(
            |p| (positions[p.0], positions[p.1])
        ).collect::<Vec<(Vec3, Vec3)>>();

        let (_, ivert_mem, _) = unsafe { self.buffer.as_mut().unwrap().mapped().align_to_mut::<IVec4>() };
        ivert_mem[0] = IVec4::new(positions.len() as i32, 0, 0, 0);
        let (_, vert_mem, _) = unsafe { self.buffer.as_mut().unwrap().mapped().align_to_mut::<Vec4>() };
        for i in 0..positions.len() {
            vert_mem[i+1] = Vec4::new(positions[i].x, positions[i].y, positions[i].z, 0.0);
        }

        let (_, iedge_mem, _) = unsafe { self.edge_buffer.as_mut().unwrap().mapped().align_to_mut::<IVec4>() };
        iedge_mem[0] = IVec4::new(edges.len() as i32, 0, 0, 0);
        let (_, edge_mem, _) = unsafe { self.edge_buffer.as_mut().unwrap().mapped().align_to_mut::<Vec4>() };
        for i in 0..edges.len() {
            edge_mem[i*2+1] = Vec4::new(edges[i].0.x, edges[i].0.y, edges[i].0.z, 0.0);
            edge_mem[i*2+2] = Vec4::new(edges[i].1.x, edges[i].1.y, edges[i].1.z, 0.0);
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

        let image_bindings = [self.image.as_ref().unwrap().binding(vk::ImageLayout::GENERAL)];
        let image_write_descriptor_set = WriteDescriptorSet::default()
            .dst_binding(0)
            .dst_array_element(0)
            .descriptor_type(vk::DescriptorType::STORAGE_IMAGE)
            .image_info(&image_bindings);

        let buffer_bindings = [self.buffer.as_ref().unwrap().binding()];
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

        command_buffer.dispatch(500, 1, 1 );

        // Render edges
        let compute = renderer.pipeline_store().get(self.edge_pipeline.unwrap()).unwrap();

        command_buffer.bind_pipeline(&compute);

        let edge_buffer_bindings = [self.edge_buffer.as_ref().unwrap().binding()];
        let edge_buffer_write_descriptor_set = WriteDescriptorSet::default()
            .dst_binding(1)
            .dst_array_element(0)
            .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
            .buffer_info(&edge_buffer_bindings);

        command_buffer.bind_push_descriptor(
            &compute,
            0,
            &[image_write_descriptor_set, edge_buffer_write_descriptor_set]
        );

        command_buffer.dispatch(500, 1, 1 );

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

        context.input(|x| {

            if x.pointer.button_down(egui::PointerButton::Primary) {
                let rot_x = glam::Mat3::from_rotation_y(-x.pointer.delta().x / 60.0);
                let rot_y = glam::Mat3::from_rotation_x(-x.pointer.delta().y / 60.0);
                self.graph.lock().unwrap().get_nodes_mut().iter_mut().for_each(|node| {
                    node.pos = rot_x * rot_y * node.pos;
                })
            }
        });

        egui::Window::new("Database")
            .resizable(true)
            .title_bar(true)
            .show(context, |ui| unsafe {
                let mut lock = self.graph.lock().unwrap();
                ui.add(
                    Slider::new(lock.get_edge_strength(), 0.0..=300.0)
                );
                ui.add(
                Slider::new(lock.get_repulsion(), 0.0..=1.0)
                );

                // if ui.button("Reload").clicked() {
                //     self.reload_graph();
                // }
            });

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
                                    ui.label(self.concepts[row_index].name.to_string());
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
            .width(1600)
            .height(900)
            .log_fps(true)
            .vsync(true),
        application.clone(),
        Some(application.clone())
    );

    Ok(())
}
