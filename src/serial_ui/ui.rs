//! # UI Components Module
//!
//! This module provides individual UI components for serial port configuration and control.

use crate::serial::Serials;
use crate::serial::port::{COMMON_BAUD_RATES, DataType, PortChannelData, Serial, TEXT_MODELS};
use bevy::prelude::*;
use bevy_egui::{EguiContexts, egui};
use egui_commonmark::{CommonMarkCache, CommonMarkViewer};

use std::sync::MutexGuard;
use tokio_serial::{DataBits, FlowControl, Parity, StopBits};

/// Shared text edit height for serial and LLM input boxes.
pub const INPUT_TEXT_EDIT_HEIGHT: f32 = 84.0;

/// Shared bottom panel height so serial and LLM input regions stay aligned.
pub const INPUT_PANEL_HEIGHT: f32 = 180.0;

/// Shared top toolbar height for bottom input regions.
pub const INPUT_TOOLBAR_HEIGHT: f32 = 28.0;

const SIDEBAR_LABEL_WIDTH: f32 = 74.0;

#[derive(Resource, Default)]
pub struct MarkdownViewerCache(pub CommonMarkCache);

fn sidebar_row<R>(
    ui: &mut egui::Ui,
    label: &str,
    add_value: impl FnOnce(&mut egui::Ui, f32) -> R,
) -> R {
    ui.horizontal(|ui| {
        ui.add_sized([SIDEBAR_LABEL_WIDTH, 20.0], egui::Label::new(label));
        let value_width = ui.available_width().max(90.0);
        add_value(ui, value_width)
    })
    .inner
}

pub fn draw_sidebar_section(
    ui: &mut egui::Ui,
    title: &str,
    add_content: impl FnOnce(&mut egui::Ui),
) {
    ui.group(|ui| {
        ui.set_width(ui.available_width());
        ui.label(egui::RichText::new(title).strong());
        ui.add_space(6.0);
        add_content(ui);
    });
}

/// Resource for tracking the currently selected serial port.
#[derive(Resource, Default)]
pub struct Selected {
    /// The name of the selected port.
    selected: String,
}

impl Selected {
    /// Returns true if the given port name is selected.
    #[must_use]
    pub fn is_selected(&self, port_name: &str) -> bool {
        self.selected == port_name
    }

    /// Selects the given port.
    pub fn select(&mut self, port_name: &str) {
        self.selected = port_name.to_string();
    }

    /// Returns the selected port name.
    #[must_use]
    pub fn selected(&self) -> &str {
        &self.selected
    }
}

/// Draws the serial port selection dropdown and open/close button for the selected port.
pub fn draw_select_serial_ui(ui: &mut egui::Ui, serials: &mut Serials, selected: &mut Selected) {
    sidebar_row(ui, "Port", |ui, width| {
        let selected_text = if selected.selected().is_empty() {
            "Select a port".to_string()
        } else {
            selected.selected().to_string()
        };

        egui::ComboBox::from_id_salt("serial_port_selector")
            .width((width - 58.0).max(80.0))
            .selected_text(selected_text)
            .show_ui(ui, |ui| {
                for serial in &mut serials.serial {
                    let Ok(serial) = serial.lock() else {
                        continue;
                    };
                    if ui
                        .selectable_label(
                            selected.is_selected(&serial.set.port_name),
                            &serial.set.port_name,
                        )
                        .clicked()
                    {
                        selected.select(&serial.set.port_name);
                    }
                }
            });

        for serial in &mut serials.serial {
            let Ok(mut serial) = serial.lock() else {
                continue;
            };
            if selected.is_selected(&serial.set.port_name) {
                open_ui(ui, &mut serial, selected);
                return;
            }
        }

        ui.add_enabled(false, egui::Button::new("Open"));
    });
}

/// Draws the baud rate selector.
pub fn draw_baud_rate_selector(ui: &mut egui::Ui, serial: &mut MutexGuard<'_, Serial>) {
    sidebar_row(ui, "Baud Rate", |ui, width| {
        egui::ComboBox::from_id_salt(format!("{}_baud", serial.set.port_name))
            .width(width)
            .selected_text(serial.set.baud_rate().to_string())
            .show_ui(ui, |ui| {
                for baud_rate in COMMON_BAUD_RATES {
                    ui.selectable_value(serial.set.baud_rate(), *baud_rate, baud_rate.to_string())
                        .on_hover_text("Select baud rate");
                }
            })
    });
}

/// Draws the data bits selector.
pub fn draw_data_bits_selector(ui: &mut egui::Ui, serial: &mut MutexGuard<'_, Serial>) {
    sidebar_row(ui, "Data Bits", |ui, width| {
        egui::ComboBox::from_id_salt(format!("{}_data", serial.set.port_name))
            .width(width)
            .selected_text(serial.set.data_size().to_string())
            .show_ui(ui, |ui| {
                for bits in [
                    DataBits::Five,
                    DataBits::Six,
                    DataBits::Seven,
                    DataBits::Eight,
                ] {
                    ui.selectable_value(serial.set.data_size(), bits, format!("{bits}"));
                }
            })
    });
}

/// Draws the stop bits selector.
pub fn draw_stop_bits_selector(ui: &mut egui::Ui, serial: &mut MutexGuard<'_, Serial>) {
    sidebar_row(ui, "Stop Bits", |ui, width| {
        egui::ComboBox::from_id_salt(format!("{}_stop", serial.set.port_name))
            .width(width)
            .selected_text(serial.set.stop_bits().to_string())
            .show_ui(ui, |ui| {
                for bits in [StopBits::One, StopBits::Two] {
                    ui.selectable_value(serial.set.stop_bits(), bits, format!("{bits}"));
                }
            })
    });
}

/// Draws the flow control selector.
pub fn draw_flow_control_selector(ui: &mut egui::Ui, serial: &mut MutexGuard<'_, Serial>) {
    sidebar_row(ui, "Flow Ctrl", |ui, width| {
        egui::ComboBox::from_id_salt(format!("{}_flow", serial.set.port_name))
            .width(width)
            .selected_text(serial.set.flow_control().to_string())
            .show_ui(ui, |ui| {
                for flow in [
                    FlowControl::None,
                    FlowControl::Software,
                    FlowControl::Hardware,
                ] {
                    ui.selectable_value(serial.set.flow_control(), flow, format!("{flow}"));
                }
            })
    });
}

/// Draws the parity selector.
pub fn draw_parity_selector(ui: &mut egui::Ui, serial: &mut MutexGuard<'_, Serial>) {
    sidebar_row(ui, "Parity", |ui, width| {
        egui::ComboBox::from_id_salt(format!("{}_parity", serial.set.port_name))
            .width(width)
            .selected_text(serial.set.parity().to_string())
            .show_ui(ui, |ui| {
                for parity in [Parity::None, Parity::Odd, Parity::Even] {
                    ui.selectable_value(serial.set.parity(), parity, format!("{parity}"));
                }
            })
    });
}

/// Draws the timeout selector.
pub fn draw_timeout_selector(ui: &mut egui::Ui, serial: &mut MutexGuard<'_, Serial>) {
    sidebar_row(ui, "Timeout", |ui, width| {
        // Convert timeout from Duration to milliseconds for display (capped at u64::MAX)
        let timeout_ms = serial.set.timeout.as_millis().min(u64::MAX.into()) as u64;

        egui::ComboBox::from_id_salt(format!("{}_timeout", serial.set.port_name))
            .width(width)
            .selected_text(format!("{timeout_ms} ms"))
            .show_ui(ui, |ui| {
                // Common timeout values in milliseconds
                for &timeout_opt in &[1, 5, 10, 50, 100, 500, 1000, 2000, 5000] {
                    if ui
                        .selectable_label(timeout_ms == timeout_opt, format!("{timeout_opt} ms"))
                        .clicked()
                    {
                        *serial.set.timeout() = std::time::Duration::from_millis(timeout_opt);
                    }
                }
            })
    });
}

/// Draws the open/close port button.
pub fn open_ui(ui: &mut egui::Ui, serial: &mut MutexGuard<'_, Serial>, selected: &mut Selected) {
    if serial.is_close() {
        if ui.button("Open").clicked() {
            selected.select(&serial.set.port_name);
            debug!("Opening port {}", serial.set.port_name);

            // Clone settings before borrowing tx_channel to avoid borrow conflict
            let settings = serial.set.clone();
            if let Some(tx) = serial.tx_channel() {
                match tx.send(PortChannelData::PortOpen(settings)) {
                    Ok(_) => {
                        debug!("Sent open port message");
                    }
                    Err(e) => warn!("Failed to open port: {e}"),
                }
                let _ = std::fs::create_dir_all("logs");
                let time = chrono::Local::now().format("%Y%m%d_%H%M%S_%f").to_string();
                let port_name = &serial.set.port_name;
                let safe_port = port_name.trim_start_matches('/').replace('/', "_");
                let file_name = format!("logs/{}_{}.txt", safe_port, time);
                serial.data().add_source_file(file_name);
            }
        }
    } else if serial.is_open() && ui.button("Close").clicked() {
        selected.select(&serial.set.port_name);
        debug!("Closing port {}", serial.set.port_name);
        let port_name = serial.set.port_name.clone();

        if let Some(tx) = serial.tx_channel() {
            match tx.send(PortChannelData::PortClose(port_name)) {
                Ok(_) => {
                    debug!("Sent close port message");
                }
                Err(e) => warn!("Failed to close port: {e}"),
            }
        }
    }
}

/// Draws the serial setting status UI.
pub fn draw_serial_setting_ui(ui: &mut egui::Ui, selected: &mut Selected) {
    sidebar_row(ui, "Selected", |ui, width| {
        let text = if selected.selected().is_empty() {
            "No port selected"
        } else {
            selected.selected()
        };
        ui.add_sized(
            [width, 20.0],
            egui::Label::new(egui::RichText::new(text).weak()).truncate(),
        );
    });
}

/// Draws the serial context label in the tab bar.
pub fn draw_serial_context_label_ui(
    ui: &mut egui::Ui,
    selected: &mut Selected,
    serial: &mut MutexGuard<'_, Serial>,
) {
    if serial.is_open()
        && ui
            .selectable_label(
                selected.is_selected(&serial.set.port_name),
                egui::RichText::new(&serial.set.port_name),
            )
            .clicked()
    {
        selected.select(&serial.set.port_name);
    }
}

/// Draws error windows for ports in error state.
pub fn draw_serial_context_ui(serials: Query<&Serials>, mut context: EguiContexts) {
    let Ok(serials) = serials.single() else {
        return;
    };

    let Ok(ctx) = context.ctx_mut() else {
        return;
    };

    for serial in &serials.serial {
        let Ok(mut serial) = serial.lock() else {
            continue;
        };
        if serial.is_error() {
            egui::Window::new(format!("{} Error", serial.set.port_name)).show(ctx, |ui| {
                ui.label(
                    egui::RichText::new(format!("{} Error", serial.set.port_name))
                        .color(egui::Color32::RED)
                        .strong(),
                );
                if ui.button("Clear Error").clicked() {
                    serial.close();
                }
            });
        }
    }
}

/// Draws the data type selector.
pub fn data_type_ui(ui: &mut egui::Ui, serial: &mut MutexGuard<'_, Serial>) {
    ui.add(egui::Label::new(egui::RichText::new("Data Type:")));
    egui::ComboBox::from_id_salt(format!("{}_datatype", serial.set.port_name))
        .width(90f32)
        .selected_text(serial.data().data_type().as_str_en())
        .show_ui(ui, |ui| {
            for data_type in [
                DataType::Hex,
                DataType::Utf8,
                DataType::Ascii,
                DataType::Binary,
                DataType::Utf16,
                DataType::Utf32,
                DataType::Gbk,
            ] {
                ui.selectable_value(serial.data().data_type(), data_type, data_type.as_str_en());
            }
        });
}

/// Draws the line feed toggle button.
pub fn data_line_feed_ui(ui: &mut egui::Ui, serial: &mut MutexGuard<'_, Serial>) {
    ui.horizontal(|ui| {
        let (button_text, hover_text) = if *serial.data().line_feed() {
            ("No LF", "Disable line feed in sent data")
        } else {
            ("With LF", "Include line feed in sent data")
        };

        if ui.button(button_text).on_hover_text(hover_text).clicked() {
            *serial.data().line_feed() = !*serial.data().line_feed();
        }
    });
}

/// Draws the clear-log button for the current serial log view.
pub fn clear_log_ui(ui: &mut egui::Ui, serial: &mut MutexGuard<'_, Serial>) {
    if ui
        .button("Clear Log")
        .on_hover_text("Clear the current serial log view")
        .clicked()
    {
        serial.data().clear_display_buffer();
    }
}

/// Draws the model selector for LLM (global config).
pub fn draw_llm_model_selector(ui: &mut egui::Ui, config: &mut crate::serial_ui::PanelWidths) {
    sidebar_row(ui, "Model", |ui, width| {
        egui::ComboBox::from_id_salt("llm_model_selector")
            .width(width)
            .selected_text(&config.llm_model)
            .show_ui(ui, |ui| {
                for (model_id, display_name) in TEXT_MODELS {
                    ui.selectable_value(&mut config.llm_model, model_id.to_string(), *display_name);
                }
            })
    });
}

/// Draws the API key input for LLM (global config).
pub fn draw_llm_key_input(ui: &mut egui::Ui, config: &mut crate::serial_ui::PanelWidths) {
    sidebar_row(ui, "API Key", |ui, width| {
        ui.add(
            egui::TextEdit::singleline(&mut config.llm_key)
                .password(true)
                .desired_width(width),
        );
    });
}

/// Draws the coding plan toggle for LLM (global config).
pub fn draw_llm_coding_plan_toggle(ui: &mut egui::Ui, config: &mut crate::serial_ui::PanelWidths) {
    sidebar_row(ui, "Coding", |ui, _width| {
        let with_coding = config.llm_with_coding_plan;
        let button_text = if with_coding {
            "Coding: ON"
        } else {
            "Coding: OFF"
        };
        if ui
            .button(button_text)
            .on_hover_text("Toggle coding plan mode")
            .clicked()
        {
            config.llm_with_coding_plan = !with_coding;
        }
    });
}

/// Draws the conversation history for LLM with bubble chat styling.
pub fn draw_llm_conversation(
    ui: &mut egui::Ui,
    serial: &mut MutexGuard<'_, Serial>,
    markdown_cache: &mut MarkdownViewerCache,
) {
    let visuals = ui.visuals().clone();
    let available_height = ui.available_height().max(120.0);

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .max_height(available_height)
        .stick_to_bottom(true)
        .show(ui, |ui| {
            for msg in &serial.llm().messages {
                let is_user = msg.role == "user";

                // Choose bubble colors based on role and theme
                let (bubble_color, text_color, role_color, role_text) = if is_user {
                    (
                        egui::Color32::from_rgb(37, 99, 235),
                        egui::Color32::WHITE,
                        egui::Color32::from_rgb(59, 130, 246),
                        "You",
                    )
                } else if visuals.dark_mode {
                    (
                        egui::Color32::from_rgb(55, 65, 81),
                        egui::Color32::from_rgb(229, 231, 235),
                        egui::Color32::from_rgb(16, 185, 129),
                        "AI",
                    )
                } else {
                    (
                        egui::Color32::from_rgb(243, 244, 246),
                        egui::Color32::from_rgb(31, 41, 55),
                        egui::Color32::from_rgb(5, 150, 105),
                        "AI",
                    )
                };

                // Align user messages to the right, AI to the left
                ui.with_layout(
                    egui::Layout::top_down(if is_user {
                        egui::Align::RIGHT
                    } else {
                        egui::Align::LEFT
                    })
                    .with_cross_align(if is_user {
                        egui::Align::RIGHT
                    } else {
                        egui::Align::LEFT
                    }),
                    |ui| {
                        // Message header: role + timestamp
                        ui.horizontal(|ui| {
                            if is_user {
                                ui.label(egui::RichText::new(&msg.timestamp).weak().small());
                                ui.label(egui::RichText::new(role_text).strong().color(role_color));
                            } else {
                                ui.label(egui::RichText::new(role_text).strong().color(role_color));
                                ui.label(egui::RichText::new(&msg.timestamp).weak().small());
                            }
                        });

                        // Bubble frame
                        let frame = egui::Frame::new()
                            .fill(bubble_color)
                            .corner_radius(10.0)
                            .inner_margin(egui::Margin::symmetric(12, 10));
                        frame.show(ui, |ui| {
                            let max_w = ui.available_width().min(280.0);
                            ui.set_max_width(max_w);
                            render_message_content(
                                ui,
                                &msg.content,
                                text_color,
                                &mut markdown_cache.0,
                            );
                        });
                    },
                );

                ui.add_space(10.0);
            }

            if serial.llm().is_processing {
                ui.with_layout(
                    egui::Layout::top_down(egui::Align::LEFT).with_cross_align(egui::Align::LEFT),
                    |ui| {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label(
                                egui::RichText::new("AI is thinking...")
                                    .italics()
                                    .color(egui::Color32::GRAY),
                            );
                        });
                    },
                );
                ui.add_space(4.0);
            }
        });
}

/// Renders message content with code block highlighting.
fn render_message_content(
    ui: &mut egui::Ui,
    content: &str,
    default_color: egui::Color32,
    markdown_cache: &mut CommonMarkCache,
) {
    if content.trim().is_empty() {
        return;
    }

    ui.scope(|ui| {
        let mut style = ui.style().as_ref().clone();
        style.visuals.override_text_color = Some(default_color);
        style.visuals.hyperlink_color = if default_color == egui::Color32::WHITE {
            egui::Color32::from_rgb(191, 219, 254)
        } else {
            egui::Color32::from_rgb(37, 99, 235)
        };
        style.url_in_tooltip = true;
        ui.set_style(style);

        CommonMarkViewer::new()
            .indentation_spaces(2)
            .show(ui, markdown_cache, content);
    });
}

/// Draws the input area and send button for LLM with multi-line support.
pub fn draw_llm_input_area(
    ui: &mut egui::Ui,
    serial: &mut MutexGuard<'_, Serial>,
    config: &mut crate::serial_ui::PanelWidths,
) {
    let font = egui::FontId::new(18.0, egui::FontFamily::Monospace);
    let can_send = !serial.llm().input_buffer.trim().is_empty() && !serial.llm().is_processing;

    ui.vertical(|ui| {
        ui.add_sized(
            [ui.available_width(), INPUT_TEXT_EDIT_HEIGHT],
            egui::TextEdit::multiline(&mut serial.llm().input_buffer)
                .hint_text("Ask AI...")
                .font(font),
        );
        ui.add_space(6.0);

        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    can_send,
                    egui::Button::new(egui::RichText::new("Send").strong()),
                )
                .clicked()
            {
                submit_llm_input(serial, config);
            }

            if ui.button("Clear").clicked() {
                serial.llm().input_buffer.clear();
            }

            if serial.llm().is_processing {
                ui.label(egui::RichText::new("Waiting for response...").weak());
            } else if config.llm_key.is_empty() || config.llm_model.is_empty() {
                ui.label(egui::RichText::new("Set key/model to enable sending").weak());
            }
        });
    });
}

/// Draws the main serial input area and its action buttons.
pub fn draw_serial_input_area(ui: &mut egui::Ui, serial: &mut MutexGuard<'_, Serial>) {
    let font = egui::FontId::new(18.0, egui::FontFamily::Monospace);
    let can_send =
        serial.is_open() && !serial.data().get_cache_data().get_current_data().is_empty();

    ui.add_sized(
        [ui.available_width(), INPUT_TEXT_EDIT_HEIGHT],
        egui::TextEdit::multiline(serial.data().get_cache_data().get_current_data())
            .hint_text("Type data to send...")
            .font(font)
            .desired_width(f32::INFINITY),
    );
    ui.add_space(6.0);

    ui.horizontal(|ui| {
        if ui
            .add_enabled(
                can_send,
                egui::Button::new(egui::RichText::new("Send").strong()),
            )
            .clicked()
        {
            submit_serial_input(serial);
        }

        if ui.button("Clear").clicked() {
            serial.data().get_cache_data().clear_current_data();
        }

        if ui.button("Prev").clicked() {
            serial.data().get_cache_data().sub_history_index();
            let index = serial.data().get_cache_data().get_current_data_index();
            *serial.data().get_cache_data().get_current_data() =
                serial.data().get_cache_data().get_history_data(index);
        }

        if ui.button("Next").clicked() {
            serial.data().get_cache_data().add_history_index();
            let index = serial.data().get_cache_data().get_current_data_index();
            *serial.data().get_cache_data().get_current_data() =
                serial.data().get_cache_data().get_history_data(index);
        }

        if !serial.is_open() {
            ui.label(egui::RichText::new("Open the port before sending").weak());
        }
    });
}

/// Queues the current serial input for sending.
pub fn submit_serial_input(serial: &mut Serial) -> bool {
    if !serial.is_open() {
        return false;
    }

    let cache = serial.data().get_cache_data().get_current_data().clone();
    if cache.is_empty() {
        return false;
    }

    let data = if *serial.data().line_feed() {
        if cache.contains('\r') || cache.contains('\n') {
            cache.clone()
        } else {
            format!("{cache}\n")
        }
    } else {
        cache.replace(['\r', '\n'], "")
    };
    let history_data = cache.replace(['\r', '\n'], "");
    if history_data.is_empty() {
        return false;
    }

    serial
        .data()
        .get_cache_data()
        .add_history_data(history_data);
    serial.data().send_data(data);
    serial.data().get_cache_data().clear_current_data();
    true
}

/// Submits the current LLM input if configuration is complete.
pub fn submit_llm_input(serial: &mut Serial, config: &mut crate::serial_ui::PanelWidths) -> bool {
    if config.llm_key.is_empty() || config.llm_model.is_empty() {
        config.show_settings_panel = true;
        config.show_key_missing_popup = true;
        return false;
    }

    if serial.llm().is_processing {
        return false;
    }

    let content = serial.llm().input_buffer.trim().to_string();
    if content.is_empty() {
        return false;
    }

    serial.llm().add_user_message(&content);
    serial.llm().input_buffer.clear();
    serial.llm().is_processing = true;
    true
}

/// Draws the console mode toggle button.
/// Console mode provides better terminal experience for Linux serial consoles:
/// - No local echo (terminal handles echo)
/// - Raw data logging (no timestamps)
pub fn console_mode_ui(ui: &mut egui::Ui, serial: &mut MutexGuard<'_, Serial>) {
    ui.horizontal(|ui| {
        let console_mode = *serial.data().console_mode();
        let (button_text, hover_text) = if console_mode {
            ("Console ON", "Console mode enabled. Terminal handles echo. Toggle to disable.")
        } else {
            ("Console OFF", "Enable console mode for Linux serial terminal experience (no local echo, raw data)")
        };

        let button = ui.button(button_text).on_hover_text(hover_text);
        if button.clicked() {
            *serial.data().console_mode() = !console_mode;
            serial.data().clear_utf8_buffer();
        }
    });
}

/// Draws the timestamp display toggle button.
/// When enabled, shows timestamps and send/receive indicators in the log.
/// When disabled (default), shows raw data for cleaner display.
pub fn timestamp_ui(ui: &mut egui::Ui, serial: &mut MutexGuard<'_, Serial>) {
    ui.horizontal(|ui| {
        let show_timestamp = *serial.data().show_timestamp();
        let (button_text, hover_text) = if show_timestamp {
            (
                "Time ON",
                "Timestamps enabled. Toggle to hide timestamps and source indicators.",
            )
        } else {
            ("Time OFF", "Enable timestamps and send/receive indicators")
        };

        let button = ui.button(button_text).on_hover_text(hover_text);
        if button.clicked() {
            *serial.data().show_timestamp() = !show_timestamp;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_selected_default() {
        let selected = Selected::default();
        assert!(selected.selected().is_empty());
    }

    #[test]
    fn test_selected_operations() {
        let mut selected = Selected::default();
        selected.select("COM1");
        assert!(selected.is_selected("COM1"));
        assert!(!selected.is_selected("COM2"));
        assert_eq!(selected.selected(), "COM1");
    }
}
