//! # Port Module
//!
//! This module provides serial port types, settings, and state management.

use log::{debug, error};
use std::collections::VecDeque;
use std::fmt;
use std::fs::OpenOptions;
use std::io::{BufWriter, Read, Write};
use tokio::sync::broadcast;
use tokio::task::JoinHandle;
use tokio::time::Duration;
use tokio_serial::SerialPortBuilderExt;

pub use tokio_serial::{DataBits, FlowControl, Parity, SerialPort, SerialStream, StopBits};

use crate::error::SerialBevyError;

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
    /// Optional serial stream.
    stream: Option<SerialStream>,
    /// Handle to the communication thread.
    thread_handle: Option<JoinHandle<Result<(), SerialBevyError>>>,
    /// Transmit channel for sending commands to the port thread.
    tx_channel: Option<broadcast::Sender<PortChannelData>>,
    /// Receive channel for receiving data from the port thread.
    rx_channel: Option<broadcast::Receiver<PortChannelData>>,
    /// LLM configuration.
    llm: LlmConfig,
}

impl Default for Serial {
    fn default() -> Self {
        Self::new()
    }
}

impl Serial {
    /// Creates a new Serial instance with default settings.
    #[must_use]
    pub fn new() -> Self {
        Self {
            set: PortSettings::default(),
            data: PortData::new(),
            stream: None,
            thread_handle: None,
            tx_channel: None,
            rx_channel: None,
            llm: LlmConfig::new(),
        }
    }

    /// Gets a reference to the port settings.
    #[must_use]
    pub const fn set(&self) -> &PortSettings {
        &self.set
    }

    /// Gets a mutable reference to the port data.
    pub const fn data(&mut self) -> &mut PortData {
        &mut self.data
    }

    /// Gets a mutable reference to the stream option.
    pub const fn stream(&mut self) -> &mut Option<SerialStream> {
        &mut self.stream
    }

    /// Gets a mutable reference to the thread handle.
    pub const fn thread_handle(&mut self) -> &mut Option<JoinHandle<Result<(), SerialBevyError>>> {
        &mut self.thread_handle
    }

    /// Gets a mutable reference to the transmit channel.
    pub const fn tx_channel(&mut self) -> &mut Option<broadcast::Sender<PortChannelData>> {
        &mut self.tx_channel
    }

    /// Gets a mutable reference to the receive channel.
    pub const fn rx_channel(&mut self) -> &mut Option<broadcast::Receiver<PortChannelData>> {
        &mut self.rx_channel
    }

    /// Opens the serial port (sets state to Ready).
    pub fn open(&mut self) {
        self.data.state().open();
    }

    /// Returns true if the port is open.
    #[must_use]
    pub fn is_open(&self) -> bool {
        self.data.state.is_open()
    }

    /// Closes the serial port.
    pub fn close(&mut self) {
        self.data.state().close();
        self.data.flush_file_writer();
        self.thread_handle = None;
    }

    /// Returns true if the port is closed.
    #[must_use]
    pub fn is_close(&self) -> bool {
        self.data.state.is_close()
    }

    /// Sets the port to error state.
    pub fn error(&mut self) {
        self.data.state().error();
    }

    /// Returns true if the port is in error state.
    #[must_use]
    pub fn is_error(&self) -> bool {
        self.data.state.is_error()
    }

    /// Gets a mutable reference to the LLM configuration.
    pub const fn llm(&mut self) -> &mut LlmConfig {
        &mut self.llm
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
            timeout: Duration::from_millis(100),
        }
    }
}

impl PortSettings {
    /// Creates new port settings with defaults.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Copies settings from another `PortSettings` instance.
    pub fn config(&mut self, other: &Self) {
        self.port_name.clone_from(&other.port_name);
        self.baud_rate = other.baud_rate;
        self.data_bits = other.data_bits;
        self.stop_bits = other.stop_bits;
        self.parity = other.parity;
        self.flow_control = other.flow_control;
        self.timeout = other.timeout;
    }

    /// Gets a mutable reference to the port name.
    pub const fn port_name(&mut self) -> &mut String {
        &mut self.port_name
    }

    /// Gets a mutable reference to the baud rate.
    pub const fn baud_rate(&mut self) -> &mut u32 {
        &mut self.baud_rate
    }

    /// Gets a mutable reference to the data bits.
    pub const fn data_size(&mut self) -> &mut DataBits {
        &mut self.data_bits
    }

    /// Gets a mutable reference to the stop bits.
    pub const fn stop_bits(&mut self) -> &mut StopBits {
        &mut self.stop_bits
    }

    /// Gets a mutable reference to the parity setting.
    pub const fn parity(&mut self) -> &mut Parity {
        &mut self.parity
    }

    /// Gets a mutable reference to the flow control setting.
    pub const fn flow_control(&mut self) -> &mut FlowControl {
        &mut self.flow_control
    }

    /// Gets a mutable reference to the timeout.
    pub const fn timeout(&mut self) -> &mut Duration {
        &mut self.timeout
    }

    /// Gets the data bits as a display string.
    #[must_use]
    pub fn databits_name(&self) -> String {
        format!("{}", self.data_bits)
    }

    /// Gets the stop bits as a display string.
    #[must_use]
    pub fn stop_bits_name(&self) -> String {
        format!("{}", self.stop_bits)
    }

    /// Gets the parity as a display string.
    #[must_use]
    pub fn parity_name(&self) -> String {
        format!("{}", self.parity)
    }

    /// Gets the flow control as a display string.
    #[must_use]
    pub fn flow_control_name(&self) -> String {
        format!("{}", self.flow_control)
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
            debug!("Successfully opened serial port: {}", settings.port_name);
        })
        .map_err(|e| {
            error!("Failed to open serial port {}: {}", settings.port_name, e);
            SerialBevyError::port_open(&settings.port_name, e.to_string())
        })
}

/// Cache for command history and current input.
pub struct CacheData {
    /// History of sent commands.
    history_data: Vec<String>,
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
            history_data: Vec::new(),
            history_index: 0,
            current_data: String::new(),
        }
    }

    /// Adds data to history if it's different from the last entry.
    pub fn add_history_data(&mut self, data: String) {
        if self.history_data.last().is_none_or(|last| *last != data) {
            self.history_data.push(data);
            self.history_index = self.history_data.len();
        }
    }

    /// Moves to the next history entry.
    pub const fn add_history_index(&mut self) -> usize {
        if self.history_index < self.history_data.len() {
            self.history_index += 1;
        }
        self.history_index
    }

    /// Moves to the previous history entry.
    pub const fn sub_history_index(&mut self) -> usize {
        if self.history_index > 1 {
            self.history_index -= 1;
        }
        self.history_index
    }

    /// Gets the current history index.
    #[must_use]
    pub const fn get_current_data_index(&self) -> usize {
        self.history_index
    }

    /// Gets history data at the specified index.
    pub fn get_history_data(&mut self, index: usize) -> String {
        if self.history_data.is_empty() {
            return String::new();
        }

        self.history_index = index.min(self.history_data.len());
        if self.history_index > 0 {
            self.history_data[self.history_index - 1].clone()
        } else {
            String::new()
        }
    }

    /// Gets a mutable reference to the current input data.
    pub const fn get_current_data(&mut self) -> &mut String {
        &mut self.current_data
    }

    /// Clears the current input data.
    pub fn clear_current_data(&mut self) {
        self.current_data.clear();
    }
}

/// Port data management for files and communication.
pub struct PortData {
    /// Source file paths for logging.
    source_file: FileData,
    /// Parse file paths.
    parse_file: FileData,
    /// Data pending to be sent.
    send_data: Vec<String>,
    /// Command cache and history.
    cache_data: CacheData,
    /// Current port state.
    state: PortState,
    /// Data encoding type.
    data_type: DataType,
    /// Whether to include line feeds in sent data.
    line_feed: bool,
    /// Buffer for incomplete UTF-8 sequences.
    utf8_buffer: Vec<u8>,
    /// Console mode flag - provides better terminal experience for Linux serial consoles.
    /// When enabled: no timestamps, local echo, line-buffered sending.
    console_mode: bool,
    /// Show timestamp in log file and display.
    /// When false (default): raw data format without timestamps.
    /// When true: adds [timestamp source] prefix to each line.
    show_timestamp: bool,
    /// In-memory display buffer to avoid reading disk every frame.
    display_buffer: VecDeque<String>,
    /// Persistent file writer for logging.
    file_writer: Option<BufWriter<std::fs::File>>,
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
            source_file: FileData { file: Vec::new() },
            parse_file: FileData { file: Vec::new() },
            send_data: Vec::new(),
            cache_data: CacheData::new(),
            state: PortState::Close,
            data_type: DataType::Utf8,
            line_feed: false,
            utf8_buffer: Vec::new(),
            console_mode: false,
            show_timestamp: false,
            display_buffer: VecDeque::new(),
            file_writer: None,
        }
    }

    /// Adds a source file for logging under the relative `logs/` directory and returns the new file count.
    ///
    /// Sanitization rules:
    /// - Leading `/` or `\` is stripped (prevents absolute paths).
    /// - Inner `/` or `\` are replaced with `_`.
    ///
    ///   The final stored path is always `logs/<sanitized_name>`.
    ///   On failure to create the file, an error is logged but the path is still recorded.
    pub fn add_source_file(&mut self, name: String) -> usize {
        // Ensure logs directory exists (best-effort; ignore errors here).
        let _ = std::fs::create_dir_all("logs");

        // Sanitize user-provided file name (e.g. "/dev/ttyUSB0_20250101_010101.txt").
        let sanitized = name
            .trim_start_matches('/')
            .trim_start_matches('\\')
            .replace(['/', '\\'], "_");

        let path = format!("logs/{sanitized}");

        match OpenOptions::new()
            .create(true)
            .read(true)
            .append(true)
            .open(&path)
        {
            Ok(file) => {
                self.file_writer = Some(BufWriter::new(file));
            }
            Err(e) => {
                error!("Failed to create source file {path}: {e}");
                self.file_writer = None;
            }
        }

        self.source_file.file.push(path);
        self.source_file.file.len()
    }

    /// Gets the number of source files.
    #[must_use]
    pub const fn source_file_index(&self) -> usize {
        self.source_file.file.len()
    }

    /// Writes data to the last source file and memory display buffer.
    /// Format depends on show_timestamp setting:
    /// - If show_timestamp is true: writes with [timestamp source] prefix
    /// - If show_timestamp is false: writes raw data without prefix
    pub fn write_source_file(&mut self, data: &[u8], source: DataSource) {
        let line = if self.show_timestamp {
            let time = chrono::Local::now()
                .format("%Y%m%d %H:%M:%S.%3f")
                .to_string();
            format!("\n[{time} {source}]{}", String::from_utf8_lossy(data))
        } else {
            String::from_utf8_lossy(data).into_owned()
        };

        // Write to persistent file writer
        if let Some(writer) = &mut self.file_writer {
            let _ = writer.write_all(line.as_bytes());
            let _ = writer.flush();
        }

        // Push to memory display buffer
        self.display_buffer.push_back(line);
        while self.display_buffer.len() > 5000 {
            self.display_buffer.pop_front();
        }
    }

    /// Reads the current display data from memory buffer.
    #[must_use]
    pub fn read_current_source_file_bytes(&self) -> Vec<u8> {
        let mut result = String::new();
        for line in &self.display_buffer {
            result.push_str(line);
        }
        result.into_bytes()
    }

    /// Clears the in-memory display buffer for the current log view.
    pub fn clear_display_buffer(&mut self) {
        self.display_buffer.clear();
    }

    /// Flushes the persistent file writer.
    pub fn flush_file_writer(&mut self) {
        if let Some(writer) = &mut self.file_writer {
            let _ = writer.flush();
        }
    }

    /// Reads a specific source file by index.
    #[must_use]
    pub fn read_source_file(&self, index: usize) -> String {
        self.source_file
            .file
            .get(index)
            .and_then(|path| {
                OpenOptions::new()
                    .read(true)
                    .open(path)
                    .ok()
                    .map(|mut file| {
                        let mut data = String::new();
                        let _ = file.read_to_string(&mut data);
                        data
                    })
            })
            .unwrap_or_default()
    }

    /// Gets a source file name by index.
    #[must_use]
    pub fn get_source_file_name(&self, index: usize) -> &str {
        self.source_file
            .file
            .get(index)
            .map(String::as_str)
            .unwrap_or_default()
    }

    /// Adds a parse file and returns the new file count.
    pub fn add_parse_file(&mut self, name: String) -> usize {
        if let Err(e) = OpenOptions::new().create(true).append(true).open(&name) {
            error!("Failed to create parse file {name}: {e}");
        }
        self.parse_file.file.push(name);
        self.parse_file.file.len()
    }

    /// Gets the number of parse files.
    #[must_use]
    pub const fn parse_file_index(&self) -> usize {
        self.parse_file.file.len()
    }

    /// Writes data to the last parse file.
    pub fn write_parse_file(&mut self, data: &[u8]) {
        if let Some(file_path) = self.parse_file.file.last()
            && let Ok(file) = OpenOptions::new().append(true).open(file_path)
        {
            let mut writer = BufWriter::new(file);
            let _ = writer.write_all(data);
            let _ = writer.write_all(b"\n");
            let _ = writer.flush();
        }
    }

    /// Reads the current parse file contents.
    #[must_use]
    pub fn read_current_parse_file(&mut self) -> String {
        self.parse_file
            .file
            .last()
            .and_then(|path| {
                OpenOptions::new()
                    .read(true)
                    .open(path)
                    .ok()
                    .map(|mut file| {
                        let mut data = String::new();
                        let _ = file.read_to_string(&mut data);
                        data
                    })
            })
            .unwrap_or_default()
    }

    /// Gets a parse file name by index.
    #[must_use]
    pub fn get_parse_file_name(&self, index: usize) -> &str {
        self.parse_file
            .file
            .get(index)
            .map(String::as_str)
            .unwrap_or_default()
    }

    /// Queues data to be sent.
    pub fn send_data(&mut self, data: String) {
        self.send_data.push(data);
    }

    /// Gets and clears the send data queue.
    pub fn get_send_data(&mut self) -> Vec<String> {
        std::mem::take(&mut self.send_data)
    }

    /// Clears the send data queue.
    pub fn clear_send_data(&mut self) {
        self.send_data.clear();
    }

    /// Sets the data encoding type.
    pub const fn set_data_type(&mut self, data_type: DataType) {
        self.data_type = data_type;
    }

    /// Gets a mutable reference to the cache data.
    pub const fn get_cache_data(&mut self) -> &mut CacheData {
        &mut self.cache_data
    }

    /// Gets a mutable reference to the port state.
    pub const fn state(&mut self) -> &mut PortState {
        &mut self.state
    }

    /// Gets a mutable reference to the data type.
    pub const fn data_type(&mut self) -> &mut DataType {
        &mut self.data_type
    }

    /// Gets a mutable reference to the line feed setting.
    pub const fn line_feed(&mut self) -> &mut bool {
        &mut self.line_feed
    }

    /// Gets a mutable reference to the console mode setting.
    pub const fn console_mode(&mut self) -> &mut bool {
        &mut self.console_mode
    }

    /// Returns true if console mode is enabled.
    #[must_use]
    pub const fn is_console_mode(&self) -> bool {
        self.console_mode
    }

    /// Gets a mutable reference to the show timestamp setting.
    pub const fn show_timestamp(&mut self) -> &mut bool {
        &mut self.show_timestamp
    }

    /// Returns true if timestamps should be shown.
    #[must_use]
    pub const fn is_show_timestamp(&self) -> bool {
        self.show_timestamp
    }

    /// Processes raw bytes with UTF-8 buffer handling.
    /// Also normalizes line endings: converts \r\n to \n and removes standalone \r
    pub fn process_raw_bytes(&mut self, data: &[u8]) -> Vec<u8> {
        // Add new data to buffer
        self.utf8_buffer.extend_from_slice(data);

        // Try to decode as much as possible
        let (valid_str, incomplete_len) = self.extract_valid_utf8();

        // Remove processed bytes from buffer
        if incomplete_len > 0 {
            self.utf8_buffer
                .drain(..(self.utf8_buffer.len() - incomplete_len));
        } else {
            self.utf8_buffer.clear();
        }

        // Normalize line endings: \r\n -> \n, standalone \r -> \n
        let normalized = valid_str.replace("\r\n", "\n").replace('\r', "\n");

        normalized.into_bytes()
    }

    /// Extracts valid UTF-8 from buffer, returns (valid_string, incomplete_bytes_count)
    fn extract_valid_utf8(&self) -> (String, usize) {
        if self.utf8_buffer.is_empty() {
            return (String::new(), 0);
        }

        // Try to decode the entire buffer
        match std::str::from_utf8(&self.utf8_buffer) {
            Ok(valid_str) => {
                // All bytes are valid UTF-8
                (valid_str.to_string(), 0)
            }
            Err(e) => {
                let valid_len = e.valid_up_to();
                if valid_len > 0 {
                    // We have some valid UTF-8 at the beginning
                    let valid_str =
                        std::str::from_utf8(&self.utf8_buffer[..valid_len]).unwrap_or("�"); // Fallback to replacement char
                    (valid_str.to_string(), self.utf8_buffer.len() - valid_len)
                } else {
                    // No valid UTF-8 at start, check if we have incomplete UTF-8 at end
                    let incomplete_len = self.count_incomplete_utf8_suffix();
                    if incomplete_len > 0 && incomplete_len < 4 {
                        // Likely incomplete UTF-8 sequence, keep it for next time
                        let valid_len = self.utf8_buffer.len() - incomplete_len;
                        if valid_len > 0 {
                            let valid_str =
                                std::str::from_utf8(&self.utf8_buffer[..valid_len]).unwrap_or("�");
                            (valid_str.to_string(), incomplete_len)
                        } else {
                            // All bytes are incomplete, keep them all
                            (String::new(), incomplete_len)
                        }
                    } else {
                        // Invalid UTF-8, replace with replacement char
                        ("�".to_string(), 0)
                    }
                }
            }
        }
    }

    /// Counts incomplete UTF-8 sequence at the end of buffer
    fn count_incomplete_utf8_suffix(&self) -> usize {
        if self.utf8_buffer.is_empty() {
            return 0;
        }

        // Check last 1-3 bytes for incomplete UTF-8 sequence
        let len = self.utf8_buffer.len();
        let check_len = std::cmp::min(3, len);

        for i in 1..=check_len {
            let start = len - i;
            let slice = &self.utf8_buffer[start..];

            // Check if this could be the start of a UTF-8 sequence
            if slice[0] >= 0x80 {
                // This is a continuation byte or start of multi-byte sequence
                // Check if it's a valid UTF-8 start byte
                if (slice[0] & 0xE0) == 0xC0 && i >= 1 && i <= 2 {
                    // 2-byte sequence
                    return if i == 1 { 1 } else { 0 };
                } else if (slice[0] & 0xF0) == 0xE0 && i >= 1 && i <= 3 {
                    // 3-byte sequence
                    return if i <= 2 { i } else { 0 };
                } else if (slice[0] & 0xF8) == 0xF0 && i >= 1 && i <= 4 {
                    // 4-byte sequence
                    return if i <= 3 { i } else { 0 };
                } else if (slice[0] & 0xC0) == 0x80 {
                    // Continuation byte
                    return i;
                }
            }
        }

        0
    }

    /// Clears the UTF-8 buffer.
    pub fn clear_utf8_buffer(&mut self) {
        self.utf8_buffer.clear();
    }
}

/// File data storage.
struct FileData {
    /// List of file paths.
    file: Vec<String>,
}

/// Serial port state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PortState {
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
    /// Binary data.
    Binary,
    /// Hexadecimal encoding.
    Hex,
    /// UTF-8 text.
    Utf8,
    /// UTF-16 text.
    Utf16,
    /// UTF-32 text.
    Utf32,
    /// GBK encoding.
    Gbk,
    /// ASCII text.
    Ascii,
}

impl fmt::Display for DataType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Binary => write!(f, "Binary"),
            Self::Hex => write!(f, "Hex"),
            Self::Utf8 => write!(f, "UTF-8"),
            Self::Utf16 => write!(f, "UTF-16"),
            Self::Utf32 => write!(f, "UTF-32"),
            Self::Gbk => write!(f, "GBK"),
            Self::Ascii => write!(f, "ASCII"),
        }
    }
}

impl DataType {
    /// Gets the English name of the data type.
    #[must_use]
    pub const fn as_str_en(&self) -> &'static str {
        match self {
            Self::Binary => "Binary",
            Self::Hex => "Hexadecimal",
            Self::Utf8 => "UTF-8",
            Self::Utf16 => "UTF-16",
            Self::Utf32 => "UTF-32",
            Self::Gbk => "GBK",
            Self::Ascii => "ASCII",
        }
    }

    /// Gets a description of the data type.
    #[must_use]
    pub const fn description(&self) -> &'static str {
        match self {
            Self::Binary => "Binary data format",
            Self::Hex => "Hexadecimal data format",
            Self::Utf8 => "UTF-8 text encoding",
            Self::Utf16 => "UTF-16 text encoding",
            Self::Utf32 => "UTF-32 text encoding",
            Self::Gbk => "GBK Chinese encoding",
            Self::Ascii => "ASCII text encoding",
        }
    }
}

/// Data for port read/write operations.
#[derive(Clone, Debug)]
pub struct PortRwData {
    /// The raw data bytes.
    pub data: Vec<u8>,
}

/// Channel data for communication between threads.
#[derive(Clone, Debug)]
pub enum PortChannelData {
    /// Available port names.
    PortName(Vec<String>),
    /// Data to write to the port.
    PortWrite(PortRwData),
    /// Data read from the port.
    PortRead(PortRwData),
    /// Request to open the port with current settings.
    PortOpen(PortSettings),
    /// Request to close the port.
    PortClose(String),
    /// Port state change.
    PortState(PortState),
    /// Port error occurred.
    PortError(PortRwData),
}

impl From<PortChannelData> for Vec<String> {
    fn from(data: PortChannelData) -> Self {
        match data {
            PortChannelData::PortName(names) => names,
            _ => Self::new(),
        }
    }
}

/// Data source identifier for logging.
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

/// Available text models for AI chat.
pub const TEXT_MODELS: &[(&str, &str)] = &[
    ("glm-4.7", "GLM-4.7"),
    ("glm-4.6", "GLM-4.6"),
    ("glm-4.5", "GLM-4.5"),
    ("glm-4.5-flash", "GLM-4.5-Flash"),
    ("glm-4.5-air", "GLM-4.5-Air"),
    ("glm-4.5-X", "GLM-4.5-X"),
    ("glm-4.5-airx", "GLM-4.5-AirX"),
];

/// LLM configuration for AI features (per-serial state).
pub struct LlmConfig {
    /// Whether LLM features are enabled for this serial port.
    pub enable: bool,
    /// Conversation history messages (role, content).
    pub messages: Vec<LlmMessage>,
    /// Current user input buffer.
    pub input_buffer: String,
    /// Whether an AI request is pending (user clicked send).
    pub is_processing: bool,
    /// Whether the request has already been dispatched to async runtime.
    /// Prevents spawning duplicate requests every frame.
    pub request_in_flight: bool,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self::new()
    }
}

impl LlmConfig {
    /// Creates a new LLM configuration.
    #[must_use]
    pub fn new() -> Self {
        Self {
            enable: false,
            messages: Vec::new(),
            input_buffer: String::new(),
            is_processing: false,
            request_in_flight: false,
        }
    }

    /// Gets a mutable reference to the enable flag.
    pub const fn enable(&mut self) -> &mut bool {
        &mut self.enable
    }

    /// Adds a user message to the conversation.
    pub fn add_user_message(&mut self, content: &str) {
        self.messages.push(LlmMessage::user(content));
    }

    /// Adds an assistant message to the conversation.
    pub fn add_assistant_message(&mut self, content: &str) {
        self.messages.push(LlmMessage::assistant(content));
    }

    /// Clears the conversation history.
    pub fn clear_messages(&mut self) {
        self.messages.clear();
    }

    /// Returns true if there are messages.
    #[must_use]
    pub fn has_messages(&self) -> bool {
        !self.messages.is_empty()
    }
}

/// A message in an LLM conversation.
#[derive(Clone, Debug)]
pub struct LlmMessage {
    /// The role (user, assistant, system).
    pub role: String,
    /// The message content.
    pub content: String,
    /// Timestamp when the message was created.
    pub timestamp: String,
}

impl LlmMessage {
    /// Creates a new user message with current timestamp.
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: String::from("user"),
            content: content.into(),
            timestamp: chrono::Local::now().format("%H:%M:%S").to_string(),
        }
    }

    /// Creates a new assistant message with current timestamp.
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: String::from("assistant"),
            content: content.into(),
            timestamp: chrono::Local::now().format("%H:%M:%S").to_string(),
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
        assert_eq!(settings.timeout, Duration::from_millis(100));
    }

    #[test]
    fn test_state_transitions() {
        let mut state = PortState::Close;
        assert!(state.is_close());

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

        assert_eq!(cache.get_current_data_index(), 2);

        cache.sub_history_index();
        let cmd = cache.get_history_data(cache.get_current_data_index());
        assert_eq!(cmd, "command1");
    }

    #[test]
    fn test_cache_data_no_duplicate() {
        let mut cache = CacheData::new();
        cache.add_history_data("command1".to_string());
        cache.add_history_data("command1".to_string());

        assert_eq!(cache.history_data.len(), 1);
    }

    #[test]
    fn test_port_channel_data_conversion() {
        let data = PortChannelData::PortName(vec!["COM1".to_string(), "COM2".to_string()]);
        let names: Vec<String> = data.into();
        assert_eq!(names.len(), 2);

        let data = PortChannelData::PortOpen(PortSettings::default());
        let names: Vec<String> = data.into();
        assert!(names.is_empty());
    }

    #[test]
    fn test_llm_config() {
        let mut config = LlmConfig::new();
        assert!(!*config.enable());
        assert!(config.messages.is_empty());
        assert!(!config.is_processing);
        assert!(!config.request_in_flight);

        config.add_user_message("Hello");
        assert_eq!(config.messages.len(), 1);
        assert_eq!(config.messages[0].role, "user");

        config.add_assistant_message("Hi there");
        assert_eq!(config.messages.len(), 2);
        assert_eq!(config.messages[1].role, "assistant");

        config.clear_messages();
        assert!(config.messages.is_empty());
    }

    #[test]
    fn test_timeout_setting() {
        let mut settings = PortSettings::default();
        assert_eq!(settings.timeout, Duration::from_millis(100));

        // Test setting different timeout values
        *settings.timeout() = Duration::from_millis(500);
        assert_eq!(settings.timeout, Duration::from_millis(500));

        *settings.timeout() = Duration::from_millis(1000);
        assert_eq!(settings.timeout, Duration::from_millis(1000));

        // Test that timeout as_millis works correctly
        assert_eq!(settings.timeout.as_millis(), 1000);
    }
}
