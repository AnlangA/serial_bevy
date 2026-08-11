//! # Data Module
//!
//! This module provides channel-based communication for serial port operations.

use super::port::PortChannelData;
use bevy::prelude::*;
use tokio::sync::broadcast;

/// Channel resource for publishing serial-port discovery snapshots.
///
/// This resource manages bidirectional communication using broadcast channels.
#[derive(Resource)]
pub struct SerialNameChannel {
    /// Discovery task sender.
    pub tx_discovery: broadcast::Sender<PortChannelData>,
    /// ECS receiver for discovery updates.
    pub rx_discovery: broadcast::Receiver<PortChannelData>,
}

impl SerialNameChannel {
    /// Initializes the serial name channel with its bounded discovery buffer.
    #[must_use]
    pub fn init() -> Self {
        let (tx_discovery, rx_discovery) = broadcast::channel(16);
        Self {
            tx_discovery,
            rx_discovery,
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
        assert!(channel.tx_discovery.receiver_count() >= 1);
    }

    #[test]
    fn test_serial_name_channel_default() {
        let channel = SerialNameChannel::default();
        assert!(channel.tx_discovery.receiver_count() >= 1);
    }
}
