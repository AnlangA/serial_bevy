use bevy::prelude::*;

use serial_bevy::serial::*;
use serial_bevy::serial_ui::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "serial_bevy".to_string(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(SerialPlugin)
        .add_plugins(SerialUiPlugin)
        .run();
}
