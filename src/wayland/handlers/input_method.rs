// SPDX-License-Identifier: GPL-3.0-only

use crate::state::{ClientState, State};
use serde::{Deserialize, Serialize};
use smithay::{
    delegate_input_method_manager, delegate_input_method_manager_v3,
    desktop::{PopupKind, PopupManager, space::SpaceElement},
    reexports::wayland_server::{Client, DisplayHandle, protocol::wl_surface::WlSurface},
    utils::Rectangle,
    wayland::{
        input_method::{InputMethodHandler, PopupSurface},
        input_method_v3::{
            InputMethodHandler as InputMethodV3Handler, PopupSurface as PopupSurfaceV3,
            PositionerState,
        },
    },
};
use std::collections::HashMap;
use std::fs;
use std::os::unix::io::AsRawFd;
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::process::Command;
use std::sync::Arc;
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

impl InputMethodV3Handler for State {
    fn new_popup(&mut self, surface: PopupSurfaceV3) {
        if let Err(err) = self.common.popups.track_popup(PopupKind::from(surface)) {
            warn!("Failed to track popup: {}", err);
        }
    }

    fn dismiss_popup(&mut self, surface: PopupSurfaceV3) {
        let parent = surface.get_parent().surface.clone();
        let _ = PopupManager::dismiss_popup(&parent, &PopupKind::from(surface));
    }

    fn popup_repositioned(&mut self, _: PopupSurfaceV3) {}

    fn popup_geometry(
        &self,
        parent: &WlSurface,
        _cursor: &Rectangle<i32, smithay::utils::Logical>,
        _positioner: &PositionerState,
    ) -> Rectangle<i32, smithay::utils::Logical> {
        self.common
            .shell
            .read()
            .element_for_surface(parent)
            .map(|e| e.geometry())
            .unwrap_or_default()
    }

    fn parent_geometry(&self, parent: &WlSurface) -> Rectangle<i32, smithay::utils::Logical> {
        self.common
            .shell
            .read()
            .element_for_surface(parent)
            .map(|e| e.geometry())
            .unwrap_or_default()
    }

    fn input_method_app_id(&self, client: &Client, _dh: &DisplayHandle) -> Option<String> {
        let client_state = client.get_data::<ClientState>()?;
        // Only compositor-launched IMEs (via socketpair) have this field set.
        client_state.input_method_app_id.clone()
    }
}

delegate_input_method_manager!(State);
delegate_input_method_manager_v3!(State);
smithay::delegate_keyboard_filter_manager_v1!(State);

// --- Input method keyboard layout mapping configuration ---

/// Entry in the input method keyboard map: app_id + command to launch
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputMethodEntry {
    /// The app_id used to identify this input method
    pub app_id: String,
    /// The command to launch the input method
    pub command: String,
}

/// Input method keyboard layout mapping configuration.
///
/// Maps keyboard layout codes to input method entries.
/// Stored at `~/.config/cosmic/com.system76.CosmicComp/v1/input_method_keyboard_map`
///
/// Format (RON):
/// ```ron
/// {
///     "zh": (app_id: "chewingwl", command: "/usr/bin/chewingwl"),
///     "jp": (app_id: "fcitx5", command: "/usr/bin/fcitx5"),
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputMethodKeyboardMap(HashMap<String, InputMethodEntry>);

impl InputMethodKeyboardMap {
    /// Load the input method keyboard mapping from the cosmic-config directory
    pub fn load() -> Self {
        if let Some(config_path) = Self::get_user_config_path() {
            if let Ok(contents) = fs::read_to_string(&config_path) {
                match ron::from_str::<HashMap<String, InputMethodEntry>>(&contents) {
                    Ok(map) => {
                        return Self(map);
                    }
                    Err(err) => {
                        error!("Failed to parse {:?}: {}", config_path, err);
                    }
                }
            } else {
                warn!("No input method keyboard map found at {:?}", config_path);
            }
        }

        Self(HashMap::new())
    }

    fn get_user_config_path() -> Option<std::path::PathBuf> {
        std::env::var("HOME").ok().map(|home| {
            std::path::PathBuf::from(home)
                .join(".config/cosmic/com.system76.CosmicComp/v1/input_method_keyboard_map")
        })
    }

    /// Get the entry for a given keyboard layout
    pub fn get_entry(&self, layout: &str) -> Option<&InputMethodEntry> {
        self.0.get(layout)
    }

    /// Check if the map is empty
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// Launch an input method process with an associated app_id.
///
/// Creates a Unix socketpair, inserts one end as a client with the given app_id
/// in its client state, and spawns the IME process with the other end as
/// `WAYLAND_SOCKET`.
pub fn launch_input_method(state: &mut State, app_id: &str, command: &str) {
    let (compositor_stream, client_stream) = match UnixStream::pair() {
        Ok(pair) => pair,
        Err(err) => {
            error!("Failed to create socketpair for IME '{}': {}", app_id, err);
            return;
        }
    };

    let mut new_state = state.new_client_state();
    new_state.input_method_app_id = Some(app_id.to_string());

    if let Err(err) = state
        .common
        .display_handle
        .insert_client(compositor_stream, Arc::new(new_state))
    {
        error!("Failed to insert IME client '{}': {}", app_id, err);
        return;
    }

    // Clear CLOEXEC on client fd so child inherits it
    let client_fd = client_stream.as_raw_fd();
    unsafe {
        let flags = libc::fcntl(client_fd, libc::F_GETFD);
        libc::fcntl(client_fd, libc::F_SETFD, flags & !libc::FD_CLOEXEC);
    }

    use std::fs::File;
    let log_file = File::create(format!("/tmp/ime-{}.log", app_id)).ok();

    let mut cmd = Command::new(command);
    cmd.env("WAYLAND_SOCKET", client_fd.to_string())
        .env_remove("WAYLAND_DISPLAY")
        .env("WAYLAND_DEBUG", "1")
        .env("RUST_LOG", "debug");
    if let Some(f) = log_file {
        let f2 = f.try_clone().unwrap();
        cmd.stdout(std::process::Stdio::from(f));
        cmd.stderr(std::process::Stdio::from(f2));
    }
    // SAFETY: pre_exec runs after fork, before exec in the child process.
    // Rust's Command closes all fds > 2 before exec (via close_range or /proc/self/fd).
    // We must re-clear CLOEXEC in the child to ensure our wayland socket fd survives exec.
    unsafe {
        cmd.pre_exec(move || {
            let flags = libc::fcntl(client_fd, libc::F_GETFD);
            if flags != -1 {
                libc::fcntl(client_fd, libc::F_SETFD, flags & !libc::FD_CLOEXEC);
            }
            Ok(())
        });
    }
    match cmd.spawn() {
        Ok(child) => {
            info!(
                "Launched IME '{}' (pid: {}) with command: {}",
                app_id,
                child.id(),
                command
            );
        }
        Err(err) => {
            error!("Failed to spawn IME '{}' ({}): {}", app_id, command, err);
        }
    }

    // Drop client_stream - child has inherited the fd
    drop(client_stream);
}

/// Launch all configured input methods at startup.
/// Call this after the display and seat are ready.
pub fn launch_all_input_methods(state: &mut State) {
    let mapping = InputMethodKeyboardMap::load();
    if mapping.is_empty() {
        return;
    }

    // Collect unique (app_id, command) pairs to avoid launching duplicates
    let mut launched: std::collections::HashSet<String> = std::collections::HashSet::new();
    for entry in mapping.0.values() {
        if launched.contains(&entry.app_id) {
            continue;
        }
        launched.insert(entry.app_id.clone());
        info!(
            "Eagerly launching IME '{}': {}",
            entry.app_id, entry.command
        );
        launch_input_method(state, &entry.app_id, &entry.command);
    }
}

/// Synchronize the active input method with the current keyboard layout.
///
/// Assumes all IMEs have been eagerly launched at startup.
/// Simply switches which instance is active based on the current layout.
pub fn sync_input_method_with_layout(
    state: &mut State,
    seat: &smithay::input::Seat<State>,
    layout: &str,
) {
    use smithay::wayland::input_method_v3::InputMethodSeat;
    use smithay::wayland::text_input::TextInputSeat;
    let input_method_handle = seat.input_method_v3();

    let mapping = InputMethodKeyboardMap::load();
    if mapping.is_empty() {
        input_method_handle.deactivate_input_method(state);
        return;
    }

    let active_layout_code = if let Some(keyboard) = seat.get_keyboard() {
        keyboard.with_xkb_state(state, |xkb| {
            let active_layout_idx = xkb.xkb().lock().unwrap().active_layout();
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

    if let Some(entry) = mapping.get_entry(&active_layout_code) {
        let app_id = &entry.app_id;

        // Try to set the corresponding input method as active
        if input_method_handle.set_active_instance(app_id) {
            info!(
                "sync_input_method_with_layout: set active instance to '{}'",
                app_id
            );
            // Only activate (and install interceptor) if a text_input client has enabled.
            // Otherwise, just setting the active instance is enough — the interceptor will
            // be activated later when the client sends text_input enable+commit.
            let text_input = seat.text_input();
            if text_input.has_active_text_input() {
                if let Some(keyboard) = seat.get_keyboard() {
                    if let Some(focus) = keyboard.current_focus() {
                        use smithay::wayland::seat::WaylandFocus;
                        if let Some(surface) = focus.wl_surface() {
                            input_method_handle.activate_input_method(state, &surface);
                            input_method_handle.done();
                        }
                    }
                }
            }
        } else {
            warn!(
                "Input method '{}' for layout '{}' not registered yet",
                app_id, active_layout_code
            );
        }
    } else {
        // No mapping for this layout - clear active instance
        input_method_handle.clear_active_instance(state);
    }
}
