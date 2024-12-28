use super::port::PortChannelData;
use bevy::prelude::*;
use std::default::Default;
use tokio::sync::broadcast;

/// serial channel
#[derive(Resource)]
pub struct SerialNameChannel {
    pub tx_world2_serial: broadcast::Sender<PortChannelData>,
    pub rx_serial2_world: broadcast::Receiver<PortChannelData>,
}

/// serial channel implementation
impl SerialNameChannel {
    pub fn init() -> Self {
        let (tx_world2_serial, rx_serial2_world) = broadcast::channel(100);
        Self {
            tx_world2_serial,
            rx_serial2_world,
        }
    }
}

/// serial channel default implementation
impl Default for SerialNameChannel {
    fn default() -> Self {
        Self::init()
    }
}
