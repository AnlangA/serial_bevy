pub mod data;
pub mod port;

use bevy::prelude::*;
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
        if serial.thread_handle().is_none() {
            setup_serial_thread(&mut serial, &runtime);
        }
    }
}

/// setup serial thread
fn setup_serial_thread(serial: &mut Serial, runtime: &Runtime) {
    let (tx, mut rx) = broadcast::channel(100);
    let (tx1, rx1) = broadcast::channel(100);
    let rx_shutdown = tx.subscribe();

    *serial.tx_channel() = Some(tx);
    *serial.rx_channel() = Some(rx1);

    let port_settings = serial.set.clone();
    let port_name = port_settings.port_name.clone();

    let handle = runtime.rt.spawn(async move {
        #[allow(unused_assignments)]
        let mut port: Option<SerialStream> = None;
        port = match wait_for_port_open(&mut rx, &tx1, port_settings).await {
            Ok(p) => Some(p),
            Err(e) => {
                return Err(e.into());
            }
        };

        info!("open serial port: {}", port_name);
        if let Err(e) = notify_port_ready(&tx1) {
            return Err(e.into());
        }
        let port = port.unwrap();
        let (read, write) = io::split(port);
        let read_handle = spawn_read_thread(read, tx1.clone(), rx_shutdown, &port_name);

        handle_write_thread(write, rx, tx1, &port_name).await;

        // clean up
        read_handle.abort();
        info!("serial port thread exit");
        Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
    });

    *serial.thread_handle() = Some(handle);
}

/// wait for port open
async fn wait_for_port_open(
    rx: &mut broadcast::Receiver<PortChannelData>,
    tx1: &broadcast::Sender<PortChannelData>,
    port_settings: PortSettings,
) -> Result<tokio_serial::SerialStream, Box<dyn std::error::Error + Send + Sync>> {
    loop {
        if let Ok(data) = rx.recv().await {
            if let PortChannelData::PortOpen = data {
                if let Some(port) = open_port(port_settings).await {
                    return Ok(port);
                } else {
                    match tx1.send(PortChannelData::PortError(PorRWData {
                        data: b"open port failed".to_vec(),
                    })) {
                        Ok(_) => {}
                        Err(e) => error!("发送端口关闭消息失败: {}", e),
                    }
                    return Err("Failed to open port".into());
                }
            }
        }
    }
}

/// notify port ready
fn notify_port_ready(
    tx1: &broadcast::Sender<PortChannelData>,
) -> Result<(), broadcast::error::SendError<PortChannelData>> {
    match tx1.send(PortChannelData::PortState(port::State::Ready)) {
        Ok(_) => Ok(()),
        Err(e) => {
            error!("Failed to send port ready state: {}", e);
            Err(e)
        }
    }
}

/// spawn read thread
fn spawn_read_thread(
    mut read: io::ReadHalf<tokio_serial::SerialStream>,
    tx1_read: broadcast::Sender<PortChannelData>,
    mut rx_shutdown: broadcast::Receiver<PortChannelData>,
    port_name: &str,
) -> tokio::task::JoinHandle<()> {
    let port_name = port_name.to_owned();
    tokio::spawn(async move {
        let mut buffer = [0u8; 1024];
        loop {
            tokio::select! {
                result = rx_shutdown.recv() => {
                    if let Ok(PortChannelData::PortClose(name)) = result {
                        info!("close serial port read thread: {}", name);
                        break;
                    }
                }
                result = read.read(&mut buffer) => {
                    match result {
                        Ok(n) => {
                            let data = PorRWData {
                                data: buffer[..n].to_vec(),
                            };
                            match tx1_read.send(PortChannelData::PortRead(data.clone())) {
                                Ok(_) => {info!("{} read : {:?}", port_name, data.data)},
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
    })
}

/// handle write thread
async fn handle_write_thread(
    mut write: io::WriteHalf<tokio_serial::SerialStream>,
    mut rx: broadcast::Receiver<PortChannelData>,
    tx1: broadcast::Sender<PortChannelData>,
    port_name: &str,
) {
    loop {
        if let Ok(data) = rx.recv().await {
            match data {
                PortChannelData::PortWrite(data) => {
                    info!("{} write : {:?}", port_name, data.data);
                    if write.write(&data.data).await.is_err() {
                        info!("{} write error", port_name);
                        break;
                    }
                }
                PortChannelData::PortClose(name) => {
                    info!("close serial port write thread: {}", name);
                    tx1.send(PortChannelData::PortState(port::State::Close))
                        .unwrap();
                    break;
                }
                _ => {}
            }
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

        serial.data().write_source_file(&data, DataSource::Write);
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
                    serial
                        .data()
                        .write_source_file(&data.data, DataSource::Read);
                }
                PortChannelData::PortError(data) => {
                    serial.error();
                    serial
                        .data()
                        .write_source_file(&data.data, DataSource::Error);
                }
                _ => {}
            }
        }
    }
}
