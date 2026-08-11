//! # Serial Module
//!
//! This module provides the core serial port communication functionality.
//! It includes:
//!
//! - Port discovery and management
//! - Async read/write operations
//! - Data encoding/decoding (Hex, UTF-8, etc.)
//! - Thread-safe communication channels
//! - LLM integration for AI-assisted chat

// ---------------------------------------------------------------------------
// Sub-modules
// ---------------------------------------------------------------------------
pub mod ai;
pub mod data;
pub mod data_types;
pub mod discovery;
pub mod encoding;
pub mod io;
pub mod llm;
pub mod port;
pub mod port_data;
pub mod selection;
pub mod state;

// ---------------------------------------------------------------------------
// Internal imports needed by this module's definitions
// ---------------------------------------------------------------------------
use std::sync::Mutex;

use bevy::prelude::*;

use ai::{process_ai_requests, receive_ai_responses};
use data::{AiChannel, SerialNameChannel};
use discovery::{Runtime, spawn_port_discovery, update_serial_port_names};
use io::{create_serial_port_threads, receive_serial_data, send_serial_data};

// ---------------------------------------------------------------------------
// Public re-exports – maintain backward compatibility for existing consumers
// ---------------------------------------------------------------------------
pub use encoding::*;
pub use port::*;
pub use selection::*;

// ---------------------------------------------------------------------------
// Serials – collection of Mutex‑protected Serial instances
// ---------------------------------------------------------------------------

/// Container for managing multiple serial ports.
///
/// This component holds a collection of serial port instances,
/// each protected by a mutex for thread-safe access.
#[derive(Component)]
pub struct Serials {
    /// Vector of mutex-protected serial port instances.
    pub serial: Vec<Mutex<Serial>>,
}

impl std::fmt::Debug for Serials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut debug = f.debug_list();
        for serial in &self.serial {
            if let Ok(s) = serial.lock() {
                debug.entry(&format!("{}: {}bps", s.set.port_name, s.set.baud_rate));
            }
        }
        debug.finish()
    }
}

impl Default for Serials {
    fn default() -> Self {
        Self::new()
    }
}

impl Serials {
    /// Creates a new empty Serials container.
    #[must_use]
    pub const fn new() -> Self {
        Self { serial: vec![] }
    }

    /// Adds a serial port to the container.
    pub fn add(&mut self, serial: Serial) {
        self.serial.push(Mutex::new(serial));
    }

    /// Synchronizes the managed serial ports with the currently discovered port names.
    pub fn sync_discovered_ports(&mut self, port_names: &[String]) {
        self.serial.retain(|port| {
            port.lock()
                .map(|serial| port_names.contains(&serial.set.port_name))
                .unwrap_or(false)
        });

        for name in port_names {
            let already_exists = self.serial.iter().any(|port| {
                port.lock()
                    .map(|serial| serial.set.port_name == *name)
                    .unwrap_or(false)
            });

            if !already_exists {
                let mut serial = Serial::new();
                serial.set.port_name = name.clone();
                self.add(serial);
            }
        }
    }

    /// Removes a serial port at the specified index.
    ///
    /// # Panics
    ///
    /// Panics if the index is out of bounds.
    pub fn remove(&mut self, index: usize) {
        self.serial.remove(index);
    }

    /// Gets a reference to the mutex-protected serial port at the specified index.
    ///
    /// # Panics
    ///
    /// Panics if the index is out of bounds.
    #[must_use]
    pub fn get(&self, index: usize) -> &Mutex<Serial> {
        &self.serial[index]
    }

    /// Returns the number of serial ports.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.serial.len()
    }

    /// Returns true if there are no serial ports.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.serial.is_empty()
    }

    /// Returns the first managed port name, if any.
    #[must_use]
    pub fn first_port_name(&self) -> Option<String> {
        self.serial.first().and_then(|serial| {
            serial
                .lock()
                .ok()
                .map(|serial| serial.set.port_name.clone())
        })
    }
}

// ---------------------------------------------------------------------------
// SerialPlugin – Bevy plugin that wires up the serial system
// ---------------------------------------------------------------------------

/// The main serial communication plugin.
///
/// This plugin provides:
/// - Serial port discovery
/// - Async read/write operations
/// - Port state management
/// - AI chat integration
#[derive(Default)]
pub struct SerialPlugin;

impl Plugin for SerialPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Runtime::init())
            .insert_resource(SerialNameChannel::init())
            .insert_resource(AiChannel::init())
            .add_systems(Startup, (init_serial_components, spawn_port_discovery))
            .add_systems(
                Update,
                (
                    update_serial_port_names,
                    create_serial_port_threads,
                    send_serial_data,
                    receive_serial_data,
                    process_ai_requests,
                    receive_ai_responses,
                )
                    .chain(),
            );
    }
}

/// Initializes the serial components by spawning a `Serials` entity.
fn init_serial_components(mut commands: Commands) {
    commands.spawn(Serials::new());
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serials_new() {
        let serials = Serials::new();
        assert!(serials.is_empty());
    }

    #[test]
    fn test_serials_add() {
        let mut serials = Serials::new();
        serials.add(Serial::new());
        assert_eq!(serials.len(), 1);
    }

    #[test]
    fn test_runtime_creation() {
        let runtime = Runtime::init();
        // Just verify it doesn't panic
        drop(runtime);
    }
}
