use bevy::prelude::*;
use bevy_egui::{EguiContexts, egui};
use std::collections::HashMap;

use crate::serial::llm::LlmMessage;
use crate::serial::{Selected, Serials};

use super::config::PanelWidths;
use super::global_llm::GlobalLlmState;
use super::ui::{
    INPUT_PANEL_HEIGHT, INPUT_TEXT_EDIT_HEIGHT, INPUT_TOOLBAR_HEIGHT, MarkdownViewerCache,
    clear_log_ui, console_mode_ui, data_line_feed_ui, data_type_ui, draw_baud_rate_selector,
    draw_data_bits_selector, draw_flow_control_selector, draw_llm_coding_plan_toggle,
    draw_llm_conversation, draw_llm_input_area, draw_llm_key_input, draw_llm_model_selector,
    draw_parity_selector, draw_select_serial_ui, draw_serial_context_label_ui,
    draw_serial_input_area, draw_serial_setting_ui, draw_sidebar_section, draw_stop_bits_selector,
    draw_timeout_selector, render_message_content, timestamp_ui,
};

#[derive(Clone)]
struct SerialTextSegment {
    text: String,
    foreground_color: Option<egui::Color32>,
    background_color: Option<egui::Color32>,
}

#[derive(Default)]
struct CachedSerialOutput {
    revision: u64,
    lines: Vec<Vec<SerialTextSegment>>,
}

/// Parsed serial output cache keyed by port name.
///
/// Serial display text is already cached in `PortData`; this cache avoids
/// reparsing ANSI escape sequences every egui frame when no new data arrived.
#[derive(Resource, Default)]
pub struct SerialOutputCache {
    ports: HashMap<String, CachedSerialOutput>,
}

impl SerialOutputCache {
    fn needs_refresh(&self, port_name: &str, revision: u64) -> bool {
        self.ports
            .get(port_name)
            .map(|cached| cached.revision != revision)
            .unwrap_or(true)
    }

    fn lines_for(
        &mut self,
        port_name: &str,
        revision: u64,
        refreshed_text: Option<&str>,
    ) -> &[Vec<SerialTextSegment>] {
        if let Some(text) = refreshed_text {
            let lines = parse_serial_output_lines(text);
            self.ports.insert(
                port_name.to_string(),
                CachedSerialOutput { revision, lines },
            );
        }

        self.ports
            .get(port_name)
            .map(|cached| cached.lines.as_slice())
            .unwrap_or(&[])
    }
}

fn parse_serial_output_lines(text: &str) -> Vec<Vec<SerialTextSegment>> {
    if text.is_empty() {
        return Vec::new();
    }

    let mut parser = egui_sgr::AnsiParser::new();
    let colored_segments = parser.parse(text);
    let mut lines: Vec<Vec<SerialTextSegment>> = Vec::new();
    let mut current_line: Vec<SerialTextSegment> = Vec::new();

    for seg in colored_segments {
        for text_part in seg.text.split_inclusive('\n') {
            let ends_with_newline = text_part.ends_with('\n');
            let text = text_part.trim_end_matches('\n');
            if !text.is_empty() {
                current_line.push(SerialTextSegment {
                    text: text.to_string(),
                    foreground_color: seg.foreground_color,
                    background_color: seg.background_color,
                });
            }

            if ends_with_newline && !current_line.is_empty() {
                lines.push(std::mem::take(&mut current_line));
            }
        }
    }

    if !current_line.is_empty() {
        lines.push(current_line);
    }

    lines
}

fn selected_serial_exists(serials: &Serials, selected: &Selected) -> bool {
    serials.serial.iter().any(|serial_ref| {
        serial_ref
            .lock()
            .map(|serial| selected.is_selected(&serial.set.port_name))
            .unwrap_or(false)
    })
}

fn selected_serial_name(serials: &Serials, selected: &Selected) -> Option<String> {
    serials.serial.iter().find_map(|serial_ref| {
        serial_ref.lock().ok().and_then(|serial| {
            if selected.is_selected(&serial.set.port_name) {
                Some(serial.set.port_name.clone())
            } else {
                None
            }
        })
    })
}

fn draw_top_bar(
    ctx: &egui::Context,
    serials: &mut Serials,
    selected: &Selected,
    panel_widths: &mut PanelWidths,
    selected_serial_exists: bool,
) {
    egui::TopBottomPanel::top("serial_ui_topbar").show(ctx, |ui| {
        ui.horizontal(|ui| {
            if ui
                .selectable_label(panel_widths.show_settings_panel, "Settings")
                .clicked()
            {
                panel_widths.show_settings_panel = !panel_widths.show_settings_panel;
            }

            let llm_response = ui.add(egui::Button::selectable(panel_widths.show_llm_panel, "LLM"));
            if llm_response.clicked() {
                panel_widths.show_llm_panel = !panel_widths.show_llm_panel;
                if selected_serial_exists {
                    for serial_ref in &mut serials.serial {
                        let Ok(mut serial) = serial_ref.lock() else {
                            continue;
                        };
                        if selected.is_selected(&serial.set.port_name) {
                            *serial.llm().enable() = panel_widths.show_llm_panel;
                            break;
                        }
                    }
                }
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                egui::widgets::global_theme_preference_switch(ui);
            });
        });
    });
}

fn draw_left_panel(
    serials: &mut Serials,
    selected: &mut Selected,
    ctx: &egui::Context,
    panel_widths: &mut PanelWidths,
) {
    if panel_widths.show_settings_panel {
        let left_show = egui::SidePanel::left("serial_ui_left")
            .resizable(true)
            .default_width(panel_widths.left_width)
            .min_width(120.0)
            .max_width(600.0)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        draw_sidebar_section(ui, "Connection", |ui| {
                            draw_select_serial_ui(ui, serials, selected);
                            ui.add_space(6.0);
                            draw_serial_setting_ui(ui, selected);
                        });

                        ui.add_space(8.0);

                        draw_sidebar_section(ui, "Serial Settings", |ui| {
                            let mut drew_selected_serial = false;
                            for serial in &mut serials.serial {
                                let Ok(mut serial) = serial.lock() else {
                                    continue;
                                };
                                if selected.is_selected(&serial.set.port_name) {
                                    drew_selected_serial = true;
                                    draw_baud_rate_selector(ui, &mut serial);
                                    draw_data_bits_selector(ui, &mut serial);
                                    draw_stop_bits_selector(ui, &mut serial);
                                    draw_parity_selector(ui, &mut serial);
                                    draw_flow_control_selector(ui, &mut serial);
                                    draw_timeout_selector(ui, &mut serial);
                                    break;
                                }
                            }
                            if !drew_selected_serial {
                                ui.label(
                                    egui::RichText::new(
                                        "Select a port to edit its serial settings.",
                                    )
                                    .weak(),
                                );
                            }
                        });

                        ui.add_space(8.0);

                        draw_sidebar_section(ui, "LLM Settings", |ui| {
                            draw_llm_key_input(ui, panel_widths);
                            draw_llm_model_selector(ui, panel_widths);
                            draw_llm_coding_plan_toggle(ui, panel_widths);
                        });
                        ui.add_space(8.0);
                    });
            });
        panel_widths.left_width = left_show.response.rect.width();
    }
}

fn draw_serial_output(
    ui: &mut egui::Ui,
    port_name: &str,
    lines: &[Vec<SerialTextSegment>],
    is_empty: bool,
    data_height: f32,
) {
    egui::ScrollArea::vertical()
        .stick_to_bottom(true)
        .auto_shrink([false, false])
        .max_height(data_height)
        .show(ui, |ui| {
            if is_empty {
                ui.heading(
                    egui::RichText::new(format!("{port_name} Data Receive Window"))
                        .color(egui::Color32::GRAY),
                );
            } else {
                for line in lines {
                    ui.horizontal(|ui| {
                        for segment in line {
                            let mut rt = egui::RichText::new(&segment.text).monospace();
                            if let Some(color) = segment.foreground_color {
                                rt = rt.color(color);
                            }
                            if let Some(color) = segment.background_color {
                                rt = rt.background_color(color);
                            }
                            ui.label(rt);
                        }
                    });
                }
            }
        });
}

fn draw_central_panel(
    serials: &mut Serials,
    selected: &mut Selected,
    ctx: &egui::Context,
    output_cache: &mut SerialOutputCache,
) {
    egui::CentralPanel::default().show(ctx, |ui| {
        ui.horizontal(|ui| {
            for serial in &mut serials.serial {
                let Ok(mut serial) = serial.lock() else {
                    continue;
                };
                draw_serial_context_label_ui(ui, selected, &mut serial);
            }
        });
        ui.separator();

        let available_height = ui.available_height();
        let input_height = INPUT_PANEL_HEIGHT;
        let data_height = (available_height - input_height).max(0.0);

        let mut selected_output = None;
        for serial in &mut serials.serial {
            let Ok(serial) = serial.lock() else {
                continue;
            };
            if selected.is_selected(&serial.set.port_name) {
                let port_name = serial.set.port_name.clone();
                let revision = serial.data_ref().display_revision();
                let is_empty = serial.data_ref().is_display_empty();
                let refreshed_text = output_cache
                    .needs_refresh(&port_name, revision)
                    .then(|| serial.data_ref().display_text().to_string());
                selected_output = Some((port_name, revision, is_empty, refreshed_text));
                break;
            }
        }

        if let Some((port_name, revision, is_empty, refreshed_text)) = selected_output {
            let lines = output_cache.lines_for(&port_name, revision, refreshed_text.as_deref());
            draw_serial_output(ui, &port_name, lines, is_empty, data_height);
        }

        ui.separator();

        ui.allocate_ui_with_layout(
            egui::Vec2::new(ui.available_width(), input_height),
            egui::Layout::top_down(egui::Align::LEFT),
            |ui| {
                for serial in &mut serials.serial {
                    let Ok(mut serial) = serial.lock() else {
                        continue;
                    };
                    if selected.is_selected(&serial.set.port_name) {
                        ui.allocate_ui_with_layout(
                            egui::Vec2::new(ui.available_width(), INPUT_TOOLBAR_HEIGHT),
                            egui::Layout::left_to_right(egui::Align::Center),
                            |ui| {
                                data_type_ui(ui, &mut serial);
                                data_line_feed_ui(ui, &mut serial);
                                timestamp_ui(ui, &mut serial);
                                console_mode_ui(ui, &mut serial);
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        clear_log_ui(ui, &mut serial);
                                    },
                                );
                            },
                        );

                        draw_serial_input_area(ui, &mut serial);
                        ui.add_space(8.0);
                    }
                }
            },
        );

        ui.add_space(5.0);
    });
}

fn draw_global_llm_conversation(
    ui: &mut egui::Ui,
    global_state: &mut GlobalLlmState,
    markdown_cache: &mut MarkdownViewerCache,
) {
    let visuals = ui.visuals().clone();
    let available_height = ui.available_height().max(120.0);

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .max_height(available_height)
        .stick_to_bottom(true)
        .show(ui, |ui| {
            for msg in &global_state.messages {
                let is_user = msg.role == "user";

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
                        ui.horizontal(|ui| {
                            if is_user {
                                ui.label(egui::RichText::new(&msg.timestamp).weak().small());
                                ui.label(egui::RichText::new(role_text).strong().color(role_color));
                            } else {
                                ui.label(egui::RichText::new(role_text).strong().color(role_color));
                                ui.label(egui::RichText::new(&msg.timestamp).weak().small());
                            }
                        });

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

            if global_state.is_processing {
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

fn draw_global_llm_input_area(
    ui: &mut egui::Ui,
    panel_widths: &mut PanelWidths,
    global_state: &mut GlobalLlmState,
) {
    let font = egui::FontId::new(18.0, egui::FontFamily::Monospace);
    let can_send = !global_state.input_buffer.trim().is_empty() && !global_state.is_processing;

    ui.vertical(|ui| {
        ui.add_sized(
            [ui.available_width(), INPUT_TEXT_EDIT_HEIGHT],
            egui::TextEdit::multiline(&mut global_state.input_buffer)
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
                if panel_widths.llm_key.is_empty() || panel_widths.llm_model.is_empty() {
                    panel_widths.show_settings_panel = true;
                    global_state.show_key_missing_popup = true;
                } else if !global_state.is_processing {
                    let content = global_state.input_buffer.trim().to_string();
                    if !content.is_empty() {
                        global_state.messages.push(LlmMessage::user(&content));
                        global_state.input_buffer.clear();
                        global_state.is_processing = true;
                    }
                }
            }

            if ui.button("Clear").clicked() {
                global_state.input_buffer.clear();
            }

            if global_state.is_processing {
                ui.label(egui::RichText::new("Waiting for response...").weak());
            } else if panel_widths.llm_key.is_empty() || panel_widths.llm_model.is_empty() {
                ui.label(egui::RichText::new("Set key/model to enable sending").weak());
            }
        });
    });
}

fn draw_right_panel(
    serials: &mut Serials,
    selected: &Selected,
    ctx: &egui::Context,
    panel_widths: &mut PanelWidths,
    global_state: &mut GlobalLlmState,
    markdown_cache: &mut MarkdownViewerCache,
    selected_serial_exists: bool,
) {
    if panel_widths.show_llm_panel {
        let llm_context = if selected_serial_exists {
            selected_serial_name(serials, selected)
        } else {
            None
        };

        let right_show = egui::SidePanel::right("serial_ui_right")
            .resizable(true)
            .default_width(panel_widths.right_width)
            .min_width(200.0)
            .max_width(400.0)
            .show(ctx, |ui| {
                let llm_input_height = INPUT_PANEL_HEIGHT;
                if let Some(ref port_name) = llm_context {
                    for serial_ref in &mut serials.serial {
                        let Ok(mut serial) = serial_ref.lock() else {
                            continue;
                        };
                        if selected.is_selected(&serial.set.port_name) {
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new(format!("LLM: {port_name}")).strong());
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if ui
                                            .button("Clear")
                                            .on_hover_text("Clear conversation history")
                                            .clicked()
                                        {
                                            serial.llm().clear_messages();
                                        }
                                    },
                                );
                            });
                            ui.separator();
                            ui.allocate_ui_with_layout(
                                egui::Vec2::new(
                                    ui.available_width(),
                                    (ui.available_height() - llm_input_height).max(120.0),
                                ),
                                egui::Layout::top_down(egui::Align::LEFT),
                                |ui| {
                                    draw_llm_conversation(ui, &mut serial, markdown_cache);
                                },
                            );
                            ui.separator();
                            ui.allocate_ui_with_layout(
                                egui::Vec2::new(ui.available_width(), llm_input_height),
                                egui::Layout::top_down(egui::Align::LEFT),
                                |ui| {
                                    ui.allocate_ui_with_layout(
                                        egui::Vec2::new(ui.available_width(), INPUT_TOOLBAR_HEIGHT),
                                        egui::Layout::left_to_right(egui::Align::Center),
                                        |_ui| {},
                                    );
                                    draw_llm_input_area(
                                        ui,
                                        &mut serial,
                                        panel_widths,
                                        &mut global_state.show_key_missing_popup,
                                    );
                                },
                            );
                            break;
                        }
                    }
                } else {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("LLM (standalone)").strong());
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui
                                .button("Clear")
                                .on_hover_text("Clear conversation history")
                                .clicked()
                            {
                                global_state.messages.clear();
                            }
                        });
                    });
                    ui.separator();
                    ui.allocate_ui_with_layout(
                        egui::Vec2::new(
                            ui.available_width(),
                            (ui.available_height() - llm_input_height).max(120.0),
                        ),
                        egui::Layout::top_down(egui::Align::LEFT),
                        |ui| {
                            draw_global_llm_conversation(ui, global_state, markdown_cache);
                        },
                    );
                    ui.separator();
                    ui.allocate_ui_with_layout(
                        egui::Vec2::new(ui.available_width(), llm_input_height),
                        egui::Layout::top_down(egui::Align::LEFT),
                        |ui| {
                            ui.allocate_ui_with_layout(
                                egui::Vec2::new(ui.available_width(), INPUT_TOOLBAR_HEIGHT),
                                egui::Layout::left_to_right(egui::Align::Center),
                                |_ui| {},
                            );
                            draw_global_llm_input_area(ui, panel_widths, global_state);
                        },
                    );
                }
                ui.add_space(8.0);
                ui.add_space(5.0);
            });
        panel_widths.right_width = right_show.response.rect.width();
    }
}

fn draw_missing_config_popup(ctx: &egui::Context, global_state: &mut GlobalLlmState) {
    if global_state.show_key_missing_popup {
        egui::Window::new("LLM Configuration Required")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                ui.label(
                    "Please enter your LLM API key and select a model in the left settings panel.",
                );
                ui.horizontal(|ui| {
                    ui.add_space(ui.available_width() / 2.0 - 40.0);
                    if ui.button("  OK  ").clicked() {
                        global_state.show_key_missing_popup = false;
                    }
                });
            });
    }
}

/// Main serial UI layout system.
pub fn serial_ui(
    mut contexts: EguiContexts,
    mut serials: Query<&mut Serials>,
    mut selected: ResMut<Selected>,
    mut panel_widths: ResMut<PanelWidths>,
    mut global_state: ResMut<GlobalLlmState>,
    mut markdown_cache: ResMut<MarkdownViewerCache>,
    mut output_cache: ResMut<SerialOutputCache>,
) {
    let Ok(mut serials_data) = serials.single_mut() else {
        return;
    };
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };

    let selected_serial_exists = selected_serial_exists(&serials_data, &selected);

    draw_top_bar(
        ctx,
        &mut serials_data,
        selected.as_ref(),
        &mut panel_widths,
        selected_serial_exists,
    );
    draw_left_panel(&mut serials_data, selected.as_mut(), ctx, &mut panel_widths);
    draw_central_panel(&mut serials_data, selected.as_mut(), ctx, &mut output_cache);
    draw_right_panel(
        &mut serials_data,
        selected.as_ref(),
        ctx,
        &mut panel_widths,
        &mut global_state,
        &mut markdown_cache,
        selected_serial_exists,
    );
    draw_missing_config_popup(ctx, &mut global_state);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_serial_output_lines_preserves_ansi_colors() {
        let lines = parse_serial_output_lines("plain\n\x1b[31mred\x1b[0m");

        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0][0].text, "plain");
        assert_eq!(lines[1][0].text, "red");
        assert!(lines[1][0].foreground_color.is_some());
    }

    #[test]
    fn serial_output_cache_reuses_lines_until_revision_changes() {
        let mut cache = SerialOutputCache::default();

        assert!(cache.needs_refresh("COM1", 1));
        let first_ptr = {
            let lines = cache.lines_for("COM1", 1, Some("one"));
            assert_eq!(lines[0][0].text, "one");
            lines.as_ptr()
        };

        assert!(!cache.needs_refresh("COM1", 1));
        let second_ptr = cache.lines_for("COM1", 1, None).as_ptr();
        assert_eq!(first_ptr, second_ptr);

        assert!(cache.needs_refresh("COM1", 2));
        let lines = cache.lines_for("COM1", 2, Some("two"));
        assert_eq!(lines[0][0].text, "two");
    }
}
