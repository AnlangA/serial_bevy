//! # Serial UI Module
//!
//! This module provides the user interface components for serial port communication,
//! using fixed layout regions composed of:
//! - Left `egui::SidePanel`: serial port selection & configuration
//! - Central `egui::CentralPanel`: data receive window + input/editor
//! - Right `egui::SidePanel` (conditional): LLM related info when enabled
//!
//! Configuration is persisted directly to `config/app_memory.ron` via serde,
//! independent of egui memory. LLM settings are saved regardless of whether
//! the LLM panel is currently visible.

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
    INPUT_PANEL_HEIGHT, INPUT_TOOLBAR_HEIGHT, MarkdownViewerCache, Selected, clear_log_ui,
    console_mode_ui, data_line_feed_ui, data_type_ui, draw_baud_rate_selector,
    draw_data_bits_selector, draw_flow_control_selector, draw_llm_coding_plan_toggle,
    draw_llm_conversation, draw_llm_input_area, draw_llm_key_input, draw_llm_model_selector,
    draw_parity_selector, draw_select_serial_ui, draw_serial_context_label_ui,
    draw_serial_context_ui, draw_serial_input_area, draw_serial_setting_ui, draw_sidebar_section,
    draw_stop_bits_selector, draw_timeout_selector, submit_serial_input, timestamp_ui,
};

/// Configuration file path for app persistence.
const CONFIG_FILE: &str = "config/app_memory.ron";

/// Resource storing current (and persisted) UI configuration.
/// Saved to disk directly, independent of egui memory.
#[derive(Resource, Clone, Serialize, Deserialize, PartialEq)]
pub struct PanelWidths {
    /// Current (user-adjustable) width of the left side panel.
    pub left_width: f32,
    /// Current (user-adjustable) width of the right side panel.
    pub right_width: f32,
    /// Whether the settings side panel is visible.
    #[serde(default = "default_true")]
    pub show_settings_panel: bool,
    /// Whether the LLVM side panel is visible.
    #[serde(default)]
    pub show_llm_panel: bool,
    /// Global LLM API key (shared across all serial ports).
    #[serde(default)]
    pub llm_key: String,
    /// Global LLM model selection (shared across all serial ports).
    #[serde(default)]
    pub llm_model: String,
    /// Global LLM coding plan toggle (shared across all serial ports).
    #[serde(default)]
    pub llm_with_coding_plan: bool,
    /// Whether to show the "missing API key" popup warning.
    #[serde(skip)]
    pub show_key_missing_popup: bool,
}

impl Default for PanelWidths {
    fn default() -> Self {
        Self {
            left_width: 160.0,
            right_width: 220.0,
            show_settings_panel: true,
            show_llm_panel: false,
            llm_key: String::new(),
            llm_model: String::from("glm-4.5-air"),
            llm_with_coding_plan: false,
            show_key_missing_popup: false,
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

const fn default_true() -> bool {
    true
}

/// Load configuration directly from disk file.
fn load_config_from_disk() -> Option<PanelWidths> {
    if let Ok(data) = std::fs::read_to_string(CONFIG_FILE) {
        match ron::from_str::<PanelWidths>(&data) {
            Ok(mut widths) => {
                widths.clamp();
                log::debug!("[serial_ui] Loaded panel config from disk");
                return Some(widths);
            }
            Err(e) => {
                log::warn!("[serial_ui] Failed to parse config file: {e}, using defaults");
            }
        }
    }
    None
}

/// Save configuration directly to disk file.
fn save_config_to_disk(widths: &PanelWidths) {
    log::debug!(
        "[serial_ui] Saving panel config to disk: left={}, right={}",
        widths.left_width,
        widths.right_width
    );

    if let Err(e) = std::fs::create_dir_all("config") {
        eprintln!("[serial_ui] Failed to create config directory: {e}");
        return;
    }

    match ron::to_string(widths) {
        Ok(data) => {
            if let Err(e) = std::fs::write(CONFIG_FILE, data) {
                eprintln!("[serial_ui] Failed to write config file: {e}");
            } else {
                log::debug!("[serial_ui] Saved panel config to disk");
            }
        }
        Err(e) => {
            eprintln!("[serial_ui] Failed to serialize config: {e}");
        }
    }
}

/// System: initialize panel config resource, loading from disk if available.
fn init_panel_widths(mut commands: Commands) {
    let config = load_config_from_disk().unwrap_or_default();
    commands.insert_resource(config);
}

/// System: save configuration directly from resource when app is exiting.
fn save_config_on_exit(panel_widths: Res<PanelWidths>, mut exit_events: MessageReader<AppExit>) {
    if !exit_events.is_empty() {
        exit_events.clear();
        log::debug!("[serial_ui] App exit detected, saving configuration...");
        save_config_to_disk(&panel_widths);
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
            .insert_resource(MarkdownViewerCache::default())
            .add_systems(Startup, setup_camera_system)
            .add_systems(Startup, init_panel_widths)
            .add_systems(Last, save_config_on_exit) // Use Last schedule for exit handling
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
    mut markdown_cache: ResMut<MarkdownViewerCache>,
) {
    let Ok(mut serials_data) = serials.single_mut() else {
        return;
    };
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };

    let mut selected_serial_exists = false;
    let mut selected_llm_enabled = false;
    for serial_ref in &mut serials_data.serial {
        let Ok(mut serial) = serial_ref.lock() else {
            continue;
        };
        if selected.is_selected(&serial.set.port_name) {
            selected_serial_exists = true;
            selected_llm_enabled = *serial.llm().enable();
            break;
        }
    }
    panel_widths.show_llm_panel = selected_llm_enabled;

    egui::TopBottomPanel::top("serial_ui_topbar").show(ctx, |ui| {
        ui.horizontal(|ui| {
            if ui
                .selectable_label(panel_widths.show_settings_panel, "Settings")
                .clicked()
            {
                panel_widths.show_settings_panel = !panel_widths.show_settings_panel;
            }

            let llvm_response = ui.add_enabled(
                selected_serial_exists,
                egui::Button::selectable(panel_widths.show_llm_panel, "LLVM"),
            );
            if llvm_response.clicked() {
                panel_widths.show_llm_panel = !panel_widths.show_llm_panel;
                for serial_ref in &mut serials_data.serial {
                    let Ok(mut serial) = serial_ref.lock() else {
                        continue;
                    };
                    if selected.is_selected(&serial.set.port_name) {
                        *serial.llm().enable() = panel_widths.show_llm_panel;
                        break;
                    }
                }
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                egui::widgets::global_theme_preference_switch(ui);
            });
        });
    });

    // ---------------- Left Side Panel ----------------
    if panel_widths.show_settings_panel {
        let left_show = egui::SidePanel::left("serial_ui_left")
            .resizable(true)
            .default_width(panel_widths.left_width)
            .min_width(120.0)
            .max_width(600.0)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        draw_sidebar_section(ui, "Connection", |ui| {
                            draw_select_serial_ui(ui, &mut serials_data, selected.as_mut());
                            ui.add_space(6.0);
                            draw_serial_setting_ui(ui, selected.as_mut());
                        });

                        ui.add_space(8.0);

                        draw_sidebar_section(ui, "Serial Settings", |ui| {
                            let mut drew_selected_serial = false;
                            for serial in &mut serials_data.serial {
                                let Ok(mut serial) = serial.lock() else {
                                    continue;
                                };
                                if selected.is_selected(&serial.set.port_name) {
                                    drew_selected_serial = true;
                                    draw_baud_rate_selector(ui, &mut serial);
                                    draw_data_bits_selector(ui, &mut serial);
                                    draw_stop_bits_selector(ui, &mut serial);
                                    draw_parity_selector(ui, &mut serial);
                                    draw_flow_control_selector(ui, &mut serial);
                                    draw_timeout_selector(ui, &mut serial);
                                    break;
                                }
                            }
                            if !drew_selected_serial {
                                ui.label(
                                    egui::RichText::new(
                                        "Select a port to edit its serial settings.",
                                    )
                                    .weak(),
                                );
                            }
                        });

                        ui.add_space(8.0);

                        draw_sidebar_section(ui, "LLM Settings", |ui| {
                            draw_llm_key_input(ui, &mut panel_widths);
                            draw_llm_model_selector(ui, &mut panel_widths);
                            draw_llm_coding_plan_toggle(ui, &mut panel_widths);
                        });
                        ui.add_space(8.0);
                    });
            });
        panel_widths.left_width = left_show.response.rect.width();
    }

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
        let input_height = INPUT_PANEL_HEIGHT; // Reserve height for input area and action buttons
        let data_height = (available_height - input_height).max(0.0);

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
                        ui.allocate_ui_with_layout(
                            egui::Vec2::new(ui.available_width(), INPUT_TOOLBAR_HEIGHT),
                            egui::Layout::left_to_right(egui::Align::Center),
                            |ui| {
                                data_type_ui(ui, &mut serial);
                                data_line_feed_ui(ui, &mut serial);
                                timestamp_ui(ui, &mut serial);
                                console_mode_ui(ui, &mut serial);
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        clear_log_ui(ui, &mut serial);
                                    },
                                );
                            },
                        );

                        // Text input area with improved bottom margin
                        draw_serial_input_area(ui, &mut serial);

                        // Add extra vertical space at bottom to ensure distance from edge
                        ui.add_space(8.0);
                    }
                }
            },
        );

        // Add additional space after input area to ensure distance from window bottom
        ui.add_space(5.0);
    });

    // ---------------- Right Side Panel (LLM) ----------------
    if panel_widths.show_llm_panel && selected_serial_exists {
        let right_show = egui::SidePanel::right("serial_ui_right")
            .resizable(true)
            .default_width(panel_widths.right_width)
            .min_width(200.0)
            .max_width(400.0)
            .show(ctx, |ui| {
                for serial_ref in &mut serials_data.serial {
                    let Ok(mut serial) = serial_ref.lock() else {
                        continue;
                    };
                    if selected.is_selected(&serial.set.port_name) {
                        let llm_input_height = INPUT_PANEL_HEIGHT;
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(format!("LLM: {}", serial.set.port_name))
                                    .strong(),
                            );
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui
                                        .button("Clear")
                                        .on_hover_text("Clear conversation history")
                                        .clicked()
                                    {
                                        serial.llm().clear_messages();
                                    }
                                },
                            );
                        });
                        ui.separator();
                        ui.allocate_ui_with_layout(
                            egui::Vec2::new(
                                ui.available_width(),
                                (ui.available_height() - llm_input_height).max(120.0),
                            ),
                            egui::Layout::top_down(egui::Align::LEFT),
                            |ui| {
                                draw_llm_conversation(ui, &mut serial, &mut markdown_cache);
                            },
                        );
                        ui.separator();
                        ui.allocate_ui_with_layout(
                            egui::Vec2::new(ui.available_width(), llm_input_height),
                            egui::Layout::top_down(egui::Align::LEFT),
                            |ui| {
                                ui.allocate_ui_with_layout(
                                    egui::Vec2::new(ui.available_width(), INPUT_TOOLBAR_HEIGHT),
                                    egui::Layout::left_to_right(egui::Align::Center),
                                    |_ui| {},
                                );
                                draw_llm_input_area(ui, &mut serial, &mut panel_widths);
                            },
                        );
                        ui.add_space(8.0);
                        break;
                    }
                }
                ui.add_space(5.0);
            });
        panel_widths.right_width = right_show.response.rect.width();
    }

    // Show "missing API key or model" popup if triggered
    if panel_widths.show_key_missing_popup {
        egui::Window::new("LLM Configuration Required")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                ui.label(
                    "Please enter your LLM API key and select a model in the left settings panel.",
                );
                ui.horizontal(|ui| {
                    ui.add_space(ui.available_width() / 2.0 - 40.0);
                    if ui.button("  OK  ").clicked() {
                        panel_widths.show_key_missing_popup = false;
                    }
                });
            });
    }
}

/// System: send cached data if newline present (user pressed Enter).
fn send_cache_data(mut serials: Query<&mut Serials>) {
    let Ok(mut serials) = serials.single_mut() else {
        return;
    };
    for serial in &mut serials.serial {
        let Ok(mut serial) = serial.lock() else {
            continue;
        };
        if serial.is_open() {
            let should_submit = {
                let current = serial.data().get_cache_data().get_current_data();
                current.contains('\r') || current.contains('\n')
            };
            if should_submit {
                submit_serial_input(&mut serial);
            }
        }
    }
}

/// System: navigate cached input history with Up/Down arrows for current open port.
fn history_data_checkout(
    mut serials: Query<&mut Serials>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    selected: ResMut<Selected>,
    mut contexts: EguiContexts,
) {
    // Skip if egui is capturing keyboard input (e.g. user is typing in a text field)
    if let Ok(ctx) = contexts.ctx_mut() {
        if ctx.wants_keyboard_input() {
            return;
        }
    }

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
