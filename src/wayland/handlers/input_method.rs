// SPDX-License-Identifier: GPL-3.0-only

use crate::shell::Shell;
use crate::state::{ClientState, State};
use crate::utils::geometry::{PointExt, SizeExt};
use crate::utils::prelude::OutputExt;
use cosmic_comp_config::CosmicCompConfig;
use cosmic_config::CosmicConfigEntry;
use smithay::{
    desktop::{PopupKind, PopupManager, space::SpaceElement},
    output::Output,
    reexports::wayland_server::{Client, DisplayHandle, protocol::wl_surface::WlSurface},
    utils::{Logical, Rectangle},
    wayland::input_method::{InputMethodHandler, InputMethodSeat, PopupSurface, PositionerState},
};
use tracing::warn;

impl InputMethodHandler for State {
    fn new_popup(&mut self, surface: PopupSurface) {
        if surface.repositions_via_configure() {
            schedule_ime_popup_render(self, &surface);
        } else {
            self.common
                .shell
                .read()
                .unconstrain_popup(&PopupKind::from(surface.clone()));
        }
        if let Err(err) = self.common.popups.track_popup(PopupKind::from(surface)) {
            warn!("Failed to track popup: {err}");
        }
    }

    fn dismiss_popup(&mut self, surface: PopupSurface) {
        if let Some(parent) = surface.get_parent().map(|parent| parent.surface) {
            let _ = PopupManager::dismiss_popup(&parent, &PopupKind::from(surface));
        }
    }

    fn ime_popup_configure_sent(&mut self, popup: PopupSurface) {
        schedule_ime_popup_render(self, &popup);
    }

    fn popup_repositioned(&mut self, popup: PopupSurface) {
        if !popup.repositions_via_configure() {
            self.common.shell.read().unconstrain_popup(&popup.into());
        }
    }

    fn popup_geometry(
        &self,
        parent: &WlSurface,
        cursor: &Rectangle<i32, Logical>,
        positioner: &PositionerState,
    ) -> Rectangle<i32, Logical> {
        let shell = self.common.shell.read();
        let target = ime_popup_target_rect(&shell, parent).unwrap_or_else(|| {
            let size = shell
                .element_for_surface(parent)
                .map(|e| e.geometry().size)
                .unwrap_or_default();
            Rectangle::new((0, 0).into(), size)
        });
        positioner.get_unconstrained_geometry(*cursor, target)
    }

    fn parent_geometry(&self, parent: &WlSurface) -> Rectangle<i32, Logical> {
        self.common
            .shell
            .read()
            .element_for_surface(parent)
            .and_then(|elem| {
                elem.windows()
                    .find(|(w, _)| w.surface_offset(parent).is_some())
                    .map(|(w, _)| w.geometry())
            })
            .unwrap_or_default()
    }

    fn input_method_app_id(&self, client: &Client, _dh: &DisplayHandle) -> Option<String> {
        client_security_app_id(client).map(str::to_owned)
    }

    fn input_method_instance_registered(
        &mut self,
        seat: &smithay::input::Seat<State>,
        app_id: &str,
    ) {
        let layout = self.common.config.cosmic_conf.xkb_config.layout.clone();
        let Some(wanted) = ime_app_id_for_seat(self, seat, &layout) else {
            return;
        };
        if wanted == app_id {
            seat.input_method()
                .set_active_instance(self, seat, app_id, false);
        }
    }
}

fn schedule_ime_popup_render(state: &mut State, popup: &PopupSurface) {
    let Some(parent) = popup.get_parent() else {
        return;
    };
    if let Some(output) = output_for_surface(&state.common.shell.read(), &parent.surface) {
        state.backend.schedule_render(&output);
    }
}

fn client_security_app_id(client: &Client) -> Option<&str> {
    client
        .get_data::<ClientState>()?
        .security_context
        .as_ref()?
        .app_id
        .as_deref()
}

fn xkb_layout_codes(layout: &str) -> Vec<&str> {
    layout.split(',').map(str::trim).collect()
}

fn seats(state: &State) -> Vec<smithay::input::Seat<State>> {
    state.common.shell.read().seats.iter().cloned().collect()
}

fn output_for_surface(shell: &Shell, surface: &WlSurface) -> Option<Output> {
    shell
        .visible_output_for_surface(surface)
        .cloned()
        .or_else(|| {
            let elem = shell.element_for_surface(surface)?;
            shell.space_for(elem).map(|ws| ws.output.clone())
        })
}

/// Output in text-input surface coords (element_geo − CSD inset + surface_offset).
fn ime_popup_target_rect(shell: &Shell, parent: &WlSurface) -> Option<Rectangle<i32, Logical>> {
    let elem = shell.element_for_surface(parent)?;
    let output = output_for_surface(shell, parent)?;
    let parent_geo = shell.element_geometry(elem)?;
    let origin =
        parent_geo.loc - elem.geometry().loc.as_global() + elem.surface_offset(parent)?.as_global();
    let output_geo = output.geometry();
    Some(Rectangle::new(
        (output_geo.loc.x - origin.x, output_geo.loc.y - origin.y).into(),
        output_geo.size.as_logical(),
    ))
}

pub fn is_privileged_ime_client(client: &Client) -> bool {
    let Some(app_id) = client_security_app_id(client) else {
        return false;
    };
    let Ok(helper) =
        cosmic_config::Config::new("com.system76.CosmicComp", CosmicCompConfig::VERSION)
    else {
        return false;
    };
    CosmicCompConfig::get_entry(&helper)
        .unwrap_or_else(|(_, c)| c)
        .input_method_map
        .values()
        .any(|e| e.app_id == app_id)
}

pub fn apply_saved_active_layout(state: &mut State) {
    use smithay::input::keyboard::Layout;

    let active = state.common.config.cosmic_conf.active_layout.trim();
    if active.is_empty() {
        return;
    }
    let layout_string = state.common.config.cosmic_conf.xkb_config.layout.clone();
    let layouts = xkb_layout_codes(&layout_string);
    let Some(idx) = layouts.iter().position(|l| *l == active) else {
        warn!(
            active,
            layout_string, "saved active_layout not in layout list"
        );
        return;
    };

    for seat in seats(state) {
        if let Some(keyboard) = seat.get_keyboard() {
            keyboard.with_xkb_state(state, |mut xkb| {
                if xkb.xkb().lock().unwrap().active_layout().0 as usize != idx {
                    xkb.set_layout(Layout(idx as u32));
                }
            });
        }
        sync_input_method_with_layout(state, &seat, &layout_string);
    }
}

pub fn sync_input_methods_all_seats(state: &mut State) {
    let layout = state.common.config.cosmic_conf.xkb_config.layout.clone();
    for seat in seats(state) {
        sync_input_method_with_layout(state, &seat, &layout);
    }
}

fn ime_app_id_for_seat(
    state: &mut State,
    seat: &smithay::input::Seat<State>,
    layout: &str,
) -> Option<String> {
    if state.common.config.cosmic_conf.input_method_map.is_empty() {
        return None;
    }

    let layouts = xkb_layout_codes(layout);
    let code = if let Some(kb) = seat.get_keyboard() {
        kb.with_xkb_state(state, |xkb| {
            let idx = xkb.xkb().lock().unwrap().active_layout().0 as usize;
            layouts
                .get(idx)
                .or_else(|| layouts.first())
                .copied()
                .unwrap_or("")
                .to_string()
        })
    } else {
        let saved = state.common.config.cosmic_conf.active_layout.trim();
        if !saved.is_empty() && layouts.contains(&saved) {
            saved.to_string()
        } else {
            layouts.first().copied().unwrap_or("").to_string()
        }
    };

    state
        .common
        .config
        .cosmic_conf
        .input_method_map
        .get(&code)
        .map(|e| e.app_id.clone())
}

pub fn sync_input_method_with_layout(
    state: &mut State,
    seat: &smithay::input::Seat<State>,
    layout: &str,
) {
    let im = seat.input_method();
    let Some(app_id) = ime_app_id_for_seat(state, seat, layout) else {
        im.clear_active_instance(state, seat);
        return;
    };
    if !im.set_active_instance(state, seat, &app_id, true) {
        // Don't clear when the target IME is not registered yet (eats filter buffer).
        if im.active_app_id().is_some_and(|id| id != app_id) {
            im.clear_active_instance(state, seat);
        }
        warn!("Input method '{}' for layout not registered yet", app_id);
    }
}
