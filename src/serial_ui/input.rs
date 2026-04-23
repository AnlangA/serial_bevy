use bevy::prelude::*;
use bevy_egui::EguiContexts;

use crate::serial::{Selected, Serials};

use super::ui::submit_serial_input;

/// System: send cached data if newline present (user pressed Enter).
pub fn send_cache_data(mut serials: Query<&mut Serials>) {
    let Ok(mut serials) = serials.single_mut() else {
        return;
    };
    for serial in &mut serials.serial {
        let Ok(mut serial) = serial.lock() else {
            continue;
        };
        if serial.is_open() {
            let should_submit = {
                let current = serial.data().get_cache_data().get_current_data();
                current.contains('\r') || current.contains('\n')
            };
            if should_submit {
                submit_serial_input(&mut serial);
            }
        }
    }
}

/// System: navigate cached input history with Up/Down arrows for current open port.
pub fn history_data_checkout(
    mut serials: Query<&mut Serials>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    selected: Res<Selected>,
    mut contexts: EguiContexts,
) {
    if let Ok(ctx) = contexts.ctx_mut()
        && ctx.wants_keyboard_input()
    {
        return;
    }

    let Ok(mut serials) = serials.single_mut() else {
        return;
    };
    for serial in &mut serials.serial {
        let Ok(mut serial) = serial.lock() else {
            continue;
        };
        if selected.is_selected(&serial.set.port_name) && serial.is_open() {
            if keyboard_input.just_pressed(KeyCode::ArrowUp) {
                serial.data().get_cache_data().sub_history_index();
                let index = serial.data().get_cache_data().get_current_data_index();
                *serial.data().get_cache_data().get_current_data() =
                    serial.data().get_cache_data().get_history_data(index);
            }
            if keyboard_input.just_pressed(KeyCode::ArrowDown) {
                serial.data().get_cache_data().add_history_index();
                let index = serial.data().get_cache_data().get_current_data_index();
                *serial.data().get_cache_data().get_current_data() =
                    serial.data().get_cache_data().get_history_data(index);
            }
        }
    }
}
