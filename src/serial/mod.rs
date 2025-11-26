//! # Serial Module
//!
//! This module provides the core serial port communication functionality.
//! It includes:
//!
//! - Port discovery and management
//! - Async read/write operations
//! - Data encoding/decoding (Hex, UTF-8)
//! - Thread-safe communication channels

pub mod data;
pub mod encoding;
pub mod port;

use bevy::prelude::*;
use data::SerialNameChannel;
use log::{error, info};
use std::sync::Mutex;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::broadcast;
use tokio_serial::{SerialPortType, available_ports};

use crate::error::SerialBevyError;

// Re-exports for convenience
pub use encoding::*;
pub use port::*;

/// Tokio runtime resource for async operations.
///
/// This resource wraps the Tokio runtime to enable async operations
/// within the Bevy ECS framework.
#[derive(Resource)]
pub struct Runtime {
    /// The Tokio runtime instance.
    rt: tokio::runtime::Runtime,
}

impl Runtime {
    /// Creates a new Runtime instance.
    ///
    /// # Panics
    ///
    /// Panics if the Tokio runtime cannot be created.
    #[must_use]
    pub fn init() -> Self {
        Self {
            rt: tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime"),
        }
    }

    /// Spawns an async task on the runtime.
    pub fn spawn<F>(&self, future: F) -> tokio::task::JoinHandle<F::Output>
    where
        F: std::future::Future + Send + 'static,
        F::Output: Send + 'static,
    {
        self.rt.spawn(future)
    }
}

impl Default for Runtime {
    fn default() -> Self {
        Self::init()
    }
}

/// Container for managing multiple serial ports.
///
/// This component holds a collection of serial port instances,
/// each protected by a mutex for thread-safe access.
#[derive(Component)]
pub struct Serials {
    /// Vector of mutex-protected serial port instances.
    pub serial: Vec<Mutex<Serial>>,
}

impl std::fmt::Debug for Serials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut debug = f.debug_list();
        for serial in &self.serial {
            if let Ok(s) = serial.lock() {
                debug.entry(&format!("{}: {}bps", s.set.port_name, s.set.baud_rate));
            }
        }
        debug.finish()
    }
}

impl Default for Serials {
    fn default() -> Self {
        Self::new()
    }
}

impl Serials {
    /// Creates a new empty Serials container.
    #[must_use]
    pub const fn new() -> Self {
        Self { serial: vec![] }
    }

    /// Adds a serial port to the container.
    pub fn add(&mut self, serial: Serial) {
        self.serial.push(Mutex::new(serial));
    }

    /// Removes a serial port at the specified index.
    ///
    /// # Panics
    ///
    /// Panics if the index is out of bounds.
    pub fn remove(&mut self, index: usize) {
        self.serial.remove(index);
    }

    /// Gets a reference to the mutex-protected serial port at the specified index.
    ///
    /// # Panics
    ///
    /// Panics if the index is out of bounds.
    #[must_use]
    pub fn get(&self, index: usize) -> &Mutex<Serial> {
        &self.serial[index]
    }

    /// Returns the number of serial ports.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.serial.len()
    }

    /// Returns true if there are no serial ports.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.serial.is_empty()
    }
}

/// The main serial communication plugin.
///
/// This plugin provides:
/// - Serial port discovery
/// - Async read/write operations
/// - Port state management
#[derive(Default)]
pub struct SerialPlugin;

impl Plugin for SerialPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Runtime::init())
            .insert_resource(SerialNameChannel::init())
            .add_systems(Startup, (init_serial_components, spawn_port_discovery))
            .add_systems(
                Update,
                (
                    update_serial_port_names,
                    create_serial_port_threads,
                    send_serial_data,
                    receive_serial_data,
                )
                    .chain(),
            );
    }
}

/// Initializes the serial components.
fn init_serial_components(mut commands: Commands) {
    commands.spawn(Serials::new());
}

/// Spawns the port discovery background task.
fn spawn_port_discovery(channel: Res<SerialNameChannel>, runtime: Res<Runtime>) {
    let tx = channel.tx_world2_serial.clone();
    runtime.spawn(async move {
        info!(
            "Starting port discovery task. Available ports: {:?}",
            available_ports()
        );
        loop {
            let port_names = discover_usb_ports();
            if let Err(e) = tx.send(PortChannelData::PortName(port_names)) {
                error!("Failed to send port names: {e:?}");
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        }
    });
}

/// Discovers available USB serial ports.
fn discover_usb_ports() -> Vec<String> {
    match available_ports() {
        Ok(ports) => ports
            .into_iter()
            .filter_map(|p| match p.port_type {
                SerialPortType::UsbPort(_) => Some(p.port_name),
                _ => None,
            })
            .collect(),
        Err(e) => {
            info!("Error listing ports: {e}");
            Vec::new()
        }
    }
}

/// Updates the serial port names based on discovery results.
fn update_serial_port_names(
    mut channel: ResMut<SerialNameChannel>,
    mut serials: Query<&mut Serials>,
) {
    let Ok(mut serials) = serials.single_mut() else {
        return;
    };

    if let Ok(names) = channel.rx_serial2_world.try_recv() {
        let port_names: Vec<String> = names.into();

        // Remove ports that are no longer available
        serials.serial.retain(|port| {
            port.lock()
                .map(|p| port_names.contains(&p.set.port_name))
                .unwrap_or(false)
        });

        // Add new ports
        for name in &port_names {
            let already_exists = serials.serial.iter().any(|port| {
                port.lock()
                    .map(|p| p.set.port_name == *name)
                    .unwrap_or(false)
            });

            if !already_exists {
                let mut serial = Serial::new();
                serial.set.port_name = name.clone();
                serials.add(serial);
            }
        }
    }
}

/// Creates threads for serial ports that don't have one.
fn create_serial_port_threads(mut serials: Query<&mut Serials>, runtime: Res<Runtime>) {
    let Ok(mut serials) = serials.single_mut() else {
        return;
    };

    for serial in &mut serials.serial {
        let Ok(mut serial) = serial.lock() else {
            continue;
        };
        if serial.thread_handle().is_none() {
            setup_serial_thread(&mut serial, &runtime);
        }
    }
}

/// Sets up the serial port communication thread.
fn setup_serial_thread(serial: &mut Serial, runtime: &Runtime) {
    let (tx, mut rx) = broadcast::channel(100);
    let (tx1, rx1) = broadcast::channel(100);
    let rx_shutdown = tx.subscribe();

    *serial.tx_channel() = Some(tx);
    *serial.rx_channel() = Some(rx1);

    let port_settings = serial.set.clone();
    let port_name = port_settings.port_name.clone();

    let handle = runtime.spawn(async move {
        let port = match wait_for_port_open(&mut rx, &tx1, port_settings).await {
            Ok(p) => p,
            Err(e) => {
                error!("Failed to open port: {e:?}");
                return Err(e);
            }
        };

        info!("Opened serial port: {port_name}");
        if let Err(e) = notify_port_ready(&tx1) {
            return Err(SerialBevyError::channel(e.to_string()));
        }

        let (read, write) = tokio::io::split(port);
        let read_handle = spawn_read_thread(read, tx1.clone(), rx_shutdown, &port_name);

        handle_write_thread(write, rx, tx1, &port_name).await;

        read_handle.abort();
        info!("Serial port thread exited: {port_name}");
        Ok(())
    });

    *serial.thread_handle() = Some(handle);
}

/// Waits for a port open request and opens the port.
async fn wait_for_port_open(
    rx: &mut broadcast::Receiver<PortChannelData>,
    tx1: &broadcast::Sender<PortChannelData>,
    port_settings: PortSettings,
) -> Result<SerialStream, SerialBevyError> {
    loop {
        if matches!(rx.recv().await, Ok(PortChannelData::PortOpen)) {
            return match open_port(&port_settings).await {
                Ok(port) => Ok(port),
                Err(e) => {
                    let _ = tx1.send(PortChannelData::PortError(PorRWData {
                        data: b"open port failed".to_vec(),
                    }));
                    Err(e)
                }
            };
        }
    }
}

/// Notifies that the port is ready.
fn notify_port_ready(
    tx1: &broadcast::Sender<PortChannelData>,
) -> Result<(), broadcast::error::SendError<PortChannelData>> {
    tx1.send(PortChannelData::PortState(PortState::Ready))?;
    Ok(())
}

/// Spawns the read thread for a serial port.
fn spawn_read_thread(
    mut read: tokio::io::ReadHalf<SerialStream>,
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
                        info!("Closing serial port read thread: {name}");
                        break;
                    }
                }
                result = read.read(&mut buffer) => {
                    match result {
                        Ok(n) if n > 0 => {
                            let data = PorRWData {
                                data: buffer[..n].to_vec(),
                            };
                            if let Err(e) = tx1_read.send(PortChannelData::PortRead(data.clone())) {
                                error!("Failed to send read data: {e}");
                            } else {
                                info!("{} read: {:?}", port_name, data.data);
                            }
                        }
                        Ok(_) => {
                            // Zero bytes read, connection closed
                            break;
                        }
                        Err(e) => {
                            error!("Read error on {port_name}: {e}");
                            break;
                        }
                    }
                }
            }
        }
    })
}

/// Handles writing data to the serial port.
async fn handle_write_thread(
    mut write: tokio::io::WriteHalf<SerialStream>,
    mut rx: broadcast::Receiver<PortChannelData>,
    tx1: broadcast::Sender<PortChannelData>,
    port_name: &str,
) {
    loop {
        if let Ok(data) = rx.recv().await {
            match data {
                PortChannelData::PortWrite(data) => {
                    info!("{} write: {:?}", port_name, data.data);
                    if write.write_all(&data.data).await.is_err() {
                        error!("{port_name} write error");
                        break;
                    }
                }
                PortChannelData::PortClose(name) => {
                    info!("Closing serial port write thread: {name}");
                    let _ = tx1.send(PortChannelData::PortState(PortState::Close));
                    break;
                }
                _ => {}
            }
        }
    }
}

/// Sends data to serial ports.
fn send_serial_data(mut serials: Query<&mut Serials>) {
    let Ok(mut serials) = serials.single_mut() else {
        return;
    };

    for serial in &mut serials.serial {
        let Ok(mut serial) = serial.lock() else {
            continue;
        };

        let data = serial.data().get_send_data();
        if data.is_empty() {
            continue;
        }

        let file_data = data.join("\n");
        let mut data_vec_u8: Vec<u8> = vec![];
        for string in data {
            let data_u8 = encode_string(&string, *serial.data().data_type());
            data_vec_u8.extend(data_u8);
        }

        serial
            .data()
            .write_source_file(file_data.as_bytes(), DataSource::Write);

        if serial.is_open()
            && let Some(tx) = serial.tx_channel()
            && let Err(e) = tx.send(PortChannelData::PortWrite(PorRWData { data: data_vec_u8 }))
        {
            error!("Failed to send data: {e}");
        }
    }
}

/// Receives data from serial ports.
fn receive_serial_data(mut serials: Query<&mut Serials>) {
    let Ok(mut serials) = serials.single_mut() else {
        return;
    };

    for serial in &mut serials.serial {
        let Ok(mut serial) = serial.lock() else {
            continue;
        };

        let Some(rx) = serial.rx_channel() else {
            continue;
        };

        if let Ok(data) = rx.try_recv() {
            match data {
                PortChannelData::PortState(state) => match state {
                    PortState::Ready | PortState::Close => {
                        if state == PortState::Ready {
                            serial.open();
                        } else {
                            serial.close();
                            serial.data().clear_utf8_buffer();
                        }
                        serial.data().clear_send_data();
                    }
                    PortState::Error => {
                        serial.error();
                        serial.data().clear_utf8_buffer();
                    }
                },
                PortChannelData::PortRead(data) => {
                    let processed_data = if *serial.data().data_type() == DataType::Utf8 {
                        // Use UTF-8 buffer processing for UTF-8 data
                        serial.data().process_raw_bytes(&data.data)
                    } else {
                        // For other data types, use raw data directly
                        data.data.clone()
                    };
                    
                    let decoded = decode_bytes(&processed_data, *serial.data().data_type());
                    serial
                        .data()
                        .write_source_file(decoded.as_bytes(), DataSource::Read);
                }
                PortChannelData::PortError(data) => {
                    let decoded = decode_bytes(&data.data, *serial.data().data_type());
                    serial.error();
                    serial
                        .data()
                        .write_source_file(decoded.as_bytes(), DataSource::Error);
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serials_new() {
        let serials = Serials::new();
        assert!(serials.is_empty());
    }

    #[test]
    fn test_serials_add() {
        let mut serials = Serials::new();
        serials.add(Serial::new());
        assert_eq!(serials.len(), 1);
    }

    #[test]
    fn test_runtime_creation() {
        let runtime = Runtime::init();
        // Just verify it doesn't panic
        drop(runtime);
    }
}
