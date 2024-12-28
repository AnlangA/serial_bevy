use super::port::PortChannelData;
use bevy::prelude::*;
use std::default::Default;
use tokio::sync::broadcast;

/// serial channel
#[derive(Resource)]
pub struct SerialChannel {
    pub tx_world2_serial: broadcast::Sender<PortChannelData>,
    pub rx_serial2_world: broadcast::Receiver<PortChannelData>,
    pub tx_serial2_world: broadcast::Sender<PortChannelData>,
    pub rx_world2_serial: broadcast::Receiver<PortChannelData>,
}

/// serial channel implementation
impl SerialChannel {
    pub fn init() -> Self {
        let (tx_world2_serial, rx_serial2_world) = broadcast::channel(100);
        let (tx_serial2_world, rx_world2_serial) = broadcast::channel(100);
        Self {
            tx_world2_serial,
            rx_serial2_world,
            tx_serial2_world,
            rx_world2_serial,
        }
    }
}

/// serial channel default implementation
impl Default for SerialChannel {
    fn default() -> Self {
        Self::init()
    }
}
