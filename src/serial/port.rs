//! # Port Module
//!
//! This module provides serial port types, settings, and state management.

use log::{debug, error};
use tokio::sync::broadcast;
use tokio::task::JoinHandle;
use tokio::time::Duration;
use tokio_serial::SerialPortBuilderExt;

pub use tokio_serial::{DataBits, FlowControl, Parity, SerialPort, SerialStream, StopBits};

use crate::error::SerialBevyError;

// Re-exports for backward compatibility (types that were previously defined in this module).
// These also serve as imports for the types used in this module's struct definitions.
pub use super::data_types::DataType;
pub use super::llm::{LlmConfig, LlmMessage, TEXT_MODELS};
pub use super::port_data::PortData;
pub use super::state::{DataSource, PortChannelData, PortRwData, PortState};
// Note: these re-exports maintain the public API so that
// `use crate::serial::port::*` and direct paths like
// `crate::serial::port::DataType` continue to work.

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
        self.data.state_ref().is_open()
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
        self.data.state_ref().is_close()
    }

    /// Sets the port to error state.
    pub fn error(&mut self) {
        self.data.state().error();
    }

    /// Returns true if the port is in error state.
    #[must_use]
    pub fn is_error(&self) -> bool {
        self.data.state_ref().is_error()
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
