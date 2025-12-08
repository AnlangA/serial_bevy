# Serial Bevy

[中文版](README_CN.md) | English

A modern serial port communication tool built with the Bevy game engine, providing an intuitive GUI for serial port operations.

## Features

- **Automatic Port Discovery**: Automatically detects and lists available serial ports
- **Full Serial Configuration**: 
  - Configurable baud rate (4800 - 2000000 bps)
  - Data bits (5, 6, 7, 8)
  - Stop bits (1, 2)
  - Parity (None, Odd, Even)
  - Flow control (None, Software, Hardware)
  - Adjustable timeout settings
- **Multiple Data Encodings**: Support for Hex and UTF-8 data formats
- **Command History**: Navigate previous commands using arrow keys (↑/↓)
- **Data Logging**: Automatic timestamped logging of all communications
- **LLM Integration**: Optional AI assistant features for data analysis
- **Resizable Panels**: Customizable UI layout with persistent panel widths

## Installation

### Prerequisites

- Rust 1.70 or later
- Cargo package manager

### Build from Source

```bash
# Clone the repository
git clone https://github.com/AnlangA/serial_bevy.git
cd serial_bevy

# Build the project
cargo build --release

# Run the application
cargo run --release
```

## Usage

### Opening a Serial Port

1. Launch the application
2. Select a port from the left panel
3. Configure the port settings (baud rate, data bits, etc.)
4. Click "Open" to establish connection

### Sending Data

1. Select the data type (Hex or UTF-8)
2. Type your message in the input area
3. Press Enter to send
4. Use "With LF"/"No LF" button to toggle line feed

### Viewing Logs

All communications are automatically logged to the `logs/` directory with timestamps. The current session's data is displayed in the central panel.

### LLM Features

Click "Enable LLM" to access AI-powered features in the right sidebar (when enabled).

## Configuration

Port settings can be adjusted in the left panel:
- **Baud Rate**: Communication speed
- **Data Bits**: Number of data bits per character
- **Stop Bits**: Number of stop bits
- **Parity**: Error checking method
- **Flow Ctrl**: Flow control mechanism

Panel widths are automatically saved to `panel_widths.txt` and restored on next launch.

## Project Structure

```
serial_bevy/
├── src/
│   ├── main.rs           # Application entry point
│   ├── lib.rs            # Library root
│   ├── error.rs          # Error handling
│   ├── serial/           # Serial port logic
│   │   ├── mod.rs
│   │   ├── port.rs       # Port management
│   │   ├── data.rs       # Data handling
│   │   └── encoding.rs   # Data encoding
│   ├── serial_ui/        # User interface
│   │   ├── mod.rs        # UI layout
│   │   └── ui.rs         # UI components
│   └── fonts/            # Font configuration
├── assets/
│   ├── fonts/            # Font files
│   └── images/           # Image assets
└── logs/                 # Auto-generated log files
```

## Dependencies

- **bevy**: Game engine for UI and application framework
- **bevy_egui**: Immediate mode GUI integration
- **tokio**: Async runtime
- **tokio-serial**: Serial port communication
- **chrono**: Timestamp generation for logging
- **zhipuai-rs**: LLM integration (optional)

## Development

### Running Tests

```bash
cargo test
```

### Linting

```bash
cargo clippy
```

### Building for Release

```bash
cargo build --release
```

The optimized binary will be in `target/release/`.

## License

MIT

## Author

AnlangA

## Repository

https://github.com/AnlangA/serial_bevy

## Contributing

Contributions are welcome! Please feel free to submit issues and pull requests.
