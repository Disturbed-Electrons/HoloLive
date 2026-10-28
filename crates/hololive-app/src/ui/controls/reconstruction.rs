use eframe::egui::{self, ComboBox, Sense};
use hololive_core::DisplayMode;

use crate::{
    pipeline::FrameSample,
    ui::{Action, HoloLiveApp},
};

pub(super) fn section<T: FrameSample>(app: &mut HoloLiveApp<T>, ui: &mut egui::Ui) -> bool {
    let mut reconstruction_changed = false;
    ui.heading("Reconstruction");
    ui.horizontal(|ui| {
        let mut mode = app.config.reconstruction.display_mode;
        ui.radio_value(&mut mode, DisplayMode::Phase, "Phase")
            .on_hover_text("Switch Phase / Amplitude (Tab)");
        ui.radio_value(&mut mode, DisplayMode::Amplitude, "Amplitude")
            .on_hover_text("Switch Phase / Amplitude (Tab)");
        reconstruction_changed |= app.apply(Action::SetDisplayMode(mode));
        ui.add_enabled_ui(
            app.config.reconstruction.display_mode == DisplayMode::Phase,
            |ui| {
                let mut unwrap = app.config.reconstruction.phase_unwrap;
                if ui
                    .checkbox(&mut unwrap, "Unwrap")
                    .on_hover_text("Toggle Unwrap (U)")
                    .changed()
                {
                    reconstruction_changed |= app.apply(Action::SetPhaseUnwrap(unwrap));
                }
            },
        );
    });
    let phase_controls_enabled = app.config.reconstruction.display_mode == DisplayMode::Phase;
    let amplification_enabled = phase_controls_enabled && !app.config.reconstruction.phase_unwrap;
    let row_height = ui.spacing().interact_size.y;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        ui.set_height(row_height);
        ui.add_enabled(amplification_enabled, egui::Label::new("Amplification"))
            .on_hover_text("Cycle Amplification (A)");
        let combo_size = egui::vec2(48.0, row_height);
        let (combo_rect, combo_response) = ui.allocate_exact_size(combo_size, Sense::hover());
        combo_response.on_hover_text("Cycle Amplification (A)");
        let mut combo_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(combo_rect)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );
        if !amplification_enabled {
            combo_ui.disable();
        }
        let mut amplification = app.config.reconstruction.phase_amplification;
        ComboBox::from_id_salt("phase-amplification")
            .width(combo_size.x)
            .wrap_mode(egui::TextWrapMode::Extend)
            .selected_text(format!("{amplification}×"))
            .show_ui(&mut combo_ui, |ui| {
                for value in 1..=4 {
                    ui.selectable_value(&mut amplification, value, format!("{value}×"));
                }
            });
        reconstruction_changed |= app.apply(Action::SetPhaseAmplification(amplification));
        ui.add_space(12.0);
        ui.label("Divisor")
            .on_hover_text("Detector size is divided by this, then snapped to a fast FFT length");
        let divisor_changed = ui
            .add_sized(
                [44.0, row_height],
                egui::DragValue::new(&mut app.config.output_divisor)
                    .range(1.0..=16.0)
                    .speed(0.1)
                    .fixed_decimals(1),
            )
            .on_hover_text("Detector size is divided by this, then snapped to a fast FFT length")
            .changed();
        if divisor_changed {
            reconstruction_changed |= app.sync_output_size(app.status.width, app.status.height);
        }
    });
    reconstruction_changed
}
