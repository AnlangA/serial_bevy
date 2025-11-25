//! # Data Module
//!
//! This module provides channel-based communication for serial port operations.

use super::port::PortChannelData;
use bevy::prelude::*;
use tokio::sync::broadcast;

/// Channel resource for communication between the main app and serial port threads.
///
/// This resource manages bidirectional communication using broadcast channels.
#[derive(Resource)]
pub struct SerialNameChannel {
    /// Sender for messages from the world to the serial thread.
    pub tx_world2_serial: broadcast::Sender<PortChannelData>,
    /// Receiver for messages from the serial thread to the world.
    pub rx_serial2_world: broadcast::Receiver<PortChannelData>,
}

impl SerialNameChannel {
    /// Initializes the serial name channel with a buffer size of 100.
    #[must_use]
    pub fn init() -> Self {
        let (tx_world2_serial, rx_serial2_world) = broadcast::channel(100);
        Self {
            tx_world2_serial,
            rx_serial2_world,
        }
    }
}

impl Default for SerialNameChannel {
    fn default() -> Self {
        Self::init()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serial_name_channel_creation() {
        let channel = SerialNameChannel::init();
        // Verify that channels are created properly by checking sender capacity
        assert!(channel.tx_world2_serial.receiver_count() >= 1);
    }

    #[test]
    fn test_serial_name_channel_default() {
        let channel = SerialNameChannel::default();
        assert!(channel.tx_world2_serial.receiver_count() >= 1);
    }
}
