//! # Serial UI Module
//!
//! This module provides the user interface components for serial port communication,
//! using layout regions composed of:
//! - Left `egui::Panel`: serial port selection & configuration
//! - Central `egui::CentralPanel`: data receive window + input/editor
//!
//! The side panel is resizable and its width is persisted in the user's config
//! directory, with portable fallbacks for restricted environments.

pub mod ui;

use crate::serial::{MAX_COMMAND_INPUT_CHARS, Serials};
use bevy::app::AppExit;
use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPlugin, EguiPrimaryContextPass, egui};
use log::warn;
use std::io;
use std::path::PathBuf;
use ui::{
    Selected, data_line_feed_ui, data_type_ui, draw_baud_rate_selector, draw_data_bits_selector,
    draw_flow_control_selector, draw_parity_selector, draw_select_serial_ui,
    draw_serial_context_label_ui, draw_serial_context_ui, draw_serial_setting_ui,
    draw_stop_bits_selector, log_timeout_ui, timestamp_ui,
};

/// Panel width persistence file name.
const PANEL_WIDTHS_FILE: &str = "panel_widths.txt";

/// Resource storing the current (and persisted) side panel width.
#[derive(Resource, Clone)]
pub struct PanelWidths {
    /// Current (user-adjustable) width of the left side panel.
    pub left_width: f32,
}

impl Default for PanelWidths {
    fn default() -> Self {
        Self { left_width: 160.0 }
    }
}

/// Attempt to load panel widths from disk; fall back to defaults if parsing fails.
fn load_panel_widths_from_disk() -> PanelWidths {
    load_panel_widths_from_paths(panel_width_path_candidates())
}

fn user_config_directory() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    let base = std::env::var_os("APPDATA").map(PathBuf::from);

    #[cfg(target_os = "macos")]
    let base = std::env::var_os("HOME")
        .map(PathBuf::from)
        .map(|home| home.join("Library").join("Application Support"));

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .map(|home| home.join(".config"))
        });

    base.filter(|path| path.is_absolute())
        .map(|path| path.join("serial_bevy"))
}

fn panel_width_path_candidates() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(config_directory) = user_config_directory() {
        paths.push(config_directory.join(PANEL_WIDTHS_FILE));
    }

    // Keep reading the legacy working-directory file so existing preferences
    // are migrated naturally on the next successful save.
    paths.push(PathBuf::from(PANEL_WIDTHS_FILE));

    if let Ok(executable) = std::env::current_exe()
        && let Some(parent) = executable.parent()
    {
        paths.push(parent.join(PANEL_WIDTHS_FILE));
    }
    paths.push(
        std::env::temp_dir()
            .join("serial_bevy")
            .join(PANEL_WIDTHS_FILE),
    );
    let mut unique_paths = Vec::with_capacity(paths.len());
    for path in paths {
        if !unique_paths.contains(&path) {
            unique_paths.push(path);
        }
    }
    unique_paths
}

fn load_panel_widths_from_paths(paths: impl IntoIterator<Item = PathBuf>) -> PanelWidths {
    paths
        .into_iter()
        .find_map(|path| {
            std::fs::read_to_string(path)
                .ok()
                .and_then(|raw| parse_panel_widths(&raw))
        })
        .unwrap_or_default()
}

fn parse_panel_widths(raw: &str) -> Option<PanelWidths> {
    let mut parts = raw.split_whitespace();
    let left_width = parts.next()?.parse::<f32>().ok()?;
    // Accept one legacy right-panel value written by older versions.
    if let Some(legacy_right_width) = parts.next()
        && !legacy_right_width.parse::<f32>().ok()?.is_finite()
    {
        return None;
    }
    if parts.next().is_some() || !left_width.is_finite() {
        return None;
    }

    Some(PanelWidths {
        left_width: left_width.clamp(120.0, 600.0),
    })
}

/// Persist panel widths (best-effort).
fn save_panel_widths_to_disk(widths: &PanelWidths) {
    if let Err(e) = save_panel_widths_to_paths(widths, panel_width_path_candidates()) {
        warn!("Failed to write panel widths: {e}");
    }
}

fn save_panel_widths_to_paths(
    widths: &PanelWidths,
    paths: impl IntoIterator<Item = PathBuf>,
) -> io::Result<PathBuf> {
    let data = widths.left_width.to_string();
    let mut failures = Vec::new();

    for path in paths {
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            && let Err(error) = std::fs::create_dir_all(parent)
        {
            failures.push(format!("{}: {error}", parent.display()));
            continue;
        }

        match std::fs::write(&path, &data) {
            Ok(()) => return Ok(path),
            Err(error) => failures.push(format!("{}: {error}", path.display())),
        }
    }

    Err(io::Error::other(format!(
        "no writable preferences path ({})",
        failures.join("; ")
    )))
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
                (serial_ui, draw_serial_context_ui).chain(),
            );
    }
}

/// Composite UI: a resizable side panel and the central serial console.
fn serial_ui(
    mut contexts: EguiContexts,
    mut serials: Query<&mut Serials>,
    mut selected: ResMut<Selected>,
    mut panel_widths: ResMut<PanelWidths>,
) {
    let Ok(mut serials_data) = serials.single_mut() else {
        return;
    };
    if !serials_data
        .serial
        .iter()
        .any(|serial| selected.is_selected(&serial.set.port_name))
    {
        selected.select(
            serials_data
                .serial
                .first()
                .map_or("", |serial| &serial.set.port_name),
        );
    }
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };
    let mut viewport_ui = egui::Ui::new(
        ctx.clone(),
        "serial_bevy_root".into(),
        egui::UiBuilder::new()
            .layer_id(egui::LayerId::background())
            .max_rect(ctx.viewport_rect()),
    );

    // ---------------- Left Side Panel ----------------
    let left_show = egui::Panel::left("serial_ui_left")
        .resizable(true)
        .default_size(panel_widths.left_width)
        .min_size(120.0)
        .max_size(600.0)
        .show(&mut viewport_ui, |ui| {
            ui.horizontal(|ui| {
                ui.strong("Serial Ports");
                egui::widgets::global_theme_preference_switch(ui);
            });
            let ports_height = (ui.available_height() - 280.0).max(80.0);
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .max_height(ports_height)
                .show(ui, |ui| {
                    draw_select_serial_ui(ui, &mut serials_data, selected.as_mut());
                });

            ui.separator();
            draw_serial_setting_ui(ui, selected.as_mut());
            for serial in &mut serials_data.serial {
                if selected.is_selected(&serial.set.port_name) {
                    let settings_enabled = serial.is_close();
                    ui.add_enabled_ui(settings_enabled, |ui| {
                        draw_baud_rate_selector(ui, serial);
                        draw_data_bits_selector(ui, serial);
                        draw_stop_bits_selector(ui, serial);
                        draw_parity_selector(ui, serial);
                        draw_flow_control_selector(ui, serial);
                    });
                    if !settings_enabled {
                        ui.small("Close the port to change connection settings.");
                    }
                    log_timeout_ui(ui, serial);
                    if let Some(path) = serial.data().source_path() {
                        ui.small(format!("Log: {}", path.display()))
                            .on_hover_text("Current session-log path");
                    }
                }
            }
        });
    panel_widths.left_width = left_show.response.rect.width();

    // ---------------- Central Panel ----------------
    egui::CentralPanel::default().show(&mut viewport_ui, |ui| {
        // Tab-like labels for active serials
        egui::ScrollArea::horizontal().show(ui, |ui| {
            ui.horizontal(|ui| {
                for serial in &mut serials_data.serial {
                    draw_serial_context_label_ui(ui, selected.as_mut(), serial);
                }
            });
        });
        ui.separator();

        // Use remaining vertical space for data receive area
        let available_height = ui.available_height();
        let input_height = 80.0;
        let data_height = (available_height - input_height).max(0.0);

        // Data receive area with fixed height
        for serial in &mut serials_data.serial {
            if selected.is_selected(&serial.set.port_name) {
                let port_name = serial.set.port_name.clone();
                let data = serial.data().recent_log();
                egui::ScrollArea::vertical()
                    .stick_to_bottom(true)
                    .auto_shrink([false, false])
                    .max_height(data_height)
                    .show(ui, |ui| {
                        if data.is_empty() {
                            ui.heading(
                                egui::RichText::new(format!("{} Data Receive Window", port_name))
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
                    if selected.is_selected(&serial.set.port_name) {
                        // Control buttons at top of input area
                        ui.horizontal(|ui| {
                            data_type_ui(ui, serial);
                            data_line_feed_ui(ui, serial);
                            timestamp_ui(ui, serial);
                        });

                        let font = egui::FontId::new(18.0, egui::FontFamily::Monospace);
                        let is_open = serial.is_open();
                        let (editor_response, send_clicked) = ui
                            .horizontal(|ui| {
                                let editor_width = (ui.available_width() - 64.0).max(40.0);
                                let response = ui
                                    .add_enabled(
                                        is_open,
                                        egui::TextEdit::singleline(
                                            serial.data().get_cache_data().get_current_data(),
                                        )
                                        .font(font)
                                        .char_limit(MAX_COMMAND_INPUT_CHARS)
                                        .desired_width(editor_width)
                                        .hint_text("Enter a command"),
                                    )
                                    .on_disabled_hover_text(
                                        "Open the port before entering a command",
                                    );
                                let send_clicked =
                                    ui.add_enabled(is_open, egui::Button::new("Send")).clicked();
                                (response, send_clicked)
                            })
                            .inner;

                        let enter_pressed = ui.input(|input| {
                            input.key_pressed(egui::Key::Enter)
                                && (editor_response.has_focus() || editor_response.lost_focus())
                        });
                        if editor_response.has_focus()
                            && ui.input(|input| input.key_pressed(egui::Key::ArrowUp))
                        {
                            serial.data().get_cache_data().previous_history();
                        } else if editor_response.has_focus()
                            && ui.input(|input| input.key_pressed(egui::Key::ArrowDown))
                        {
                            serial.data().get_cache_data().next_history();
                        }

                        if send_clicked || enter_pressed {
                            submit_cached_data(serial);
                            editor_response.request_focus();
                        }
                        if !serial.is_error()
                            && let Some(message) = serial.last_error()
                        {
                            ui.colored_label(egui::Color32::RED, message);
                        }
                    }
                }
            },
        );
    });
}

fn submit_cached_data(serial: &mut crate::serial::Serial) {
    let data = serial
        .data()
        .get_cache_data()
        .get_current_data()
        .replace(['\r', '\n'], "");
    let append_line_feed = *serial.data().line_feed();

    if data.is_empty() && !append_line_feed {
        return;
    }

    let data_type = *serial.data().data_type();
    if let Err(error) = crate::serial::encode_string(&data, data_type) {
        serial.report_error(error.to_string());
        return;
    }

    if !serial.data().send_data(data.clone()) {
        serial.report_error("Pending send queue is full; wait for it to drain and retry");
        return;
    }

    let _ = serial.data().get_cache_data().take_current_data();
    if !data.is_empty() {
        serial
            .data()
            .get_cache_data()
            .add_history_data(data.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panel_widths_are_clamped() {
        let widths = parse_panel_widths("10 900").unwrap();

        assert_eq!(widths.left_width, 120.0);
    }

    #[test]
    fn invalid_panel_widths_are_rejected() {
        assert!(parse_panel_widths("NaN 200").is_none());
        assert_eq!(parse_panel_widths("160").unwrap().left_width, 160.0);
        assert!(parse_panel_widths("160 260 extra").is_none());
    }

    #[test]
    fn panel_width_loading_skips_invalid_candidates() {
        let root = unique_test_directory("panel_load");
        std::fs::create_dir_all(&root).unwrap();
        let invalid = root.join("invalid.txt");
        let valid = root.join("valid.txt");
        std::fs::write(&invalid, "not-a-width").unwrap();
        std::fs::write(&valid, "245").unwrap();

        let widths = load_panel_widths_from_paths([invalid, valid]);

        assert_eq!(widths.left_width, 245.0);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn panel_width_saving_uses_the_next_writable_path() {
        let root = unique_test_directory("panel_save");
        std::fs::create_dir_all(&root).unwrap();
        let blocked_parent = root.join("not-a-directory");
        std::fs::write(&blocked_parent, "blocked").unwrap();
        let fallback = root.join("fallback").join(PANEL_WIDTHS_FILE);

        let written = save_panel_widths_to_paths(
            &PanelWidths { left_width: 321.0 },
            [blocked_parent.join(PANEL_WIDTHS_FILE), fallback.clone()],
        )
        .unwrap();

        assert_eq!(written, fallback);
        assert_eq!(std::fs::read_to_string(&written).unwrap(), "321");
        std::fs::remove_dir_all(root).unwrap();
    }

    fn unique_test_directory(label: &str) -> PathBuf {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "serial_bevy_{label}_{}_{}",
            std::process::id(),
            unique
        ))
    }

    #[test]
    fn submit_cached_data_queues_and_clears_input() {
        let mut serial = crate::serial::Serial::new();
        serial
            .data()
            .get_cache_data()
            .get_current_data()
            .push_str("hello");

        submit_cached_data(&mut serial);

        assert!(serial.data().get_cache_data().get_current_data().is_empty());
        assert_eq!(serial.data().get_send_data(), vec!["hello"]);
    }

    #[test]
    fn full_send_queue_preserves_the_editor_input() {
        let mut serial = crate::serial::Serial::new();
        assert!(
            serial
                .data()
                .send_data("x".repeat(MAX_COMMAND_INPUT_CHARS * 4))
        );
        serial
            .data()
            .get_cache_data()
            .get_current_data()
            .push_str("retry me");

        submit_cached_data(&mut serial);

        assert_eq!(
            serial.data().get_cache_data().get_current_data(),
            "retry me"
        );
        assert!(serial.last_error().is_some());
    }

    #[test]
    fn invalid_hex_input_is_preserved_for_correction() {
        let mut serial = crate::serial::Serial::new();
        *serial.data().data_type() = crate::serial::DataType::Hex;
        serial
            .data()
            .get_cache_data()
            .get_current_data()
            .push_str("GG");

        submit_cached_data(&mut serial);

        assert_eq!(serial.data().get_cache_data().get_current_data(), "GG");
        assert!(serial.data().get_send_data().is_empty());
        assert!(serial.last_error().is_some());
    }
}
