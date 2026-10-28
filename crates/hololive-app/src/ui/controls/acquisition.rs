use eframe::egui::{self, ComboBox};

use crate::{
    pipeline::FrameSample,
    ui::{HoloLiveApp, LiveView},
};

pub(super) fn section<T: FrameSample>(
    app: &mut HoloLiveApp<T>,
    ui: &mut egui::Ui,
    acquisition_changed: &mut bool,
) {
    ui.heading("Acquisition");
    egui::Grid::new("acquisition-grid")
        .num_columns(2)
        .spacing([12.0, 8.0])
        .show(ui, |ui| {
            ui.label("Display");
            let mut view = app.view;
            ComboBox::from_id_salt("view")
                .selected_text(view.label())
                .show_ui(ui, |ui| {
                    for option in LiveView::ALL {
                        ui.selectable_value(&mut view, option, option.label());
                    }
                });
            if view != app.view {
                app.set_view(view);
            }
            ui.end_row();
            ui.label("Binning");
            ComboBox::from_id_salt("binning")
                .width(48.0)
                .wrap_mode(egui::TextWrapMode::Extend)
                .selected_text(format!("{}×", app.config.binning))
                .show_ui(ui, |ui| {
                    for &binning in &app.config.supported_binnings {
                        *acquisition_changed |= ui
                            .selectable_value(
                                &mut app.config.binning,
                                binning,
                                format!("{binning}×"),
                            )
                            .changed();
                    }
                });
            ui.end_row();
            ui.label("Exposure");
            ui.scope(|ui| {
                ui.spacing_mut().interact_size.x = exposure_value_width(ui);
                ui.spacing_mut().slider_width = 116.0;
                *acquisition_changed |= ui
                    .add(
                        egui::Slider::new(&mut app.config.exposure_seconds, 0.01..=10.0)
                            .logarithmic(true)
                            .suffix(" s")
                            .fixed_decimals(3),
                    )
                    .changed();
            });
            ui.end_row();
        });
}

fn exposure_value_width(ui: &egui::Ui) -> f32 {
    let font_id = ui.style().drag_value_text_style.resolve(ui.style());
    let text_width = ui
        .painter()
        .layout_no_wrap("10.000 s".to_owned(), font_id, egui::Color32::WHITE)
        .size()
        .x;
    (text_width + ui.spacing().button_padding.x * 2.0).ceil()
}
