//! # Serial Bevy
//!
//! A serial port communication tool built with the Bevy game engine.
//!
//! This crate provides a modular, plugin-based architecture for serial port
//! communication with an intuitive GUI interface.
//!
//! ## Features
//!
//! - **Plugin Architecture**: Highly modular design with separate plugins for
//!   serial communication and UI.
//! - **Async Serial Communication**: Non-blocking serial port operations using
//!   Tokio async runtime.
//! - **Data Encoding**: Support for Hex and UTF-8 data encoding/decoding.
//! - **History Management**: Command history with navigation support.
//! - **AI Integration**: Optional LLM integration for intelligent assistance.
//!
//! ## Architecture
//!
//! The project is organized into the following modules:
//!
//! - [`serial`]: Core serial port communication functionality
//! - [`serial_ui`]: User interface components for serial communication
//! - [`error`]: Custom error types for the application

#![allow(clippy::mut_mutex_lock)]

pub mod error;
pub mod serial;
pub mod serial_ui;

/// Re-exports for convenience
pub mod prelude {
    pub use crate::error::*;
    pub use crate::serial::SerialPlugin;
    pub use crate::serial_ui::SerialUiPlugin;
}
