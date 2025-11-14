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
//! input method app_ids using the configuration file loaded from the cosmic-config
//! directory (typically /usr/share/cosmic/com.system76.CosmicComp/v1/input_method_keyboard_map).
//!
//! The `InputMethodKeyboardMap` utility provides access to this mapping.

use crate::state::State;
use serde::{Deserialize, Serialize};
use smithay::{
    delegate_input_method_manager,
    desktop::{PopupKind, PopupManager, space::SpaceElement},
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::Rectangle,
    wayland::input_method::{InputMethodHandler, PopupSurface},
};
use std::collections::HashMap;
use std::fs;
use tracing::{debug, error, warn};

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
                        debug!(
                            "Loaded input method keyboard map from {:?} with {} entries",
                            config_path,
                            map.len()
                        );
                        return Self(map);
                    }
                    Err(err) => {
                        error!("Failed to parse {:?}: {}", config_path, err);
                    }
                }
            }
        }
    }

    /// Get the user config path for the input method keyboard map
    fn get_user_config_path() -> Option<std::path::PathBuf> {
        std::env::var("HOME").ok().map(|home| {
            std::path::PathBuf::from(home)
                .join(".config/cosmic/com.system76.CosmicComp/v1/input_method_keyboard_map")
        })
    }

    /// Get the system config path for the input method keyboard map
    fn get_system_config_path() -> Option<std::path::PathBuf> {
        Some(std::path::PathBuf::from(
            "/usr/share/cosmic/com.system76.CosmicComp/v1/input_method_keyboard_map",
        ))
    }

    /// Get the input method app_id for a given keyboard layout
    /// Returns None if the layout is not in the configuration
    pub fn get_app_id(&self, layout: &str) -> Option<&str> {
        self.0.get(layout).map(|s| s.as_str())
    }
}
