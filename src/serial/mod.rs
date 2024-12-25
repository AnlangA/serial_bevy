pub mod port;
use bevy::prelude::{Component, Plugin};
use port::Serial;



/// serial ports
#[derive(Component)]
pub struct Serials {
    pub serial: Vec<Serial>,
}

/// serial ports implementation
impl Serials {
    /// serial ports initialization
    pub fn new() -> Self {
        Serials {
            serial: vec![],
        }
    }

    /// add serial port
    pub fn add(&mut self, serial: Serial) {
        self.serial.push(serial);
    }

    /// remove serial port
    pub fn remove(&mut self, index: usize) {
        self.serial.remove(index);
    }

    /// get serial port
    pub fn get(&self, index: usize) -> &Serial {
        &self.serial[index]
    }
}
