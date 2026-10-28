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
        if app
            .result
            .as_ref()
            .is_some_and(|result| result.view == LiveView::Spectrum)
            && let Some(result) = app.current_reconstruction()
        {
            draw_spectrum_overlay(
                ui,
                &response.rect,
                &result,
                &app.config.reconstruction,
                app.config.crop.to_settings(),
            );
        }
    });
}

fn draw_spectrum_overlay(
    ui: &egui::Ui,
    image_rect: &egui::Rect,
    result: &hololive_core::ReconstructionInfo,
    settings: &ReconstructionSettings,
    crop_radius: Option<usize>,
) {
    let to_screen = |point: Sideband| {
        egui::pos2(
            image_rect.left()
                + point.x as f32 / result.frame_width.max(1) as f32 * image_rect.width(),
            image_rect.top()
                + point.y as f32 / result.frame_height.max(1) as f32 * image_rect.height(),
        )
    };
    let overlay_sideband = settings.locked_sideband.unwrap_or(result.sideband);
    let overlay_radius = crop_radius.unwrap_or(result.crop_radius);
    let position = to_screen(overlay_sideband);
    let center = to_screen(Sideband {
        x: result.frame_width / 2,
        y: result.frame_height / 2,
    });
    let mask_radius_pixels =
        result.frame_width.min(result.frame_height) as f32 * settings.center_mask_percent / 100.0;
    let mask_radius_x = mask_radius_pixels / result.frame_width.max(1) as f32 * image_rect.width();
    let mask_radius_y =
        mask_radius_pixels / result.frame_height.max(1) as f32 * image_rect.height();
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
    ui.painter().circle_stroke(
        center,
        0.5 * (mask_radius_x + mask_radius_y),
        centerband_stroke,
    );
    let radius_x = overlay_radius as f32 / result.frame_width.max(1) as f32 * image_rect.width();
    let radius_y = overlay_radius as f32 / result.frame_height.max(1) as f32 * image_rect.height();
    let radius = 0.5 * (radius_x + radius_y);
    ui.painter()
        .circle_stroke(position, radius, sideband_stroke);
    let arm = 7.0;
    ui.painter().line_segment(
        [
            position + egui::vec2(-arm, -arm),
            position + egui::vec2(arm, arm),
        ],
        sideband_stroke,
    );
    ui.painter().line_segment(
        [
            position + egui::vec2(-arm, arm),
            position + egui::vec2(arm, -arm),
        ],
        sideband_stroke,
    );
}
