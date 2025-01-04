use crate::serial::port::Serial;
use crate::serial::*;
use bevy::{
    prelude::*,
    render::camera::RenderTarget,
    window::{PresentMode, WindowClosing, WindowRef, WindowResolution},
};
use bevy_egui::{EguiContext, EguiContexts, EguiPlugin, egui};
use std::sync::MutexGuard;
use tokio_serial::{DataBits, FlowControl, Parity, StopBits};

#[derive(Resource)]
pub struct Selected {
    selected: String,
}
impl Default for Selected {
    fn default() -> Self {
        Self { selected: "".to_string() }
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
pub fn draw_select_serial_ui(ui: &mut egui::Ui, serials: &mut Serials, mut selected: &mut Selected, mut commands: Commands) {
    for serial in serials.serial.iter_mut() {
        let mut serial = serial.lock().unwrap();
        ui.horizontal(|ui| {
        if serial.is_open() {
            if ui.selectable_label(selected.is_selected(&serial.set.port_name), egui::RichText::new(serial.set.port_name.clone()).color(egui::Color32::ORANGE).strong()).clicked(){
                selected.select(&serial.set.port_name);
            }
        } else {
            if ui.selectable_label(selected.is_selected(&serial.set.port_name), egui::RichText::new(serial.set.port_name.clone()).color(egui::Color32::GREEN).strong()).clicked(){
                selected.select(&serial.set.port_name);
            }
        }
        open_ui(ui, &mut serial, &mut commands, &mut selected);
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

pub fn open_ui(ui: &mut egui::Ui, serial: &mut MutexGuard<'_, Serial>, commands: &mut Commands, selected: &mut Selected) {
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
            match serial.window() {
                Some(window) => {
                    commands.entity(window.clone()).despawn_recursive();
                }
                None => {}
            };

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

pub fn close_event_system(
    mut window_close_events: EventReader<WindowClosing>,
    mut serials: Query<&mut Serials>,
) {
    for event in window_close_events.read() {
        let mut serial = serials.get_single_mut().unwrap();
        for serial in serial.serial.iter_mut() {
            let mut serial = serial.lock().unwrap();
            let port_name = serial.set.port_name.clone();
            if serial.is_open() {
                if let Some(window) = serial.window() {
                    if *window == event.window {
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
        }
    }
}

pub fn serial_window(mut commands: Commands, mut serials: Query<&mut Serials>) {
    let mut serials = serials.single_mut();
    for serial in serials.serial.iter_mut() {
        let mut serial = serial.lock().unwrap();
        if serial.is_open() {
            if let None = serial.window() {
                let window_id = commands
                    .spawn(Window {
                        title: serial.set.port_name().to_owned(),
                        resolution: WindowResolution::new(800.0, 600.0),
                        present_mode: PresentMode::AutoVsync,
                        ..Default::default()
                    })
                    .id();
                // second window camera
                let camera_id = commands
                    .spawn((
                        Camera3d::default(),
                        Camera {
                            target: RenderTarget::Window(WindowRef::Entity(window_id)),
                            ..Default::default()
                        },
                        Transform::from_xyz(6.0, 0.0, 0.0).looking_at(Vec3::ZERO, Vec3::Y),
                    ))
                    .id();
                info!("{} window id: {}", serial.set.port_name(), window_id);
                *serial.window() = Some(window_id);
                *serial.camera() = Some(camera_id);
            }
        }
    }
}

#[derive(Resource)]
pub struct Flag {
    pub flag: bool,
}

pub fn serial_window_ui(
    mut commands: Commands,
    mut egui_ctx: Query<&mut Serials>,
    mut flag: ResMut<Flag>,
    asset_server: Res<AssetServer>,
) {
    let mut serials = egui_ctx.single_mut();
    for serial in serials.serial.iter_mut() {
        let mut serial = serial.lock().unwrap();
        if serial.camera().is_some() {
            if flag.flag {
                commands.spawn((
                    Text::new("你好aaa"),
                    TextFont {
                        font: asset_server.load("fonts/STSong.ttf"),
                        font_size: 100.0,
                        ..default()
                    },
                    Transform::from_xyz(0.0, 0.0, 0.0),
                    // Since we are using multiple cameras, we need to specify which camera UI should be rendered to
                    TargetCamera(serial.camera().unwrap()),
                ));

                flag.flag = false;
            }
        }
    }
}

pub fn draw_serial_setting_ui(ui: &mut egui::Ui, selected: &mut Selected) {
    ui.horizontal(|ui| {
        if selected.selected() != "" {
            ui.label("当前选中:");
            ui.label(selected.selected());
        }else {
            ui.label("当前未选中串口");
        }
    });
    ui.separator();
}