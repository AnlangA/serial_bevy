//! # Discovery Module
//!
//! Port discovery and tokio runtime management.

use bevy::prelude::*;
use log::{debug, error, warn};
use tokio_serial::available_ports;

use super::Serials;
use super::data::SerialNameChannel;
use super::selection::Selected;
use super::state::PortChannelData;

/// Tokio runtime resource for async operations.
///
/// This resource wraps the Tokio runtime to enable async operations
/// within the Bevy ECS framework.
#[derive(Resource)]
pub struct Runtime {
    /// The Tokio runtime instance.
    rt: tokio::runtime::Runtime,
}

impl Runtime {
    /// Creates a new Runtime instance.
    ///
    /// # Panics
    ///
    /// Panics if the Tokio runtime cannot be created.
    #[must_use]
    pub fn init() -> Self {
        Self {
            rt: tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime"),
        }
    }

    /// Spawns an async task on the runtime.
    pub fn spawn<F>(&self, future: F) -> tokio::task::JoinHandle<F::Output>
    where
        F: std::future::Future + Send + 'static,
        F::Output: Send + 'static,
    {
        self.rt.spawn(future)
    }
}

impl Default for Runtime {
    fn default() -> Self {
        Self::init()
    }
}

/// Spawns the port discovery background task.
pub fn spawn_port_discovery(channel: Res<SerialNameChannel>, runtime: Res<Runtime>) {
    let tx = channel.tx_world2_serial.clone();
    runtime.spawn(async move {
        debug!(
            "Starting port discovery task. Available ports: {:?}",
            available_ports()
        );
        let mut previous_names = Vec::new();
        loop {
            if let Some(port_names) = discover_ports()
                && port_names != previous_names
            {
                previous_names.clone_from(&port_names);
                if let Err(e) = tx.send(PortChannelData::PortName(port_names)) {
                    error!("Failed to send port names: {e:?}");
                    break;
                }
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        }
    });
}

/// Discovers available USB serial ports.
fn discover_ports() -> Option<Vec<String>> {
    match available_ports() {
        Ok(ports) => {
            let mut names: Vec<_> = ports.into_iter().map(|port| port.port_name).collect();
            names.sort_unstable();
            names.dedup();
            Some(names)
        }
        Err(e) => {
            warn!("Error listing ports: {e}");
            None
        }
    }
}

/// Updates the serial port names based on discovery results.
pub fn update_serial_port_names(
    mut channel: ResMut<SerialNameChannel>,
    mut serials: Query<&mut Serials>,
    mut selected: ResMut<Selected>,
) {
    let Ok(mut serials) = serials.single_mut() else {
        return;
    };

    let mut latest_names = None;
    loop {
        match channel.rx_serial2_world.try_recv() {
            Ok(PortChannelData::PortName(names)) => latest_names = Some(names),
            Ok(_) | Err(tokio::sync::broadcast::error::TryRecvError::Lagged(_)) => {}
            Err(
                tokio::sync::broadcast::error::TryRecvError::Empty
                | tokio::sync::broadcast::error::TryRecvError::Closed,
            ) => break,
        }
    }

    if let Some(port_names) = latest_names {
        serials.sync_discovered_ports(&port_names);

        // Auto-select the first port if no port is currently selected
        if selected.selected().is_empty()
            && let Some(first_port_name) = serials.first_port_name()
        {
            selected.select(&first_port_name);
        }
    }
}
