use log::{error, info};
use std::fs::{File, OpenOptions};
use std::io::Write;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;
use tokio::time::Duration;
use tokio_serial::SerialPortBuilderExt;
pub use tokio_serial::{DataBits, FlowControl, Parity, SerialPort, SerialStream, StopBits};

/// serial port baud rate
pub const COMMON_BAUD_RATES: &[u32] = &[
    4800, 9600, 19200, 38400, 57600, 115200, 230400, 460800, 500000, 576000, 921600, 1000000,
    1500000, 2000000,
];

/// serial port
pub struct Serial {
    pub set: PortSettings,
    data: PortData,
    stream: Option<SerialStream>,
    thread_handle: Option<JoinHandle<()>>,
    tx_channel: Option<broadcast::Sender<PortChannelData>>,
    rx_channel: Option<broadcast::Receiver<PortChannelData>>,
}

/// serial port implementation
impl Serial {
    /// serial port initialization
    pub fn new() -> Self {
        Serial {
            set: PortSettings::new(),
            data: PortData::new(),
            stream: None,
            thread_handle: None,
            tx_channel: None,
            rx_channel: None,
        }
    }

    /// get port settings
    pub fn set(&self) -> &PortSettings {
        &self.set
    }

    /// get port data
    pub fn data(&mut self) -> &mut PortData {
        &mut self.data
    }

    /// get stream
    pub fn stream(&mut self) -> &mut Option<SerialStream> {
        &mut self.stream
    }

    /// get thread handle
    pub fn thread_handle(&mut self) -> &mut Option<JoinHandle<()>> {
        &mut self.thread_handle
    }

    /// get tx channel
    pub fn tx_channel(&mut self) -> &mut Option<broadcast::Sender<PortChannelData>> {
        &mut self.tx_channel
    }

    /// get rx channel
    pub fn rx_channel(&mut self) -> &mut Option<broadcast::Receiver<PortChannelData>> {
        &mut self.rx_channel
    }
}

/// serial port settings
#[derive(Clone, Debug)]
pub struct PortSettings {
    pub port_name: String,
    pub baud_rate: u32,
    pub data_bits: DataBits,
    pub stop_bits: StopBits,
    pub parity: Parity,
    pub flow_control: FlowControl,
    pub timeout: Duration,
}

/// serial port settings implementation
impl PortSettings {
    /// serial port settings initialization
    pub fn new() -> Self {
        PortSettings {
            port_name: String::from("请选择一个串口"),
            baud_rate: 115200,
            data_bits: DataBits::Eight,
            stop_bits: StopBits::One,
            parity: Parity::None,
            flow_control: FlowControl::None,
            timeout: Duration::from_micros(500),
        }
    }

    /// serial port settings copy
    pub fn config(&mut self, port_settings: &PortSettings) {
        self.port_name = port_settings.port_name.clone();
        self.baud_rate = port_settings.baud_rate;
        self.data_bits = port_settings.data_bits;
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
        &mut self.data_bits
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
        format!("{}", self.data_bits)
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

/// open serial port
pub async fn open_port(port_data: PortSettings) -> Option<SerialStream> {
    let mut port_settings = PortSettings::new();
    port_settings.config(&port_data);
    match tokio_serial::new(port_settings.port_name, port_settings.baud_rate)
        .data_bits(port_data.data_bits)
        .parity(port_data.parity)
        .stop_bits(port_data.stop_bits)
        .flow_control(port_data.flow_control)
        .timeout(port_data.timeout)
        .open_native_async()
    {
        Ok(stream) => {
            info!("成功打开串口: {}", port_data.port_name);
            Some(stream)
        }
        Err(e) => {
            error!("无法打开串口 {}: {}", port_data.port_name, e);
            None
        }
    }
}

/// serial port data
pub struct PortData {
    /// source file
    source_file: FileData,
    /// parse file
    parse_file: FileData,
    /// send data
    send_data: Vec<String>,
    /// serial port state
    state: State,
    /// serial port data type
    data_type: Type,
}

impl PortData {
    pub fn new() -> Self {
        PortData {
            source_file: FileData { file: vec![] },
            parse_file: FileData { file: vec![] },
            send_data: vec![],
            state: State::Close,
            data_type: Type::Utf8,
        }
    }

    /// add receive file and add it's index
    pub fn add_source_file(&mut self, name: String) -> usize {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(name)
            .unwrap();
        self.source_file.file.push(file);
        self.source_file.file.len()
    }

    /// get receive file index
    pub fn source_file_index(&self) -> usize {
        self.source_file.file.len()
    }

    /// write data to last source file
    pub fn write_source_file(&mut self, data: &[u8]) {
        let mut file = self.source_file.file.last().unwrap();
        file.write_all(data).unwrap();
        file.write_all(b"\n").unwrap();
    }

    /// add parse file and add it's index
    pub fn add_parse_file(&mut self, name: String) -> usize {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(name)
            .unwrap();
        self.parse_file.file.push(file);
        self.parse_file.file.len()
    }

    /// get parse file index
    pub fn parse_file_index(&self) -> usize {
        self.parse_file.file.len()
    }

    /// write data to last parse file
    pub fn write_parse_file(&mut self, data: &[u8]) {
        let mut file = self.parse_file.file.last().unwrap();
        file.write_all(data).unwrap();
    }

    /// add send data
    pub fn send_data(&mut self, data: String) {
        self.send_data.push(data);
    }

    /// get send data
    pub fn get_send_data(&mut self) -> Vec<String> {
        let data = self.send_data.clone();
        self.send_data.clear();
        data
    }

    /// clear send data
    pub fn clear_send_data(&mut self) {
        self.send_data.clear();
    }

    /// set state
    pub fn set_state(&mut self, state: State) {
        self.state = state;
    }

    /// set data type
    pub fn set_data_type(&mut self, data_type: Type) {
        self.data_type = data_type;
    }

    /// get state
    pub fn state(&self) -> &State {
        &self.state
    }

    /// get data type
    pub fn data_type(&self) -> &Type {
        &self.data_type
    }
}

/// file data
struct FileData {
    /// file
    file: Vec<File>,
}

/// serial port state
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    /// serial port is ready
    Ready,
    /// serial port is busy
    Close,
    /// serial port is error
    Error,
}

/// serial port data type
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Type {
    /// binary data
    Binary,
    /// hex data
    Hex,
    /// utf8 data
    Utf8,
    /// utf16 data
    Utf16,
    /// utf32 data
    Utf32,
    /// gbk data
    GBK,
    /// gb2312 data
    ASCII,
}

/// serial port write and read data, used to communicate with different threads
#[derive(Clone, Debug)]
pub struct PorRWData {
    /// data
    pub data: Vec<u8>,
}

/// serial port data, used to communicate with different threads
#[derive(Clone, Debug)]
pub enum PortChannelData {
    /// get all available serial ports
    PortName(Vec<String>),
    /// write data to serial port
    PortWrite(PorRWData),
    /// read data from serial port
    PortRead(PorRWData),
    /// open serial port
    PortOpen,
    /// close serial port
    PortClose(String),
    /// serial state
    PortState(State),
    /// error
    PortError(PorRWData),
}

/// convert PortChannelData to Vec<String>
impl Into<Vec<String>> for PortChannelData {
    fn into(self) -> Vec<String> {
        match self {
            PortChannelData::PortName(names) => names,
            _ => Vec::new(),
        }
    }
}
