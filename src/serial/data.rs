//! # Data Module
//!
//! This module provides channel-based communication for serial port operations.

use super::state::PortChannelData;
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

/// Response from an AI chat request.
#[derive(Clone, Debug)]
pub struct AiResponse {
    /// The port name associated with this request.
    pub port_name: String,
    /// The response content (AI message or error text).
    pub content: String,
    /// Whether this response represents an error.
    pub is_error: bool,
}

/// Channel resource for AI chat communication.
#[derive(Resource)]
pub struct AiChannel {
    /// Sender for AI responses from async tasks back to the Bevy world.
    pub tx: std::sync::Mutex<std::sync::mpsc::Sender<AiResponse>>,
    /// Receiver for AI responses in Bevy systems.
    pub rx: std::sync::Mutex<std::sync::mpsc::Receiver<AiResponse>>,
}

impl AiChannel {
    /// Initializes the AI channel.
    #[must_use]
    pub fn init() -> Self {
        let (tx, rx) = std::sync::mpsc::channel();
        Self {
            tx: std::sync::Mutex::new(tx),
            rx: std::sync::Mutex::new(rx),
        }
    }
}

impl Default for AiChannel {
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

    #[test]
    fn test_ai_channel_creation() {
        let channel = AiChannel::init();
        let response = AiResponse {
            port_name: "COM1".to_string(),
            content: "Hello".to_string(),
            is_error: false,
        };
        assert!(channel.tx.lock().unwrap().send(response).is_ok());
        let received = channel.rx.lock().unwrap().recv();
        assert!(received.is_ok());
        assert_eq!(received.unwrap().port_name, "COM1");
    }
}
