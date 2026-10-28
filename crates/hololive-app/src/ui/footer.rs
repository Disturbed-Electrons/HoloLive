use eframe::egui::{self, Color32, RichText};

use crate::{
    pipeline::FrameSample,
    ui::{FOOTER_HEIGHT, HoloLiveApp},
};

pub(crate) fn footer<T: FrameSample>(app: &HoloLiveApp<T>, root: &mut egui::Ui) {
    egui::Panel::bottom("footer")
        .exact_size(FOOTER_HEIGHT)
        .show(root, |ui| {
            ui.horizontal_centered(|ui| {
                if let Some(result) = &app.result {
                    ui.label(format!(
                        "{}×{}",
                        result.display_width, result.display_height
                    ));
                    ui.separator();
                    ui.label(
                        RichText::new(format!("Age {:>4.0} ms", app.displayed_age_ms)).monospace(),
                    );
                    if let Some(reconstruction) = result.reconstruction {
                        ui.separator();
                        ui.label(
                            RichText::new(format!(
                                "FFT {:>5.2} ms",
                                reconstruction.timings.forward_fft_ms
                                    + reconstruction.timings.inverse_fft_ms
                            ))
                            .monospace(),
                        );
                        ui.separator();
                        ui.label(
                            RichText::new(format!(
                                "Reconstruct {:>5.2} ms",
                                reconstruction.timings.total_ms
                            ))
                            .monospace(),
                        );
                        ui.separator();
                        ui.label(
                            RichText::new(format!(
                                "Contrast {:>5.1}%",
                                reconstruction.metrics.fringe_contrast * 100.0
                            ))
                            .monospace(),
                        );
                        ui.separator();
                        ui.label(
                            RichText::new(format!(
                                "Spacing {:>5.2} px",
                                reconstruction.metrics.fringe_spacing_pixels
                            ))
                            .monospace(),
                        );
                    }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let failures =
                        RichText::new(format!("Missed {}", app.status.acquisition_failures)).color(
                            if app.status.acquisition_failures == 0 {
                                ui.visuals().text_color()
                            } else {
                                Color32::from_rgb(245, 175, 70)
                            },
                        );
                    let response = ui.label(failures);
                    if let Some(error) = &app.status.last_acquisition_error {
                        response.on_hover_text(format!("Last Acquisition Error: {error}"));
                    }
                    ui.separator();
                    ui.label(app.precision.label());
                });
            });
        });
}
