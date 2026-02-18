//! # UI Components Module
//!
//! This module provides individual UI components for serial port configuration and control.

use crate::serial::Serials;
use crate::serial::port::{COMMON_BAUD_RATES, DataType, PortChannelData, Serial};
use bevy::prelude::*;
use bevy_egui::{EguiContexts, egui};
use log::info;
use std::sync::MutexGuard;
use tokio_serial::{DataBits, FlowControl, Parity, StopBits};

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

/// Draws the serial port selection list.
pub fn draw_select_serial_ui(ui: &mut egui::Ui, serials: &mut Serials, selected: &mut Selected) {
    for serial in &mut serials.serial {
        let Ok(mut serial) = serial.lock() else {
            continue;
        };
        ui.horizontal(|ui| {
            // Removed custom color usage; default theme color will be applied.

            if ui
                .selectable_label(
                    selected.is_selected(&serial.set.port_name),
                    egui::RichText::new(&serial.set.port_name).strong(),
                )
                .clicked()
            {
                selected.select(&serial.set.port_name);
            }
            open_ui(ui, &mut serial, selected);
        });
    }
}

/// Draws the baud rate selector.
pub fn draw_baud_rate_selector(ui: &mut egui::Ui, serial: &mut MutexGuard<'_, Serial>) {
    ui.horizontal(|ui| {
        ui.label("Baud Rate");
        egui::ComboBox::from_id_salt(format!("{}_baud", serial.set.port_name))
            .width(60f32)
            .selected_text(serial.set.baud_rate().to_string())
            .show_ui(ui, |ui| {
                for baud_rate in COMMON_BAUD_RATES {
                    ui.selectable_value(serial.set.baud_rate(), *baud_rate, baud_rate.to_string())
                        .on_hover_text("Select baud rate");
                }
            });
    });
}

/// Draws the data bits selector.
pub fn draw_data_bits_selector(ui: &mut egui::Ui, serial: &mut MutexGuard<'_, Serial>) {
    ui.horizontal(|ui| {
        ui.label("Data Bits");
        egui::ComboBox::from_id_salt(format!("{}_data", serial.set.port_name))
            .width(60f32)
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
            });
    });
}

/// Draws the stop bits selector.
pub fn draw_stop_bits_selector(ui: &mut egui::Ui, serial: &mut MutexGuard<'_, Serial>) {
    ui.horizontal(|ui| {
        ui.label("Stop Bits");
        egui::ComboBox::from_id_salt(format!("{}_stop", serial.set.port_name))
            .width(60f32)
            .selected_text(serial.set.stop_bits().to_string())
            .show_ui(ui, |ui| {
                for bits in [StopBits::One, StopBits::Two] {
                    ui.selectable_value(serial.set.stop_bits(), bits, format!("{bits}"));
                }
            });
    });
}

/// Draws the flow control selector.
pub fn draw_flow_control_selector(ui: &mut egui::Ui, serial: &mut MutexGuard<'_, Serial>) {
    ui.horizontal(|ui| {
        ui.label("Flow Ctrl");
        egui::ComboBox::from_id_salt(format!("{}_flow", serial.set.port_name))
            .width(60f32)
            .selected_text(serial.set.flow_control().to_string())
            .show_ui(ui, |ui| {
                for flow in [
                    FlowControl::None,
                    FlowControl::Software,
                    FlowControl::Hardware,
                ] {
                    ui.selectable_value(serial.set.flow_control(), flow, format!("{flow}"));
                }
            });
    });
}

/// Draws the parity selector.
pub fn draw_parity_selector(ui: &mut egui::Ui, serial: &mut MutexGuard<'_, Serial>) {
    ui.horizontal(|ui| {
        ui.label("Parity   ");
        egui::ComboBox::from_id_salt(format!("{}_parity", serial.set.port_name))
            .width(60f32)
            .selected_text(serial.set.parity().to_string())
            .show_ui(ui, |ui| {
                for parity in [Parity::None, Parity::Odd, Parity::Even] {
                    ui.selectable_value(serial.set.parity(), parity, format!("{parity}"));
                }
            });
    });
}

/// Draws the timeout selector.
pub fn draw_timeout_selector(ui: &mut egui::Ui, serial: &mut MutexGuard<'_, Serial>) {
    ui.horizontal(|ui| {
        ui.label("Timeout  ");

        // Convert timeout from Duration to milliseconds for display (capped at u64::MAX)
        let timeout_ms = serial.set.timeout.as_millis().min(u64::MAX.into()) as u64;

        egui::ComboBox::from_id_salt(format!("{}_timeout", serial.set.port_name))
            .width(60f32)
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
            });
    });
}

/// Draws the open/close port button.
pub fn open_ui(ui: &mut egui::Ui, serial: &mut MutexGuard<'_, Serial>, selected: &mut Selected) {
    if serial.is_close() {
        if ui.button("Open").clicked() {
            selected.select(&serial.set.port_name);
            info!("Opening port {}", serial.set.port_name);

            // Clone settings before borrowing tx_channel to avoid borrow conflict
            let settings = serial.set.clone();
            if let Some(tx) = serial.tx_channel() {
                match tx.send(PortChannelData::PortOpen(settings)) {
                    Ok(_) => {
                        info!("Sent open port message");
                    }
                    Err(e) => info!("Failed to open port: {e}"),
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
        info!("Closing port {}", serial.set.port_name);
        let port_name = serial.set.port_name.clone();

        if let Some(tx) = serial.tx_channel() {
            match tx.send(PortChannelData::PortClose(port_name)) {
                Ok(_) => {
                    info!("Sent close port message");
                }
                Err(e) => info!("Failed to close port: {e}"),
            }
        }
    }
}

/// Draws the serial setting status UI.
pub fn draw_serial_setting_ui(ui: &mut egui::Ui, selected: &mut Selected) {
    ui.horizontal(|ui| {
        if selected.selected().is_empty() {
            ui.label("No port selected");
        } else {
            ui.label("Selected:");
            ui.label(selected.selected());
        }
    });
    ui.separator();
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
pub fn draw_serial_context_ui(mut serials: Query<&mut Serials>, mut context: EguiContexts) {
    let Ok(mut serials) = serials.single_mut() else {
        return;
    };

    let Ok(ctx) = context.ctx_mut() else {
        return;
    };

    for serial in &mut serials.serial {
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
        .width(60f32)
        .selected_text(serial.data().data_type().as_str_en())
        .show_ui(ui, |ui| {
            for data_type in [DataType::Hex, DataType::Utf8] {
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

/// Draws the LLM toggle button.
pub fn llm_ui(ui: &mut egui::Ui, serial: &mut MutexGuard<'_, Serial>) {
    ui.horizontal(|ui| {
        let llm_enable = *serial.llm().enable();
        let button_text = if llm_enable {
            "Disable LLM"
        } else {
            "Enable LLM"
        };
        if ui.button(button_text).clicked() {
            *serial.llm().enable() = !llm_enable;
        }
    });
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
