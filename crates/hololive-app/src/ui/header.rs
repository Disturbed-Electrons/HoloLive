use eframe::egui::{self, Color32, RichText, Sense};

use crate::{
    pipeline::{ConnectionState, FrameSample},
    ui::{Action, HEADER_HEIGHT, HEADER_RIGHT_BUTTON_GROUP_WIDTH, HoloLiveApp},
};

pub(crate) fn header<T: FrameSample>(app: &mut HoloLiveApp<T>, root: &mut egui::Ui) {
    egui::Panel::top("header")
        .exact_size(HEADER_HEIGHT)
        .show(root, |ui| {
            ui.horizontal_centered(|ui| {
                ui.add_space(8.0);
                ui.heading(RichText::new("HoloLive").strong());
                ui.separator();
                let (color, label) = match app.status.connection {
                    ConnectionState::Connected => (Color32::from_rgb(67, 205, 128), "Connected"),
                    ConnectionState::Connecting => (Color32::YELLOW, "Connecting"),
                    ConnectionState::Reconnecting => {
                        (Color32::from_rgb(255, 160, 64), "Reconnecting")
                    }
                    ConnectionState::Disconnected => {
                        (Color32::from_rgb(235, 80, 90), "Disconnected")
                    }
                };
                let connection = ui
                    .horizontal(|ui| {
                        let (rect, _) =
                            ui.allocate_exact_size(egui::vec2(8.0, 8.0), Sense::hover());
                        ui.painter().circle_filled(rect.center(), 4.0, color);
                        ui.colored_label(color, label);
                    })
                    .response;
                if let Some(error) = &app.status.last_acquisition_error {
                    connection.on_hover_text(error);
                }
                ui.label(
                    RichText::new(if app.simulator.is_some() {
                        "Simulated"
                    } else {
                        app.status.camera.as_str()
                    })
                    .color(Color32::from_rgb(95, 190, 255))
                    .strong(),
                );
                ui.label(
                    RichText::new(format!(
                        "Acquisition {:>4.1} FPS",
                        app.status.acquisition_fps
                    ))
                    .monospace(),
                );
                if let Some(notice) = &app.notice.message {
                    let notice_width =
                        (ui.available_width() - HEADER_RIGHT_BUTTON_GROUP_WIDTH).max(0.0);
                    if notice_width > 0.0 {
                        let color = app.notice.kind.color();
                        ui.add_sized(
                            [notice_width, 22.0],
                            egui::Label::new(RichText::new(notice).color(color)).truncate(),
                        );
                    }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(8.0);
                    if ui.button("Save").on_hover_text("Save PNG (S)").clicked() {
                        let _ = app.apply(Action::Save);
                    }
                    let reference_label = if app.status.reference_active {
                        "Clear"
                    } else {
                        "Capture"
                    };
                    let reference_fill = if app.status.reference_active {
                        Color32::from_rgb(167, 100, 35)
                    } else {
                        ui.visuals().widgets.inactive.bg_fill
                    };
                    if ui
                        .add(egui::Button::new(reference_label).fill(reference_fill))
                        .on_hover_text(if app.status.reference_active {
                            "Clear Reference Wave (R)"
                        } else {
                            "Capture Reference Wave (R)"
                        })
                        .clicked()
                    {
                        let _ = app.apply(Action::ToggleReference);
                    }
                    let pause_label = if app.pipeline.paused() {
                        "Resume"
                    } else {
                        "Pause"
                    };
                    let pause_fill = if app.pipeline.paused() {
                        Color32::from_rgb(160, 65, 55)
                    } else {
                        ui.visuals().widgets.inactive.bg_fill
                    };
                    if ui
                        .add(egui::Button::new(pause_label).fill(pause_fill))
                        .on_hover_text(if app.pipeline.paused() {
                            "Resume Acquisition (P)"
                        } else {
                            "Pause Acquisition (P)"
                        })
                        .clicked()
                    {
                        let _ = app.apply(Action::TogglePause);
                    }
                });
            });
        });
}
