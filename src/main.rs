//! # Serial Bevy
//!
//! A serial port communication tool built with the Bevy game engine.
//!
//! This application provides an intuitive GUI for serial port communication,
//! supporting features like:
//!
//! - Automatic port discovery
//! - Configurable baud rate, data bits, stop bits, parity, and flow control
//! - Hex and UTF-8 data encoding
//! - Command history with arrow key navigation
//! - Optional LLM integration

use bevy::prelude::*;
use serial_bevy::prelude::*;

/// Application entry point.
fn main() {
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Serial Bevy - Serial Port Tool".to_string(),
                        ..default()
                    }),
                    ..default()
                })
                .build(),
        )
        .add_plugins(SerialPlugin)
        .add_plugins(EguiFontPlugin::default().with_font("Song", "assets/fonts/STSong.ttf"))
        .add_plugins(SerialUiPlugin)
        .run();
}
