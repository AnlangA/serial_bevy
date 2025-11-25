//! # Error Module
//!
//! This module provides custom error types for the `serial_bevy` application.
//! It uses the `thiserror` crate for ergonomic error handling.

use thiserror::Error;

/// Result type alias for `serial_bevy` operations.
pub type Result<T> = std::result::Result<T, SerialBevyError>;

/// Main error type for the `serial_bevy` application.
#[derive(Debug, Error)]
pub enum SerialBevyError {
    /// Serial port operation failed.
    #[error("Serial port error: {0}")]
    SerialPort(String),

    /// Failed to open serial port.
    #[error("Failed to open serial port '{port_name}': {reason}")]
    PortOpen { port_name: String, reason: String },

    /// Failed to read from serial port.
    #[error("Failed to read from serial port: {0}")]
    PortRead(String),

    /// Failed to write to serial port.
    #[error("Failed to write to serial port: {0}")]
    PortWrite(String),

    /// Channel communication error.
    #[error("Channel communication error: {0}")]
    Channel(String),

    /// Data encoding/decoding error.
    #[error("Data encoding error: {0}")]
    Encoding(String),

    /// File I/O error.
    #[error("File I/O error: {0}")]
    FileIo(#[from] std::io::Error),

    /// Invalid configuration.
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),
}

impl SerialBevyError {
    /// Creates a new serial port error.
    #[must_use]
    pub fn serial_port(msg: impl Into<String>) -> Self {
        Self::SerialPort(msg.into())
    }

    /// Creates a new port open error.
    #[must_use]
    pub fn port_open(port_name: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::PortOpen {
            port_name: port_name.into(),
            reason: reason.into(),
        }
    }

    /// Creates a new channel error.
    #[must_use]
    pub fn channel(msg: impl Into<String>) -> Self {
        Self::Channel(msg.into())
    }

    /// Creates a new encoding error.
    #[must_use]
    pub fn encoding(msg: impl Into<String>) -> Self {
        Self::Encoding(msg.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serial_port_error() {
        let error = SerialBevyError::serial_port("Connection refused");
        assert!(error.to_string().contains("Connection refused"));
    }

    #[test]
    fn test_port_open_error() {
        let error = SerialBevyError::port_open("/dev/ttyUSB0", "Permission denied");
        let msg = error.to_string();
        assert!(msg.contains("/dev/ttyUSB0"));
        assert!(msg.contains("Permission denied"));
    }

    #[test]
    fn test_channel_error() {
        let error = SerialBevyError::channel("Receiver dropped");
        assert!(error.to_string().contains("Receiver dropped"));
    }

    #[test]
    fn test_encoding_error() {
        let error = SerialBevyError::encoding("Invalid hex string");
        assert!(error.to_string().contains("Invalid hex string"));
    }
}
