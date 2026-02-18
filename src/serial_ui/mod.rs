//! # Serial UI Module
//!
//! This module provides the user interface components for serial port communication,
//! using fixed layout regions composed of:
//! - Left `egui::SidePanel`: serial port selection & configuration
//! - Central `egui::CentralPanel`: data receive window + input/editor
//! - Right `egui::SidePanel` (conditional): LLM related info when enabled
//!
//! This version uses egui's built-in memory persistence for configuration storage:
//! - Panel widths are stored in `egui::Memory::data` using `insert_persisted`/`get_persisted`
//! - Configuration is automatically serialized to `config/app_memory.ron` on app exit
//! - Configuration is loaded from the same file on startup

pub mod ui;

use crate::serial::Serials;
use bevy::app::AppExit;
use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPlugin, EguiPrimaryContextPass, egui};
use egui_sgr;
use serde::{Deserialize, Serialize};

/// Converts bytes to string, skipping control characters but preserving ANSI sequences.
/// The ANSI sequences will be processed by egui_sgr later.
fn bytes_to_str_with_ansi(data: &[u8]) -> String {
    let mut result = String::with_capacity(data.len());
    let mut i = 0;
    while i < data.len() {
        let b = data[i];
        // Skip NULL and CR (carriage return)
        // CR is normalized to LF in process_raw_bytes, but we skip it here as a safety measure
        if b == 0x00 || b == 0x0D {
            i += 1;
            continue;
        }
        // Valid single byte ASCII (including ESC for ANSI sequences)
        if b < 0x80 {
            result.push(b as char);
            i += 1;
            continue;
        }
        // Multi-byte UTF-8
        let len = if b & 0xE0 == 0xC0 {
            2
        } else if b & 0xF0 == 0xE0 {
            3
        } else if b & 0xF8 == 0xF0 {
            4
        } else {
            i += 1;
            continue;
        };
        if i + len <= data.len()
            && let Ok(s) = std::str::from_utf8(&data[i..i + len])
        {
            result.push_str(s);
        }
        i += len;
    }
    result
}
use ui::{
    Selected, console_mode_ui, data_line_feed_ui, data_type_ui, draw_baud_rate_selector,
    draw_data_bits_selector, draw_flow_control_selector, draw_parity_selector,
    draw_select_serial_ui, draw_serial_context_label_ui, draw_serial_context_ui,
    draw_serial_setting_ui, draw_stop_bits_selector, draw_timeout_selector, llm_ui, timestamp_ui,
};

/// Configuration file path for egui memory persistence.
const CONFIG_FILE: &str = "config/app_memory.ron";

/// Unique ID for storing panel widths in egui memory.
const PANEL_WIDTHS_ID: &str = "panel_widths";

/// Resource storing current (and persisted) side panel widths.
/// Uses serde for serialization with egui's persistence system.
#[derive(Resource, Clone, Serialize, Deserialize, PartialEq)]
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

impl PanelWidths {
    /// Clamp widths to valid ranges.
    fn clamp(&mut self) {
        self.left_width = self.left_width.clamp(120.0, 600.0);
        self.right_width = self.right_width.clamp(160.0, 800.0);
    }
}

/// System: initialize panel widths resource with defaults.
/// The actual loading from egui memory happens in the first UI frame.
fn init_panel_widths(mut commands: Commands) {
    commands.insert_resource(PanelWidths::default());
}

/// Load configuration from disk file into egui memory on startup.
fn load_config_from_disk(ctx: &egui::Context) {
    // Ensure config directory exists
    if let Err(e) = std::fs::create_dir_all("config") {
        eprintln!("[serial_ui] Failed to create config directory: {e}");
        return;
    }

    // Try to load and deserialize the memory file
    if let Ok(data) = std::fs::read_to_string(CONFIG_FILE) {
        match ron::from_str::<PanelWidths>(&data) {
            Ok(widths) => {
                let mut widths = widths;
                widths.clamp();
                ctx.memory_mut(|mem| {
                    mem.data.insert_persisted(egui::Id::new(PANEL_WIDTHS_ID), widths);
                });
                log::info!("[serial_ui] Loaded panel widths from config file");
            }
            Err(e) => {
                log::warn!("[serial_ui] Failed to parse config file: {e}, using defaults");
            }
        }
    }
}

/// Save configuration from egui memory to disk file on exit.
fn save_config_to_disk(ctx: &egui::Context) {
    log::info!("[serial_ui] Saving configuration to disk...");
    let widths = ctx.memory_mut(|mem| {
        mem.data.get_persisted::<PanelWidths>(egui::Id::new(PANEL_WIDTHS_ID))
            .unwrap_or_default()
    });
    
    log::info!("[serial_ui] Panel widths to save: left={}, right={}", widths.left_width, widths.right_width);

    // Ensure config directory exists
    if let Err(e) = std::fs::create_dir_all("config") {
        eprintln!("[serial_ui] Failed to create config directory: {e}");
        return;
    }

    match ron::to_string(&widths) {
        Ok(data) => {
            if let Err(e) = std::fs::write(CONFIG_FILE, data) {
                eprintln!("[serial_ui] Failed to write config file: {e}");
            } else {
                log::info!("[serial_ui] Saved panel widths to config file");
            }
        }
        Err(e) => {
            eprintln!("[serial_ui] Failed to serialize config: {e}");
        }
    }
}

/// System: save configuration when app is exiting.
/// Uses Last schedule to ensure it runs even during app shutdown.
fn save_config_on_exit(
    mut contexts: EguiContexts,
    mut exit_events: MessageReader<AppExit>,
) {
    if !exit_events.is_empty() {
        exit_events.clear();
        log::info!("[serial_ui] App exit detected, saving configuration...");
        if let Ok(ctx) = contexts.ctx_mut() {
            save_config_to_disk(&ctx);
        } else {
            log::warn!("[serial_ui] Could not get egui context to save config");
        }
    }
}

/// System: load config from disk on first frame and sync to resource.
/// Uses a local state to track if config has been loaded.
fn load_config_and_sync(
    mut contexts: EguiContexts,
    mut panel_widths: ResMut<PanelWidths>,
    mut loaded: Local<bool>,
) {
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };

    // Load from disk only once
    if !*loaded {
        load_config_from_disk(&ctx);
        *loaded = true;
    }

    // Sync resource from egui memory
    let widths = ctx.memory_mut(|mem| {
        mem.data.get_persisted::<PanelWidths>(egui::Id::new(PANEL_WIDTHS_ID))
            .unwrap_or_default()
    });

    *panel_widths = widths;
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
            .add_systems(Startup, init_panel_widths)
            .add_systems(Last, save_config_on_exit)  // Use Last schedule for exit handling
            .add_systems(
                EguiPrimaryContextPass,
                (
                    load_config_and_sync,  // Load from disk and sync resource
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
                        draw_timeout_selector(ui, &mut serial);
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

    // Update egui memory with new left width
    ctx.memory_mut(|mem| {
        let stored = mem.data.get_persisted_mut_or_insert_with(
            egui::Id::new(PANEL_WIDTHS_ID),
            PanelWidths::default,
        );
        stored.left_width = panel_widths.left_width;
    });

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
        let input_height = 140.0; // Reserve height for input area (increased for bottom spacing)
        let data_height = available_height - input_height;

        // Data receive area with fixed height
        for serial in &mut serials_data.serial {
            let Ok(mut serial) = serial.lock() else {
                continue;
            };
            if selected.is_selected(&serial.set.port_name) {
                let data = serial.data().read_current_source_file_bytes();
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
                            let text = bytes_to_str_with_ansi(&data);

                            // Use AnsiParser to get colored segments with color info
                            let mut parser = egui_sgr::AnsiParser::new();
                            let colored_segments = parser.parse(&text);

                            // Strategy: Track current line content and flush when we see newline
                            let mut current_line: Vec<(
                                String,
                                Option<egui::Color32>,
                                Option<egui::Color32>,
                            )> = Vec::new();

                            for seg in &colored_segments {
                                let seg_text = &seg.text;
                                let fg = seg.foreground_color;
                                let bg = seg.background_color;

                                let mut chars = seg_text.chars().peekable();
                                let mut current_part = String::new();

                                while let Some(ch) = chars.next() {
                                    if ch == '\n' {
                                        // Flush current part to line
                                        if !current_part.is_empty() {
                                            current_line.push((current_part.clone(), fg, bg));
                                            current_part.clear();
                                        }
                                        // Flush the line
                                        if !current_line.is_empty() {
                                            ui.horizontal(|ui| {
                                                for (text, fg, bg) in &current_line {
                                                    let mut rt =
                                                        egui::RichText::new(text).monospace();
                                                    if let Some(color) = fg {
                                                        rt = rt.color(*color);
                                                    }
                                                    if let Some(color) = bg {
                                                        rt = rt.background_color(*color);
                                                    }
                                                    ui.label(rt);
                                                }
                                            });
                                            current_line.clear();
                                        }
                                    } else {
                                        current_part.push(ch);
                                    }
                                }

                                // Add remaining part to current line
                                if !current_part.is_empty() {
                                    current_line.push((current_part, fg, bg));
                                }
                            }

                            // Flush any remaining content (last line without newline)
                            if !current_line.is_empty() {
                                ui.horizontal(|ui| {
                                    for (text, fg, bg) in &current_line {
                                        let mut rt = egui::RichText::new(text).monospace();
                                        if let Some(color) = fg {
                                            rt = rt.color(*color);
                                        }
                                        if let Some(color) = bg {
                                            rt = rt.background_color(*color);
                                        }
                                        ui.label(rt);
                                    }
                                });
                            }
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
                            timestamp_ui(ui, &mut serial);
                            console_mode_ui(ui, &mut serial);
                            llm_ui(ui, &mut serial);
                        });

                        // Text input area with improved bottom margin
                        let available_height = ui.available_height() - 40.0; // Adjusted margin for better aesthetics
                        let font = egui::FontId::new(18.0, egui::FontFamily::Monospace);
                        ui.add_sized(
                            [ui.available_width(), available_height],
                            egui::TextEdit::multiline(
                                serial.data().get_cache_data().get_current_data(),
                            )
                            .font(font)
                            .desired_width(f32::INFINITY),
                        );

                        // Add extra vertical space at bottom to ensure distance from edge
                        ui.add_space(20.0); // Increased space to prevent overlap with window bottom
                    }
                }
            },
        );

        // Add additional space after input area to ensure distance from window bottom
        ui.add_space(5.0);
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
        // Update egui memory with new right width
        ctx.memory_mut(|mem| {
            let stored = mem.data.get_persisted_mut_or_insert_with(
                egui::Id::new(PANEL_WIDTHS_ID),
                PanelWidths::default,
            );
            stored.right_width = panel_widths.right_width;
        });
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
