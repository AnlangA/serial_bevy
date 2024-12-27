use bevy::prelude::*;
use serial_bevy::serial::*;

fn main() {
    App::new().add_plugins(DefaultPlugins)
    .add_plugins(SerialPlugin)
    .run();
}
