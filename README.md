# Serial Bevy

Serial Bevy is a cross-platform serial monitor built with Rust, Bevy, and egui. It provides automatic port discovery, configurable serial parameters, UTF-8 and hexadecimal I/O, command history, and persistent session logs.

## Features

- Discovers serial ports without blocking the UI.
- Configures baud rate, data bits, stop bits, parity, and flow control.
- Sends and receives UTF-8 text or hexadecimal bytes.
- Optionally appends an LF byte to each submitted command.
- Preserves split UTF-8 code points across serial reads.
- Writes complete session logs with optional timestamps and receive-burst separators. It prefers `logs/` in the working directory, then beside the executable, and finally uses the system temporary directory.
- Keeps only the latest 1 MiB of log text in the UI, so long sessions remain responsive.
- Bounds command input and pending sends, reporting backpressure without discarding the editor contents.
- Supports input history with the Up and Down arrow keys.
- Persists the resizable side-panel width in the platform user-config directory, with working-directory, executable-directory, and temporary-directory fallbacks.

## Build and run

Install Rust 1.95 or newer, then run:

```sh
cargo run --release
```

Linux builds require the development packages for udev, Wayland, and xkbcommon. On Ubuntu or Debian:

```sh
sudo apt-get install libudev-dev libwayland-dev libxkbcommon-dev pkg-config
```

The application loads `assets/fonts/STSong.ttf` at runtime, so keep the `assets` directory beside the executable when distributing it.

## Usage

1. Select a detected port in the left panel and configure it while closed.
2. Click **Open**. A new log file is created for that session.
3. Choose UTF-8 or Hexadecimal input, enter a command, then press Enter or click **Send**.
4. Enable **Append LF** when the target expects a trailing `0A` byte.
5. Set **Log Timeout** to insert a blank separator when receive bursts are farther apart than the chosen number of milliseconds. Set it to `0` to disable separators.

Hex input accepts whitespace, `_`, `,`, `:`, and `-` separators, plus optional `0x` prefixes. Invalid tokens are rejected with a visible error instead of being silently altered. An odd total number of hexadecimal digits is padded with a leading zero.

## Quality checks

```sh
cargo fmt --all -- --check
cargo +1.95.0 check --locked --all-targets
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-targets
RUSTDOCFLAGS="-D warnings" cargo doc --locked --no-deps
cargo audit
```

## License

Licensed under the MIT License. See [LICENSE](LICENSE).
