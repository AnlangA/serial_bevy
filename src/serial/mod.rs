pub mod data;
pub mod port;

use bevy::prelude::*;
use data::SerialNameChannel;
use log::info;
use port::*;
use std::fmt::Debug;
use std::sync::Mutex;
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
                    update_serial_port_state,
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

            *serial.tx_channel() = Some(tx);
            *serial.rx_channel() = Some(rx1);
            let port_settings = serial.set.clone();
            let port_name = port_settings.port_name.clone();

            let handle = runtime.rt.spawn(async move {
                let port = loop {
                    if let Ok(data) = rx.recv().await {
                        match data {
                            PortChannelData::PortOpen => {
                                match open_port(port_settings).await {
                                    Some(port) => {
                                        break port;
                                    }
                                    None => {
                                        tx1.send(PortChannelData::PortClose(String::from("串口打开失败"))).unwrap();
                                        info!("open serial port error: ",);
                                        return;
                                    }
                                }
                            }
                            
                            _ => {}
                        }
                    }
                };
                info!("open serial port: {}", port_name);
                tx1.send(PortChannelData::PortState(port::State::Ready))
                    .unwrap();

                let (mut read, mut write) = io::split(port);

                loop {
                    if let Ok(data) = rx.recv().await {
                        match data {
                            PortChannelData::PortWrite(data) => {
                                write.write(&data.data).await.unwrap();
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
                    let mut buffer: [u8; 1024] = [0; 1024];
                    if let Ok(n) = read.read(&mut buffer).await {
                        let data = PorRWData {
                            data: buffer[..n].to_vec(),
                        };
                        tx1.send(PortChannelData::PortRead(data)).unwrap();
                    }
                }
                info!("serial port thread exit");
            });
            *serial.thread_handle() = Some(handle);
        }
    }
}

/// update serial port state
fn update_serial_port_state(mut serials: Query<&mut Serials>) {
    let mut serials = serials.single_mut();
    for serial in serials.serial.iter_mut() {
        let mut serial = serial.lock().unwrap();
        if let Some(rx) = serial.rx_channel().as_mut() {
            if let Ok(data) = rx.try_recv() {
                match data {
                    PortChannelData::PortState(data) => match data {
                        port::State::Ready => {
                            serial.open();
                            serial.data().clear_send_data();
                        }
                        port::State::Close => {
                            serial.close();
                            serial.data().clear_send_data();
                        }
                        _ => {}
                    },
                    _ => {}
                }
            }
        }
    }
}

/// send serial data
fn send_serial_data(mut serials: Query<&mut Serials>) {
    let mut serials = serials.single_mut();
    for serial in serials.serial.iter_mut() {
        let mut serial = serial.lock().unwrap();

        //将数据转换成对于的格式
        let data = serial.data().get_send_data();
        if data.is_empty() {
            continue;
        }

        //将数据转换成u8，后续按 `port::Type` 进行转换
        let data = data
            .iter()
            .flat_map(|d| d.as_bytes().iter().copied())
            .collect::<Vec<u8>>();
        let mut file_data = String::from("Write:").as_bytes().to_vec(); 
        file_data.append(&mut data.clone());
        serial.data().write_source_file(&file_data);

        if serial.is_open() {
            if let Some(tx) = serial.tx_channel() {
                tx.send(PortChannelData::PortWrite(PorRWData { data }))
                    .unwrap();
            }
        }
    }
}

/// receive serial data
fn receive_serial_data(mut serials: Query<&mut Serials>) {
    let mut serials = serials.single_mut();
    for serial in serials.serial.iter_mut() {
        let mut serial = serial.lock().unwrap();
        if let Some(rx) = serial.rx_channel() {
            if let Ok(data) = rx.try_recv() {
                match data {
                    PortChannelData::PortRead(data) => {
                        info!("receive serial data: {:?}", data);
                        let mut data = data.data;
                        let rd = String::from("Read:");
                        let mut file_data = rd.as_bytes().to_vec();
                        file_data.append(&mut data);
                        serial.data().write_source_file(&file_data);
                    }
                    _ => {}
                }
            }
        }
    }
}
