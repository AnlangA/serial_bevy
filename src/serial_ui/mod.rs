pub mod ui;

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
use ui::*;

/// serial ui plugin
pub struct SerialUiPlugin;

/// serial ui plugin implementation
impl Plugin for SerialUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(EguiPlugin)
            .insert_resource(ClearColor(Color::srgb(0.96875, 0.96875, 0.96875)))
            .insert_resource(Flag { flag: true })
            .insert_resource(Selected::default())
            .add_systems(Startup, ui_init)
            .add_systems(
                Update,
                (
                    serial_ui,
                    serial_window,
                    close_event_system,
                    serial_window_ui,
                )
                    .chain(),
            );
    }
}

/// set theme
fn ui_init(mut ctx: EguiContexts, _commands: Commands) {
    // Start with the default fonts (we will be adding to them rather than replacing thereplacing them).
    let mut fonts = egui::FontDefinitions::default();

    // Install my own font (maybe supporting non-latin characters).
    // .ttf and .otf files supported.
    fonts.font_data.insert(
        "Song".to_owned(),
        egui::FontData::from_static(include_bytes!("../../assets/fonts/STSong.ttf")),
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

    ctx.ctx_mut().set_theme(egui::Theme::Light);
    
}


/// serial settings ui
fn serial_ui(mut contexts: EguiContexts, mut serials: Query<&mut Serials>, mut commands: Commands, mut selected: ResMut<Selected>) {
    
    egui::SidePanel::left("serial_ui")
        .resizable(false)
        .min_width(120.0)
        .max_width(120.0)
        .show(contexts.ctx_mut(), |ui| {
            ui.horizontal(|ui| {
                egui::widgets::global_theme_preference_switch(ui);
            });
            ui.separator();
            egui::ScrollArea::both().show(ui, |ui| {
                draw_select_serial_ui(ui, &mut serials.single_mut(), selected.as_mut(), commands);
            });
            ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                for serial in serials.single_mut().serial.iter_mut() {
                    let mut serial = serial.lock().unwrap();
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
    egui::CentralPanel::default().show(contexts.ctx_mut(), |ui| {
        ui.label("主面板");
    });
}

