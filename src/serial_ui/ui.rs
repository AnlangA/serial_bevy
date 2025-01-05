use crate::serial::port::Serial;
use crate::serial::*;
use bevy::prelude::*;
use bevy_egui::{EguiContexts, egui};
use std::sync::MutexGuard;
use tokio_serial::{DataBits, FlowControl, Parity, StopBits};

#[derive(Resource)]
pub struct Selected {
    selected: String,
}
impl Default for Selected {
    fn default() -> Self {
        Self {
            selected: "".to_string(),
        }
    }
}
impl Selected {
    pub fn is_selected(&self, port_name: &str) -> bool {
        self.selected == port_name
    }
    pub fn select(&mut self, port_name: &str) {
        self.selected = port_name.to_string();
    }
    pub fn selected(&self) -> &str {
        &self.selected
    }
}

/// draw serial selector
pub fn draw_select_serial_ui(
    ui: &mut egui::Ui,
    serials: &mut Serials,
    mut selected: &mut Selected,
) {
    for serial in serials.serial.iter_mut() {
        let mut serial = serial.lock().unwrap();
        ui.horizontal(|ui| {
            if serial.is_open() {
                if ui
                    .selectable_label(
                        selected.is_selected(&serial.set.port_name),
                        egui::RichText::new(serial.set.port_name.clone())
                            .color(egui::Color32::ORANGE)
                            .strong(),
                    )
                    .clicked()
                {
                    selected.select(&serial.set.port_name);
                }
            } else {
                if ui
                    .selectable_label(
                        selected.is_selected(&serial.set.port_name),
                        egui::RichText::new(serial.set.port_name.clone())
                            .color(egui::Color32::GREEN)
                            .strong(),
                    )
                    .clicked()
                {
                    selected.select(&serial.set.port_name);
                }
            }
            open_ui(ui, &mut serial, &mut selected);
        });
    }
}

/// draw baud rate selector
pub fn draw_baud_rate_selector(ui: &mut egui::Ui, serial: &mut MutexGuard<'_, Serial>) {
    ui.horizontal(|ui| {
        ui.label("波特率");
        egui::ComboBox::from_id_salt(serial.set.port_name.clone() + "0")
            .width(60f32)
            .selected_text(serial.set.baud_rate().to_string())
            .show_ui(ui, |ui| {
                for baud_rate in port::COMMON_BAUD_RATES.iter() {
                    ui.selectable_value(serial.set.baud_rate(), *baud_rate, baud_rate.to_string())
                        .on_hover_text("选择正确的波特率");
                }
            });
    });
}

/// draw data bits selector
pub fn draw_data_bits_selector(ui: &mut egui::Ui, serial: &mut MutexGuard<'_, Serial>) {
    ui.horizontal(|ui| {
        ui.label("数据位");
        egui::ComboBox::from_id_salt(serial.set.port_name.clone() + "1")
            .width(60f32)
            .selected_text(serial.set.data_size().to_string())
            .show_ui(ui, |ui| {
                for bits in [
                    DataBits::Five,
                    DataBits::Six,
                    DataBits::Seven,
                    DataBits::Eight,
                ] {
                    ui.selectable_value(serial.set.data_size(), bits, format!("{}", bits));
                }
            });
    });
}

/// draw stop bits selector
pub fn draw_stop_bits_selector(ui: &mut egui::Ui, serial: &mut MutexGuard<'_, Serial>) {
    ui.horizontal(|ui| {
        ui.label("停止位");
        egui::ComboBox::from_id_salt(serial.set.port_name.clone() + "2")
            .width(60f32)
            .selected_text(serial.set.stop_bits().to_string())
            .show_ui(ui, |ui| {
                for bits in [StopBits::One, StopBits::Two] {
                    ui.selectable_value(serial.set.stop_bits(), bits, format!("{}", bits));
                }
            });
    });
}

/// draw flow control selector
pub fn draw_flow_control_selector(ui: &mut egui::Ui, serial: &mut MutexGuard<'_, Serial>) {
    ui.horizontal(|ui| {
        ui.label("流控    ");
        egui::ComboBox::from_id_salt(serial.set.port_name.clone() + "3")
            .width(60f32)
            .selected_text(serial.set.flow_control().to_string())
            .show_ui(ui, |ui| {
                for flow in [
                    FlowControl::None,
                    FlowControl::Software,
                    FlowControl::Hardware,
                ] {
                    ui.selectable_value(serial.set.flow_control(), flow, format!("{}", flow));
                }
            });
    });
}

/// draw parity selector
pub fn draw_parity_selector(ui: &mut egui::Ui, serial: &mut MutexGuard<'_, Serial>) {
    ui.horizontal(|ui| {
        ui.label("校验    ");
        egui::ComboBox::from_id_salt(serial.set.port_name.clone() + "4")
            .width(60f32)
            .selected_text(serial.set.parity().to_string())
            .show_ui(ui, |ui| {
                for parity in [Parity::None, Parity::Odd, Parity::Even] {
                    ui.selectable_value(serial.set.parity(), parity, format!("{}", parity));
                }
            });
    });
}

pub fn open_ui(ui: &mut egui::Ui, serial: &mut MutexGuard<'_, Serial>, selected: &mut Selected) {
    if serial.is_close() {
        if ui.button("打开").clicked() {
            selected.select(&serial.set.port_name);
            info!("Open port {}", serial.set.port_name);
            if let Some(tx) = serial.tx_channel() {
                match tx.send(port::PortChannelData::PortOpen) {
                    Ok(_) => {
                        info!("Send open port message");
                    }
                    Err(e) => error!("Failed to open port: {}", e),
                }
                let time = chrono::Local::now().format("%Y%m%d_%H%M%S").to_string();
                let port_name = serial.set.port_name.clone();
                let file_name = format!("{}_{}.txt", port_name, time);
                serial.data().add_source_file(file_name);
            }
        }
    } else if serial.is_open() {
        if ui.button("关闭").clicked() {
            selected.select(&serial.set.port_name);
            info!("关闭串口 {}", serial.set.port_name);
            let port_name = serial.set.port_name.clone();

            if let Some(tx) = serial.tx_channel() {
                match tx.send(port::PortChannelData::PortClose(port_name)) {
                    Ok(_) => {
                        info!("Send close port message");
                    }
                    Err(e) => error!("Failed to close port: {}", e),
                }
            }
        }
    }
}

/// draw serial setting ui
pub fn draw_serial_setting_ui(ui: &mut egui::Ui, selected: &mut Selected) {
    ui.horizontal(|ui| {
        if selected.selected() != "" {
            ui.label("当前选中:");
            ui.label(selected.selected());
        } else {
            ui.label("当前未选中串口");
        }
    });
    ui.separator();
}

/// draw serial context label ui
pub fn draw_serial_context_label_ui(
    ui: &mut egui::Ui,
    selacted: &mut Selected,
    serial: &mut MutexGuard<'_, Serial>,
) {
    if serial.is_open() {
        if ui
            .selectable_label(
                selacted.is_selected(&serial.set.port_name),
                egui::RichText::new(format!("{}", serial.set.port_name)),
            )
            .clicked()
        {
            selacted.select(&serial.set.port_name);
        }
    }
}

/// draw serial context ui
pub fn draw_serial_context_ui(mut serials: Query<&mut Serials>, mut context: EguiContexts) {
    let mut serials = serials.single_mut();
    for serial in serials.serial.iter_mut() {
        let mut serial = serial.lock().unwrap();
        if serial.is_error() {
            egui::Window::new(format!("{}", serial.set.port_name) + "错误").show(
                context.ctx_mut(),
                |ui| {
                    ui.label(
                        egui::RichText::new(format!("{} 错误", serial.set.port_name))
                            .color(egui::Color32::RED)
                            .strong(),
                    );
                    if ui.button("清除错误").clicked() {
                        serial.close();
                    }
                },
            );
        }
    }
}

pub fn data_type_ui(ui: &mut egui::Ui, serial: &mut MutexGuard<'_, Serial>) {
    ui.add(egui::Label::new(egui::RichText::new("数据类型:")));
    egui::ComboBox::from_id_salt(serial.set.port_name.clone() + "3")
        .width(60f32)
        .selected_text(serial.data().data_type().to_string())
        .show_ui(ui, |ui| {
            for flow in [
                port::Type::Binary,
                port::Type::Hex,
                port::Type::Utf8,
                port::Type::Utf16,
                port::Type::Utf32,
                port::Type::GBK,
                port::Type::ASCII,
            ] {
                ui.selectable_value(serial.data().data_type(), flow, format!("{}", flow));
            }
        });
}

/// data line feed
pub fn data_line_feed_ui(ui: &mut egui::Ui, serial: &mut MutexGuard<'_, Serial>) {
    ui.horizontal(|ui| {
        if *serial.data().line_feed() {
            if ui
                .button("不换行")
                .on_hover_text("发送的数据中不包含换行符")
                .clicked()
            {
                *serial.data().line_feed() = false;
            }
        } else {
            if ui
                .button("换行")
                .on_hover_text("发送的数据中包含换行符")
                .clicked()
            {
                *serial.data().line_feed() = true;
            }
        }
    });
}
