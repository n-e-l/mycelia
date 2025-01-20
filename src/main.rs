use std::sync::{Arc, Mutex};
use ash::vk;
use cen::app::App;
use cen::app::app::AppConfig;
use cen::app::gui::GuiComponent;
use cen::graphics::Renderer;
use cen::graphics::renderer::RenderComponent;
use cen::vulkan::CommandBuffer;
use dotenv::dotenv;
use egui::TextWrapMode;
use crate::database::Database;

mod database;

struct Application {
    database: Database,
    concepts: Vec<String>,
    messages: Vec<String>,
}

impl Application {

    async fn new() -> Application {
        let database = Database::new().await;
        let concepts = database.get_concepts().await;
        let messages = database.get_messages().await;

        Self {
            database,
            concepts,
            messages,
        }
    }
}

impl RenderComponent for Application {
    fn initialize(&mut self, renderer: &mut Renderer) {
    }

    fn render(&mut self, renderer: &mut Renderer, command_buffer: &mut CommandBuffer, swapchain_image: &vk::Image, swapchain_image_view: &vk::ImageView) {

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

        // Clear the swapchain
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
    }
}

impl GuiComponent for Application {
    fn gui(&mut self, context: &egui::Context) {

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
