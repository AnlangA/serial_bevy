pub mod ui;
use crate::serial::*;
use bevy::prelude::*;
use bevy_egui::{egui::{self, RichText}, EguiContexts, EguiPlugin};
use epaint::text::{FontInsert, InsertFontFamily};
use ui::*;

/// serial ui plugin
pub struct SerialUiPlugin;

/// serial ui plugin implementation
impl Plugin for SerialUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(EguiPlugin)
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

/// set theme
fn ui_init(mut ctx: EguiContexts, _commands: Commands) {
    // Start with the default fonts (we will be adding to them rather than replacing thereplacing them).

    ctx.ctx_mut().add_font(FontInsert::new(
        "Song",
        egui::FontData::from_static(include_bytes!(
            "../../assets/fonts/STSong.ttf"
        )),
        vec![
            InsertFontFamily {
                family: egui::FontFamily::Proportional,
                priority: egui::epaint::text::FontPriority::Highest,
            },
            InsertFontFamily {
                family: egui::FontFamily::Monospace,
                priority: egui::epaint::text::FontPriority::Lowest,
            },
        ],
    ));

    ctx.ctx_mut().set_theme(egui::Theme::Light);
}

/// serial settings ui
fn serial_ui(
    mut contexts: EguiContexts,
    mut serials: Query<&mut Serials>,
    mut selected: ResMut<Selected>,
) {
    egui::SidePanel::left("serial_ui_left")
        .resizable(false)
        .min_width(120.0)
        .max_width(120.0)
        .show(contexts.ctx_mut(), |ui| {
            ui.horizontal(|ui| {
                egui::widgets::global_theme_preference_switch(ui);
            });
            ui.separator();
            egui::ScrollArea::both().show(ui, |ui| {
                draw_select_serial_ui(ui, &mut serials.single_mut(), selected.as_mut());
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
        let mut serials = serials.single_mut();
        ui.horizontal(|ui| {
            for serial in serials.serial.iter_mut() {
                let mut serial = serial.lock().unwrap();
                draw_serial_context_label_ui(ui, selected.as_mut(), &mut serial);
            }
        });
        ui.separator();
        for serial in serials.serial.iter_mut() {
            let mut serial = serial.lock().unwrap();
            if selected.is_selected(&serial.set.port_name) {
                let data = serial.data().read_current_source_file();
                egui::ScrollArea::vertical()
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
                                    egui::RichText::new(
                                        serial.set.port_name.clone() + "接收数据窗口",
                                    )
                                    .color(egui::Color32::GRAY),
                                );
                            } else {
                                ui.monospace(egui::RichText::new(data));
                            }
                        })
                    });
            }
        }

        ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
            for serial in serials.serial.iter_mut() {
                let mut serial = serial.lock().unwrap();
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
    let mut serials = serials.single_mut();
    let (mut serial, llm_flag) = {
        let mut result = (None, false); // 初始化返回值
        for serial_ref in serials.serial.iter_mut() {
            let mut serial = serial_ref.lock().unwrap();
            if selected.is_selected(&serial.set.port_name) {
                if serial.is_open() & *serial.llm().enable() {
                    result = (Some(serial), true);
                    break;
                }
            }
        }
        result
    };
    
    if llm_flag{
        let serial = serial.as_mut().unwrap();
        egui::SidePanel::right("serial_ui_right")
        .resizable(false)
        .min_width(240.0)
        .max_width(240.0)
        .show(contexts.ctx_mut(), |ui|{
            let datas = serial.llm().get_stored_message_vec();
            egui::ScrollArea::vertical()
                    .min_scrolled_width(ui.available_width() - 20.)
                    .max_width(ui.available_width() - 20.)
                    .max_height(ui.available_height() - 127.)
                    .stick_to_bottom(true)
                    .auto_shrink(egui::Vec2b::FALSE)
                    .show(ui, |ui| {
                        let mut index = 0usize;
                        for data in datas{
                            index += 1;
                            if index % 2 == 0 {
                                ui.add(egui::Label::new(RichText::new(data.to_string()).color(egui::Color32::RED)));
                            }else{
                                ui.add(egui::Label::new(RichText::new(data.to_string()).color(egui::Color32::GREEN)));
                            }
                        }
                    });
            if ui.button("hi").clicked(){
                info!("LLM button");
            }
            let font = egui::FontId::proportional(14.);
            egui::ScrollArea::vertical().id_salt("llm input")
                    .min_scrolled_width(ui.available_width())
                    .max_width(ui.available_width())
                    .max_height(ui.available_height())
                    .stick_to_bottom(true)
                    .auto_shrink(egui::Vec2b::FALSE)
                    .show(ui, |ui| {
                        ui.add(
                            egui::TextEdit::multiline(
                                serial.data().get_cache_data().get_current_data(),
                            )
                            .font(font)
                            .desired_rows(5)
                            .desired_width(ui.available_width()),
                        );
                    });
        });
    }
    
}

/// send cache data
fn send_cache_data(mut serials: Query<&mut Serials>) {
    for mut serials in serials.iter_mut() {
        for serial in serials.serial.iter_mut() {
            let mut serial = serial.lock().unwrap();
            if serial.is_open() {
                let catch = serial.data().get_cache_data().get_current_data().clone();
                if catch.contains('\r') || catch.contains('\n') {
                    #[allow(unused_assignments)]
                    let mut data = String::new();
                    if *serial.data().line_feed() {
                        data = catch.to_string();
                    } else {
                        data = catch.replace('\r', "").replace('\n', "");
                    }
                    let history_data = data.clone().replace('\r', "").replace('\n', "");
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

/// history data checkout
fn history_data_checkout(
    mut serials: Query<&mut Serials>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    selected: ResMut<Selected>,
) {
    let mut serials = serials.single_mut();
    for serial in serials.serial.iter_mut() {
        let mut serial = serial.lock().unwrap();
        if selected.is_selected(&serial.set.port_name) & serial.is_open() {
            if keyboard_input.just_pressed(KeyCode::ArrowUp) {
                serial.data().get_cache_data().sub_history_index();
                let index = serial.data().get_cache_data().get_current_data_index();
                info!("index: {}", index);
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
