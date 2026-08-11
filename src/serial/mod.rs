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
use log::{debug, error, info, warn};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::sync::broadcast::error::TryRecvError as BroadcastTryRecvError;
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::{TryRecvError as MpscTryRecvError, TrySendError};
use tokio_serial::available_ports;

use crate::error::SerialBevyError;

const SERIAL_CHANNEL_CAPACITY: usize = 1024;

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
/// Serial state is owned by Bevy's world; background tasks communicate only
/// through channels, so no per-port mutex is required.
#[derive(Component)]
pub struct Serials {
    /// Serial port instances owned by this ECS component.
    pub serial: Vec<Serial>,
}

impl std::fmt::Debug for Serials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut debug = f.debug_list();
        for serial in &self.serial {
            debug.entry(&format!(
                "{}: {}bps",
                serial.set.port_name, serial.set.baud_rate
            ));
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
        self.serial.push(serial);
    }

    /// Removes a serial port at the specified index.
    ///
    /// # Panics
    ///
    /// Panics if the index is out of bounds.
    pub fn remove(&mut self, index: usize) {
        self.serial.remove(index);
    }

    /// Gets a serial port at the specified index.
    ///
    /// # Panics
    ///
    /// Panics if the index is out of bounds.
    #[must_use]
    pub fn get(&self, index: usize) -> &Serial {
        &self.serial[index]
    }

    /// Gets a mutable serial port at the specified index.
    ///
    /// # Panics
    ///
    /// Panics if the index is out of bounds.
    #[must_use]
    pub fn get_mut(&mut self, index: usize) -> &mut Serial {
        &mut self.serial[index]
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
    let tx = channel.tx_discovery.clone();
    runtime.spawn(async move {
        info!("Starting serial port discovery task");
        let mut previous_names = Vec::new();
        loop {
            if let Some(port_names) = discover_serial_ports()
                && port_names != previous_names
            {
                previous_names.clone_from(&port_names);
                if let Err(e) = tx.send(PortChannelData::PortName(port_names)) {
                    error!("Failed to send port names: {e:?}");
                    break;
                }
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        }
    });
}

/// Discovers all serial ports supported by the host platform.
///
/// A discovery error is represented separately from an empty successful scan,
/// so a transient OS error does not disconnect every active session.
fn discover_serial_ports() -> Option<Vec<String>> {
    match available_ports() {
        Ok(ports) => {
            let mut names: Vec<_> = ports.into_iter().map(|port| port.port_name).collect();
            names.sort_unstable();
            names.dedup();
            Some(names)
        }
        Err(e) => {
            warn!("Failed to list serial ports: {e}");
            None
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

    let mut latest_names = None;
    loop {
        match channel.rx_discovery.try_recv() {
            Ok(PortChannelData::PortName(names)) => latest_names = Some(names),
            Ok(_) | Err(BroadcastTryRecvError::Lagged(_)) => {}
            Err(BroadcastTryRecvError::Empty | BroadcastTryRecvError::Closed) => break,
        }
    }

    if let Some(port_names) = latest_names {
        // Remove ports that are no longer available
        serials
            .serial
            .retain(|port| port_names.contains(&port.set.port_name));

        // Add new ports
        for name in &port_names {
            let already_exists = serials
                .serial
                .iter()
                .any(|port| port.set.port_name == *name);

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
        if serial.thread_handle().is_none() {
            setup_serial_thread(serial, &runtime);
        }
    }
}

/// Sets up the serial port communication thread.
fn setup_serial_thread(serial: &mut Serial, runtime: &Runtime) {
    let (tx, mut rx) = mpsc::channel(SERIAL_CHANNEL_CAPACITY);
    let (tx1, rx1) = mpsc::channel(SERIAL_CHANNEL_CAPACITY);

    *serial.tx_channel() = Some(tx);
    *serial.rx_channel() = Some(rx1);

    let port_name = serial.set.port_name.clone();

    let handle = runtime.spawn(async move {
        let port = match wait_for_port_open(&mut rx, &tx1).await {
            Ok(p) => p,
            Err(e) => {
                error!("Failed to open port: {e:?}");
                return Err(e);
            }
        };

        info!("Opened serial port: {port_name}");
        if let Err(e) = tx1.send(PortChannelData::PortState(PortState::Ready)).await {
            return Err(SerialBevyError::channel(e.to_string()));
        }

        let (read, write) = tokio::io::split(port);
        run_serial_session(read, write, rx, tx1, &port_name).await;

        info!("Serial port thread exited: {port_name}");
        Ok(())
    });

    *serial.thread_handle() = Some(handle);
}

/// Waits for a port open request and opens the port.
async fn wait_for_port_open(
    rx: &mut mpsc::Receiver<PortChannelData>,
    tx1: &mpsc::Sender<PortChannelData>,
) -> Result<SerialStream, SerialBevyError> {
    loop {
        match rx.recv().await {
            Some(PortChannelData::PortOpen(port_settings)) => {
                return match open_port(&port_settings).await {
                    Ok(port) => Ok(port),
                    Err(e) => {
                        let _ = tx1
                            .send(PortChannelData::PortError(PortIoData {
                                data: e.to_string().into_bytes(),
                            }))
                            .await;
                        Err(e)
                    }
                };
            }
            Some(_) => {}
            None => {
                return Err(SerialBevyError::channel(
                    "serial command channel closed before the port was opened",
                ));
            }
        }
    }
}

/// Runs a complete open-port session in one cancellable task.
///
/// Keeping both halves in this future ensures aborting the owning task drops
/// the complete serial handle; no detached read task can outlive the session.
async fn run_serial_session<R, W>(
    mut read: R,
    mut write: W,
    mut rx: mpsc::Receiver<PortChannelData>,
    tx1: mpsc::Sender<PortChannelData>,
    port_name: &str,
) where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let mut buffer = [0u8; 1024];
    loop {
        tokio::select! {
            command = rx.recv() => {
                match command {
                    Some(PortChannelData::PortWrite(data)) => {
                        debug!("{} write: {:?}", port_name, data.data);
                        if let Err(e) = write.write_all(&data.data).await {
                            let message = format!("write failed: {e}");
                            error!("{port_name} {message}");
                            send_task_error(&tx1, message).await;
                            break;
                        }
                    }
                    Some(_) => {}
                    None => break,
                }
            }
            result = read.read(&mut buffer) => {
                match result {
                    Ok(n) if n > 0 => {
                        let data = PortIoData {
                            data: buffer[..n].to_vec(),
                        };
                        debug!("{} read: {:?}", port_name, data.data);
                        if let Err(e) = tx1.send(PortChannelData::PortRead(data)).await {
                            error!("Failed to send read data: {e}");
                            break;
                        }
                    }
                    Ok(_) => {
                        send_task_error(&tx1, "serial port closed while reading").await;
                        break;
                    }
                    Err(e) => {
                        let message = format!("read failed: {e}");
                        error!("{port_name} {message}");
                        send_task_error(&tx1, message).await;
                        break;
                    }
                }
            }
        }
    }
}

async fn send_task_error(tx: &mpsc::Sender<PortChannelData>, message: impl Into<String>) {
    let _ = tx
        .send(PortChannelData::PortError(PortIoData {
            data: message.into().into_bytes(),
        }))
        .await;
}

/// Sends data to serial ports.
fn send_serial_data(mut serials: Query<&mut Serials>) {
    let Ok(mut serials) = serials.single_mut() else {
        return;
    };

    for serial in &mut serials.serial {
        if !serial.is_open() {
            continue;
        }

        let data = serial.data().get_send_data();
        if data.is_empty() {
            continue;
        }

        let file_data = data.join("\n");
        let data_type = *serial.data().data_type();
        let append_line_feed = *serial.data().line_feed();
        let encoded = match encode_pending_data(&data, data_type, append_line_feed) {
            Ok(encoded) => encoded,
            Err(e) => {
                let message = e.to_string();
                error!("{message}");
                serial.report_error(message.clone());
                write_serial_log(serial, message.as_bytes(), DataSource::Error);
                continue;
            }
        };
        let Some(tx) = serial.tx_channel().as_ref().cloned() else {
            let message = format!(
                "Serial port {} has no command channel",
                serial.set.port_name
            );
            error!("{message}");
            serial.error(message.clone());
            write_serial_log(serial, message.as_bytes(), DataSource::Error);
            continue;
        };

        match tx.try_send(PortChannelData::PortWrite(PortIoData { data: encoded })) {
            Ok(_) => {
                serial.clear_error();
                write_serial_log(serial, file_data.as_bytes(), DataSource::Write);
            }
            Err(TrySendError::Full(_)) => {
                serial.data().restore_send_data(data);
                serial.report_error("Serial send queue is busy; retrying");
            }
            Err(TrySendError::Closed(_)) => {
                let e = "serial command channel is closed";
                let message = format!("Failed to queue serial data: {e}");
                error!("{message}");
                serial.error(message.clone());
                write_serial_log(serial, message.as_bytes(), DataSource::Error);
            }
        }
    }
}

fn encode_pending_data(
    data: &[String],
    data_type: DataType,
    append_line_feed: bool,
) -> crate::error::Result<Vec<u8>> {
    let extra_capacity = usize::from(append_line_feed) * data.len();
    let mut encoded =
        Vec::with_capacity(data.iter().map(String::len).sum::<usize>() + extra_capacity);

    for command in data {
        encoded.extend(encode_string(command, data_type)?);
        if append_line_feed {
            encoded.push(b'\n');
        }
    }

    Ok(encoded)
}

fn write_serial_log(serial: &mut Serial, data: &[u8], source: DataSource) {
    if let Err(e) = serial.data().write_source_file(data, source) {
        let log_error = format!("Failed to write the session log: {e}");
        let message = serial.last_error().map_or_else(
            || log_error.clone(),
            |previous| format!("{previous}\n{log_error}"),
        );
        error!("{message}");
        serial.report_error(message);
    }
}

fn write_log_separator(serial: &mut Serial) {
    if let Err(e) = serial.data().write_log_separator() {
        let message = format!("Failed to write the session log: {e}");
        error!("{message}");
        serial.report_error(message);
    }
}

fn decode_task_error(data: &[u8]) -> String {
    String::from_utf8_lossy(data).into_owned()
}

/// Receives data from serial ports.
fn receive_serial_data(mut serials: Query<&mut Serials>) {
    const MAX_MESSAGES_PER_PORT_PER_FRAME: usize = 256;

    let Ok(mut serials) = serials.single_mut() else {
        return;
    };

    for serial in &mut serials.serial {
        for _ in 0..MAX_MESSAGES_PER_PORT_PER_FRAME {
            let data = match serial.rx_channel().as_mut().map(|rx| rx.try_recv()) {
                Some(Ok(data)) => data,
                Some(Err(MpscTryRecvError::Empty | MpscTryRecvError::Disconnected)) | None => break,
            };

            match data {
                PortChannelData::PortState(state) => match state {
                    PortState::Ready => {
                        serial.open();
                        serial.data().reset_receive_time();
                        serial.data().clear_send_data();
                    }
                    PortState::Close => serial.close(),
                    PortState::Opening => {}
                    PortState::Error => {
                        serial.error("The serial task entered an error state");
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

                    // Check if timeout line break is needed
                    let should_break = serial.data().update_receive_time();
                    if should_break {
                        write_log_separator(serial);
                    }

                    let decoded = decode_bytes(&processed_data, *serial.data().data_type());
                    if !decoded.is_empty() {
                        write_serial_log(serial, decoded.as_bytes(), DataSource::Read);
                    }
                }
                PortChannelData::PortError(data) => {
                    // Internal task errors are always UTF-8 text, independent of
                    // the payload display mode selected for serial data.
                    let decoded = decode_task_error(&data.data);
                    serial.error(decoded.clone());
                    write_serial_log(serial, decoded.as_bytes(), DataSource::Error);
                    break;
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

    #[test]
    fn wait_for_port_open_stops_when_command_channel_closes() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let (command_tx, mut command_rx) = mpsc::channel(1);
        let (event_tx, _event_rx) = mpsc::channel(1);
        drop(command_tx);

        let result = runtime.block_on(wait_for_port_open(&mut command_rx, &event_tx));

        assert!(matches!(result, Err(SerialBevyError::Channel(_))));
    }

    #[test]
    fn wait_for_port_open_uses_settings_from_open_request() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let (command_tx, mut command_rx) = mpsc::channel(1);
        let (event_tx, _event_rx) = mpsc::channel(1);
        let settings = PortSettings {
            port_name: "serial-bevy-port-that-does-not-exist".to_string(),
            baud_rate: 230_400,
            ..PortSettings::default()
        };

        command_tx
            .try_send(PortChannelData::PortOpen(settings.clone()))
            .unwrap();
        let result = runtime.block_on(wait_for_port_open(&mut command_rx, &event_tx));

        assert!(matches!(
            result,
            Err(SerialBevyError::PortOpen { port_name, .. }) if port_name == settings.port_name
        ));
    }

    #[test]
    fn serial_session_reads_writes_and_closes() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            let (port, mut peer) = tokio::io::duplex(64);
            let (read, write) = tokio::io::split(port);
            let (command_tx, command_rx) = mpsc::channel(4);
            let (event_tx, mut event_rx) = mpsc::channel(4);
            let session = tokio::spawn(run_serial_session(
                read,
                write,
                command_rx,
                event_tx,
                "test-port",
            ));

            command_tx
                .send(PortChannelData::PortWrite(PortIoData {
                    data: b"ping".to_vec(),
                }))
                .await
                .unwrap();
            let mut request = [0; 4];
            peer.read_exact(&mut request).await.unwrap();
            assert_eq!(&request, b"ping");

            peer.write_all(b"pong").await.unwrap();
            assert!(matches!(
                event_rx.recv().await,
                Some(PortChannelData::PortRead(PortIoData { data })) if data == b"pong"
            ));

            drop(command_tx);
            tokio::time::timeout(std::time::Duration::from_secs(1), session)
                .await
                .expect("serial session did not stop after its command channel closed")
                .unwrap();

            let mut byte = [0];
            assert_eq!(peer.read(&mut byte).await.unwrap(), 0);
        });
    }

    #[test]
    fn test_encode_pending_hex_data_with_line_feed() {
        let data = vec!["41 42".to_string()];

        assert_eq!(
            encode_pending_data(&data, DataType::Hex, true).unwrap(),
            vec![0x41, 0x42, b'\n']
        );
    }

    #[test]
    fn test_encode_pending_data_terminates_each_command() {
        let data = vec!["first".to_string(), "second".to_string()];

        assert_eq!(
            encode_pending_data(&data, DataType::Utf8, true).unwrap(),
            b"first\nsecond\n"
        );
        assert_eq!(
            encode_pending_data(&data, DataType::Utf8, false).unwrap(),
            b"firstsecond"
        );
    }

    #[test]
    fn test_encode_pending_data_rejects_invalid_hex() {
        let data = vec!["not-hex".to_string()];

        let error = encode_pending_data(&data, DataType::Hex, false).unwrap_err();

        assert!(error.to_string().contains("not"));
    }

    #[test]
    fn task_errors_are_decoded_as_text() {
        assert_eq!(
            decode_task_error(b"write failed: disconnected"),
            "write failed: disconnected"
        );
    }
}
