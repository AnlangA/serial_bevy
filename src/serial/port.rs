use log::{error, info};
use std::fmt;
use std::fs::OpenOptions;
use std::io::{BufReader, BufWriter, Read, Write};
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
    thread_handle: Option<JoinHandle<Result<(), Box<dyn std::error::Error + Send + Sync>>>>,
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
    pub fn thread_handle(
        &mut self,
    ) -> &mut Option<JoinHandle<Result<(), Box<dyn std::error::Error + Send + Sync>>>> {
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

    /// open serial port
    pub fn open(&mut self) {
        self.data.state().open();
    }

    /// is serial port open
    pub fn is_open(&mut self) -> bool {
        self.data.state().is_open()
    }

    /// close serial port
    pub fn close(&mut self) {
        self.data.state().close();
        self.thread_handle = None;
    }

    /// is serial port close
    pub fn is_close(&mut self) -> bool {
        self.data.state().is_close()
    }

    /// get error
    pub fn error(&mut self) {
        self.data.state().error();
    }

    /// is error
    pub fn is_error(&mut self) -> bool {
        self.data.state().is_error()
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

/// cache data
pub struct CacheData {
    pub history_data: Vec<String>,
    pub history_index: usize,
    pub current_data: String,
}

/// cache data implementation
impl CacheData {
    /// cache data initialization
    pub fn new() -> Self {
        CacheData {
            history_data: vec![],
            history_index: 0,
            current_data: String::new(),
        }
    }

    /// add history data
    pub fn add_history_data(&mut self, data: String) {
        match self.history_data.last(){
            Some(history_data) =>{
                if history_data.to_owned() == data {
                    return;
                }
            }
            None => {}
        }
        self.history_data.push(data);
        self.history_index = self.history_data.len();
    }

    /// add one to ['history_index']
    pub fn add_history_index(&mut self) -> usize {
        if self.history_index < self.history_data.len() {
            self.history_index = self.history_index + 1;
        }
        self.history_index
    }

    /// subtract one to ['history_index']
    pub fn sub_history_index(&mut self) -> usize {
        if self.history_index > 1usize {
            self.history_index = self.history_index - 1;
        }
        self.history_index
    }

    /// get history data index
    pub fn get_current_data_index(&self) -> usize {
        self.history_index
    }

    /// get history data
    pub fn get_history_data(&mut self, index: usize) -> String {
        if self.history_data.len() == 0usize {
            let no_history = String::new();
            no_history
        } else {
            if index >= self.history_data.len() {
                self.history_index = self.history_data.len();
            } else {
                self.history_index = index;
            }
            self.history_data[self.history_index - 1].clone()
        }
    }

    /// get current data
    pub fn get_current_data(&mut self) -> &mut String {
        &mut self.current_data
    }

    /// clear current data
    pub fn clear_current_data(&mut self) {
        self.current_data.clear();
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
    /// cache data
    cache_data: CacheData,
    /// serial port state
    state: State,
    /// serial port data type
    data_type: Type,
    /// line feed
    line_feed: bool,
}

impl PortData {
    pub fn new() -> Self {
        PortData {
            source_file: FileData { file: vec![] },
            parse_file: FileData { file: vec![] },
            send_data: vec![],
            cache_data: CacheData::new(),
            state: State::Close,
            data_type: Type::Utf8,
            line_feed: false,
        }
    }

    /// add receive file and add it's index
    pub fn add_source_file(&mut self, name: String) -> usize {
        let _ = OpenOptions::new()
            .create(true)
            .write(true)
            .read(true)
            .append(true)
            .open(name.clone())
            .unwrap();
        self.source_file.file.push(name);
        self.source_file.file.len()
    }

    /// get receive file index
    pub fn source_file_index(&self) -> usize {
        self.source_file.file.len()
    }

    /// write data to last source file
    pub fn write_source_file(&mut self, data: &[u8], source: DataSource) {
        let time = chrono::Local::now()
            .format("%Y-%m-%d %H:%M:%S.%3f")
            .to_string();
        let source = source.to_string();
        let head = format!("[{}-{}]", time, source);
        let file = self.source_file.file.last().unwrap();
        let file = OpenOptions::new()
            .write(true)
            .append(true)
            .open(file)
            .unwrap();
        let mut write = BufWriter::new(&file);
        let mut combined = Vec::new();
        combined.extend_from_slice(head.as_bytes());
        combined.extend_from_slice(data);
        write.write_all(b"\n").unwrap();
        write.write_all(&combined).unwrap();
        write.flush().unwrap();
    }

    /// read current source file
    pub fn read_current_source_file(&mut self) -> String {
        match self.source_file.file.last() {
            Some(file) => {
                let file = OpenOptions::new().read(true).open(file).unwrap();
                let mut data = String::new();
                let mut reader = BufReader::new(&file);
                reader.read_to_string(&mut data).unwrap();
                data
            }
            None => String::new(),
        }
    }

    /// read source file
    pub fn read_source_file(&self, index: usize) -> String {
        match self.source_file.file.get(index) {
            Some(file) => {
                let mut file = OpenOptions::new().read(true).open(file).unwrap();
                let mut data = String::new();
                file.read_to_string(&mut data).unwrap();
                data
            }
            None => String::new(),
        }
    }

    /// get source file name
    pub fn get_source_file_name(&self, index: usize) -> &str {
        &self.source_file.file[index]
    }

    /// add parse file and add it's index
    pub fn add_parse_file(&mut self, name: String) -> usize {
        let _ = OpenOptions::new()
            .create(true)
            .append(true)
            .open(name.clone())
            .unwrap();
        self.parse_file.file.push(name);
        self.parse_file.file.len()
    }

    /// get parse file index
    pub fn parse_file_index(&self) -> usize {
        self.parse_file.file.len()
    }

    /// write data to last parse file
    pub fn write_parse_file(&mut self, data: &[u8]) {
        let file = self.parse_file.file.last().unwrap();
        let file = OpenOptions::new()
            .write(true)
            .append(true)
            .open(file)
            .unwrap();
        let mut write = BufWriter::new(&file);
        write.write_all(data).unwrap();
        write.write_all(b"\n").unwrap();
        write.flush().unwrap();
    }

    /// read current parse file
    pub fn read_current_parse_file(&mut self) -> String {
        match self.parse_file.file.last() {
            Some(file) => {
                let mut file = OpenOptions::new().read(true).open(file).unwrap();
                let mut data = String::new();
                file.read_to_string(&mut data).unwrap();
                data
            }
            None => String::new(),
        }
    }

    /// get parse file name
    pub fn get_parse_file_name(&self, index: usize) -> &str {
        &self.parse_file.file[index]
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

    /// set data type
    pub fn set_data_type(&mut self, data_type: Type) {
        self.data_type = data_type;
    }

    /// get cache data
    pub fn get_cache_data(&mut self) -> &mut CacheData {
        &mut self.cache_data
    }

    /// get state
    pub fn state(&mut self) -> &mut State {
        &mut self.state
    }

    /// get data type
    pub fn data_type(&mut self) -> &mut Type {
        &mut self.data_type
    }

    /// get line feed
    pub fn line_feed(&mut self) -> &mut bool {
        &mut self.line_feed
    }
}

/// file data
struct FileData {
    /// file
    file: Vec<String>,
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

impl State {
    /// serial port is open
    pub fn is_open(&self) -> bool {
        matches!(self, State::Ready)
    }

    /// serial port is close
    pub fn is_close(&self) -> bool {
        matches!(self, State::Close)
    }

    /// is error
    pub fn is_error(&self) -> bool {
        matches!(self, State::Error)
    }

    /// open serial port
    pub fn open(&mut self) {
        *self = State::Ready;
    }

    /// close serial port
    pub fn close(&mut self) {
        *self = State::Close;
    }

    /// set error
    pub fn error(&mut self) {
        *self = State::Error;
    }
}

/// serial port data type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

/// 实现 Display trait 用于友好的字符串表示
impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Type::Binary => write!(f, "二进制"),
            Type::Hex => write!(f, "十六进制"),
            Type::Utf8 => write!(f, "UTF-8"),
            Type::Utf16 => write!(f, "UTF-16"),
            Type::Utf32 => write!(f, "UTF-32"),
            Type::GBK => write!(f, "GBK"),
            Type::ASCII => write!(f, "ASCII"),
        }
    }
}

/// 可选：实现一个方法来获取英文描述
impl Type {
    pub fn as_str_en(&self) -> &'static str {
        match self {
            Type::Binary => "Binary",
            Type::Hex => "Hexadecimal",
            Type::Utf8 => "UTF-8",
            Type::Utf16 => "UTF-16",
            Type::Utf32 => "UTF-32",
            Type::GBK => "GBK",
            Type::ASCII => "ASCII",
        }
    }

    /// 获取编码描述
    pub fn description(&self) -> &'static str {
        match self {
            Type::Binary => "二进制数据格式",
            Type::Hex => "十六进制数据格式",
            Type::Utf8 => "UTF-8 文本编码",
            Type::Utf16 => "UTF-16 文本编码",
            Type::Utf32 => "UTF-32 文本编码",
            Type::GBK => "GBK 中文编码",
            Type::ASCII => "ASCII 文本编码",
        }
    }
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

/// data source
pub enum DataSource {
    /// write data
    Write,
    /// read data
    Read,
    /// error
    Error,
}

/// implement Display for DataSource
impl fmt::Display for DataSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DataSource::Write => write!(f, "写入"),
            DataSource::Read => write!(f, "读取"),
            DataSource::Error => write!(f, "错误"),
        }
    }
}
