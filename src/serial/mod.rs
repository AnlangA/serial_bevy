pub mod data;
pub mod port;

use bevy::prelude::*;
use data::SerialNameChannel;
use log::info;
use port::*;
use std::fmt::Debug;
use std::sync::Mutex;
use std::sync::OnceLock;
use tokio::io::{self, AsyncReadExt, AsyncWriteExt};
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

/// serial ports debug
impl Debug for Serials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = String::new();
        for serial in self.serial.iter() {
            s.push_str(&format!(
                "name: {:?} ",
                serial.lock().unwrap().set.port_name
            ));
            s.push_str(&format!(
                "baud_rate: {:?} ",
                serial.lock().unwrap().set.baud_rate
            ));
            s.push_str(&format!(
                "data_bits: {:?} ",
                serial.lock().unwrap().set.data_bits
            ));
            s.push_str(&format!(
                "stop_bits: {:?} ",
                serial.lock().unwrap().set.stop_bits
            ));
            s.push_str(&format!("parity: {:?} ", serial.lock().unwrap().set.parity));
            s.push_str(&format!(
                "flow_control: {:?} ",
                serial.lock().unwrap().set.flow_control
            ));
            s.push_str(&format!(
                "timeout: {:?} ",
                serial.lock().unwrap().set.timeout
            ));
            s.push_str("\n");
        }
        write!(f, "{}", s)
    }
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
        app.insert_resource(SerialNameChannel::init())
            .add_systems(Startup, (init, spawn_serach_name))
            .add_systems(Update, (update_serial_port_name, create_serial_port_thread));
    }
}

/// serial components initialization
fn init(mut commands: Commands) {
    commands.spawn(Serials::new());
}

/// serach serial port's name
fn spawn_serach_name(channel: Res<SerialNameChannel>) {
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
                Ok(_) => {}
                Err(e) => {
                    println!("error: {:?}", e);
                }
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
        }
    });
}

/// update serial port's name
fn update_serial_port_name(
    mut channel: ResMut<SerialNameChannel>,
    mut serials: Query<&mut Serials>,
) {
    let mut serials = serials.single_mut();

    match channel.rx_serial2_world.try_recv() {
        Ok(names) => {
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

/// create serial port thread
fn create_serial_port_thread(mut serials: Query<&mut Serials>) {
    let mut serials = serials.single_mut();
    for serial in serials.serial.iter_mut() {
        let mut serial = serial.lock().unwrap();
        if serial.data().state().to_owned() == port::State::Open {
            // create thread
            if serial.thread_handle().is_none() {
                let (tx, mut rx) = broadcast::channel(100);
                let (tx1, rx1) = broadcast::channel(100);

                serial.set_tx_channel(tx);
                serial.set_rx_channel(rx1);
                let port_settings = serial.set.clone();
                let handle = get_runtime().spawn(async move {
                    let port = open_port(port_settings).await.unwrap();
                    let (mut read, mut write) = io::split(port);

                    tokio::spawn(async move {
                        while let Ok(data) = rx.recv().await {
                            if let PortChannelData::PortWrite(data) = data {
                                write.write(&data.data).await.unwrap();
                            }
                        }
                    });

                    tokio::spawn(async move {
                        let mut buffer: [u8; 1024] = [0; 1024];
                        while let Ok(n) = read.read(&mut buffer).await {
                            let data = PorRWData {
                                data: buffer[..n].to_vec(),
                            };
                            tx1.send(PortChannelData::PortRead(data)).unwrap();
                        }
                    });
                });

                serial.set_thread_handle(handle);
            }
        }
    }
}
