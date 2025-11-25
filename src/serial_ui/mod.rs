//! # Serial UI Module
//!
//! This module provides the user interface components for serial port communication.
//! It uses the `bevy_egui` crate for GUI rendering.

pub mod ui;

use crate::serial::Serials;
use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPlugin, egui};
use ui::{
    Selected, data_line_feed_ui, data_type_ui, draw_baud_rate_selector, draw_data_bits_selector,
    draw_flow_control_selector, draw_parity_selector, draw_select_serial_ui,
    draw_serial_context_label_ui, draw_serial_context_ui, draw_serial_setting_ui,
    draw_stop_bits_selector, llm_ui,
};

/// Plugin for the serial port user interface.
///
/// This plugin provides:
/// - Theme customization
/// - Serial port selection and configuration
/// - Data display and input
pub struct SerialUiPlugin;

impl Plugin for SerialUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(EguiPlugin::default())
            .insert_resource(ClearColor(Color::srgb(0.96875, 0.96875, 0.96875)))
            .insert_resource(Selected::default())
            .add_systems(Startup, ui_init)
            .add_systems(
                Update,
                (
                    serial_ui,
                    draw_serial_context_ui,
                    send_cache_data,
                    history_data_checkout,
                )
                    .chain(),
            );
    }
}

/// Initializes the UI theme and fonts.
fn ui_init(mut ctx: EguiContexts, _commands: Commands) {
    let Ok(ctx) = ctx.ctx_mut() else {
        return;
    };

    let mut fonts = egui::FontDefinitions::default();

    // Install custom Chinese font
    fonts.font_data.insert(
        "Song".to_owned(),
        egui::FontData::from_static(include_bytes!("../../assets/fonts/STSong.ttf")).into(),
    );
    fonts.families.insert(
        egui::FontFamily::Name("Song".into()),
        vec!["Song".to_owned()],
    );

    // Set as primary proportional font
    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .insert(0, "Song".to_owned());

    // Add as fallback for monospace
    fonts
        .families
        .entry(egui::FontFamily::Monospace)
        .or_default()
        .push("Song".to_owned());

    ctx.set_fonts(fonts);
    ctx.set_theme(egui::Theme::Light);
}

/// Main serial UI system.
fn serial_ui(
    mut contexts: EguiContexts,
    mut serials: Query<&mut Serials>,
    mut selected: ResMut<Selected>,
) {
    let Ok(mut serials_data) = serials.single_mut() else {
        return;
    };

    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };

    // Left panel - port selection and settings
    egui::SidePanel::left("serial_ui_left")
        .resizable(false)
        .min_width(120.0)
        .max_width(120.0)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                egui::widgets::global_theme_preference_switch(ui);
            });
            ui.separator();
            egui::ScrollArea::both().show(ui, |ui| {
                draw_select_serial_ui(ui, &mut serials_data, selected.as_mut());
            });
            ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
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
                draw_serial_setting_ui(ui, selected.as_mut());
            });
        });

    // Central panel - data display and input
    egui::CentralPanel::default().show(ctx, |ui| {
        ui.horizontal(|ui| {
            for serial in &mut serials_data.serial {
                let Ok(mut serial) = serial.lock() else {
                    continue;
                };
                draw_serial_context_label_ui(ui, selected.as_mut(), &mut serial);
            }
        });
        ui.separator();

        for serial in &mut serials_data.serial {
            let Ok(mut serial) = serial.lock() else {
                continue;
            };
            if selected.is_selected(&serial.set.port_name) {
                let data = serial.data().read_current_source_file();
                egui::ScrollArea::both()
                    .min_scrolled_width(ui.available_width() - 20.)
                    .max_width(ui.available_width() - 20.)
                    .max_height(ui.available_height() - 127.)
                    .stick_to_bottom(true)
                    .auto_shrink(egui::Vec2b::FALSE)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.add_space(20.);
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
                    });
            }
        }

        ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
            for serial in &mut serials_data.serial {
                let Ok(mut serial) = serial.lock() else {
                    continue;
                };
                if selected.is_selected(&serial.set.port_name) {
                    let font = egui::FontId::new(18.0, egui::FontFamily::Monospace);
                    ui.add(
                        egui::TextEdit::multiline(
                            serial.data().get_cache_data().get_current_data(),
                        )
                        .font(font)
                        .desired_width(ui.available_width()),
                    );
                    ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                        ui.horizontal(|ui| {
                            data_type_ui(ui, &mut serial);
                            data_line_feed_ui(ui, &mut serial);
                            llm_ui(ui, &mut serial);
                        });
                    });
                    ui.separator();
                }
            }
        });
    });

    // Right panel - LLM (if enabled)
    let llm_info = {
        let mut result = None;
        for serial_ref in &mut serials_data.serial {
            let Ok(mut serial) = serial_ref.lock() else {
                continue;
            };
            if selected.is_selected(&serial.set.port_name) && *serial.llm().enable() {
                result = Some(serial.set.port_name.clone());
                break;
            }
        }
        result
    };

    if let Some(port_name) = llm_info {
        egui::SidePanel::right("serial_ui_right")
            .resizable(false)
            .min_width(240.0)
            .max_width(240.0)
            .show(ctx, |ui| {
                ui.label(&port_name);
            });
    }
}

/// Sends cached data when Enter is pressed.
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

/// Handles history navigation with arrow keys.
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
