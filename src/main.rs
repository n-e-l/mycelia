use std::ops::Mul;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use ash::vk::{Image, ImageView};
use cen::app::App;
use cen::app::app::AppConfig;
use cen::app::gui::GuiComponent;
use cen::graphics::Renderer;
use cen::graphics::renderer::RenderComponent;
use cen::vulkan::CommandBuffer;
use dotenv::dotenv;
use egui::{Align2, Checkbox, Slider, TextWrapMode, Vec2};
use glam::{Mat4, Vec3, Vec4, Vec4Swizzles};
use ordered_float::OrderedFloat;
use world::World;
use crate::gpu_physics::PhysicsComponent;
use crate::renderer::{GraphRenderer, RenderNode};

mod world;
mod renderer;
mod barnes_hut;
mod barnes_hut_no_stack;
mod gpu_physics;

struct Application {
    physics_components: PhysicsComponent,
    graph_renderer: Arc<Mutex<GraphRenderer>>,
    graph: Arc<Mutex<World>>,
    view_transform: Mat4,
    screen_transform_ortho: Mat4,
    screen_transform: Mat4,
    transform_pers: Mat4,
    perspective_camera: bool,
    octree_mesh: Vec<(Vec4, Vec4)>
}

impl Application {

    async fn reload_graph(&mut self) {
        let mut lock = self.graph.lock().unwrap();
    }

    async fn new(graph_renderer: Arc<Mutex<GraphRenderer>>) -> Application {

        let mut graph = World::new();

        // Transform
        let scaling = 1.;
        let width = 1600.;
        let height = 900.;
        let aspect_ratio = width / height;

        // ortho
        let scale = Mat4::from_scale(Vec3::new(1. ,aspect_ratio, 1.));
        let projection_ortho = Mat4::orthographic_rh_gl(0., width * 2., 0., height * 2., -1., 1.).inverse();
        let screen_transform_ortho = projection_ortho * scale;

        // pers
        let translate = Mat4::from_translation(Vec3::new(0., 0., 0.8));
        let projection = Mat4::perspective_rh(1.2, aspect_ratio, 0.01, 10.);
        let transform_pers = projection * translate;

        let screen_translate = Mat4::from_translation(Vec3::new(width, height, 1.));
        let screen_scale = Mat4::from_scale(Vec3::new(width * 2., height * 2., 1.));
        let screen_transform = screen_translate * screen_scale;

        let world = World::new();
        let octree_mesh = world.get_octree().mesh_lines();

        Self {
            physics_components: PhysicsComponent::new(),
            graph: Arc::new(Mutex::new(world)),
            octree_mesh,
            graph_renderer: graph_renderer.clone(),
            screen_transform_ortho,
            transform_pers,
            screen_transform,
            view_transform: Mat4::from_scale(Vec3::new(1., 1., 1.)),
            perspective_camera: true,
        }
    }
}

impl GuiComponent for Application {
    fn gui(&mut self, context: &egui::Context) {

        // Todo: Move to an update call
        // Update graph data
        let mut lock = self.graph.lock().unwrap();
        lock.update();

        // Gui code
        context.input(|x| {

            if x.pointer.button_down(egui::PointerButton::Primary) {
                let rot_x = glam::Mat4::from_rotation_y(x.pointer.delta().x * 0.5 / 60.0);
                let rot_y = glam::Mat4::from_rotation_x(-x.pointer.delta().y * 0.5 / 60.0);
                self.view_transform = rot_x * rot_y * self.view_transform;
            }

            if x.pointer.button_pressed(egui::PointerButton::Primary) {

                if let Some(mut p) = x.pointer.press_origin() {
                    p = p * 2.;

                    let mat = self.screen_transform * self.transform_pers * self.view_transform;
                    let mut wp = mat.inverse() * Vec4::new(p.x, p.y, 0., 1.);
                    wp = wp / wp.w;
                    let mut wp_f = mat.inverse() * Vec4::new(p.x, p.y, 10., 1.);
                    wp_f = wp_f / wp_f.w;
                    let mut dir = (wp_f - wp).xyz().normalize();
                    // println!("wp {:?}", wp);
                    // println!("wpf {:?}", wp_f);
                    // println!("dir {:?}", dir);

                    // Do ray marching
                    let mut t = 0.0;
                    for _ in 0..100 {
                        let rp = wp.xyz() + dir * t;
                        let near = lock.nodes().enumerate()
                            .min_by_key(|(i, n)| {
                                OrderedFloat((n.pos - rp).length() - 0.01)
                            })
                            .map(|(i, n)| {
                                (i, (n.pos - rp).length() - 0.01)
                            }).unwrap();

                        t += near.1;

                        if near.1 < 0.0001 {
                            // We have a hit
                            lock.nodes().nth( near.0 ).unwrap().selected = true;
                            break;
                        }

                        if near.1 > 1000. {
                            break;
                        }
                    }
                }
            }
        });

        let (nodes, lines) = lock.get_mesh();
        let mut positions = nodes.iter().map(
            |n| {
                RenderNode {
                    p: n.pos,
                    v: if n.selected { 1 } else { 0 }
                }
            }
        ).collect::<Vec<RenderNode>>();
        let edges = lines.iter().map(
            |p| {
                let p0 = positions[p.0].p;
                let p1 = positions[p.1].p;
                return (Vec4::new(p0.x, p0.y, p0.z, 0.), Vec4::new(p1.x, p1.y, p1.z, 1.));
            }).collect::<Vec<(Vec4, Vec4)>>();
        // let edges = lock.get_octree().mesh_lines();
        self.graph_renderer.lock().unwrap().graph_data(self.physics_components.node_count(), self.physics_components.node_buffer(), positions, edges);

        // Show selected nodes' details
        // for n in self.selected_nodes.iter() {
        //     let node = &lock.get_nodes_mut()[*n];
        //     let mut screen_pos = self.screen_transform_pers * self.view_transform * Vec4::new(node.pos.x, node.pos.y, node.pos.z, 1.0);
        //     screen_pos = screen_pos / screen_pos.w;
        //
        //     egui::Window::new(format!("Node {}", n))
        //         .resizable(false)
        //         .title_bar(false)
        //         .anchor(Align2::LEFT_BOTTOM, [screen_pos.x / 2., screen_pos.y / 2. - 900.])
        //         .show(context, |ui| unsafe {
        //             ui.label(format!("{}", n));
        //         });
        //
        // }

        // Show details on hover
        // if let Some(p) = context.pointer_latest_pos() {
        //     for (id, node) in lock.get_nodes_mut().iter().enumerate() {
        //         let mut screen_pos = self.screen_transform_pers * self.view_transform * Vec4::new(node.pos.x, node.pos.y, node.pos.z, 1.0);
        //         screen_pos = screen_pos / screen_pos.w;
        //
        //         let dist = (Vec2::new(screen_pos.x, screen_pos.y) - Vec2::new(p.x, p.y) * 2.).length();
        //         if dist < 15. {
        //
        //             if self.selected_nodes.contains(&id) {
        //                 // Already selected nodes already have their details shown
        //                 continue;
        //             }
        //
        //             egui::Window::new(format!("Node {}", id))
        //                 .title_bar(false)
        //                 .resizable(false)
        //                 .anchor(Align2::LEFT_BOTTOM, [screen_pos.x / 2., screen_pos.y / 2. - 900.])
        //                 .show(context, |ui| unsafe {
        //                     ui.label(format!("{}", id));
        //                 });
        //
        //             break;
        //         }
        //     }
        // }

        egui::Window::new("Nodes")
            .resizable(true)
            .title_bar(true)
            .show(context, |ui| unsafe {
                ui.label("Edge attraction");
                ui.add(
                    Slider::new(lock.get_edge_strength(), 0.0..=900.0)
                );
                ui.label("Repulsion");
                ui.add(
                    Slider::new(self.physics_components.repulsion(), 0.0..=1.0)
                );
                ui.label("Center attraction");
                ui.add(
                    Slider::new(lock.get_center_attraction_mut(), 0.0..=20200.0)
                );

                ui.label("Theta");
                ui.add(
                    Slider::new(lock.get_bh_theta(), 0.0..=2.0)
                );

                ui.add(Checkbox::new(&mut self.perspective_camera, "Use perspective camera"));

                ui.add(Checkbox::new(&mut lock.bh_physics(), "B-h physics"));

                ui.add(Checkbox::new(&mut lock.run_physics(), "simulate"));

                if self.perspective_camera {
                    self.graph_renderer.lock().unwrap().transform(self.transform_pers * self.view_transform);
                } else {
                    self.graph_renderer.lock().unwrap().transform(self.screen_transform_ortho * self.view_transform);
                }

                // if ui.button("Randomize").clicked() {
                //     lock.randomize();
                // }

                // if ui.button("Connect").clicked() {
                //     for n in self.selected_nodes.chunks(2) {
                //         lock.add_edge(n[0], n[1]);
                //     }
                // }

                // if ui.button("Remove connection").clicked() {
                //     for n in self.selected_nodes.chunks(2) {
                //         lock.delete_edge(n[0], n[1]);
                //     }
                // }

                // if ui.button("Add node").clicked() {
                //     if let Some(n) = self.selected_nodes.first() {
                //         lock.add_node();
                //         let id = lock.get_nodes_mut().len() - 1;
                //         lock.add_edge(*n, id);
                //     }
                // }
            });
    }
}

impl RenderComponent for Application {
    fn initialize(&mut self, renderer: &mut Renderer) {
        self.physics_components.initialize(renderer);
        self.graph_renderer.lock().unwrap().initialize(renderer);
    }

    fn render(&mut self, renderer: &mut Renderer, command_buffer: &mut CommandBuffer, swapchain_image: &Image, swapchain_image_view: &ImageView) {
        self.physics_components.render(renderer, command_buffer, swapchain_image, swapchain_image_view);
        self.graph_renderer.lock().unwrap().render(renderer, command_buffer, swapchain_image, swapchain_image_view);
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
            .vsync(false),
        application.clone(),
        Some(application.clone())
    );

    Ok(())
}
