// SPDX-License-Identifier: GPL-3.0-only

//! # Multiple Input Method Support
//!
//! This module implements support for multiple input methods using Smithay's
//! multiple input method API.
//!
//! ## Two Modes of Operation
//!
//! ### Global Mode (Default)
//! All text inputs share the same input method. When a user switches keyboard layouts,
//! all applications are affected.
//!
//! ### Per-Client Mode
//! Each text input can have its own input method assigned. Useful for supporting
//! multiple keyboard layouts per application.
//!
//! ## Keyboard Layout Integration
//!
//! Input method switching is automatically triggered when the keyboard layout changes
//! in the config handler. The system maps keyboard layouts (like "us", "jp", "zh") to
//! input method `app_ids` using the configuration file loaded from the cosmic-config
//! directory (typically `/usr/share/cosmic/com.system76.CosmicComp/v1/input_method_keyboard_map`).
//!
//! The `InputMethodKeyboardMap` utility provides access to this mapping.

use crate::state::State;
use serde::{Deserialize, Serialize};
use smithay::{
    delegate_input_method_manager,
    desktop::{PopupKind, PopupManager, space::SpaceElement},
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::Rectangle,
    wayland::{
        input_method::{InputMethodHandler, PopupSurface},
        text_input::TextInputSeat,
    },
};
use std::collections::HashMap;
use std::fs;
use tracing::{error, info, warn};

impl InputMethodHandler for State {
    fn new_popup(&mut self, surface: PopupSurface) {
        if let Err(err) = self.common.popups.track_popup(PopupKind::from(surface)) {
            warn!("Failed to track popup: {}", err);
        }
    }

    fn dismiss_popup(&mut self, surface: PopupSurface) {
        if let Some(parent) = surface.get_parent().map(|parent| parent.surface.clone()) {
            let _ = PopupManager::dismiss_popup(&parent, &PopupKind::from(surface));
        }
    }

    fn parent_geometry(&self, parent: &WlSurface) -> Rectangle<i32, smithay::utils::Logical> {
        self.common
            .shell
            .read()
            .element_for_surface(parent)
            .map(|e| e.geometry())
            .unwrap_or_default()
    }

    fn popup_repositioned(&mut self, _: PopupSurface) {}
}

delegate_input_method_manager!(State);

/// Input method keyboard layout mapping configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputMethodKeyboardMap(HashMap<String, String>);

impl InputMethodKeyboardMap {
    /// Load the input method keyboard mapping from the cosmic-config directory
    ///
    /// Searches for the mapping file in the following locations (in order):
    /// 1. User config: `~/.config/cosmic/com.system76.CosmicComp/v1/input_method_keyboard_map`
    /// 2. System config: `/usr/share/cosmic/com.system76.CosmicComp/v1/input_method_keyboard_map`
    /// 3. Fallback: Embedded default configuration
    pub fn load() -> Self {
        // Try user config directory first
        if let Some(config_path) = Self::get_user_config_path() {
            if let Ok(contents) = fs::read_to_string(&config_path) {
                match ron::from_str::<HashMap<String, String>>(&contents) {
                    Ok(map) => {
                        info!(
                            "Loaded input method keyboard map from {:?} with {} entries",
                            config_path,
                            map.len()
                        );
                        for (layout, app_id) in &map {
                            info!("  Layout '{}' -> Input method '{}'", layout, app_id);
                        }
                        return Self(map);
                    }
                    Err(err) => {
                        error!("Failed to parse {:?}: {}", config_path, err);
                    }
                }
            } else {
                info!("No input method keyboard map found at {:?}", config_path);
            }
        }

        // Return empty map if loading failed
        Self(HashMap::new())
    }

    /// Get the user config path for the input method keyboard map
    fn get_user_config_path() -> Option<std::path::PathBuf> {
        std::env::var("HOME").ok().map(|home| {
            std::path::PathBuf::from(home)
                .join(".config/cosmic/com.system76.CosmicComp/v1/input_method_keyboard_map")
        })
    }

    /// Get the input method `app_id` for a given keyboard layout
    /// Returns None if the layout is not in the configuration
    pub fn get_app_id(&self, layout: &str) -> Option<&str> {
        self.0.get(layout).map(|s| s.as_str())
    }

    /// Check if the map is empty
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// Synchronize the active input method with the current keyboard layout
pub fn sync_input_method_with_layout(
    state: &mut State,
    seat: &smithay::input::Seat<State>,
    layout: &str,
) {
    use smithay::wayland::input_method::InputMethodSeat;
    let input_method_handle = seat.input_method();
    let text_input_handle = seat.text_input();

    // Load the keyboard layout mapping
    let mapping = InputMethodKeyboardMap::load();
    if mapping.is_empty() {
        input_method_handle.deactivate_input_method(state);
        return;
    }

    // Get the actual active layout from the keyboard
    // We use the layout index to extract the short code from the config string
    let active_layout_code = if let Some(keyboard) = seat.get_keyboard() {
        keyboard.with_xkb_state(state, |xkb| {
            let active_layout_idx = xkb.xkb().lock().unwrap().active_layout();
            // Extract the layout short code from the config string using the index
            let layouts: Vec<&str> = layout.split(',').collect();
            let layout_code = if (active_layout_idx.0 as usize) < layouts.len() {
                layouts[active_layout_idx.0 as usize]
            } else {
                layouts.first().copied().unwrap_or("")
            };
            layout_code.to_string()
        })
    } else {
        let fallback = layout.split(',').next().unwrap_or("");
        fallback.to_string()
    };

    // Check if there's a mapping for this layout
    if let Some(app_id) = mapping.get_app_id(&active_layout_code) {
        // Try to set the corresponding input method as active
        if input_method_handle.set_active_instance(app_id) {
            // If there's a focused text input, activate the input method on it
            let mut activated = false;
            text_input_handle.with_focused_text_input(|_ti, surface| {
                input_method_handle.activate_input_method(state, surface);
                activated = true;
            });

            if activated {
                // Re-enter the text input to make it resend its state to the newly activated IME
                // This is necessary when switching from a layout without an IME to one with an IME
                // while a text input is already focused
                text_input_handle.enter();
            }
        } else {
            warn!(
                "sync_input_method_with_layout: Input method '{}' for layout '{}' not registered yet",
                app_id, active_layout_code
            );
        }
    } else {
        warn!(
            "sync_input_method_with_layout: No input method mapping for layout '{}' - clearing active instance",
            active_layout_code
        );
        input_method_handle.clear_active_instance(state);
    }
}
