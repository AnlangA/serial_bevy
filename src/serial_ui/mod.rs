//! # Serial UI Module
//!
//! This module provides the user interface components for serial port communication,
//! using fixed layout regions composed of:
//! - Left `egui::SidePanel`: serial port selection & configuration
//! - Central `egui::CentralPanel`: data receive window + input/editor
//! - Right `egui::SidePanel` (conditional): LLM related info when enabled
//!
//! This version restores side panels (not floating windows) and introduces:
//! 1. Resizable side panels (`resizable(true)`).
//! 2. Persistence of panel widths across runs (simple text file `panel_widths.txt`).
//!
//! File persistence is deliberately minimal (no extra crates).
//! Format of `panel_widths.txt`: `<left_width> <right_width>`
//!
//! If the file is missing or invalid, defaults are used. Widths are saved on application exit.

pub mod ui;

use crate::serial::Serials;
use bevy::app::AppExit;
use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPlugin, EguiPrimaryContextPass, egui};
use ui::{
    Selected, data_line_feed_ui, data_type_ui, draw_baud_rate_selector, draw_data_bits_selector,
    draw_flow_control_selector, draw_parity_selector, draw_select_serial_ui,
    draw_serial_context_label_ui, draw_serial_context_ui, draw_serial_setting_ui,
    draw_stop_bits_selector, llm_ui,
};

/// Panel width persistence file name.
const PANEL_WIDTHS_FILE: &str = "panel_widths.txt";

/// Resource storing current (and persisted) side panel widths.
#[derive(Resource, Clone)]
pub struct PanelWidths {
    /// Current (user-adjustable) width of the left side panel.
    pub left_width: f32,
    /// Current (user-adjustable) width of the right side panel.
    pub right_width: f32,
}

impl Default for PanelWidths {
    fn default() -> Self {
        Self {
            left_width: 160.0,
            right_width: 260.0,
        }
    }
}

/// Attempt to load panel widths from disk; fall back to defaults if parsing fails.
fn load_panel_widths_from_disk() -> PanelWidths {
    if let Ok(raw) = std::fs::read_to_string(PANEL_WIDTHS_FILE) {
        let parts: Vec<_> = raw.split_whitespace().collect();
        if parts.len() == 2
            && let (Ok(lw), Ok(rw)) = (parts[0].parse::<f32>(), parts[1].parse::<f32>())
        {
            return PanelWidths {
                left_width: lw.clamp(120.0, 600.0),
                right_width: rw.clamp(160.0, 800.0),
            };
        }
    }
    PanelWidths::default()
}

/// Persist panel widths (best-effort).
fn save_panel_widths_to_disk(widths: &PanelWidths) {
    let data = format!("{} {}", widths.left_width, widths.right_width);
    if let Err(e) = std::fs::write(PANEL_WIDTHS_FILE, data) {
        eprintln!("[serial_ui] Failed to write panel widths: {e}");
    }
}

/// System: load panel widths at startup.
fn load_panel_widths(mut commands: Commands) {
    commands.insert_resource(load_panel_widths_from_disk());
}

/// System: save panel widths when app is exiting.
fn save_panel_widths_on_exit(panel_widths: Res<PanelWidths>, exit_events: MessageReader<AppExit>) {
    if !exit_events.is_empty() {
        save_panel_widths_to_disk(&panel_widths);
    }
}

/// Plugin for the serial UI.
pub struct SerialUiPlugin;

fn setup_camera_system(mut commands: Commands) {
    // Basic 2D camera required for egui overlay.
    commands.spawn(Camera2d);
}

impl Plugin for SerialUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(EguiPlugin::default())
            .insert_resource(ClearColor(Color::srgb(0.96875, 0.96875, 0.96875)))
            .insert_resource(Selected::default())
            .add_systems(Startup, setup_camera_system)
            .add_systems(Startup, load_panel_widths)
            .add_systems(PostUpdate, save_panel_widths_on_exit)
            .add_systems(
                EguiPrimaryContextPass,
                (
                    serial_ui,              // main UI layout
                    draw_serial_context_ui, // error popup windows
                    send_cache_data,        // auto-send on newline
                    history_data_checkout,  // input history navigation
                )
                    .chain(),
            );
    }
}

/// Composite UI: left & right side panels (resizable, persistent widths) + central content.
fn serial_ui(
    mut contexts: EguiContexts,
    mut serials: Query<&mut Serials>,
    mut selected: ResMut<Selected>,
    mut panel_widths: ResMut<PanelWidths>,
) {
    let Ok(mut serials_data) = serials.single_mut() else {
        return;
    };
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };

    // ---------------- Left Side Panel ----------------
    let left_show = egui::SidePanel::left("serial_ui_left")
        .resizable(true)
        .default_width(panel_widths.left_width)
        .min_width(120.0)
        .max_width(600.0)
        .show(ctx, |ui| {
            // Top-aligned theme switch and port list (no spacing), settings anchored at bottom via nested bottom_up layout.
            // Layout strategy:
            // 1. Render top header (theme switch) flush at top.
            // 2. Render port list directly beneath.
            // 3. Separator.
            // 4. Bottom-up block: settings content pinned to panel bottom.
            ui.horizontal(|ui| {
                egui::widgets::global_theme_preference_switch(ui);
            });
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    draw_select_serial_ui(ui, &mut serials_data, selected.as_mut());
                });

            ui.separator();

            // Bottom anchored settings block
            ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                ui.add_space(20.0);
                for serial in &mut serials_data.serial {
                    let Ok(mut serial) = serial.lock() else {
                        continue;
                    };
                    if selected.is_selected(&serial.set.port_name) {
                        draw_flow_control_selector(ui, &mut serial);
                        draw_parity_selector(ui, &mut serial);
                        draw_stop_bits_selector(ui, &mut serial);
                        draw_data_bits_selector(ui, &mut serial);
                        draw_baud_rate_selector(ui, &mut serial);
                    }
                }
                ui.separator();
                draw_serial_setting_ui(ui, selected.as_mut());
            });
        });
    panel_widths.left_width = left_show.response.rect.width();

    // ---------------- Central Panel ----------------
    egui::CentralPanel::default().show(ctx, |ui| {
        // Tab-like labels for active serials
        ui.horizontal(|ui| {
            for serial in &mut serials_data.serial {
                let Ok(mut serial) = serial.lock() else {
                    continue;
                };
                draw_serial_context_label_ui(ui, selected.as_mut(), &mut serial);
            }
        });
        ui.separator();

        // Use remaining vertical space for data receive area
        let available_height = ui.available_height();
        let input_height = 120.0; // Reserve height for input area
        let data_height = available_height - input_height;

        // Data receive area with fixed height
        for serial in &mut serials_data.serial {
            let Ok(mut serial) = serial.lock() else {
                continue;
            };
            if selected.is_selected(&serial.set.port_name) {
                let data = serial.data().read_current_source_file();
                egui::ScrollArea::vertical()
                    .stick_to_bottom(true)
                    .auto_shrink([false, false])
                    .max_height(data_height)
                    .show(ui, |ui| {
                        if data.is_empty() {
                            ui.heading(
                                egui::RichText::new(format!(
                                    "{} Data Receive Window",
                                    serial.set.port_name
                                ))
                                .color(egui::Color32::GRAY),
                            );
                        } else {
                            ui.monospace(egui::RichText::new(data));
                        }
                    });
            }
        }

        // Add separator before input area
        ui.separator();

        // Bottom input area with fixed height
        ui.allocate_ui_with_layout(
            egui::Vec2::new(ui.available_width(), input_height),
            egui::Layout::top_down(egui::Align::LEFT),
            |ui| {
                for serial in &mut serials_data.serial {
                    let Ok(mut serial) = serial.lock() else {
                        continue;
                    };
                    if selected.is_selected(&serial.set.port_name) {
                        // Control buttons at top of input area
                        ui.horizontal(|ui| {
                            data_type_ui(ui, &mut serial);
                            data_line_feed_ui(ui, &mut serial);
                            llm_ui(ui, &mut serial);
                        });
                        
                        // Text input area
                        let available_height = ui.available_height() - 30.0; // Leave space for margins
                        let font = egui::FontId::new(18.0, egui::FontFamily::Monospace);
                        ui.add_sized(
                            [ui.available_width(), available_height],
                            egui::TextEdit::multiline(
                                serial.data().get_cache_data().get_current_data(),
                            )
                            .font(font)
                            .desired_width(f32::INFINITY),
                        );
                    }
                }
            },
        );
    });

    // ---------------- Right Side Panel (LLM) ----------------
    let mut llm_enabled_for_selected = false;
    let mut llm_port_name = String::new();
    for serial_ref in &mut serials_data.serial {
        let Ok(mut serial) = serial_ref.lock() else {
            continue;
        };
        if selected.is_selected(&serial.set.port_name) && *serial.llm().enable() {
            llm_enabled_for_selected = true;
            llm_port_name = serial.set.port_name.clone();
            break;
        }
    }

    if llm_enabled_for_selected {
        let right_show = egui::SidePanel::right("serial_ui_right")
            .resizable(true)
            .default_width(panel_widths.right_width)
            .min_width(160.0)
            .max_width(800.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!("LLM Port: {llm_port_name}"))
                            .strong()
                            .color(egui::Color32::from_rgb(40, 40, 160)),
                    );
                });
                ui.separator();
                ui.label("LLM 功能侧边栏（可拓展：对话、分析、日志等）");
            });
        panel_widths.right_width = right_show.response.rect.width();
    }
}

/// System: send cached data if newline present (user pressed Enter).
fn send_cache_data(mut serials: Query<&mut Serials>) {
    for mut serials in &mut serials {
        for serial in &mut serials.serial {
            let Ok(mut serial) = serial.lock() else {
                continue;
            };
            if serial.is_open() {
                let cache = serial.data().get_cache_data().get_current_data().clone();
                if cache.contains('\r') || cache.contains('\n') {
                    let data = if *serial.data().line_feed() {
                        cache.clone()
                    } else {
                        cache.replace(['\r', '\n'], "")
                    };
                    let history_data = data.replace(['\r', '\n'], "");
                    serial
                        .data()
                        .get_cache_data()
                        .add_history_data(history_data);
                    serial.data().send_data(data);
                    serial.data().get_cache_data().clear_current_data();
                }
            }
        }
    }
}

/// System: navigate cached input history with Up/Down arrows for current open port.
fn history_data_checkout(
    mut serials: Query<&mut Serials>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    selected: ResMut<Selected>,
) {
    let Ok(mut serials) = serials.single_mut() else {
        return;
    };
    for serial in &mut serials.serial {
        let Ok(mut serial) = serial.lock() else {
            continue;
        };
        if selected.is_selected(&serial.set.port_name) && serial.is_open() {
            if keyboard_input.just_pressed(KeyCode::ArrowUp) {
                serial.data().get_cache_data().sub_history_index();
                let index = serial.data().get_cache_data().get_current_data_index();
                *serial.data().get_cache_data().get_current_data() =
                    serial.data().get_cache_data().get_history_data(index);
            }
            if keyboard_input.just_pressed(KeyCode::ArrowDown) {
                serial.data().get_cache_data().add_history_index();
                let index = serial.data().get_cache_data().get_current_data_index();
                *serial.data().get_cache_data().get_current_data() =
                    serial.data().get_cache_data().get_history_data(index);
            }
        }
    }
}
