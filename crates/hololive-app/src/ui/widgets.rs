use eframe::egui::{self, Color32, Sense, Stroke};

use crate::ui::{CONTROL_ROW_GAP, SLIDER_LABEL_WIDTH, SLIDER_VALUE_WIDTH, TOGGLE_BUTTON_SIZE};

pub(crate) enum SquareToggle {
    Lock,
    Letter(&'static str),
}

pub(crate) fn sideband_row(ui: &mut egui::Ui, id: &'static str, add: impl FnOnce(&mut egui::Ui)) {
    let width = ui.available_width();
    ui.push_id(id, |ui| {
        ui.allocate_ui_with_layout(
            egui::vec2(width, TOGGLE_BUTTON_SIZE),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.set_max_width(width);
                ui.spacing_mut().item_spacing.x = CONTROL_ROW_GAP;
                add(ui);
            },
        );
    });
}

pub(crate) fn sideband_slider<Num: egui::emath::Numeric>(
    ui: &mut egui::Ui,
    value: &mut Num,
    range: std::ops::RangeInclusive<Num>,
    label: &str,
    enabled: bool,
    decimals: usize,
    suffix: &str,
) -> bool {
    let height = ui.spacing().interact_size.y;
    let slider_width =
        (ui.available_width() - SLIDER_VALUE_WIDTH - SLIDER_LABEL_WIDTH - CONTROL_ROW_GAP * 2.0)
            .max(40.0);
    let mut changed = false;
    ui.push_id(label, |ui| {
        ui.allocate_ui(egui::vec2(slider_width, height), |ui| {
            ui.spacing_mut().slider_width = slider_width;
            if !enabled {
                ui.disable();
            }
            changed |= ui
                .add(unlabeled_slider(value, range.clone(), decimals))
                .changed();
        });
        ui.allocate_ui(egui::vec2(SLIDER_VALUE_WIDTH, height), |ui| {
            if !enabled {
                ui.disable();
            }
            changed |= ui
                .add_sized(
                    ui.available_size(),
                    egui::DragValue::new(value)
                        .range(range)
                        .speed(0.1f64.powi(decimals as i32))
                        .fixed_decimals(decimals)
                        .suffix(suffix),
                )
                .changed();
        });
        ui.allocate_ui_with_layout(
            egui::vec2(SLIDER_LABEL_WIDTH, height),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.add(egui::Label::new(label).truncate());
            },
        );
    });
    changed
}

fn unlabeled_slider<Num: egui::emath::Numeric>(
    value: &mut Num,
    range: std::ops::RangeInclusive<Num>,
    decimals: usize,
) -> egui::Slider<'_> {
    let mut slider = egui::Slider::new(value, range)
        .show_value(false)
        .smart_aim(false);
    if decimals > 0 {
        slider = slider
            .max_decimals(decimals)
            .step_by(0.1f64.powi(decimals as i32));
    }
    slider
}

pub(crate) fn sideband_axis_drag(
    ui: &mut egui::Ui,
    id: &'static str,
    label: &str,
    value: &mut usize,
    range: std::ops::RangeInclusive<usize>,
    enabled: bool,
) -> bool {
    ui.label(label);
    ui.push_id(id, |ui| {
        ui.add_enabled(enabled, egui::DragValue::new(value).range(range))
    })
    .inner
    .changed()
}

pub(crate) fn square_toggle_button(
    ui: &mut egui::Ui,
    size: egui::Vec2,
    icon: SquareToggle,
    on: bool,
    enabled: bool,
) -> egui::Response {
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(size, sense);
    let fill = if on {
        Color32::from_rgb(167, 100, 35)
    } else {
        ui.visuals().widgets.inactive.bg_fill
    };
    let fg = if enabled {
        ui.visuals().widgets.inactive.fg_stroke.color
    } else {
        ui.visuals().weak_text_color()
    };
    let rounding = 4.0;
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, rounding, fill);
    match icon {
        SquareToggle::Lock => paint_lock_icon(&painter, rect, fg),
        SquareToggle::Letter(letter) => {
            ui.painter_at(rect).text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                letter,
                egui::FontId::proportional((size.y * 0.48).max(13.0)),
                fg,
            );
        }
    }
    response
}

fn paint_lock_icon(painter: &egui::Painter, rect: egui::Rect, color: Color32) {
    let width = rect.width();
    let center = rect.center();
    let stroke = Stroke::new(1.6, color);
    let body = egui::Rect::from_center_size(
        egui::pos2(center.x, center.y + width * 0.08),
        egui::vec2(width * 0.42, width * 0.34),
    );
    painter.rect_stroke(body, 1.2, stroke, egui::StrokeKind::Middle);
    let radius = body.width() * 0.32;
    let origin = egui::pos2(center.x, body.top());
    let mut previous = None;
    for step in 0..=8 {
        let angle = std::f32::consts::PI * (1.0 + step as f32 / 8.0);
        let point = egui::pos2(
            origin.x + radius * angle.cos(),
            origin.y + radius * angle.sin(),
        );
        if let Some(previous) = previous {
            painter.line_segment([previous, point], stroke);
        }
        previous = Some(point);
    }
}
