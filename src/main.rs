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
use crate::renderer::GraphRenderer;

mod database;
mod graph;
mod renderer;

struct Application {
    database: Database,
    graph_renderer: Arc<Mutex<GraphRenderer>>,
    graph: Arc<Mutex<Graph>>,
    concepts: Vec<Concept>,
    relations: Vec<Relation>,
    messages: Vec<String>,
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

    async fn new(graph_renderer: Arc<Mutex<GraphRenderer>>) -> Application {

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
            graph_renderer: graph_renderer.clone(),
        }
    }
}

impl GuiComponent for Application {
    fn gui(&mut self, context: &egui::Context) {

        // Todo: Move to an update call
        let mut lock = self.graph.lock().unwrap();
        lock.update();
        let positions = lock.get_positions();
        let edges = lock.get_edges().iter().map(
            |p| (positions[p.0], positions[p.1])
        ).collect::<Vec<(Vec3, Vec3)>>();
        self.graph_renderer.lock().unwrap().update_graph(positions, edges);

        // Gui code
        context.input(|x| {

            if x.pointer.button_down(egui::PointerButton::Primary) {
                let rot_x = glam::Mat3::from_rotation_y(-x.pointer.delta().x / 60.0);
                let rot_y = glam::Mat3::from_rotation_x(-x.pointer.delta().y / 60.0);
                lock.get_nodes_mut().iter_mut().for_each(|node| {
                    node.pos = rot_x * rot_y * node.pos;
                })
            }
        });

        egui::Window::new("Database")
            .resizable(true)
            .title_bar(true)
            .show(context, |ui| unsafe {
                ui.add(
                    Slider::new(lock.get_edge_strength(), 0.0..=300.0)
                );
                ui.add(
                Slider::new(lock.get_repulsion(), 0.0..=1.0)
                );
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

    let renderer = Arc::new(Mutex::new(GraphRenderer::new()));
    let application = Arc::new(Mutex::new(Application::new(renderer.clone()).await));
    App::run(
        AppConfig::default()
            .width(1600)
            .height(900)
            .log_fps(true)
            .vsync(true),
        renderer,
        Some(application.clone())
    );

    Ok(())
}
