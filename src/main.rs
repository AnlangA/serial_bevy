use bevy::prelude::*;
use serial_bevy::serial::*;
use std::time::Duration;
#[derive(Resource)]
pub struct GameTimer(Timer);

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(SerialPlugin)
        .add_systems(Startup, setup)
        .add_systems(Update, send_serial_data)
        .run();
}

fn setup(mut commands: Commands) {
    commands.insert_resource(GameTimer(Timer::new(Duration::from_secs(2), TimerMode::Repeating)));
}

fn send_serial_data(mut serials: Query<&mut Serials>, mut timer: ResMut<GameTimer>, time: Res<Time>) {
    if !timer.0.tick(time.delta()).just_finished() {
        return;
    }
    info!("发送数据");
    let mut serials = serials.single_mut();
    for serial in serials.serial.iter_mut() {
        let mut serial = serial.lock().unwrap();
        if serial.set.port_name == "COM9" {
            if serial.data().state().to_owned() != port::State::Ready {
                if let Some(tx) = serial.tx_channel(){
                    tx.send(port::PortChannelData::PortOpen).unwrap();
                }
            } else {
                serial.data().send_data("你好".to_string());
            }
        }
    }
}
