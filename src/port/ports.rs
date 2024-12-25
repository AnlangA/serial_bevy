use log::{error, info};

use tokio::{
    io::{self, AsyncReadExt, AsyncWriteExt},
    sync::{broadcast, mpsc},
    time::Duration,
};

pub use tokio_serial::{
    available_ports, DataBits, FlowControl, Parity, 
    SerialPort, SerialStream, StopBits,SerialPortBuilderExt
};

use bevy::prelude::*;

/// serial port baud rate
pub const COMMON_BAUD_RATES: &[u32] = &[
    4800, 9600, 19200, 38400, 57600, 115200, 230400, 460800, 500000, 576000, 921600, 1000000,
    1500000, 2000000,
];

/// serial port settings
#[derive(Clone, Debug)]
pub struct PortSettings {
    pub port_name: String,
    pub baud_rate: u32,
    pub databits: DataBits,
    pub stop_bits: StopBits,
    pub parity: Parity,
    pub flow_control: FlowControl,
    pub timeout: Duration,
}

impl PortSettings {
    /// serial port settings initialization
    pub fn new() -> Self {
        PortSettings {
            port_name: String::from("请选择一个串口"),
            baud_rate: 115200,
            databits: DataBits::Eight,
            stop_bits: StopBits::One,
            parity: Parity::None,
            flow_control: FlowControl::None,
            timeout: Duration::from_micros(500),
        }
    }
    /// serial port settings copy
    pub fn config(&mut self, port_settings: &mut PortSettings) {
        self.port_name = port_settings.port_name.clone();
        self.baud_rate = port_settings.baud_rate;
        self.databits = port_settings.databits;
        self.stop_bits = port_settings.stop_bits;
        self.parity = port_settings.parity;
        self.flow_control = port_settings.flow_control;
        self.timeout = port_settings.timeout;
    }
    /// get mutable serial port name
    pub fn port_name(&mut self) -> &mut String {
        &mut self.port_name
    }
    /// get mutable serial port baud rate
    pub fn baud_rate(&mut self) -> &mut u32 {
        &mut self.baud_rate
    }
    /// get mutable serial port data bits
    pub fn data_size(&mut self) -> &mut DataBits {
        &mut self.databits
    }
    /// get mutable serial port stop bits
    pub fn stop_bits(&mut self) -> &mut StopBits {
        &mut self.stop_bits
    }
    /// get mutable serial port parity
    pub fn parity(&mut self) -> &mut Parity {
        &mut self.parity
    }
    /// get mutable serial port flow control
    pub fn flow_control(&mut self) -> &mut FlowControl {
        &mut self.flow_control
    }
    /// get mutable serial port timeout
    pub fn timeout(&mut self) -> &mut Duration {
        &mut self.timeout
    }
    /// get serial port data bits name
    pub fn databits_name(&self) -> String {
        format!("{}", self.databits)
    }
    /// get serial port stop bits name
    pub fn stop_bits_name(&self) -> String {
        format!("{}", self.stop_bits)
    }
    /// get serial port parity name
    pub fn parity_name(&self) -> String {
        format!("{}", self.parity)
    }
    /// get serial port flow control name
    pub fn flow_control_name(&self) -> String {
        format!("{}", self.flow_control)
    }
}

/// serial port write and read data, used to communicate with different threads
#[derive(Clone, Debug)]
pub struct PorRWData {
    pub port_name: String,
    pub data: String,
}

/// serial port data, used to communicate with different threads
#[derive(Clone, Debug)]
pub enum PortChannelData {
    /// get all available serial ports
    PortName(Vec<String>),
    /// open serial port
    PortOpen(PortSettings),
    /// write data to serial port
    PortWrite(PorRWData),
    /// read data from serial port
    PortRead(PorRWData),
    /// close serial port
    PortClose,
    /// error
    PortError(PorRWData),
}
