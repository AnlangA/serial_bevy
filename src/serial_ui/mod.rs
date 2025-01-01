pub mod ui;

use crate::serial::port::Serial;
use crate::serial::*;
use bevy::{
    prelude::*,
    render::camera::RenderTarget,
    window::{PresentMode, WindowClosing, WindowRef, WindowResolution,PrimaryWindow},
};
use bevy_egui::{EguiContext, EguiContexts, EguiPlugin, egui};
use std::sync::MutexGuard;
use tokio_serial::{DataBits, FlowControl, Parity, StopBits};

/// serial ui plugin
pub struct SerialUiPlugin;

/// serial ui plugin implementation
impl Plugin for SerialUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(EguiPlugin)
            .add_systems(Startup, ui_init)
            .add_systems(Update, (serial_ui, serial_window,close_event_system).chain());
    }
}

/// set theme
fn ui_init(mut ctx: EguiContexts) {
    // Start with the default fonts (we will be adding to them rather than replacing thereplacing them).
    let mut fonts = egui::FontDefinitions::default();

    // Install my own font (maybe supporting non-latin characters).
    // .ttf and .otf files supported.
    fonts.font_data.insert(
        "Song".to_owned(),
        egui::FontData::from_static(include_bytes!("../fonts/STSong.ttf")),
    );
    fonts
        .families
        .insert(egui::FontFamily::Name("Song".into()), vec![
            "Song".to_owned(),
        ]);
    // Put my font first (highest priority) for proportional text:
    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .insert(0, "Song".to_owned());

    // Put my font as last fallback for monospace:
    fonts
        .families
        .entry(egui::FontFamily::Monospace)
        .or_default()
        .push("Song".to_owned());
    // Tell egui to use these fonts:
    ctx.ctx_mut().set_fonts(fonts);
}

/// serial settings ui
fn serial_ui(mut contexts: EguiContexts, mut serials: Query<&mut Serials>, mut commands: Commands) {
    for serial in serials.single_mut().serial.iter_mut() {
        let mut serial = serial.lock().unwrap();
        egui::Window::new(serial.set.port_name.clone()).show(contexts.ctx_mut(), |ui| {
            ui.add_enabled_ui(serial.data().state().is_close(), |ui| {
                draw_baud_rate_selector(ui, &mut serial);
                draw_data_bits_selector(ui, &mut serial);
                draw_stop_bits_selector(ui, &mut serial);
                draw_flow_control_selector(ui, &mut serial);
                draw_parity_selector(ui, &mut serial);
            });
            open_ui(ui, &mut serial, &mut commands);
        });
    }
}

/// draw baud rate selector
fn draw_baud_rate_selector(ui: &mut egui::Ui, serial: &mut MutexGuard<'_, Serial>) {
    ui.horizontal(|ui| {
        ui.label("波特率");
        egui::ComboBox::from_id_salt("波特率")
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
fn draw_data_bits_selector(ui: &mut egui::Ui, serial: &mut MutexGuard<'_, Serial>) {
    ui.horizontal(|ui| {
        ui.label("数据位");
        egui::ComboBox::from_id_salt("数据位")
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
fn draw_stop_bits_selector(ui: &mut egui::Ui, serial: &mut MutexGuard<'_, Serial>) {
    ui.horizontal(|ui| {
        ui.label("停止位");
        egui::ComboBox::from_id_salt("停止位")
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
fn draw_flow_control_selector(ui: &mut egui::Ui, serial: &mut MutexGuard<'_, Serial>) {
    ui.horizontal(|ui| {
        ui.label("流控    ");
        egui::ComboBox::from_id_salt("流控")
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
fn draw_parity_selector(ui: &mut egui::Ui, serial: &mut MutexGuard<'_, Serial>) {
    ui.horizontal(|ui| {
        ui.label("校验    ");
        egui::ComboBox::from_id_salt("校验")
            .width(60f32)
            .selected_text(serial.set.parity().to_string())
            .show_ui(ui, |ui| {
                for parity in [Parity::None, Parity::Odd, Parity::Even] {
                    ui.selectable_value(serial.set.parity(), parity, format!("{}", parity));
                }
            });
    });
}

fn open_ui(ui: &mut egui::Ui, serial: &mut MutexGuard<'_, Serial>, commands: &mut Commands) {
    if serial.is_close() {
        if ui.button("打开").clicked() {
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

fn close_event_system(
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

fn serial_window(mut commands: Commands, mut serials: Query<&mut Serials>) {
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
                commands.spawn((
                    Camera3d::default(),
                    Camera {
                        target: RenderTarget::Window(WindowRef::Entity(window_id)),
                        ..Default::default()
                    },
                    Transform::from_xyz(0.0, 0.0, 0.0).looking_at(Vec3::ZERO, Vec3::Y),
                ));
                info!("{} window id: {}", serial.set.port_name(), window_id);
                *serial.window() = Some(window_id);
            }
        }
    }
}

/* 
fn serial_window_ui(
    mut commands: Commands,
    mut serials: Query<&mut Serials>,
    mut ctx_entity: Query<(&mut EguiContext, Entity), Without<PrimaryWindow>>,
) {
    let mut serials = serials.single_mut();
    for (mut contexts, entity) in ctx_entity.iter_mut() {
        for serial in serials.serial.iter_mut() {
            let mut serial = serial.lock().unwrap();
            if serial.window().is_some() && serial.window().unwrap() == entity {
                info!("{} window id: {}", serial.set.port_name(), entity);
                egui::Window::new(serial.set.port_name.clone() + "windows").show(contexts.get_mut(), |ui| {
                    ui.label("hello");
                });
            }
        }
    }
}
*/
/* 
fn serial_window_ui(
    mut egui_ctx: Query<&mut EguiContext, Without<PrimaryWindow>>,
) {
    let Ok(mut ctx) = egui_ctx.get_single_mut() else {
        return;
    };
    info!("serial_window_ui");
    egui::Window::new("Second Window")
        .show(ctx.get_mut(), |ui| {
            ui.label("hello");
        });
}*/
