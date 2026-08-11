//! # IO Module
//!
//! Serial port I/O operations including thread lifecycle management,
//! read/write handling, and data transfer between Bevy ECS and async serial threads.

use bevy::prelude::*;
use log::{debug, error, info};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::{TryRecvError, TrySendError};

use super::Serials;
use super::data_types::DataType;
use super::discovery::Runtime;
use super::encoding::encode_string;
use super::port::Serial;
use super::port::open_port;
use super::state::{DataSource, PortChannelData, PortRwData, PortState};
use crate::error::SerialBevyError;

// SerialStream comes from tokio_serial, re-exported via super::port
use tokio_serial::SerialStream;

const SERIAL_CHANNEL_CAPACITY: usize = 1024;

/// Creates threads for serial ports that don't have one.
///
/// This system runs every frame and checks if any managed serial port
/// is missing its async communication thread, spawning one if needed.
pub fn create_serial_port_threads(mut serials: Query<&mut Serials>, runtime: Res<Runtime>) {
    let Ok(mut serials) = serials.single_mut() else {
        return;
    };

    for serial in &mut serials.serial {
        let Ok(mut serial) = serial.lock() else {
            continue;
        };
        if serial.is_error() {
            continue;
        }
        if serial.thread_handle().is_none() {
            setup_serial_thread(&mut serial, &runtime);
        }
    }
}

/// Sets up the serial port communication thread.
///
/// Creates bounded point-to-point channels between ECS and one async worker.
/// The worker:
/// 1. Waits for a port open command
/// 2. Splits the serial stream into read/write halves
/// 3. Owns both read and write halves in one cancellable session
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

/// Waits for a port open request on the command channel and opens the serial port
/// with the provided settings.
///
/// Returns an open `SerialStream` once the user triggers a port open command.
async fn wait_for_port_open(
    rx: &mut mpsc::Receiver<PortChannelData>,
    tx1: &mpsc::Sender<PortChannelData>,
) -> Result<SerialStream, SerialBevyError> {
    loop {
        match rx.recv().await {
            Some(PortChannelData::PortOpen(settings)) => {
                return match open_port(&settings).await {
                    Ok(port) => Ok(port),
                    Err(e) => {
                        let _ = tx1
                            .send(PortChannelData::PortError(PortRwData {
                                data: format!("open port failed: {e}").into_bytes(),
                            }))
                            .await;
                        Err(e)
                    }
                };
            }
            Some(_) => {}
            None => {
                return Err(SerialBevyError::channel("port command channel closed"));
            }
        }
    }
}

/// Runs one complete session so aborting the owner drops both stream halves.
async fn run_serial_session<R, W>(
    mut read: R,
    mut write: W,
    mut rx: mpsc::Receiver<PortChannelData>,
    tx: mpsc::Sender<PortChannelData>,
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
                            send_task_error(&tx, format!("{port_name} write error: {e}")).await;
                            break;
                        }
                    }
                    Some(PortChannelData::PortClose(name)) => {
                        debug!("Closing serial port session: {name}");
                        let _ = tx.send(PortChannelData::PortState(PortState::Close)).await;
                        break;
                    }
                    Some(_) => {}
                    None => break,
                }
            }
            result = read.read(&mut buffer) => {
                match result {
                    Ok(n) if n > 0 => {
                        let data = PortRwData {
                            data: buffer[..n].to_vec(),
                        };
                        if let Err(e) = tx.send(PortChannelData::PortRead(data.clone())).await {
                            error!("Failed to send read data: {e}");
                        } else {
                            debug!("{} read: {:?}", port_name, data.data);
                        }
                    }
                    Ok(_) => {
                        send_task_error(&tx, format!("{port_name} read returned zero bytes")).await;
                        break;
                    }
                    Err(e) => {
                        error!("Read error on {port_name}: {e}");
                        send_task_error(&tx, format!("{port_name} read error: {e}")).await;
                        break;
                    }
                }
            }
        }
    }
}

async fn send_task_error(tx: &mpsc::Sender<PortChannelData>, message: String) {
    let _ = tx
        .send(PortChannelData::PortError(PortRwData {
            data: message.into_bytes(),
        }))
        .await;
}

/// Sends data queued on each serial port's send buffer to the port's async thread.
///
/// Encodes queued string data according to the port's configured `DataType`,
/// then dispatches it via the broadcast channel to the serial port write thread.
/// In non-console mode, the sent data is also written to the log file with a
/// "Write" source indicator.
pub fn send_serial_data(mut serials: Query<&mut Serials>) {
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

        // Write sent data to log file
        // In console mode: skip local echo (terminal will echo back)
        // In normal mode: write with Write source indicator
        if !serial.data().is_console_mode() {
            serial
                .data()
                .write_source_file(file_data.as_bytes(), DataSource::Write);
        }

        if serial.is_open()
            && let Some(tx) = serial.tx_channel()
            && let Err(e) =
                tx.try_send(PortChannelData::PortWrite(PortRwData { data: data_vec_u8 }))
        {
            match e {
                TrySendError::Full(_) => error!("Serial send queue is full"),
                TrySendError::Closed(_) => error!("Serial send queue is closed"),
            }
        }
    }
}

/// Receives data from serial ports and routes it to the port data manager.
///
/// Polls each serial port's receive channel for state changes, incoming data,
/// and error messages. Updates the port state and writes received/error data
/// to the source file with appropriate source indicators.
pub fn receive_serial_data(mut serials: Query<&mut Serials>) {
    const MAX_MESSAGES_PER_PORT_PER_FRAME: usize = 256;
    let Ok(mut serials) = serials.single_mut() else {
        return;
    };

    for serial in &mut serials.serial {
        let Ok(mut serial) = serial.lock() else {
            continue;
        };

        for _ in 0..MAX_MESSAGES_PER_PORT_PER_FRAME {
            let data = {
                let Some(rx) = serial.rx_channel() else {
                    break;
                };
                match rx.try_recv() {
                    Ok(data) => data,
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        serial.error();
                        break;
                    }
                }
            };

            match data {
                PortChannelData::PortState(state) => match state {
                    PortState::Ready | PortState::Close => {
                        if state == PortState::Ready {
                            serial.open();
                        } else {
                            serial.close();
                        }
                        serial.data().clear_send_data();
                    }
                    PortState::Error => {
                        serial.error();
                    }
                },
                PortChannelData::PortRead(data) => {
                    let processed_data = if *serial.data().data_type() == DataType::Utf8 {
                        serial.data().process_raw_bytes(&data.data)
                    } else {
                        data.data.clone()
                    };

                    serial
                        .data()
                        .write_source_file(&processed_data, DataSource::Read);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn receive_port_error_marks_serial_error_and_clears_pending_data() {
        let (tx, rx) = mpsc::channel(4);
        let mut serial = Serial::new();
        serial.data().send_data("pending".to_string());
        *serial.rx_channel() = Some(rx);

        tx.try_send(PortChannelData::PortError(PortRwData {
            data: b"read error".to_vec(),
        }))
        .expect("send port error");

        let mut app = App::new();
        let mut serials = Serials::new();
        serials.add(serial);
        app.world_mut().spawn(serials);
        app.add_systems(Update, receive_serial_data);
        app.update();

        let mut query = app.world_mut().query::<&Serials>();
        let serials = query.single(app.world()).expect("serials entity");
        let mut serial = serials.get(0).lock().expect("serial lock");

        assert!(serial.is_error());
        assert!(serial.data().get_send_data().is_empty());
    }
}
