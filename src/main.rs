use bevy::{
    prelude::*,
    render::camera::RenderTarget,
    window::{PresentMode, PrimaryWindow, WindowRef, WindowResolution},
};
use bevy::window::WindowClosing;

use serial_bevy::screen::*;
use serial_bevy::serial::*;
use serial_bevy::serial_ui::*;
use std::time::Duration;

#[derive(Resource)]
pub struct GameTimer(Timer);

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(SerialPlugin)
        .add_plugins(SerialUiPlugin)
        .add_plugins(ScreenPlugin)
        .add_systems(Startup, setup)
        .add_systems(Update, (send_serial_data, print_keyboard_event_system))
        .run();
}

fn setup(mut commands: Commands) {
    commands.insert_resource(GameTimer(Timer::new(
        Duration::from_secs(2),
        TimerMode::Repeating,
    )));
}

fn send_serial_data(
    mut commands: Commands,
    mut serials: Query<&mut Serials>,
    mut timer: ResMut<GameTimer>,
    time: Res<Time>,
) {
    if !timer.0.tick(time.delta()).just_finished() {
        return;
    }

    let mut serials = serials.single_mut();
    for serial in serials.serial.iter_mut() {
        let mut serial = serial.lock().unwrap();
        if serial.is_open() {
            if let None = serial.window() {
                let window_id = commands
                    .spawn(Window {
                        title: serial.set.port_name().to_owned(),
                        resolution: WindowResolution::new(800.0, 600.0),
                        present_mode: PresentMode::AutoVsync,
                        ..Default::default()
                    })
                    .id();

                // second window camera
                commands.spawn((
                    Camera3d::default(),
                    Camera {
                        target: RenderTarget::Window(WindowRef::Entity(window_id)),
                        ..Default::default()
                    },
                    Transform::from_xyz(6.0, 0.0, 0.0).looking_at(Vec3::ZERO, Vec3::Y),
                ));
                info!("{} window id: {}", serial.set.port_name(), window_id);
                *serial.window() = Some(window_id);
            }
            serial.data().send_data("你好呀".to_string());
        }
    }
}

fn print_keyboard_event_system(mut keyboard_input_events: EventReader<WindowClosing>, mut serials: Query<&mut Serials>) {
    for event in keyboard_input_events.read() {
        let mut serial = serials.get_single_mut().unwrap();
        for serial in serial.serial.iter_mut() {
            let mut serial = serial.lock().unwrap();
            let port_name = serial.set.port_name.clone();
            if serial.is_open() {
                if let Some(window) = serial.window() {
                    if *window == event.window {
                        *serial.window() = None;
                        if let Some(tx) = serial.tx_channel() {
                            match tx.send(port::PortChannelData::PortClose(port_name)) {
                                Ok(_) => {
                                    info!("Send close port message");
                                }
                                Err(e) => error!("Failed to close port: {}", e),
                            }
                        }
                    }
                }
            }
        }
    }
}