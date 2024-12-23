use log::{error, info};
use tokio::{
    io::{self, AsyncReadExt, AsyncWriteExt},
    runtime::Handle,
    sync::{broadcast, mpsc},
    time::Duration,
};
use tokio_serial::SerialPortBuilderExt;
pub use tokio_serial::{
    available_ports, new, ClearBuffer, DataBits, Error, ErrorKind, FlowControl, Parity, SerialPort,
    SerialPortBuilder, SerialPortInfo, SerialPortType, SerialStream, StopBits, UsbPortInfo,
};
/// 串口波特率
pub const COMMON_BAUD_RATES: &[u32] = &[
    4800, 9600, 19200, 38400, 57600, 115200, 230400, 460800, 500000, 576000, 921600, 1000000,
    1500000, 2000000,
];
/// 串口设置
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
    /// 获取串口名称
    pub fn select_port_name(&mut self) -> &String {
        &self.port_name
    }
    /// 获可变串口名称
    pub fn select_port_name_mut(&mut self) -> &mut String {
        &mut self.port_name
    }
    /// 获取可变串口波特率
    pub fn select_port_baud_rate_mut(&mut self) -> &mut u32 {
        &mut self.baud_rate
    }
    /// 获取可变串口数据位
    pub fn select_port_data_size_mut(&mut self) -> &mut DataBits {
        &mut self.databits
    }
    /// 获取可变串口停止位
    pub fn select_port_stop_bits_mut(&mut self) -> &mut StopBits {
        &mut self.stop_bits
    }
    /// 获取可变串口奇偶校验
    pub fn select_port_parity_mut(&mut self) -> &mut Parity {
        &mut self.parity
    }
    /// 获取可变串口流控
    pub fn select_port_flow_control_mut(&mut self) -> &mut FlowControl {
        &mut self.flow_control
    }
    /// 获取串口数据位名称
    pub fn get_databits_name(&self) -> String {
        format!("{}", self.databits)
    }
    /// 获取串口停止位名称
    pub fn get_stop_bits_name(&self) -> String {
        format!("{}", self.stop_bits)
    }
    /// 获取串口奇偶校验名称
    pub fn get_parity_name(&self) -> String {
        format!("{}", self.parity)
    }
    /// 获取串口流控名称
    pub fn get_flow_control_name(&self) -> String {
        format!("{}", self.flow_control)
    }
    /// 串口设置初始化
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
    /// 串口设置复制
    pub fn config(&mut self, port_settings: &mut PortSettings) {
        self.port_name = port_settings.port_name.clone();
        self.baud_rate = port_settings.baud_rate;
        self.databits = port_settings.databits;
        self.stop_bits = port_settings.stop_bits;
        self.parity = port_settings.parity;
        self.flow_control = port_settings.flow_control;
        self.timeout = port_settings.timeout;
    }
    /// 获取串口名称
    pub fn get_port_name(&self) -> String {
        self.port_name.clone()
    }
    /// 获取串口波特率
    pub fn get_baud_rate(&mut self) -> &mut u32 {
        &mut self.baud_rate
    }
    /// 获取串口数据位
    pub fn get_databits(&mut self) -> &mut DataBits {
        &mut self.databits
    }
    /// 获取串口停止位
    pub fn get_stop_bits(&mut self) -> &mut StopBits {
        &mut self.stop_bits
    }
    /// 获取串口奇偶校验
    pub fn get_parity(&mut self) -> &mut Parity {
        &mut self.parity
    }
    /// 获取串口流控
    pub fn get_flow_control(&mut self) -> &mut FlowControl {
        &mut self.flow_control
    }
    /// 获取串口超时时间
    pub fn get_timeout(&mut self) -> &mut Duration {
        &mut self.timeout
    }
}

/// 与egui交互的通道数据
#[derive(Clone, Debug)]
pub enum PortData {
    PortName(Vec<String>),
    PortOpen(PortSettings),
    PortWrite(String),
    PortRead(String),
    PortClose,
    PortError(String),
}
/// 串口数据处理内部指令，用于通知tokio线程进行对应的串口操作
#[derive(Clone)]
pub enum PortInner {
    Write(String),
    Close,
}
/// 串口数据处理任务
pub async fn port_deal(tx: mpsc::Sender<PortData>, tx1: broadcast::Sender<PortData>) {
    let list_ports_tx = tx.clone();
    let mut rx = tx1.subscribe();

    let _hand = tokio::spawn(async move {
        list_ports(list_ports_tx).await;
    });

    let (handle_tx, _) = broadcast::channel(32);
    let port_inner_tx = handle_tx.clone();
    while let Ok(data) = rx.recv().await {
        match data {
            PortData::PortOpen(settings) => {
                info!("Port open: {:#?}", settings);
                match open_port(settings.clone()).await {
                    Some(port_handle) => {
                        info!("串口打开成功");
                        //添加读串口数据的线程
                        let port_tx = tx.clone();
                        let inner_tx = port_inner_tx.clone();
                        port_read_wrire(port_handle, port_tx, inner_tx).await;
                    }
                    None => {
                        info!("串口打开失败");
                        let _ = tx
                            .send(PortData::PortError("串口打开失败".to_string()))
                            .await;
                    }
                };
            }
            PortData::PortWrite(data) => {
                let _ = port_inner_tx.send(PortInner::Write(data));
            }
            PortData::PortClose => {
                info!("关闭");
                let _ = port_inner_tx.send(PortInner::Close);
            }
            _ => {}
        }
    }
}
/// 串口列表,一直更新串口列表
pub async fn list_ports(tx: mpsc::Sender<PortData>) {
    loop {
        let port_names: Vec<String> = match available_ports() {
            Ok(ports) => ports.into_iter().map(|p| p.port_name).collect(),
            Err(e) => {
                info!("Error listing ports: {}", e);
                Vec::<String>::new()
            }
        };
        let _ = tx.send(PortData::PortName(port_names.clone())).await;
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}
/// 打开串口
async fn open_port(mut port_data: PortSettings) -> Option<SerialStream> {
    let mut port_settings = PortSettings::new();
    port_settings.config(&mut port_data);
    match tokio_serial::new(port_settings.port_name, port_settings.baud_rate)
        .data_bits(port_data.databits)
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
/// 创建三个协程
/// 1：接收串口数据
/// 2：发送串口数据
/// 3：关闭串口：收到关闭指令后关闭接收、发送协程，并结束自身协程
async fn port_read_wrire(
    port: SerialStream,
    port_data_tx: mpsc::Sender<PortData>,
    inner_tx: broadcast::Sender<PortInner>,
) {
    let (mut rec, mut send) = io::split(port);
    let handrec = tokio::spawn(async move {
        info!("Read port");
        let mut buffer: [u8; 1024] = [0; 1024];
        while let Ok(data) = rec.read(&mut buffer[..]).await {
            if data > 0 {
                let _ = port_data_tx
                    .send(PortData::PortRead(
                        String::from_utf8_lossy(&buffer[..data]).to_string(),
                    ))
                    .await;
            }
        }
    });

    let mut inner_rx = inner_tx.clone().subscribe();
    let handwrite = tokio::spawn(async move {
        info!("write port");
        while let Ok(data) = inner_rx.recv().await {
            match data {
                PortInner::Write(data) => {
                    let _ = send.write(data.as_bytes()).await;
                }
                _ => {}
            }
        }
    });

    let mut inner_close = inner_tx.clone().subscribe();
    tokio::spawn(async move {
        loop {
            if let Ok(data) = inner_close.recv().await {
                match data {
                    PortInner::Close => {
                        handrec.abort();
                        handwrite.abort();
                        info!("close port");
                        break;
                    }
                    _ => {}
                }
            }
        }
    });
}
