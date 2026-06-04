// SPDX-License-Identifier: GPL-3.0-only

use crate::state::{ClientState, State};
use crate::utils::geometry::SizeExt;
use crate::utils::prelude::OutputExt;
use smithay::{
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
use std::os::unix::io::AsRawFd;
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::process::Command;
use std::sync::Arc;
use tracing::{error, warn};

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
        cursor: &Rectangle<i32, smithay::utils::Logical>,
        positioner: &PositionerState,
    ) -> Rectangle<i32, smithay::utils::Logical> {
        let shell = self.common.shell.read();

        // Find the element and its workspace to get the correct output
        let elem = shell.element_for_surface(parent);
        let output = elem.and_then(|e| {
            shell.space_for(e).map(|ws| ws.output.clone()).or_else(|| {
                shell
                    .workspaces
                    .sets
                    .iter()
                    .find(|(_, set)| set.sticky_layer.mapped().any(|m| m == e))
                    .map(|(o, _)| o.clone())
            })
        });

        let Some(output) = output else {
            tracing::warn!(
                "popup_geometry: no output found for parent, using unconstrained default"
            );
            return positioner.get_unconstrained_geometry(*cursor, Rectangle::default());
        };

        let parent_geo_global = elem
            .and_then(|e| shell.element_geometry(e))
            .unwrap_or_default();

        // output.geometry() is in Global coords
        let output_geo = output.geometry();

        // Target rectangle: the output bounds expressed relative to the parent surface.
        // Both parent_geo_global and output_geo are in Global coordinates.
        // The popup position (from positioner) is relative to the parent surface origin,
        // so we express the output rect in the parent's coordinate system.
        let target = Rectangle::new(
            (
                output_geo.loc.x - parent_geo_global.loc.x,
                output_geo.loc.y - parent_geo_global.loc.y,
            )
                .into(),
            output_geo.size.as_logical(),
        );
        // Use positioner's constraint adjustment (flip_y, slide_x, etc.)
        let result = positioner.get_unconstrained_geometry(*cursor, target);
        result
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

    fn input_method_instance_registered(&mut self) {
        // Sync layout state now that this IME is available
        let seats: Vec<_> = self.common.shell.read().seats.iter().cloned().collect();
        let layout = self.common.config.cosmic_conf.xkb_config.layout.clone();
        for seat in &seats {
            sync_input_method_with_layout(self, seat, &layout);
        }
    }
}

// Re-export from cosmic-comp-config
pub use cosmic_comp_config::{InputMethodEntry, InputMethodKeyboardMap};

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
        Ok(_child) => {}
        Err(err) => {
            warn!("Failed to spawn IME '{}' ({}): {}", app_id, command, err);
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
        if input_method_handle.set_active_instance(app_id) {
            let text_input = seat.text_input();
            let has_active = text_input.has_active_text_input();
            if has_active {
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
        input_method_handle.clear_active_instance(state);
    }
}
