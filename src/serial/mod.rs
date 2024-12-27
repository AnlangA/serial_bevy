pub mod port;
pub mod data;

use bevy::prelude::*;
use port::*;
use std::sync::Mutex;
use data::SerialChannel;
use tokio_serial::available_ports;
use std::sync::Once;
use std::sync::OnceLock;
use log::info;

static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
static INIT: Once = Once::new();


/// serial ports
#[derive(Component)]
pub struct Serials {
    pub serial: Vec<Mutex<Serial>>,
}

/// serial ports implementation
impl Serials {
    /// serial ports initialization
    pub fn new() -> Self {
        Serials {
            serial: vec![],
        }
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
        app.insert_resource(data::SerialChannel::default())
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
    let tx = channel.tx_serial2_world.clone();
    let rt = tokio::runtime::Runtime::new().unwrap();
    let _ = rt.spawn(async move {
        loop {
            let port_names: Vec<String> = match available_ports() {
                Ok(ports) => ports.into_iter().map(|p| p.port_name).collect(),
                Err(e) => {
                    info!("Error listing ports: {}", e);
                    Vec::<String>::new()
                }
            };
            let _ = tx.send(PortChannelData::PortName(port_names.clone()));
            tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
        }
    });
}

/// get runtime
fn get_runtime() -> &'static tokio::runtime::Runtime {
    INIT.call_once(|| {
        RUNTIME.set(tokio::runtime::Runtime::new().unwrap())
            .expect("Failed to initialize runtime");
    });
    RUNTIME.get().unwrap()
}

/// update serial port's name
fn update_serial_port_name(channel: Res<SerialChannel>, mut serials: Query<&mut Serials>) {
    let mut rx = channel.tx_serial2_world.subscribe();
    let mut serials = serials.single_mut();
    
    let names: Vec<String> = get_runtime()
        .block_on(rx.recv())
        .unwrap_or(PortChannelData::PortName(Vec::new()))
        .into();

    println!("names: {:?}", names);

    serials.serial.retain(|port|
        names.contains(&port.lock().unwrap().set.port_name)
    );

    for name in names {
        if !serials.serial.iter().any(|port| 
            port.lock().unwrap().set.port_name == name
        ) {
            let mut serial = Serial::new();
            serial.set.port_name = name;
            serials.add(serial);
        }
    }
}
