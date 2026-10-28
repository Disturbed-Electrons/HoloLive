mod acquisition;
mod reconstruction;
mod sideband;

use eframe::egui;

use crate::{
    pipeline::{AcquisitionCommand, FrameSample},
    ui::{CONTROL_PANEL_WIDTH, HoloLiveApp, NoticeKind},
};

pub(crate) fn controls<T: FrameSample>(app: &mut HoloLiveApp<T>, root: &mut egui::Ui) {
    let mut reconstruction_changed = false;
    let mut acquisition_changed = false;
    let previous_binning = app.config.binning;
    egui::Panel::right("controls")
        .exact_size(CONTROL_PANEL_WIDTH)
        .resizable(false)
        .frame(
            egui::Frame::new()
                .fill(root.visuals().panel_fill)
                .inner_margin(egui::Margin {
                    left: 14,
                    right: 4,
                    top: 12,
                    bottom: 12,
                }),
        )
        .show(root, |ui| {
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.set_max_width(ui.available_width());
                    acquisition::section(app, ui, &mut acquisition_changed);
                    ui.add_space(14.0);
                    reconstruction_changed |= reconstruction::section(app, ui);
                    ui.add_space(14.0);
                    reconstruction_changed |= sideband::section(app, ui);
                });
        });
    if acquisition_changed {
        app.pipeline.acquisition(AcquisitionCommand {
            exposure_seconds: app.config.exposure_seconds,
            binning: app.config.binning,
        });
        if app.config.binning != previous_binning {
            let unlocked = app.unlock_sideband();
            reconstruction_changed |= unlocked;
            let message = if unlocked {
                "Re-sent Exposure After Binning Change; Sideband Unlocked"
            } else {
                "Re-sent Exposure After Binning Change"
            };
            app.notice.show(NoticeKind::Camera, message);
        }
    }
    if reconstruction_changed {
        app.send_settings();
    }
}
