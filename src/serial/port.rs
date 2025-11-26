//! # Port Module
//!
//! This module provides serial port types, settings, and state management.

use log::{error, info};
use std::fmt;
use std::fs::OpenOptions;
use std::io::{BufReader, BufWriter, Read, Write};
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
    pub fn is_open(&mut self) -> bool {
        self.data.state().is_open()
    }

    /// Closes the serial port.
    pub fn close(&mut self) {
        self.data.state().close();
        self.thread_handle = None;
    }

    /// Returns true if the port is closed.
    #[must_use]
    pub fn is_close(&mut self) -> bool {
        self.data.state().is_close()
    }

    /// Sets the port to error state.
    pub fn error(&mut self) {
        self.data.state().error();
    }

    /// Returns true if the port is in error state.
    #[must_use]
    pub fn is_error(&mut self) -> bool {
        self.data.state().is_error()
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

        if let Err(e) = OpenOptions::new()
            .create(true)
            .read(true)
            .append(true)
            .open(&path)
        {
            error!("Failed to create source file {path}: {e}");
        }

        self.source_file.file.push(path);
        self.source_file.file.len()
    }

    /// Gets the number of source files.
    #[must_use]
    pub const fn source_file_index(&self) -> usize {
        self.source_file.file.len()
    }

    /// Writes data to the last source file with timestamp.
    pub fn write_source_file(&mut self, data: &[u8], source: DataSource) {
        let Some(file_path) = self.source_file.file.last() else {
            return;
        };

        let time = chrono::Local::now()
            .format("%Y%m%d %H:%M:%S.%3f")
            .to_string();
        let head = format!("[{time} {source}]");

        if let Ok(file) = OpenOptions::new().append(true).open(file_path) {
            let mut writer = BufWriter::new(file);
            let mut combined = Vec::new();
            combined.extend_from_slice(head.as_bytes());
            combined.extend_from_slice(data);
            let _ = writer.write_all(b"\n");
            let _ = writer.write_all(&combined);
            let _ = writer.flush();
        }
    }

    /// Reads the current source file contents.
    #[must_use]
    pub fn read_current_source_file(&mut self) -> String {
        self.source_file
            .file
            .last()
            .and_then(|path| {
                OpenOptions::new().read(true).open(path).ok().map(|file| {
                    let mut data = String::new();
                    let mut reader = BufReader::new(file);
                    let _ = reader.read_to_string(&mut data);
                    data
                })
            })
            .unwrap_or_default()
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
pub struct PorRWData {
    /// The raw data bytes.
    pub data: Vec<u8>,
}

/// Channel data for communication between threads.
#[derive(Clone, Debug)]
pub enum PortChannelData {
    /// Available port names.
    PortName(Vec<String>),
    /// Data to write to the port.
    PortWrite(PorRWData),
    /// Data read from the port.
    PortRead(PorRWData),
    /// Request to open the port.
    PortOpen,
    /// Request to close the port.
    PortClose(String),
    /// Port state change.
    PortState(PortState),
    /// Port error occurred.
    PortError(PorRWData),
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

/// LLM configuration for AI features.
pub struct LlmConfig {
    /// Whether LLM features are enabled.
    pub enable: bool,
    /// API key for the LLM service.
    pub key: String,
    /// Model name.
    pub model: String,
    /// Stored conversation history.
    pub stored_message: Vec<LlmMessage>,
    /// Current conversation.
    pub current_message: Vec<LlmMessage>,
    /// Associated file names.
    pub file_name: Vec<String>,
    /// Current LLM state.
    pub state: LlmState,
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
            key: String::new(),
            model: String::from("glm-4-flash"),
            stored_message: Vec::new(),
            current_message: Vec::new(),
            file_name: Vec::new(),
            state: LlmState::default(),
        }
    }

    /// Gets a mutable reference to the enable flag.
    pub const fn enable(&mut self) -> &mut bool {
        &mut self.enable
    }

    /// Sets the API key.
    pub fn set_key(&mut self, key: &str) {
        self.key = key.to_string();
    }

    /// Sets the model name.
    pub fn set_model(&mut self, model: &str) {
        self.model = model.to_string();
    }

    /// Gets the model name.
    #[must_use]
    pub fn get_model(&self) -> &str {
        &self.model
    }

    /// Stores a message in history.
    pub fn store_message(&mut self, message: LlmMessage) {
        self.stored_message.push(message);
    }

    /// Gets stored messages.
    #[must_use]
    pub fn get_stored_message(&self) -> &[LlmMessage] {
        &self.stored_message
    }

    /// Sets the current conversation.
    pub fn set_current_message(&mut self, message: Vec<LlmMessage>) {
        self.current_message = message;
    }

    /// Gets current messages.
    #[must_use]
    pub fn get_current_message(&self) -> &[LlmMessage] {
        &self.current_message
    }

    /// Clears current messages.
    pub fn clear_current_message(&mut self) {
        self.current_message.clear();
    }

    /// Adds a file name.
    pub fn set_file_name(&mut self, file_name: &str) {
        self.file_name.push(file_name.to_string());
    }
}

/// A message in an LLM conversation.
#[derive(Clone, Debug)]
pub struct LlmMessage {
    /// The role (user, assistant, system).
    pub role: String,
    /// The message content.
    pub content: String,
}

/// LLM operation state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LlmState {
    /// Ready to process requests.
    #[default]
    Ready,
    /// Currently processing a request.
    Processing,
    /// An error occurred.
    Error,
}

impl LlmState {
    /// Returns true if ready.
    #[must_use]
    pub const fn is_ready(&self) -> bool {
        matches!(self, Self::Ready)
    }

    /// Returns true if processing.
    #[must_use]
    pub const fn is_processing(&self) -> bool {
        matches!(self, Self::Processing)
    }

    /// Returns true if in error state.
    #[must_use]
    pub const fn is_error(&self) -> bool {
        matches!(self, Self::Error)
    }

    /// Sets the state.
    pub const fn set_state(&mut self, state: Self) {
        *self = state;
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

        let data = PortChannelData::PortOpen;
        let names: Vec<String> = data.into();
        assert!(names.is_empty());
    }

    #[test]
    fn test_llm_config() {
        let mut config = LlmConfig::new();
        assert!(!*config.enable());
        assert_eq!(config.get_model(), "glm-4-flash");

        config.set_model("gpt-4");
        assert_eq!(config.get_model(), "gpt-4");
    }

    #[test]
    fn test_llm_state() {
        let mut state = LlmState::Ready;
        assert!(state.is_ready());

        state.set_state(LlmState::Processing);
        assert!(state.is_processing());

        state.set_state(LlmState::Error);
        assert!(state.is_error());
    }
}
