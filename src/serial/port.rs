//! # Port Module
//!
//! This module provides serial port types, settings, and state management.

use log::{error, info};
use std::collections::VecDeque;
use std::fmt;
use std::fs::{File, OpenOptions};
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_serial::SerialPortBuilderExt;

pub use tokio_serial::{DataBits, FlowControl, Parity, SerialPort, SerialStream, StopBits};

use crate::error::SerialBevyError;

/// Maximum amount of log text retained in memory for the live UI.
///
/// The complete session remains on disk; this bound prevents long-running
/// sessions from making rendering progressively more expensive.
const RECENT_LOG_MAX_BYTES: usize = 1024 * 1024;
const COMMAND_HISTORY_MAX_ENTRIES: usize = 500;
const LOG_FILE_NAME_MAX_BYTES: usize = 200;
const PENDING_SEND_MAX_COMMANDS: usize = 1024;
const PENDING_SEND_MAX_BYTES: usize = 4 * 1024 * 1024;

/// Maximum number of characters accepted by the command editor.
pub const MAX_COMMAND_INPUT_CHARS: usize = 1024 * 1024;

fn sanitize_log_file_name(name: &str) -> String {
    let mut sanitized: String = name
        .trim_start_matches(['/', '\\'])
        .chars()
        .map(|character| {
            if character.is_control()
                || matches!(
                    character,
                    '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*'
                )
            {
                '_'
            } else {
                character
            }
        })
        .collect();

    if sanitized.len() > LOG_FILE_NAME_MAX_BYTES {
        let mut end = LOG_FILE_NAME_MAX_BYTES;
        while !sanitized.is_char_boundary(end) {
            end -= 1;
        }
        sanitized.truncate(end);
    }

    sanitized = sanitized.trim_matches([' ', '.']).to_string();
    if sanitized.is_empty() {
        return "serial.log".to_string();
    }

    let stem = sanitized
        .split_once('.')
        .map_or(sanitized.as_str(), |(stem, _)| stem)
        .to_ascii_uppercase();
    let is_reserved = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || stem
            .strip_prefix("COM")
            .or_else(|| stem.strip_prefix("LPT"))
            .is_some_and(|number| {
                matches!(number, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9")
            });
    if is_reserved {
        sanitized.insert_str(0, "serial_");
    }

    sanitized
}

fn log_directory_candidates() -> Vec<PathBuf> {
    let mut candidates = vec![PathBuf::from("logs")];
    if let Ok(executable) = std::env::current_exe()
        && let Some(parent) = executable.parent()
    {
        candidates.push(parent.join("logs"));
    }
    candidates.push(std::env::temp_dir().join("serial_bevy").join("logs"));
    candidates
}

fn open_session_log(
    file_name: &str,
    directories: impl IntoIterator<Item = PathBuf>,
) -> io::Result<(PathBuf, File)> {
    let mut failures = Vec::new();
    for directory in directories {
        if let Err(error) = std::fs::create_dir_all(&directory) {
            failures.push(format!("{}: {error}", directory.display()));
            continue;
        }

        let path = directory.join(file_name);
        match OpenOptions::new().create(true).append(true).open(&path) {
            Ok(file) => return Ok((path, file)),
            Err(error) => failures.push(format!("{}: {error}", path.display())),
        }
    }

    Err(io::Error::other(format!(
        "no writable session-log path ({})",
        failures.join("; ")
    )))
}

/// Common baud rates for serial communication.
pub const COMMON_BAUD_RATES: &[u32] = &[
    4800, 9600, 19200, 38400, 57600, 115200, 230400, 460800, 500000, 576000, 921600, 1000000,
    1500000, 2000000,
];

/// Represents a serial port with its settings, data, and communication channels.
pub struct Serial {
    /// Port settings.
    pub set: PortSettings,
    /// Port data manager.
    data: PortData,
    /// Handle to the communication thread.
    thread_handle: Option<JoinHandle<Result<(), SerialBevyError>>>,
    /// Transmit channel for sending commands to the port thread.
    tx_channel: Option<mpsc::Sender<PortChannelData>>,
    /// Receive channel for receiving data from the port thread.
    rx_channel: Option<mpsc::Receiver<PortChannelData>>,
}

impl Default for Serial {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for Serial {
    fn drop(&mut self) {
        if let Some(handle) = &self.thread_handle {
            handle.abort();
        }
        self.data.flush_source_log();
    }
}

impl Serial {
    /// Creates a new Serial instance with default settings.
    #[must_use]
    pub fn new() -> Self {
        Self {
            set: PortSettings::default(),
            data: PortData::new(),
            thread_handle: None,
            tx_channel: None,
            rx_channel: None,
        }
    }

    /// Gets a mutable reference to the port data.
    pub const fn data(&mut self) -> &mut PortData {
        &mut self.data
    }

    /// Gets a mutable reference to the thread handle.
    pub const fn thread_handle(&mut self) -> &mut Option<JoinHandle<Result<(), SerialBevyError>>> {
        &mut self.thread_handle
    }

    /// Gets a mutable reference to the transmit channel.
    pub const fn tx_channel(&mut self) -> &mut Option<mpsc::Sender<PortChannelData>> {
        &mut self.tx_channel
    }

    /// Gets a mutable reference to the receive channel.
    pub const fn rx_channel(&mut self) -> &mut Option<mpsc::Receiver<PortChannelData>> {
        &mut self.rx_channel
    }

    /// Opens the serial port (sets state to Ready).
    pub fn open(&mut self) {
        self.data.state.open();
        self.data.last_error = None;
    }

    /// Marks that an open request is waiting for the background task.
    pub fn begin_open(&mut self) {
        self.data.state.begin_open();
        self.data.last_error = None;
    }

    /// Returns true if the port is open.
    #[must_use]
    pub const fn is_open(&self) -> bool {
        self.data.state.is_open()
    }

    /// Returns true while an open request is in progress.
    #[must_use]
    pub const fn is_opening(&self) -> bool {
        self.data.state.is_opening()
    }

    /// Closes the serial port.
    pub fn close(&mut self) {
        self.data.state.close();
        self.data.last_error = None;
        self.data.clear_send_data();
        self.data.clear_utf8_buffer();
        self.data.reset_receive_time();
        self.data.close_source_log();
        if let Some(handle) = self.thread_handle.take() {
            handle.abort();
        }
        self.tx_channel = None;
        self.rx_channel = None;
    }

    /// Returns true if the port is closed.
    #[must_use]
    pub const fn is_close(&self) -> bool {
        self.data.state.is_close()
    }

    /// Sets the port to error state and records a user-visible reason.
    pub fn error(&mut self, message: impl Into<String>) {
        self.data.state.error();
        self.report_error(message);
        if let Some(handle) = &self.thread_handle {
            handle.abort();
        }
    }

    /// Returns true if the port is in error state.
    #[must_use]
    pub const fn is_error(&self) -> bool {
        self.data.state.is_error()
    }

    /// Returns the most recent error reason, if any.
    #[must_use]
    pub fn last_error(&self) -> Option<&str> {
        self.data.last_error.as_deref()
    }

    /// Records a recoverable, user-visible error without closing the port.
    pub fn report_error(&mut self, message: impl Into<String>) {
        self.data.last_error = Some(message.into());
    }

    /// Clears the most recent user-visible error.
    pub fn clear_error(&mut self) {
        self.data.last_error = None;
    }
}

/// Serial port configuration settings.
#[derive(Clone, Debug)]
pub struct PortSettings {
    /// Port name (e.g., "COM1" or "/dev/ttyUSB0").
    pub port_name: String,
    /// Baud rate in bits per second.
    pub baud_rate: u32,
    /// Number of data bits.
    pub data_bits: DataBits,
    /// Number of stop bits.
    pub stop_bits: StopBits,
    /// Parity checking mode.
    pub parity: Parity,
    /// Flow control mode.
    pub flow_control: FlowControl,
    /// Timeout duration.
    pub timeout: Duration,
}

impl Default for PortSettings {
    fn default() -> Self {
        Self {
            port_name: String::from("Select a port"),
            baud_rate: 115200,
            data_bits: DataBits::Eight,
            stop_bits: StopBits::One,
            parity: Parity::None,
            flow_control: FlowControl::None,
            timeout: Duration::from_micros(500),
        }
    }
}

impl PortSettings {
    /// Creates new port settings with defaults.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

/// Opens a serial port with the specified settings.
///
/// # Arguments
///
/// * `settings` - The port configuration settings
///
/// # Returns
///
/// A Result containing the opened `SerialStream` or an error.
pub async fn open_port(settings: &PortSettings) -> Result<SerialStream, SerialBevyError> {
    tokio_serial::new(&settings.port_name, settings.baud_rate)
        .data_bits(settings.data_bits)
        .parity(settings.parity)
        .stop_bits(settings.stop_bits)
        .flow_control(settings.flow_control)
        .timeout(settings.timeout)
        .open_native_async()
        .inspect(|_stream| {
            info!("Successfully opened serial port: {}", settings.port_name);
        })
        .map_err(|e| {
            error!("Failed to open serial port {}: {}", settings.port_name, e);
            SerialBevyError::port_open(&settings.port_name, e.to_string())
        })
}

/// Cache for command history and current input.
pub struct CacheData {
    /// History of sent commands.
    history_data: VecDeque<String>,
    /// Current index in history.
    history_index: usize,
    /// Current input data.
    current_data: String,
}

impl Default for CacheData {
    fn default() -> Self {
        Self::new()
    }
}

impl CacheData {
    /// Creates a new `CacheData` instance.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            history_data: VecDeque::new(),
            history_index: 0,
            current_data: String::new(),
        }
    }

    /// Adds data to history if it's different from the last entry.
    pub fn add_history_data(&mut self, data: String) {
        if self.history_data.back().is_none_or(|last| *last != data) {
            if self.history_data.len() == COMMAND_HISTORY_MAX_ENTRIES {
                self.history_data.pop_front();
            }
            self.history_data.push_back(data);
        }
        self.history_index = self.history_data.len();
    }

    /// Replaces the editor contents with the previous history entry.
    pub fn previous_history(&mut self) {
        if self.history_index > 0 {
            self.history_index -= 1;
        }
        if let Some(value) = self.history_data.get(self.history_index) {
            self.current_data.clone_from(value);
        }
    }

    /// Replaces the editor contents with the next history entry, or clears it
    /// when moving past the newest command.
    pub fn next_history(&mut self) {
        if self.history_index < self.history_data.len() {
            self.history_index += 1;
        }
        if let Some(value) = self.history_data.get(self.history_index) {
            self.current_data.clone_from(value);
        } else {
            self.current_data.clear();
        }
    }

    /// Gets a mutable reference to the current input data.
    pub const fn get_current_data(&mut self) -> &mut String {
        &mut self.current_data
    }

    /// Takes the current editor contents, leaving it empty.
    pub fn take_current_data(&mut self) -> String {
        std::mem::take(&mut self.current_data)
    }
}

/// Port data management for files and communication.
pub struct PortData {
    /// Path of the active session log.
    source_path: Option<PathBuf>,
    /// Buffered writer for the active source log.
    source_writer: Option<BufWriter<File>>,
    /// Bounded live log used by the UI instead of rereading the file each frame.
    recent_log: String,
    /// Data pending to be sent.
    send_data: Vec<String>,
    /// Total UTF-8 byte length of pending send data.
    pending_send_bytes: usize,
    /// Command cache and history.
    cache_data: CacheData,
    /// Current port state.
    state: PortState,
    /// Most recent user-visible port error.
    last_error: Option<String>,
    /// Data encoding type.
    data_type: DataType,
    /// Whether to include line feeds in sent data.
    line_feed: bool,
    /// Buffer for incomplete UTF-8 sequences.
    utf8_buffer: Vec<u8>,
    /// Whether to include timestamps in logs.
    timestamp_enabled: bool,
    /// Timeout duration for auto line breaks in logs (0 = disabled).
    log_timeout: Duration,
    /// Last data receive time for timeout handling.
    last_receive_time: Option<std::time::Instant>,
}

impl Default for PortData {
    fn default() -> Self {
        Self::new()
    }
}

impl PortData {
    /// Creates a new `PortData` instance.
    #[must_use]
    pub fn new() -> Self {
        Self {
            source_path: None,
            source_writer: None,
            recent_log: String::new(),
            send_data: Vec::new(),
            pending_send_bytes: 0,
            cache_data: CacheData::new(),
            state: PortState::Close,
            last_error: None,
            data_type: DataType::Utf8,
            line_feed: false,
            utf8_buffer: Vec::new(),
            timestamp_enabled: false,
            log_timeout: Duration::from_millis(0),
            last_receive_time: None,
        }
    }

    /// Adds a source file in the first writable session-log directory.
    ///
    /// Sanitization rules:
    /// - Leading `/` or `\` is stripped (prevents absolute paths).
    /// - Inner `/` or `\` are replaced with `_`.
    ///
    /// The working-directory `logs/` is preferred, followed by `logs/` beside
    /// the executable and a temporary-directory fallback. No internal state is
    /// changed if every directory or file attempt fails.
    pub fn add_source_file(&mut self, name: String) -> crate::error::Result<()> {
        let sanitized = sanitize_log_file_name(&name);
        let (path, file) = open_session_log(&sanitized, log_directory_candidates())
            .map_err(SerialBevyError::FileIo)?;

        self.source_writer = Some(BufWriter::new(file));
        self.recent_log.clear();
        self.source_path = Some(path);
        Ok(())
    }

    /// Writes data to the last source file with optional timestamp.
    pub fn write_source_file(&mut self, data: &[u8], source: DataSource) -> std::io::Result<()> {
        if self.source_path.is_none() {
            return Ok(());
        }

        let head = if self.timestamp_enabled {
            let time = chrono::Local::now()
                .format("%Y%m%d %H:%M:%S.%3f")
                .to_string();
            format!("[{time} {source}]")
        } else {
            format!("[{source}]")
        };

        let mut record = Vec::with_capacity(head.len() + data.len() + 1);
        record.extend_from_slice(head.as_bytes());
        record.extend_from_slice(data);
        record.push(b'\n');

        let write_result = if let Some(writer) = &mut self.source_writer {
            writer.write_all(&record).and_then(|()| {
                if matches!(source, DataSource::Error) {
                    writer.flush()
                } else {
                    Ok(())
                }
            })
        } else {
            Ok(())
        };

        self.recent_log.push_str(&String::from_utf8_lossy(&record));
        self.trim_recent_log();
        write_result
    }

    /// Inserts a visual separator between receive bursts.
    pub fn write_log_separator(&mut self) -> std::io::Result<()> {
        if self.source_path.is_none() {
            return Ok(());
        }

        let write_result = self
            .source_writer
            .as_mut()
            .map_or(Ok(()), |writer| writer.write_all(b"\n"));

        self.recent_log.push('\n');
        self.trim_recent_log();
        write_result
    }

    /// Returns the bounded live log used by the receive window.
    #[must_use]
    pub fn recent_log(&self) -> &str {
        &self.recent_log
    }

    fn trim_recent_log(&mut self) {
        if self.recent_log.len() <= RECENT_LOG_MAX_BYTES {
            return;
        }

        let mut cut = self.recent_log.len() - RECENT_LOG_MAX_BYTES;
        while !self.recent_log.is_char_boundary(cut) {
            cut += 1;
        }
        if let Some(line_end) = self.recent_log[cut..].find('\n') {
            cut += line_end + 1;
        }
        self.recent_log.drain(..cut);
    }

    fn flush_source_log(&mut self) {
        if let Some(writer) = &mut self.source_writer
            && let Err(e) = writer.flush()
        {
            error!("Failed to flush serial log: {e}");
        }
    }

    fn close_source_log(&mut self) {
        self.flush_source_log();
        self.source_writer = None;
    }

    /// Returns the active session-log path.
    #[must_use]
    pub fn source_path(&self) -> Option<&Path> {
        self.source_path.as_deref()
    }

    /// Queues data to be sent, returning `false` when the bounded pending queue
    /// cannot accept the command.
    #[must_use]
    pub fn send_data(&mut self, data: String) -> bool {
        if self.send_data.len() >= PENDING_SEND_MAX_COMMANDS
            || self.pending_send_bytes.saturating_add(data.len()) > PENDING_SEND_MAX_BYTES
        {
            return false;
        }

        self.pending_send_bytes += data.len();
        self.send_data.push(data);
        true
    }

    /// Gets and clears the send data queue.
    pub fn get_send_data(&mut self) -> Vec<String> {
        self.pending_send_bytes = 0;
        std::mem::take(&mut self.send_data)
    }

    /// Restores a batch that could not yet enter the bounded command channel.
    pub fn restore_send_data(&mut self, mut data: Vec<String>) {
        data.append(&mut self.send_data);
        self.pending_send_bytes = data
            .iter()
            .fold(0usize, |total, value| total.saturating_add(value.len()));
        self.send_data = data;
    }

    /// Clears the send data queue.
    pub fn clear_send_data(&mut self) {
        self.send_data.clear();
        self.pending_send_bytes = 0;
    }

    /// Gets a mutable reference to the cache data.
    pub const fn get_cache_data(&mut self) -> &mut CacheData {
        &mut self.cache_data
    }

    /// Gets a mutable reference to the data type.
    pub const fn data_type(&mut self) -> &mut DataType {
        &mut self.data_type
    }

    /// Gets a mutable reference to the line feed setting.
    pub const fn line_feed(&mut self) -> &mut bool {
        &mut self.line_feed
    }

    /// Processes raw bytes with UTF-8 buffer handling.
    pub fn process_raw_bytes(&mut self, data: &[u8]) -> Vec<u8> {
        self.utf8_buffer.extend_from_slice(data);
        let mut decoded = String::new();
        let mut consumed = 0;

        while consumed < self.utf8_buffer.len() {
            match std::str::from_utf8(&self.utf8_buffer[consumed..]) {
                Ok(valid) => {
                    decoded.push_str(valid);
                    consumed = self.utf8_buffer.len();
                }
                Err(error) => {
                    let valid_end = consumed + error.valid_up_to();
                    // SAFETY: `valid_up_to` guarantees this prefix is valid UTF-8.
                    let valid = std::str::from_utf8(&self.utf8_buffer[consumed..valid_end])
                        .expect("validated UTF-8 prefix");
                    decoded.push_str(valid);

                    let Some(invalid_len) = error.error_len() else {
                        // The remaining bytes form a valid prefix of an incomplete
                        // code point; retain them for the next serial read.
                        consumed = valid_end;
                        break;
                    };

                    decoded.push('�');
                    consumed = valid_end + invalid_len;
                }
            }
        }

        self.utf8_buffer.drain(..consumed);
        decoded.into_bytes()
    }

    /// Clears the UTF-8 buffer.
    pub fn clear_utf8_buffer(&mut self) {
        self.utf8_buffer.clear();
    }

    /// Gets a mutable reference to the timestamp enabled flag.
    pub const fn timestamp_enabled(&mut self) -> &mut bool {
        &mut self.timestamp_enabled
    }

    /// Gets a mutable reference to the log timeout.
    pub const fn log_timeout(&mut self) -> &mut Duration {
        &mut self.log_timeout
    }

    /// Updates the last receive time and checks if timeout line break is needed.
    pub fn update_receive_time(&mut self) -> bool {
        let now = Instant::now();
        let should_break = if let Some(last_time) = self.last_receive_time {
            self.log_timeout.as_millis() > 0 && now.duration_since(last_time) >= self.log_timeout
        } else {
            false
        };
        self.last_receive_time = Some(now);
        should_break
    }

    /// Resets the receive time (called when port is opened).
    pub fn reset_receive_time(&mut self) {
        self.last_receive_time = None;
    }
}

/// Serial port state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PortState {
    /// An open request is in progress.
    Opening,
    /// Port is ready for communication.
    Ready,
    /// Port is closed.
    Close,
    /// Port encountered an error.
    Error,
}

impl PortState {
    /// Returns true if the port is open (Ready state).
    #[must_use]
    pub const fn is_open(&self) -> bool {
        matches!(self, Self::Ready)
    }

    /// Returns true if the port is being opened.
    #[must_use]
    pub const fn is_opening(&self) -> bool {
        matches!(self, Self::Opening)
    }

    /// Returns true if the port is closed.
    #[must_use]
    pub const fn is_close(&self) -> bool {
        matches!(self, Self::Close)
    }

    /// Returns true if the port is in error state.
    #[must_use]
    pub const fn is_error(&self) -> bool {
        matches!(self, Self::Error)
    }

    /// Sets the state to Ready.
    pub const fn open(&mut self) {
        *self = Self::Ready;
    }

    /// Sets the state to Opening.
    pub const fn begin_open(&mut self) {
        *self = Self::Opening;
    }

    /// Sets the state to Close.
    pub const fn close(&mut self) {
        *self = Self::Close;
    }

    /// Sets the state to Error.
    pub const fn error(&mut self) {
        *self = Self::Error;
    }
}

/// Data encoding type for serial communication.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataType {
    /// Hexadecimal encoding.
    Hex,
    /// UTF-8 text.
    Utf8,
}

impl fmt::Display for DataType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Hex => write!(f, "Hex"),
            Self::Utf8 => write!(f, "UTF-8"),
        }
    }
}

impl DataType {
    /// Gets the English name of the data type.
    #[must_use]
    pub const fn as_str_en(&self) -> &'static str {
        match self {
            Self::Hex => "Hexadecimal",
            Self::Utf8 => "UTF-8",
        }
    }

    /// Gets a description of the data type.
    #[must_use]
    pub const fn description(&self) -> &'static str {
        match self {
            Self::Hex => "Hexadecimal data format",
            Self::Utf8 => "UTF-8 text encoding",
        }
    }
}

/// Data for port read/write operations.
#[derive(Clone, Debug)]
pub struct PortIoData {
    /// The raw data bytes.
    pub data: Vec<u8>,
}

/// Channel data for communication between threads.
#[derive(Clone, Debug)]
pub enum PortChannelData {
    /// Available port names.
    PortName(Vec<String>),
    /// Data to write to the port.
    PortWrite(PortIoData),
    /// Data read from the port.
    PortRead(PortIoData),
    /// Request to open the port with the latest UI settings.
    PortOpen(PortSettings),
    /// Port state change.
    PortState(PortState),
    /// Port error occurred.
    PortError(PortIoData),
}

/// Data source identifier for logging.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DataSource {
    /// Data was written/sent.
    Write,
    /// Data was read/received.
    Read,
    /// Error message.
    Error,
}

impl fmt::Display for DataSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Write => write!(f, "T"),
            Self::Read => write!(f, "R"),
            Self::Error => write!(f, "E"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_port_settings_default() {
        let settings = PortSettings::default();
        assert_eq!(settings.baud_rate, 115200);
        assert_eq!(settings.data_bits, DataBits::Eight);
        assert_eq!(settings.stop_bits, StopBits::One);
        assert_eq!(settings.parity, Parity::None);
    }

    #[cfg(all(unix, not(any(target_os = "aix", target_os = "fuchsia"))))]
    #[test]
    fn test_open_port_over_pseudoterminal() {
        let pair = nix::pty::openpty(None, None).unwrap();
        let slave_path = nix::unistd::ttyname(&pair.slave).unwrap();
        let mut peer = File::from(pair.master);
        let slave = pair.slave;
        let (done_tx, done_rx) = std::sync::mpsc::channel();

        let peer_task = std::thread::spawn(move || {
            let mut request = [0; 4];
            std::io::Read::read_exact(&mut peer, &mut request).unwrap();
            assert_eq!(&request, b"ping");
            std::io::Write::write_all(&mut peer, b"pong").unwrap();
            // Keep the PTY master alive until the async side has consumed the reply.
            let _ = done_rx.recv();
        });

        let settings = PortSettings {
            port_name: slave_path.to_string_lossy().into_owned(),
            ..PortSettings::default()
        };
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            let mut port = open_port(&settings).await.unwrap();
            tokio::io::AsyncWriteExt::write_all(&mut port, b"ping")
                .await
                .unwrap();

            let mut response = [0; 4];
            tokio::time::timeout(
                Duration::from_secs(2),
                tokio::io::AsyncReadExt::read_exact(&mut port, &mut response),
            )
            .await
            .expect("pseudoterminal read timed out")
            .unwrap();
            assert_eq!(&response, b"pong");
        });

        done_tx.send(()).unwrap();
        peer_task.join().unwrap();
        drop(slave);
    }

    #[test]
    fn test_state_transitions() {
        let mut state = PortState::Close;
        assert!(state.is_close());

        state.begin_open();
        assert!(state.is_opening());

        state.open();
        assert!(state.is_open());

        state.error();
        assert!(state.is_error());

        state.close();
        assert!(state.is_close());
    }

    #[test]
    fn test_data_type_display() {
        assert_eq!(format!("{}", DataType::Hex), "Hex");
        assert_eq!(format!("{}", DataType::Utf8), "UTF-8");
    }

    #[test]
    fn test_cache_data_history() {
        let mut cache = CacheData::new();
        cache.add_history_data("command1".to_string());
        cache.add_history_data("command2".to_string());

        cache.previous_history();
        assert_eq!(cache.get_current_data(), "command2");

        cache.previous_history();
        assert_eq!(cache.get_current_data(), "command1");

        cache.next_history();
        assert_eq!(cache.get_current_data(), "command2");
        cache.next_history();
        assert!(cache.get_current_data().is_empty());
    }

    #[test]
    fn test_cache_data_no_duplicate() {
        let mut cache = CacheData::new();
        cache.add_history_data("command1".to_string());
        cache.previous_history();
        cache.add_history_data("command1".to_string());

        assert_eq!(cache.history_data.len(), 1);
        cache.previous_history();
        assert_eq!(cache.get_current_data(), "command1");
    }

    #[test]
    fn command_history_is_bounded() {
        let mut cache = CacheData::new();
        for index in 0..=COMMAND_HISTORY_MAX_ENTRIES {
            cache.add_history_data(format!("command-{index}"));
        }

        assert_eq!(cache.history_data.len(), COMMAND_HISTORY_MAX_ENTRIES);
        assert_eq!(cache.history_data.front().unwrap(), "command-1");
    }

    #[test]
    fn restoring_unsent_data_preserves_command_order() {
        let mut data = PortData::new();
        assert!(data.send_data("new".to_string()));
        data.restore_send_data(vec!["first".to_string(), "second".to_string()]);

        assert_eq!(data.get_send_data(), ["first", "second", "new"]);
        assert_eq!(data.pending_send_bytes, 0);
    }

    #[test]
    fn pending_send_queue_is_bounded() {
        let mut data = PortData::new();
        for _ in 0..PENDING_SEND_MAX_COMMANDS {
            assert!(data.send_data(String::new()));
        }

        assert!(!data.send_data("one too many".to_string()));
        assert_eq!(data.send_data.len(), PENDING_SEND_MAX_COMMANDS);

        data.clear_send_data();
        assert!(!data.send_data("x".repeat(PENDING_SEND_MAX_BYTES + 1)));
        assert!(data.send_data("x".repeat(PENDING_SEND_MAX_BYTES)));
    }

    #[test]
    fn test_utf8_stream_preserves_incomplete_code_point() {
        let mut data = PortData::new();

        assert_eq!(data.process_raw_bytes(&[0xE4, 0xBD]), b"");
        assert_eq!(data.utf8_buffer, vec![0xE4, 0xBD]);
        assert_eq!(data.process_raw_bytes(&[0xA0]), "你".as_bytes());
        assert!(data.utf8_buffer.is_empty());
    }

    #[test]
    fn test_utf8_stream_keeps_valid_text_after_invalid_byte() {
        let mut data = PortData::new();
        let output = data.process_raw_bytes(b"before\xFFafter");

        assert_eq!(String::from_utf8(output).unwrap(), "before�after");
        assert!(data.utf8_buffer.is_empty());
    }

    #[test]
    fn test_recent_log_is_bounded_on_utf8_boundary() {
        let mut data = PortData::new();
        data.recent_log = "你".repeat(RECENT_LOG_MAX_BYTES / 3 + 100);

        data.trim_recent_log();

        assert!(data.recent_log.len() <= RECENT_LOG_MAX_BYTES);
        assert!(std::str::from_utf8(data.recent_log.as_bytes()).is_ok());
    }

    #[test]
    fn test_recent_log_records_direction_and_separator() {
        let mut data = PortData::new();
        data.source_path = Some(PathBuf::from("unused"));

        data.write_source_file(b"hello", DataSource::Read).unwrap();
        data.write_log_separator().unwrap();
        data.write_source_file(b"world", DataSource::Write).unwrap();

        assert_eq!(data.recent_log(), "[R]hello\n\n[T]world\n");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_log_write_error_is_returned_and_kept_in_live_log() {
        let file = OpenOptions::new().write(true).open("/dev/full").unwrap();
        let mut data = PortData::new();
        data.source_path = Some(PathBuf::from("/dev/full"));
        data.source_writer = Some(BufWriter::new(file));

        let result = data.write_source_file(b"payload", DataSource::Error);

        assert!(result.is_err());
        assert_eq!(data.recent_log(), "[E]payload\n");
    }

    #[test]
    fn test_log_file_name_sanitization() {
        assert_eq!(
            sanitize_log_file_name("/dev/ttyUSB0_session.txt"),
            "dev_ttyUSB0_session.txt"
        );
        assert_eq!(sanitize_log_file_name(".."), "serial.log");
        assert_eq!(sanitize_log_file_name(""), "serial.log");
        assert_eq!(sanitize_log_file_name("CON.txt"), "serial_CON.txt");
        assert_eq!(
            sanitize_log_file_name("COM1:<invalid>?*.txt"),
            "COM1__invalid___.txt"
        );

        let long_name = format!("{}你.txt", "a".repeat(LOG_FILE_NAME_MAX_BYTES));
        let sanitized = sanitize_log_file_name(&long_name);
        assert!(sanitized.len() <= LOG_FILE_NAME_MAX_BYTES);
        assert!(std::str::from_utf8(sanitized.as_bytes()).is_ok());
    }

    #[test]
    fn session_log_uses_the_next_writable_directory() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "serial_bevy_log_fallback_{}_{}",
            std::process::id(),
            unique
        ));
        std::fs::create_dir_all(&root).unwrap();
        let blocked = root.join("not-a-directory");
        std::fs::write(&blocked, b"block directory creation").unwrap();
        let fallback = root.join("fallback");

        let (path, file) = open_session_log("session.txt", [blocked, fallback.clone()]).unwrap();

        assert_eq!(path, fallback.join("session.txt"));
        drop(file);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn test_serial_error_reason_is_cleared_on_close() {
        let mut serial = Serial::new();
        serial.error("permission denied");

        assert!(serial.is_error());
        assert_eq!(serial.last_error(), Some("permission denied"));

        serial.close();
        assert!(serial.is_close());
        assert_eq!(serial.last_error(), None);
    }

    #[test]
    fn closing_discards_session_buffers() {
        let mut serial = Serial::new();
        serial.open();
        assert!(serial.data.send_data("stale command".to_string()));
        serial.data.utf8_buffer.extend_from_slice(&[0xE4, 0xBD]);
        serial.data.last_receive_time = Some(Instant::now());

        serial.close();

        assert!(serial.data.send_data.is_empty());
        assert_eq!(serial.data.pending_send_bytes, 0);
        assert!(serial.data.utf8_buffer.is_empty());
        assert!(serial.data.last_receive_time.is_none());
    }

    #[test]
    fn test_recoverable_error_keeps_port_state() {
        let mut serial = Serial::new();
        serial.open();

        serial.report_error("invalid command");

        assert!(serial.is_open());
        assert_eq!(serial.last_error(), Some("invalid command"));
    }
}
