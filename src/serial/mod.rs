pub mod data;
pub mod port;

use bevy::prelude::*;
use data::SerialChannel;
use log::info;
use port::*;
use std::sync::Mutex;
use std::sync::OnceLock;
use tokio::sync::broadcast;
use tokio_serial::available_ports;

static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();

pub fn get_runtime() -> &'static tokio::runtime::Runtime {
    RUNTIME.get_or_init(|| tokio::runtime::Runtime::new().unwrap())
}

/// serial ports
#[derive(Component)]
pub struct Serials {
    pub serial: Vec<Mutex<Serial>>,
}

/// serial ports implementation
impl Serials {
    /// serial ports initialization
    pub fn new() -> Self {
        Serials { serial: vec![] }
    }

    /// add serial port
    pub fn add(&mut self, serial: Serial) {
        self.serial.push(Mutex::new(serial));
    }

    /// remove serial port
    pub fn remove(&mut self, index: usize) {
        self.serial.remove(index);
    }

    /// get serial port
    pub fn get(&self, index: usize) -> &Mutex<Serial> {
        &self.serial[index]
    }
}

#[derive(Default)]
pub struct SerialPlugin;

impl Plugin for SerialPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(SerialChannel::init())
            .add_systems(Startup, (init, spawn_serach_name))
            .add_systems(Update, update_serial_port_name);
    }
}

/// serial components initialization
fn init(mut commands: Commands) {
    commands.spawn(Serials::new());
}

/// serach serial port's name
fn spawn_serach_name(channel: Res<SerialChannel>) {
    let tx = channel.tx_world2_serial.clone();
    get_runtime().spawn(async move {
        loop {
            let port_names: Vec<String> = match available_ports() {
                Ok(ports) => ports.into_iter().map(|p| p.port_name).collect(),
                Err(e) => {
                    info!("Error listing ports: {}", e);
                    Vec::<String>::new()
                }
            };
            match tx.send(PortChannelData::PortName(port_names.clone())) {
                Ok(_) => {

                }
                Err(e) => {
                    println!("error: {:?}", e);
                }
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
        }
    });
}

/// update serial port's name
fn update_serial_port_name(mut channel: ResMut<SerialChannel>, mut serials: Query<&mut Serials>) {
    let mut serials = serials.single_mut();

    match channel.rx_serial2_world.try_recv() {
        Ok(names) => {
            println!("names: {:?}", names);
            let port_names: Vec<String> = names.into();

            serials.serial.retain(|port| {
                port_names
                    .iter()
                    .any(|name| port.lock().unwrap().set.port_name == *name)
            });

            for name in port_names.iter() {
                if !serials
                    .serial
                    .iter()
                    .any(|port| port.lock().unwrap().set.port_name == *name)
                {
                    let mut serial = Serial::new();
                    serial.set.port_name = name.clone();
                    serials.add(serial);
                }
            }
        }
        Err(_) => {}
    }
}
