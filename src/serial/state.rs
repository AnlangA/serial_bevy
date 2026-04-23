//! # State Module
//!
//! This module provides serial port state management types including
//! port state, channel data for communication between threads, and data source identifiers.

use std::fmt;

use super::port::PortSettings;

/// Serial port connection state.
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

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_port_channel_data_conversion() {
        let data = PortChannelData::PortName(vec!["COM1".to_string(), "COM2".to_string()]);
        let names: Vec<String> = data.into();
        assert_eq!(names.len(), 2);

        let data = PortChannelData::PortOpen(PortSettings::default());
        let names: Vec<String> = data.into();
        assert!(names.is_empty());
    }
}
