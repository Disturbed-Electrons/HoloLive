use eframe::egui;

use super::dimensions::{CONTROL_PANEL_WIDTH, FOOTER_HEIGHT, HEADER_HEIGHT};

pub(crate) fn configure_style(context: &egui::Context) {
    context.set_theme(egui::Theme::Dark);
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = egui::Color32::from_rgb(22, 27, 35);
    visuals.window_fill = egui::Color32::from_rgb(22, 27, 35);
    visuals.selection.bg_fill = egui::Color32::from_rgb(38, 115, 170);
    context.set_visuals(visuals);
    context.style_mut_of(context.theme(), |style| {
        style.spacing.item_spacing = egui::vec2(8.0, 7.0);
        style.spacing.button_padding = egui::vec2(10.0, 6.0);
    });
}

pub(crate) fn size_window_to_display(context: &egui::Context) {
    let monitor = context
        .input(|input| input.viewport().monitor_size)
        .unwrap_or_else(|| egui::vec2(1_440.0, 900.0));
    let maximum = monitor - egui::vec2(64.0, 96.0);
    let height = (monitor.y * 0.82).clamp(680.0, 1_100.0).min(maximum.y);
    let preview_width = height - HEADER_HEIGHT - FOOTER_HEIGHT;
    let width = (preview_width + CONTROL_PANEL_WIDTH).min(maximum.x);
    context.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(width, height)));
}
