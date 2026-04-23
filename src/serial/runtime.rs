//! # Runtime Module
//!
//! Background port discovery, per-port worker lifecycle, and serial I/O polling.

use super::data::PortDiscoveryChannel;
use super::encoding::encode_string;
use super::port::{
    DataSource, PortChannelData, PortRwData, PortState, PortWorker, Serial, SerialStream, open_port,
};
use super::{Runtime, Serials};
use crate::serial_ui::ui::Selected;
use bevy::prelude::*;
use log::{debug, error, info};
use std::sync::mpsc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc as tokio_mpsc;
use tokio_serial::{SerialPortInfo, SerialPortType, available_ports};

#[cfg(target_os = "macos")]
use std::collections::HashSet;

/// Initializes the serial components.
pub fn init_serial_components(mut commands: Commands) {
    commands.spawn(Serials::new());
}

/// Spawns the port discovery background task.
pub fn spawn_port_discovery(channel: Res<PortDiscoveryChannel>, runtime: Res<Runtime>) {
    let tx = channel
        .tx
        .lock()
        .expect("Port discovery channel tx poisoned")
        .clone();

    runtime.spawn(async move {
        let mut previous = Vec::new();
        loop {
            let port_names = discover_serial_ports();
            if port_names != previous {
                previous = port_names.clone();
                if let Err(err) = tx.send(port_names) {
                    error!("Failed to send discovered ports: {err}");
                    break;
                }
            }
            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        }
    });
}

/// Updates the serial port names based on discovery results.
pub fn update_serial_port_names(
    channel: Res<PortDiscoveryChannel>,
    mut serials: Query<&mut Serials>,
    mut selected: ResMut<Selected>,
) {
    let port_names = {
        let receiver = channel
            .rx
            .lock()
            .expect("Port discovery channel rx poisoned");
        match receiver.try_recv() {
            Ok(names) => names,
            Err(mpsc::TryRecvError::Empty | mpsc::TryRecvError::Disconnected) => return,
        }
    };

    let Ok(mut serials) = serials.single_mut() else {
        return;
    };

    serials.serial.retain(|port| {
        port.lock()
            .map(|serial| port_names.contains(&serial.set.port_name))
            .unwrap_or(false)
    });

    for name in &port_names {
        let already_exists = serials.serial.iter().any(|port| {
            port.lock()
                .map(|serial| serial.set.port_name == *name)
                .unwrap_or(false)
        });

        if !already_exists {
            let mut serial = Serial::new();
            serial.set.port_name = name.clone();
            serials.add(serial);
        }
    }

    let selected_exists = serials.serial.iter().any(|port| {
        port.lock()
            .map(|serial| serial.set.port_name == selected.selected())
            .unwrap_or(false)
    });

    if !selected_exists {
        if let Some(first_serial) = serials.serial.first()
            && let Ok(serial) = first_serial.lock()
        {
            selected.select(&serial.set.port_name);
            return;
        }
        selected.clear();
    }
}

/// Ensures every discovered serial port has a background worker.
pub fn ensure_serial_port_workers(mut serials: Query<&mut Serials>, runtime: Res<Runtime>) {
    let Ok(mut serials) = serials.single_mut() else {
        return;
    };

    for serial in &mut serials.serial {
        let Ok(mut serial) = serial.lock() else {
            continue;
        };

        let needs_worker = serial.worker().map(PortWorker::is_finished).unwrap_or(true);
        if needs_worker {
            let port_name = serial.set.port_name.clone();
            serial.set_worker(spawn_port_worker(&runtime, port_name));
        }
    }
}

/// Sends data to serial ports.
pub fn send_serial_data(mut serials: Query<&mut Serials>) {
    let Ok(mut serials) = serials.single_mut() else {
        return;
    };

    for serial in &mut serials.serial {
        let Ok(mut serial) = serial.lock() else {
            continue;
        };

        let pending_messages = serial.data().get_send_data();
        if pending_messages.is_empty() {
            continue;
        }

        if !serial.is_open() {
            serial.data().write_source_file(
                b"Port is not open; discarded queued send request",
                DataSource::Error,
            );
            continue;
        }

        let data_type = *serial.data().data_type();
        let should_echo = !serial.data().is_console_mode();
        let command_tx = serial.tx_channel().cloned();

        let mut payload = Vec::new();
        for message in &pending_messages {
            payload.extend(encode_string(message, data_type));
        }

        let sent_text = pending_messages.concat();
        match command_tx {
            Some(tx) => {
                if let Err(err) = tx.send(PortChannelData::PortWrite(PortRwData { data: payload }))
                {
                    serial.error();
                    serial
                        .data()
                        .set_last_error(format!("Failed to queue serial write: {err}"));
                    serial.data().write_source_file(
                        format!("Failed to queue serial write: {err}").as_bytes(),
                        DataSource::Error,
                    );
                    continue;
                }

                if should_echo {
                    serial
                        .data()
                        .write_source_file(sent_text.as_bytes(), DataSource::Write);
                }
            }
            None => {
                serial.error();
                serial
                    .data()
                    .set_last_error("Serial worker is unavailable".to_string());
                serial
                    .data()
                    .write_source_file(b"Serial worker is unavailable", DataSource::Error);
            }
        }
    }
}

/// Polls per-port worker events and reflects them into serial state.
pub fn poll_serial_port_events(mut serials: Query<&mut Serials>) {
    let Ok(mut serials) = serials.single_mut() else {
        return;
    };

    for serial in &mut serials.serial {
        let Ok(mut serial) = serial.lock() else {
            continue;
        };

        loop {
            let event = {
                let Some(rx) = serial.rx_channel() else {
                    break;
                };
                rx.try_recv().ok()
            };

            let Some(event) = event else {
                break;
            };

            match event {
                PortChannelData::PortState(state) => match state {
                    PortState::Ready => {
                        serial.open();
                        serial.data().clear_send_data();
                    }
                    PortState::Close => {
                        serial.close();
                        serial.data().clear_utf8_buffer();
                        serial.data().clear_send_data();
                    }
                    PortState::Error => {
                        serial.error();
                        serial.data().clear_utf8_buffer();
                    }
                },
                PortChannelData::PortRead(data) => {
                    let rendered = serial.data().decode_read_data(&data.data);
                    serial
                        .data()
                        .write_source_file(rendered.as_bytes(), DataSource::Read);
                }
                PortChannelData::PortError(data) => {
                    let message = String::from_utf8_lossy(&data.data).into_owned();
                    serial.error();
                    serial.data().set_last_error(message.clone());
                    serial
                        .data()
                        .write_source_file(message.as_bytes(), DataSource::Error);
                }
                _ => {}
            }
        }
    }
}

/// Discovers available serial ports and keeps the list stable between refreshes.
fn discover_serial_ports() -> Vec<String> {
    match available_ports() {
        Ok(ports) => {
            let mut port_names = normalize_discovered_ports(ports);
            port_names.sort();
            port_names.dedup();
            port_names
        }
        Err(err) => {
            debug!("Error listing ports: {err}");
            Vec::new()
        }
    }
}

fn normalize_discovered_ports(ports: Vec<SerialPortInfo>) -> Vec<String> {
    let mut port_names: Vec<String> = ports
        .into_iter()
        .filter(is_visible_serial_port)
        .map(|port| port.port_name)
        .collect();

    #[cfg(target_os = "macos")]
    prefer_callout_ports(&mut port_names);

    port_names
}

fn is_visible_serial_port(port: &SerialPortInfo) -> bool {
    !matches!(port.port_type, SerialPortType::BluetoothPort)
        && !is_hidden_port_name(&port.port_name)
}

#[cfg(target_os = "macos")]
fn is_hidden_port_name(port_name: &str) -> bool {
    port_name.to_ascii_lowercase().contains("bluetooth")
}

#[cfg(not(target_os = "macos"))]
fn is_hidden_port_name(_port_name: &str) -> bool {
    false
}

#[cfg(target_os = "macos")]
fn prefer_callout_ports(port_names: &mut Vec<String>) {
    let existing: HashSet<String> = port_names.iter().cloned().collect();
    port_names.retain(|name| {
        name.strip_prefix("/dev/tty.")
            .map(|suffix| !existing.contains(&format!("/dev/cu.{suffix}")))
            .unwrap_or(true)
    });
}

fn spawn_port_worker(runtime: &Runtime, port_name: String) -> PortWorker {
    let (command_tx, command_rx) = tokio_mpsc::unbounded_channel();
    let (event_tx, event_rx) = mpsc::channel();
    let task = runtime.spawn(port_worker_loop(port_name, command_rx, event_tx));
    PortWorker::new(command_tx, event_rx, task)
}

async fn port_worker_loop(
    port_name: String,
    mut command_rx: tokio_mpsc::UnboundedReceiver<PortChannelData>,
    event_tx: mpsc::Sender<PortChannelData>,
) {
    let mut stream: Option<SerialStream> = None;
    let mut buffer = [0u8; 1024];

    loop {
        if let Some(active_stream) = stream.as_mut() {
            let transition = tokio::select! {
                maybe_command = command_rx.recv() => {
                    match maybe_command {
                        Some(command) => handle_command_while_open(command, active_stream, &event_tx, &port_name).await,
                        None => WorkerTransition::Stop,
                    }
                }
                read_result = active_stream.read(&mut buffer) => {
                    handle_read_result(read_result, &buffer, &event_tx, &port_name)
                }
            };

            match transition {
                WorkerTransition::KeepCurrent => {}
                WorkerTransition::Replace(next_stream) => {
                    info!("Reopened serial port worker: {port_name}");
                    stream = Some(next_stream);
                }
                WorkerTransition::CloseCurrent => {
                    info!("Closed serial port worker: {port_name}");
                    stream = None;
                }
                WorkerTransition::Stop => break,
            }
        } else {
            match command_rx.recv().await {
                Some(command) => {
                    stream = handle_command_while_closed(command, &event_tx, &port_name).await;
                }
                None => break,
            }
        }
    }
}

enum WorkerTransition {
    KeepCurrent,
    Replace(SerialStream),
    CloseCurrent,
    Stop,
}

async fn handle_command_while_closed(
    command: PortChannelData,
    event_tx: &mpsc::Sender<PortChannelData>,
    port_name: &str,
) -> Option<SerialStream> {
    match command {
        PortChannelData::PortOpen(settings) => match open_port(&settings).await {
            Ok(stream) => {
                info!("Opened serial port: {port_name}");
                let _ = event_tx.send(PortChannelData::PortState(PortState::Ready));
                Some(stream)
            }
            Err(err) => {
                send_worker_error(event_tx, err.to_string());
                let _ = event_tx.send(PortChannelData::PortState(PortState::Error));
                None
            }
        },
        PortChannelData::PortWrite(_) => {
            send_worker_error(event_tx, "Port is not open".to_string());
            None
        }
        PortChannelData::PortClose(_) => {
            let _ = event_tx.send(PortChannelData::PortState(PortState::Close));
            None
        }
        _ => None,
    }
}

async fn handle_command_while_open(
    command: PortChannelData,
    stream: &mut SerialStream,
    event_tx: &mpsc::Sender<PortChannelData>,
    port_name: &str,
) -> WorkerTransition {
    match command {
        PortChannelData::PortOpen(settings) => match open_port(&settings).await {
            Ok(next_stream) => {
                let _ = event_tx.send(PortChannelData::PortState(PortState::Ready));
                WorkerTransition::Replace(next_stream)
            }
            Err(err) => {
                send_worker_error(event_tx, err.to_string());
                let _ = event_tx.send(PortChannelData::PortState(PortState::Error));
                WorkerTransition::CloseCurrent
            }
        },
        PortChannelData::PortWrite(data) => {
            debug!("{port_name} write: {:?}", data.data);
            if let Err(err) = stream.write_all(&data.data).await {
                send_worker_error(event_tx, format!("Write error on {port_name}: {err}"));
                let _ = event_tx.send(PortChannelData::PortState(PortState::Error));
                WorkerTransition::CloseCurrent
            } else {
                WorkerTransition::KeepCurrent
            }
        }
        PortChannelData::PortClose(name) => {
            debug!("Closing serial port worker: {name}");
            let _ = event_tx.send(PortChannelData::PortState(PortState::Close));
            WorkerTransition::CloseCurrent
        }
        _ => WorkerTransition::KeepCurrent,
    }
}

fn handle_read_result(
    read_result: std::io::Result<usize>,
    buffer: &[u8; 1024],
    event_tx: &mpsc::Sender<PortChannelData>,
    port_name: &str,
) -> WorkerTransition {
    match read_result {
        Ok(0) => {
            info!("Serial port closed by peer: {port_name}");
            let _ = event_tx.send(PortChannelData::PortState(PortState::Close));
            WorkerTransition::CloseCurrent
        }
        Ok(count) => {
            let _ = event_tx.send(PortChannelData::PortRead(PortRwData {
                data: buffer[..count].to_vec(),
            }));
            WorkerTransition::KeepCurrent
        }
        Err(err) => {
            send_worker_error(event_tx, format!("Read error on {port_name}: {err}"));
            let _ = event_tx.send(PortChannelData::PortState(PortState::Error));
            WorkerTransition::CloseCurrent
        }
    }
}

fn send_worker_error(event_tx: &mpsc::Sender<PortChannelData>, message: String) {
    let _ = event_tx.send(PortChannelData::PortError(PortRwData {
        data: message.into_bytes(),
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn port_info(port_name: &str, port_type: SerialPortType) -> SerialPortInfo {
        SerialPortInfo {
            port_name: port_name.to_string(),
            port_type,
        }
    }

    #[test]
    fn filters_bluetooth_ports_from_discovery() {
        let ports = normalize_discovered_ports(vec![
            port_info(
                "/dev/cu.Bluetooth-Incoming-Port",
                SerialPortType::BluetoothPort,
            ),
            port_info("/dev/cu.usbserial-01", SerialPortType::Unknown),
            port_info("/dev/ttyUSB0", SerialPortType::PciPort),
        ]);

        assert_eq!(ports, vec!["/dev/cu.usbserial-01", "/dev/ttyUSB0"]);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn prefers_callout_ports_when_matching_tty_port_exists() {
        let mut ports = vec![
            "/dev/tty.usbmodem1101".to_string(),
            "/dev/cu.usbmodem1101".to_string(),
            "/dev/cu.usbserial-01".to_string(),
        ];

        prefer_callout_ports(&mut ports);
        ports.sort();

        assert_eq!(
            ports,
            vec![
                "/dev/cu.usbmodem1101".to_string(),
                "/dev/cu.usbserial-01".to_string(),
            ]
        );
    }
}
