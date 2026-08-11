//! # UI Components Module
//!
//! This module provides individual UI components for serial port configuration and control.

use crate::serial::Serials;
use crate::serial::port::{COMMON_BAUD_RATES, DataType, PortChannelData, Serial};
use bevy::prelude::*;
use bevy_egui::{EguiContexts, egui};
use log::{error, info};
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
            open_ui(ui, serial, selected);
        });
    }
}

/// Draws the baud rate selector.
pub fn draw_baud_rate_selector(ui: &mut egui::Ui, serial: &mut Serial) {
    ui.horizontal(|ui| {
        ui.label("Baud Rate");
        egui::ComboBox::from_id_salt(format!("{}_baud", serial.set.port_name))
            .width(60f32)
            .selected_text(serial.set.baud_rate.to_string())
            .show_ui(ui, |ui| {
                for baud_rate in COMMON_BAUD_RATES {
                    ui.selectable_value(
                        &mut serial.set.baud_rate,
                        *baud_rate,
                        baud_rate.to_string(),
                    )
                    .on_hover_text("Select baud rate");
                }
            });
    });
}

/// Draws the data bits selector.
pub fn draw_data_bits_selector(ui: &mut egui::Ui, serial: &mut Serial) {
    ui.horizontal(|ui| {
        ui.label("Data Bits");
        egui::ComboBox::from_id_salt(format!("{}_data", serial.set.port_name))
            .width(60f32)
            .selected_text(serial.set.data_bits.to_string())
            .show_ui(ui, |ui| {
                for bits in [
                    DataBits::Five,
                    DataBits::Six,
                    DataBits::Seven,
                    DataBits::Eight,
                ] {
                    ui.selectable_value(&mut serial.set.data_bits, bits, format!("{bits}"));
                }
            });
    });
}

/// Draws the stop bits selector.
pub fn draw_stop_bits_selector(ui: &mut egui::Ui, serial: &mut Serial) {
    ui.horizontal(|ui| {
        ui.label("Stop Bits");
        egui::ComboBox::from_id_salt(format!("{}_stop", serial.set.port_name))
            .width(60f32)
            .selected_text(serial.set.stop_bits.to_string())
            .show_ui(ui, |ui| {
                for bits in [StopBits::One, StopBits::Two] {
                    ui.selectable_value(&mut serial.set.stop_bits, bits, format!("{bits}"));
                }
            });
    });
}

/// Draws the flow control selector.
pub fn draw_flow_control_selector(ui: &mut egui::Ui, serial: &mut Serial) {
    ui.horizontal(|ui| {
        ui.label("Flow Ctrl");
        egui::ComboBox::from_id_salt(format!("{}_flow", serial.set.port_name))
            .width(60f32)
            .selected_text(serial.set.flow_control.to_string())
            .show_ui(ui, |ui| {
                for flow in [
                    FlowControl::None,
                    FlowControl::Software,
                    FlowControl::Hardware,
                ] {
                    ui.selectable_value(&mut serial.set.flow_control, flow, format!("{flow}"));
                }
            });
    });
}

/// Draws the parity selector.
pub fn draw_parity_selector(ui: &mut egui::Ui, serial: &mut Serial) {
    ui.horizontal(|ui| {
        ui.label("Parity   ");
        egui::ComboBox::from_id_salt(format!("{}_parity", serial.set.port_name))
            .width(60f32)
            .selected_text(serial.set.parity.to_string())
            .show_ui(ui, |ui| {
                for parity in [Parity::None, Parity::Odd, Parity::Even] {
                    ui.selectable_value(&mut serial.set.parity, parity, format!("{parity}"));
                }
            });
    });
}

/// Draws the open/close port button.
pub fn open_ui(ui: &mut egui::Ui, serial: &mut Serial, selected: &mut Selected) {
    if serial.is_close() {
        if ui.button("Open").clicked() {
            selected.select(&serial.set.port_name);
            info!("Opening port {}", serial.set.port_name);
            let time = chrono::Local::now().format("%Y%m%d_%H%M%S_%f").to_string();
            let file_name = format!("{}_{}.txt", serial.set.port_name, time);
            if let Err(e) = serial.data().add_source_file(file_name) {
                let message = format!("Failed to create the session log: {e}");
                error!("{message}");
                serial.error(message);
                return;
            }

            let settings = serial.set.clone();
            let result = serial
                .tx_channel()
                .as_ref()
                .map(|tx| tx.try_send(PortChannelData::PortOpen(settings)));
            match result {
                Some(Ok(_)) => {
                    serial.begin_open();
                    info!("Sent open port message");
                }
                Some(Err(e)) => {
                    let message = format!("Failed to request opening the port: {e}");
                    error!("{message}");
                    serial.error(message);
                }
                None => {
                    let message = format!(
                        "Cannot open {}: command channel unavailable",
                        serial.set.port_name
                    );
                    error!("{message}");
                    serial.error(message);
                }
            }
        }
    } else if serial.is_opening() {
        ui.add_enabled(false, egui::Button::new("Opening…"));
    } else if serial.is_open() && ui.button("Close").clicked() {
        selected.select(&serial.set.port_name);
        info!("Closing port {}", serial.set.port_name);
        serial.close();
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
    serial: &mut Serial,
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
        if serial.is_error() {
            egui::Window::new(format!("{} Error", serial.set.port_name)).show(ctx, |ui| {
                ui.label(
                    egui::RichText::new(format!("{} Error", serial.set.port_name))
                        .color(egui::Color32::RED)
                        .strong(),
                );
                if let Some(message) = serial.last_error() {
                    ui.label(message);
                }
                if ui.button("Clear Error").clicked() {
                    serial.close();
                }
            });
        }
    }
}

/// Draws the data type selector.
pub fn data_type_ui(ui: &mut egui::Ui, serial: &mut Serial) {
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
pub fn data_line_feed_ui(ui: &mut egui::Ui, serial: &mut Serial) {
    let mut enabled = *serial.data().line_feed();
    if ui
        .checkbox(&mut enabled, "Append LF")
        .on_hover_text("Append a 0A byte to every submitted command")
        .changed()
    {
        *serial.data().line_feed() = enabled;
    }
}

/// Draws timestamp toggle button.
pub fn timestamp_ui(ui: &mut egui::Ui, serial: &mut Serial) {
    let mut enabled = *serial.data().timestamp_enabled();
    if ui
        .checkbox(&mut enabled, "Timestamps")
        .on_hover_text("Include a timestamp in each session-log record")
        .changed()
    {
        *serial.data().timestamp_enabled() = enabled;
    }
}

/// Draws log timeout setting.
pub fn log_timeout_ui(ui: &mut egui::Ui, serial: &mut Serial) {
    ui.horizontal(|ui| {
        ui.label("Log Timeout:");
        let mut timeout_ms = serial.data().log_timeout().as_millis() as u64;
        if ui
            .add(
                egui::DragValue::new(&mut timeout_ms)
                    .range(0..=3_600_000)
                    .speed(10),
            )
            .changed()
        {
            *serial.data().log_timeout() = std::time::Duration::from_millis(timeout_ms);
        }
        ui.label("ms (0=disabled)");
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

    #[test]
    fn serial_controls_render_headlessly() {
        let context = egui::Context::default();
        let mut serials = Serials::new();
        let mut serial = Serial::new();
        serial.set.port_name = "loopback".to_string();
        serials.add(serial);
        let mut selected = Selected::default();
        selected.select("loopback");

        let output = context.run_ui(egui::RawInput::default(), |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                draw_select_serial_ui(ui, &mut serials, &mut selected);
                let serial = serials.get_mut(0);
                draw_baud_rate_selector(ui, serial);
                draw_data_bits_selector(ui, serial);
                draw_stop_bits_selector(ui, serial);
                draw_parity_selector(ui, serial);
                draw_flow_control_selector(ui, serial);
                data_type_ui(ui, serial);
                data_line_feed_ui(ui, serial);
                timestamp_ui(ui, serial);
                log_timeout_ui(ui, serial);
            });
        });

        assert!(!output.shapes.is_empty());
    }
}
