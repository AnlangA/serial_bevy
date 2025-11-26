//! # Egui Font Plugin
//!
//! This module provides a configurable plugin for loading and managing fonts in egui.
//! It supports loading multiple font files from custom paths and setting up font families
//! with proper priority ordering.
//!
//! ## Usage
//!
//! Add this plugin to your Bevy app:
//!
//! ```no_run
//! use bevy::prelude::*;
//! use bevy_egui::{EguiPlugin};
//! # use serial_bevy::fonts::{EguiFontPlugin, FontConfig};
//! # use bevy_egui::egui;
//!
//! App::new()
//!     .add_plugins(DefaultPlugins)
//!     .add_plugins(EguiPlugin::default())
//!     .add_plugins(
//!         EguiFontPlugin::default()
//!             .with_font("Song", "assets/fonts/STSong.ttf")
//!             .with_font("Custom", "assets/fonts/CustomFont.ttf")
//!             .with_font_config( FontConfig::new("Song", "assets/fonts/STSong.ttf").primary() )
//!             .with_theme(egui::Theme::Light)
//!     );
//! ```

use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPreUpdateSet, egui};
use std::path::PathBuf;

/// Configuration for a single font
#[derive(Debug, Clone)]
pub struct FontConfig {
    /// Name of the font (used as the font family name)
    pub name: String,
    /// Path to the font file
    pub path: PathBuf,
    /// Whether this font should be set as primary for proportional text
    pub primary_proportional: bool,
    /// Whether this font should be set as primary for monospace text
    pub primary_monospace: bool,
}

impl FontConfig {
    /// Create a new font configuration
    pub fn new(name: impl Into<String>, path: impl Into<PathBuf>) -> Self {
        Self {
            name: name.into(),
            path: path.into(),
            primary_proportional: false,
            primary_monospace: false,
        }
    }

    /// Set this font as primary for proportional text
    pub fn primary_proportional(mut self) -> Self {
        self.primary_proportional = true;
        self
    }

    /// Set this font as primary for monospace text
    pub fn primary_monospace(mut self) -> Self {
        self.primary_monospace = true;
        self
    }

    /// Set this font as primary for both proportional and monospace text
    pub fn primary(self) -> Self {
        self.primary_proportional().primary_monospace()
    }
}

/// Resource storing the complete font configuration
#[derive(Resource, Clone)]
pub struct EguiFontConfig {
    pub fonts: egui::FontDefinitions,
    pub theme: egui::Theme,
}

impl Default for EguiFontConfig {
    fn default() -> Self {
        Self {
            fonts: egui::FontDefinitions::default(),
            theme: egui::Theme::Light,
        }
    }
}

/// Plugin for loading and configuring egui fonts
#[derive(Default)]
pub struct EguiFontPlugin {
    fonts: Vec<FontConfig>,
    theme: Option<egui::Theme>,
}

impl EguiFontPlugin {
    /// Create a new font plugin with no fonts
    pub fn new() -> Self {
        Self {
            fonts: Vec::new(),
            theme: None,
        }
    }

    /// Add a font to be loaded
    pub fn with_font(mut self, name: impl Into<String>, path: impl Into<PathBuf>) -> Self {
        self.fonts.push(FontConfig::new(name, path));
        self
    }

    /// Add a font with full configuration
    pub fn with_font_config(mut self, config: FontConfig) -> Self {
        self.fonts.push(config);
        self
    }

    /// Set the egui theme
    pub fn with_theme(mut self, theme: egui::Theme) -> Self {
        self.theme = Some(theme);
        self
    }

    /// Load font configuration in Startup system
    fn load_font_config(mut commands: Commands, font_configs: Res<FontConfigsResource>) {
        let mut fonts = egui::FontDefinitions::default();

        // Load fonts in the order they were added
        for config in &font_configs.fonts {
            match std::fs::read(&config.path) {
                Ok(bytes) => {
                    info!(
                        "Loaded font '{}' from: {}",
                        config.name,
                        config.path.display()
                    );

                    fonts.font_data.insert(
                        config.name.clone(),
                        egui::FontData::from_owned(bytes).into(),
                    );

                    // Register the font family
                    fonts.families.insert(
                        egui::FontFamily::Name(config.name.clone().into()),
                        vec![config.name.clone()],
                    );

                    // Set as primary fonts if requested
                    if config.primary_proportional {
                        fonts
                            .families
                            .entry(egui::FontFamily::Proportional)
                            .or_default()
                            .insert(0, config.name.clone());
                    }

                    if config.primary_monospace {
                        fonts
                            .families
                            .entry(egui::FontFamily::Monospace)
                            .or_default()
                            .insert(0, config.name.clone());
                    }
                }
                Err(e) => {
                    warn!(
                        "Failed to load font '{}' from: {}: {}",
                        config.name,
                        config.path.display(),
                        e
                    );
                }
            }
        }

        let theme = font_configs.theme.unwrap_or(egui::Theme::Light);

        commands.insert_resource(EguiFontConfig { fonts, theme });

        info!(
            "Font configuration prepared with {} fonts",
            font_configs.fonts.len()
        );
    }

    /// Apply font and theme configuration using EguiPreUpdateSet::InitContexts
    fn apply_font_config(
        mut contexts: EguiContexts,
        font_config: Res<EguiFontConfig>,
        mut has_applied: Local<bool>,
    ) {
        if *has_applied {
            return;
        }

        if let Ok(ctx) = contexts.ctx_mut() {
            ctx.set_fonts(font_config.fonts.clone());
            ctx.set_theme(font_config.theme);
            *has_applied = true;
            info!("Fonts and theme applied successfully");
        }
    }
}

/// Resource to store font configurations
#[derive(Resource, Default, Clone)]
struct FontConfigsResource {
    fonts: Vec<FontConfig>,
    theme: Option<egui::Theme>,
}

impl Plugin for EguiFontPlugin {
    fn build(&self, app: &mut App) {
        // Store font configurations in a resource
        app.insert_resource(FontConfigsResource {
            fonts: self.fonts.clone(),
            theme: self.theme,
        });

        // Add systems for loading and applying fonts
        app.add_systems(Startup, Self::load_font_config)
            .add_systems(
                PreUpdate,
                Self::apply_font_config.in_set(EguiPreUpdateSet::InitContexts),
            );
    }
}
