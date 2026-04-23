//! # Serial UI Module
//!
//! This module provides the UI plugin and composes focused submodules for:
//! - persisted UI configuration
//! - runtime-only global LLM state
//! - main layout rendering
//! - keyboard/input systems

pub mod config;
pub mod global_llm;
pub mod input;
pub mod layout;
pub mod ui;

use bevy::prelude::*;
use bevy_egui::{EguiPlugin, EguiPrimaryContextPass};

use crate::serial::Selected;

use config::{init_panel_widths, save_config_on_exit};
use global_llm::{
    GlobalLlmResponse, GlobalLlmState, process_global_llm_requests, receive_global_llm_responses,
};
use input::{history_data_checkout, send_cache_data};
use layout::serial_ui;
use ui::{MarkdownViewerCache, draw_serial_context_ui};

pub use config::PanelWidths;

/// Plugin for the serial UI.
pub struct SerialUiPlugin;

fn setup_camera_system(mut commands: Commands) {
    commands.spawn(Camera2d);
}

impl Plugin for SerialUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(EguiPlugin::default())
            .insert_resource(ClearColor(Color::srgb(0.96875, 0.96875, 0.96875)))
            .insert_resource(Selected::default())
            .insert_resource(MarkdownViewerCache::default())
            .insert_resource(GlobalLlmState::default())
            .insert_resource(GlobalLlmResponse::init())
            .add_systems(Startup, (setup_camera_system, init_panel_widths))
            .add_systems(Last, save_config_on_exit)
            .add_systems(
                EguiPrimaryContextPass,
                (
                    serial_ui,
                    draw_serial_context_ui,
                    send_cache_data,
                    history_data_checkout,
                )
                    .chain(),
            )
            .add_systems(
                Update,
                (process_global_llm_requests, receive_global_llm_responses).chain(),
            );
    }
}
