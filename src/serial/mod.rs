pub mod data;
pub mod port;

use bevy::prelude::*;
use bevy::text::cosmic_text::ttf_parser::name;
use data::SerialNameChannel;
use log::info;
use port::*;
use std::fmt::Debug;
use std::sync::Mutex;
use tokio;
use tokio::io::{self, AsyncReadExt, AsyncWriteExt};
use tokio::sync::broadcast;
use tokio_serial::available_ports;

/// runtime
#[derive(Resource)]
pub struct Runtime {
    rt: tokio::runtime::Runtime,
}

/// runtime implementation
impl Runtime {
    pub fn init() -> Self {
        Self {
            rt: tokio::runtime::Runtime::new().unwrap(),
        }
    }
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
        app.insert_resource(Runtime::init())
            .insert_resource(SerialNameChannel::init())
            .add_systems(Startup, (init, spawn_serach_name))
            .add_systems(
                Update,
                (
                    update_serial_port_name,
                    create_serial_port_thread,
                    send_serial_data,
                    receive_serial_data,
                )
                    .chain(),
            );
    }
}

/// serial components initialization
fn init(mut commands: Commands) {
    commands.spawn(Serials::new());
}

/// serach serial port's name
fn spawn_serach_name(channel: Res<SerialNameChannel>, runtime: Res<Runtime>) {
    let tx = channel.tx_world2_serial.clone();
    runtime.rt.spawn(async move {
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
                    serial.set.port_name = name.to_string();
                    serials.add(serial);
                }
            }
        }
        Err(_) => {}
    }
}

/// create serial port thread
fn create_serial_port_thread(mut serials: Query<&mut Serials>, runtime: Res<Runtime>) {
    let mut serials = serials.single_mut();
    for serial in serials.serial.iter_mut() {
        let mut serial = serial.lock().unwrap();

        // create thread
        if serial.thread_handle().is_none() {
            let (tx, mut rx) = broadcast::channel(100);
            let (tx1, rx1) = broadcast::channel(100);

            let mut rx_shutdown = tx.subscribe();
            *serial.tx_channel() = Some(tx);
            *serial.rx_channel() = Some(rx1);

            let port_settings = serial.set.clone();
            let port_name = port_settings.port_name.clone();

            let handle = runtime.rt.spawn(async move {
                let port = loop {
                    if let Ok(data) = rx.recv().await {
                        if let PortChannelData::PortOpen = data {
                            if let Some(port) = open_port(port_settings).await {
                                break port;
                            } else {
                                match tx1.send(PortChannelData::PortClose("open port failed".into())) {
                                    Ok(_) => {},
                                    Err(e) => error!("Failed to send port close message: {}", e),
                                }
                                return;
                            }
                        }
                    }
                };

                info!("open serial port: {}", port_name);
                match tx1.send(PortChannelData::PortState(port::State::Ready)) {
                    Ok(_) => {},
                    Err(e) => error!("Failed to send port ready state: {}", e),
                }

                let (read, write) = io::split(port);

                // read thread
                let tx1_read = tx1.clone();
                let read_handle = tokio::spawn(async move {
                    let mut read = read;
                    let mut buffer = [0u8; 1024];

                    loop {
                        tokio::select! {
                            // check shutdown signal
                            result = rx_shutdown.recv() => {
                                if let Ok(PortChannelData::PortClose(name)) = result {
                                    info!("Received close command, read thread exiting. port name: {}", name);
                                    break;
                                }
                            }
                            // read serial port data
                            result = read.read(&mut buffer) => {
                                match result {
                                    Ok(n) => {
                                        let data = PorRWData {
                                            data: buffer[..n].to_vec(),
                                        };
                                        match tx1_read.send(PortChannelData::PortRead(data)) {
                                            Ok(_) => {},
                                            Err(e) => error!("Failed to send read data: {}", e),
                                        }
                                    }
                                    Err(e) => {
                                        error!("read error: {}", e);
                                        break;
                                    }
                                }
                            }
                        }
                    }
                });

                // write thread
                let mut write = write;
                loop {
                    if let Ok(data) = rx.recv().await {
                        match data {
                            PortChannelData::PortWrite(data) => {
                                if write.write(&data.data).await.is_err() {
                                    break;
                                }
                            }
                            PortChannelData::PortClose(name) => {
                                info!("close serial port: {}", name);
                                tx1.send(PortChannelData::PortState(port::State::Close))
                                    .unwrap();
                                break;
                            }
                            _ => {}
                        }
                    }
                }

                // clean up
                read_handle.abort();
                info!("serial port thread exit");
            });

            *serial.thread_handle() = Some(handle);
        }
    }
}

/// send serial data
fn send_serial_data(mut serials: Query<&mut Serials>) {
    let mut serials = serials.single_mut();
    for serial in serials.serial.iter_mut() {
        let mut serial = serial.lock().unwrap();

        // convert data to the corresponding format
        let data = serial.data().get_send_data();
        if data.is_empty() {
            continue;
        }

        // convert data to u8, then convert to `port::Type`
        let data = data
            .iter()
            .flat_map(|d| d.as_bytes().iter().copied())
            .collect::<Vec<u8>>();
        let mut file_data = String::from("Write:").as_bytes().to_vec();
        file_data.append(&mut data.clone());
        serial.data().write_source_file(&file_data);

        if serial.is_open() {
            if let Some(tx) = serial.tx_channel() {
                match tx.send(PortChannelData::PortWrite(PorRWData { data })) {
                    Ok(_) => {}
                    Err(e) => error!("Failed to send data: {}", e),
                }
            }
        }
    }
}

/// receive serial data
fn receive_serial_data(mut serials: Query<&mut Serials>) {
    let mut serials = serials.single_mut();

    for serial in serials.serial.iter_mut() {
        let mut serial = serial.lock().unwrap();

        let rx = match serial.rx_channel() {
            Some(rx) => rx,
            None => continue,
        };

        if let Ok(data) = rx.try_recv() {
            match data {
                PortChannelData::PortState(state) => match state {
                    port::State::Ready | port::State::Close => {
                        if matches!(state, port::State::Ready) {
                            serial.open()
                        } else {
                            serial.close()
                        };
                        serial.data().clear_send_data();
                    }
                    _ => {}
                },
                PortChannelData::PortRead(data) => {
                    let mut file_data = b"Read:".to_vec();
                    file_data.extend(data.data);
                    serial.data().write_source_file(&file_data);
                }
                _ => {}
            }
        }
    }
}
