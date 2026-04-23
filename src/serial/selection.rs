use bevy::prelude::*;

/// Resource for tracking the currently selected serial port.
#[derive(Resource, Default)]
pub struct Selected {
    /// The name of the selected port.
    selected: String,
}

impl Selected {
    /// Returns true if the given port name is selected.
    #[must_use]
    pub fn is_selected(&self, port_name: &str) -> bool {
        self.selected == port_name
    }

    /// Selects the given port.
    pub fn select(&mut self, port_name: &str) {
        self.selected = port_name.to_string();
    }

    /// Returns the selected port name.
    #[must_use]
    pub fn selected(&self) -> &str {
        &self.selected
    }
}
