use std::time::Duration;

use eframe::egui::{self, Color32, Stroke};
use hololive_core::{ReconstructionSettings, Sideband};

use crate::{
    pipeline::FrameSample,
    ui::{HoloLiveApp, LiveView},
};

const VIEW_SWITCH_GRACE: Duration = Duration::from_millis(250);

pub(crate) fn viewport<T: FrameSample>(app: &HoloLiveApp<T>, root: &mut egui::Ui) {
    egui::CentralPanel::default().show(root, |ui| {
        if let Some(result) = &app.result {
            if result.view == LiveView::Reconstruction && result.reconstruction.is_none() {
                ui.centered_and_justified(|ui| {
                    ui.label("Reconstruction Unavailable");
                });
                return;
            }
            if result.view != app.view && app.view_changed_at.elapsed() >= VIEW_SWITCH_GRACE {
                ui.centered_and_justified(|ui| {
                    ui.spinner();
                    ui.label("Updating View…");
                });
                return;
            }
        }
        let Some(gpu) = &app.live.gpu else {
            ui.centered_and_justified(|ui| {
                ui.spinner();
                ui.label("Waiting For First Frame…");
            });
            return;
        };
        let available = ui.available_size();
        let texture_size = egui::vec2(gpu.width as f32, gpu.height as f32);
        let scale = (available.x / texture_size.x).min(available.y / texture_size.y);
        let size = texture_size * scale.max(0.01);
        let response = ui.add(egui::Image::new((gpu.id, size)));
        if let Some(result) = &app.result
            && result.view == LiveView::Spectrum
            && let Some(info) = &result.reconstruction
        {
            draw_spectrum_overlay(ui, &response.rect, info, &app.config.reconstruction);
        }
    });
}

fn draw_spectrum_overlay(
    ui: &egui::Ui,
    image_rect: &egui::Rect,
    result: &hololive_core::ReconstructionInfo,
    settings: &ReconstructionSettings,
) {
    let px = image_rect.width() / result.frame_width as f32;
    let py = image_rect.height() / result.frame_height as f32;
    let scale = 0.5 * (px + py);
    let to_screen = |point: Sideband| {
        image_rect.min + egui::vec2((point.x as f32 + 0.5) * px, (point.y as f32 + 0.5) * py)
    };
    let position = to_screen(result.sideband);
    let center = to_screen(Sideband {
        x: result.frame_width / 2,
        y: result.frame_height / 2,
    });
    let mask_radius =
        result.frame_width.min(result.frame_height) as f32 * settings.center_mask_percent / 100.0;
    let overlay_width = 2.5;
    let centerband_stroke = Stroke::new(overlay_width, Color32::from_rgb(70, 130, 255));
    let sideband_stroke = Stroke::new(
        overlay_width,
        if settings.locked_sideband.is_some() {
            Color32::from_rgb(230, 55, 55)
        } else {
            Color32::from_rgb(255, 150, 40)
        },
    );
    let painter = ui.painter();
    painter.circle_stroke(center, mask_radius * scale, centerband_stroke);
    let radius = result.crop_radius as f32 * scale;
    painter.circle_stroke(position, radius, sideband_stroke);
    let arm = 7.0;
    painter.line_segment(
        [
            position + egui::vec2(-arm, -arm),
            position + egui::vec2(arm, arm),
        ],
        sideband_stroke,
    );
    painter.line_segment(
        [
            position + egui::vec2(-arm, arm),
            position + egui::vec2(arm, -arm),
        ],
        sideband_stroke,
    );
}
