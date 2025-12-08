# Serial Bevy

中文版 | [English](README.md)

一个基于 Bevy 游戏引擎构建的现代化串口通信工具，提供直观的图形用户界面进行串口操作。

## 功能特性

- **自动端口发现**：自动检测并列出可用的串口
- **完整的串口配置**：
  - 可配置波特率（4800 - 2000000 bps）
  - 数据位（5、6、7、8）
  - 停止位（1、2）
  - 校验位（无、奇校验、偶校验）
  - 流控制（无、软件流控、硬件流控）
  - 可调节的超时设置
- **多种数据编码**：支持十六进制和 UTF-8 数据格式
- **命令历史**：使用方向键（↑/↓）导航历史命令
- **数据日志**：自动记录所有通信数据并添加时间戳
- **LLM 集成**：可选的 AI 助手功能，用于数据分析
- **可调整面板**：可自定义的 UI 布局，面板宽度持久化保存

## 安装

### 前置要求

- Rust 1.70 或更高版本
- Cargo 包管理器

### 从源码构建

```bash
# 克隆仓库
git clone https://github.com/AnlangA/serial_bevy.git
cd serial_bevy

# 构建项目
cargo build --release

# 运行应用程序
cargo run --release
```

## 使用方法

### 打开串口

1. 启动应用程序
2. 从左侧面板选择一个端口
3. 配置端口设置（波特率、数据位等）
4. 点击 "Open" 建立连接

### 发送数据

1. 选择数据类型（Hex 或 UTF-8）
2. 在输入区域输入您的消息
3. 按 Enter 键发送
4. 使用 "With LF"/"No LF" 按钮切换是否添加换行符

### 查看日志

所有通信数据都会自动记录到 `logs/` 目录，并添加时间戳。当前会话的数据显示在中央面板中。

### LLM 功能

点击 "Enable LLM" 以访问右侧边栏中的 AI 功能（启用时）。

## 配置

端口设置可以在左侧面板中调整：
- **Baud Rate**（波特率）：通信速度
- **Data Bits**（数据位）：每个字符的数据位数
- **Stop Bits**（停止位）：停止位数量
- **Parity**（校验位）：错误检查方法
- **Flow Ctrl**（流控制）：流控制机制

面板宽度会自动保存到 `panel_widths.txt` 文件，下次启动时恢复。

## 项目结构

```
serial_bevy/
├── src/
│   ├── main.rs           # 应用程序入口
│   ├── lib.rs            # 库根文件
│   ├── error.rs          # 错误处理
│   ├── serial/           # 串口逻辑
│   │   ├── mod.rs
│   │   ├── port.rs       # 端口管理
│   │   ├── data.rs       # 数据处理
│   │   └── encoding.rs   # 数据编码
│   ├── serial_ui/        # 用户界面
│   │   ├── mod.rs        # UI 布局
│   │   └── ui.rs         # UI 组件
│   └── fonts/            # 字体配置
├── assets/
│   ├── fonts/            # 字体文件
│   └── images/           # 图片资源
└── logs/                 # 自动生成的日志文件
```

## 依赖项

- **bevy**：用于 UI 和应用框架的游戏引擎
- **bevy_egui**：即时模式 GUI 集成
- **tokio**：异步运行时
- **tokio-serial**：串口通信
- **chrono**：日志时间戳生成
- **zhipuai-rs**：LLM 集成（可选）

## 开发

### 运行测试

```bash
cargo test
```

### 代码检查

```bash
cargo clippy
```

### 构建发布版本

```bash
cargo build --release
```

优化后的二进制文件将位于 `target/release/` 目录中。

## 许可证

MIT

## 作者

AnlangA

## 仓库地址

https://github.com/AnlangA/serial_bevy

## 贡献

欢迎贡献！请随时提交问题和拉取请求。
