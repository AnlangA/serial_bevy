use bevy::app::AppExit;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Configuration file path for app persistence.
const CONFIG_FILE: &str = "config/app_memory.ron";

/// Resource storing current (and persisted) UI configuration.
/// Saved to disk directly, independent of egui memory.
#[derive(Resource, Clone, Serialize, Deserialize, PartialEq)]
pub struct PanelWidths {
    /// Current (user-adjustable) width of the left side panel.
    pub left_width: f32,
    /// Current (user-adjustable) width of the right side panel.
    pub right_width: f32,
    /// Whether the settings side panel is visible.
    #[serde(default = "default_true")]
    pub show_settings_panel: bool,
    /// Whether the LLM side panel is visible.
    #[serde(default)]
    pub show_llm_panel: bool,
    /// Global LLM API key (shared across all serial ports).
    #[serde(default)]
    pub llm_key: String,
    /// Global LLM model selection (shared across all serial ports).
    #[serde(default)]
    pub llm_model: String,
    /// Global LLM coding plan toggle (shared across all serial ports).
    #[serde(default)]
    pub llm_with_coding_plan: bool,
}

impl Default for PanelWidths {
    fn default() -> Self {
        Self {
            left_width: 160.0,
            right_width: 220.0,
            show_settings_panel: true,
            show_llm_panel: false,
            llm_key: String::new(),
            llm_model: String::from("glm-4.5-air"),
            llm_with_coding_plan: false,
        }
    }
}

impl PanelWidths {
    /// Clamp widths to valid ranges.
    fn clamp(&mut self) {
        self.left_width = self.left_width.clamp(120.0, 600.0);
        self.right_width = self.right_width.clamp(160.0, 800.0);
    }
}

const fn default_true() -> bool {
    true
}

/// Load configuration directly from disk file.
fn load_config_from_disk() -> Option<PanelWidths> {
    if let Ok(data) = std::fs::read_to_string(CONFIG_FILE) {
        match ron::from_str::<PanelWidths>(&data) {
            Ok(mut widths) => {
                widths.clamp();
                log::debug!("[serial_ui] Loaded panel config from disk");
                return Some(widths);
            }
            Err(e) => {
                log::warn!("[serial_ui] Failed to parse config file: {e}, using defaults");
            }
        }
    }
    None
}

/// Save configuration directly to disk file.
fn save_config_to_disk(widths: &PanelWidths) {
    log::debug!(
        "[serial_ui] Saving panel config to disk: left={}, right={}",
        widths.left_width,
        widths.right_width
    );

    if let Err(e) = std::fs::create_dir_all("config") {
        eprintln!("[serial_ui] Failed to create config directory: {e}");
        return;
    }

    match ron::to_string(widths) {
        Ok(data) => {
            if let Err(e) = std::fs::write(CONFIG_FILE, data) {
                eprintln!("[serial_ui] Failed to write config file: {e}");
            } else {
                log::debug!("[serial_ui] Saved panel config to disk");
            }
        }
        Err(e) => {
            eprintln!("[serial_ui] Failed to serialize config: {e}");
        }
    }
}

/// System: initialize panel config resource, loading from disk if available.
pub fn init_panel_widths(mut commands: Commands) {
    let config = load_config_from_disk().unwrap_or_default();
    commands.insert_resource(config);
}

/// System: save configuration directly from resource when app is exiting.
pub fn save_config_on_exit(
    panel_widths: Res<PanelWidths>,
    mut exit_events: MessageReader<AppExit>,
) {
    if !exit_events.is_empty() {
        exit_events.clear();
        log::debug!("[serial_ui] App exit detected, saving configuration...");
        save_config_to_disk(&panel_widths);
    }
}
