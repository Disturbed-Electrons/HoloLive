use eframe::egui::{self, Key};
use hololive_core::SearchRegion;

use crate::{
    pipeline::FrameSample,
    ui::{Action, HoloLiveApp},
};

pub(crate) fn keyboard_shortcuts<T: FrameSample>(
    app: &mut HoloLiveApp<T>,
    context: &egui::Context,
) {
    if context.egui_wants_keyboard_input() {
        return;
    }
    let mut settings_changed = false;
    if pressed(context, Key::Tab) {
        settings_changed |= app.apply(Action::CycleDisplayMode);
    }
    if pressed(context, Key::A) {
        settings_changed |= app.apply(Action::CyclePhaseAmplification);
    }
    if pressed(context, Key::U) {
        settings_changed |= app.apply(Action::TogglePhaseUnwrap);
    }
    if pressed(context, Key::F) {
        settings_changed |= app.apply(Action::ToggleTukey);
    }
    if context.memory(|memory| memory.focused().is_none()) {
        for (key, region) in [
            (Key::ArrowUp, SearchRegion::Upper),
            (Key::ArrowDown, SearchRegion::Lower),
            (Key::ArrowLeft, SearchRegion::Left),
            (Key::ArrowRight, SearchRegion::Right),
        ] {
            if pressed(context, key) {
                settings_changed |= app.apply(Action::SetSearchRegion(region));
            }
        }
    }
    if pressed(context, Key::P) {
        let _ = app.apply(Action::TogglePause);
    }
    if pressed(context, Key::R) {
        let _ = app.apply(Action::ToggleReference);
    }
    if pressed(context, Key::S) {
        let _ = app.apply(Action::Save);
    }
    if pressed(context, Key::L) {
        settings_changed |= app.apply(Action::ToggleSidebandLock);
    }
    if settings_changed {
        app.send_settings();
    }
}

fn pressed(context: &egui::Context, key: Key) -> bool {
    context.input(|input| input.key_pressed(key))
}
