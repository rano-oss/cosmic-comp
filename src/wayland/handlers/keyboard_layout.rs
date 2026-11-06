// SPDX-License-Identifier: GPL-3.0-only

use smithay::input::keyboard::KeyboardHandle;

use crate::state::State;
use crate::wayland::handlers::input_method::sync_input_method_with_layout;
use crate::wayland::protocols::keyboard_layout::{KeyboardLayoutHandler, KeyboardLayoutState};

impl KeyboardLayoutHandler for State {
    fn keyboard_layout_state(&mut self) -> &mut KeyboardLayoutState {
        &mut self.common.keyboard_layout_state
    }

    fn group_changed(&mut self, keyboard: &KeyboardHandle<Self>) {
        self.broadcast_ei_keyboard_modifiers(keyboard);
        let seat = self
            .common
            .shell
            .read()
            .seats
            .iter()
            .find(|seat| seat.get_keyboard().as_ref() == Some(keyboard))
            .cloned();
        if let Some(seat) = seat {
            let layout = self.common.config.cosmic_conf.xkb_config.layout.clone();
            sync_input_method_with_layout(self, &seat, &layout);
        }
    }
}
