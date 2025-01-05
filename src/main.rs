use bevy::prelude::*;

use serial_bevy::serial::*;
use serial_bevy::serial_ui::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(SerialPlugin)
        .add_plugins(SerialUiPlugin)
        .run();
}
