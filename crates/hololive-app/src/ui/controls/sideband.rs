use eframe::egui::{self, ComboBox, Sense};
use hololive_core::{SearchRegion, max_output_crop_radius};

use crate::{
    pipeline::FrameSample,
    ui::{
        Action, HoloLiveApp, REGION_COMBO_WIDTH, TOGGLE_BUTTON_SIZE,
        widgets::{
            SquareToggle, sideband_axis_drag, sideband_row, sideband_slider, square_toggle_button,
        },
    },
};

pub(super) fn section<T: FrameSample>(app: &mut HoloLiveApp<T>, ui: &mut egui::Ui) -> bool {
    let mut reconstruction_changed = false;
    ui.heading("Sideband");
    let locked = app.config.reconstruction.locked_sideband.is_some();
    let crop_radius = app.config.crop.last_manual();
    let x_range = crop_axis_range(app.status.width, crop_radius);
    let y_range = crop_axis_range(app.status.height, crop_radius);
    let (mut sideband_x, mut sideband_y) = app.config.reconstruction.locked_sideband.map_or_else(
        || {
            app.current_reconstruction().map_or((0, 0), |result| {
                let sideband = result.sideband;
                (sideband.x, sideband.y)
            })
        },
        |sideband| (sideband.x, sideband.y),
    );
    let button_size = egui::vec2(TOGGLE_BUTTON_SIZE, TOGGLE_BUTTON_SIZE);
    sideband_row(ui, "sideband-position", |ui| {
        if square_toggle_button(
            ui,
            button_size,
            SquareToggle::Lock,
            locked,
            app.current_reconstruction().is_some() || locked,
        )
        .on_hover_text(if locked {
            "Unlock Sideband (L)"
        } else {
            "Lock Sideband (L)"
        })
        .clicked()
        {
            reconstruction_changed |= app.apply(Action::ToggleSidebandLock);
        }
        ui.allocate_ui_with_layout(
            egui::vec2(REGION_COMBO_WIDTH, TOGGLE_BUTTON_SIZE),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.spacing_mut().interact_size.y = TOGGLE_BUTTON_SIZE;
                ui.spacing_mut().button_padding = egui::vec2(4.0, 0.0);
                ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
                if locked {
                    ui.disable();
                }
                ComboBox::from_id_salt("search-region")
                    .width(REGION_COMBO_WIDTH)
                    .wrap_mode(egui::TextWrapMode::Truncate)
                    .selected_text(app.config.reconstruction.search_region.compass_label())
                    .show_ui(ui, |ui| {
                        let mut region = app.config.reconstruction.search_region;
                        for candidate in SearchRegion::ALL {
                            ui.selectable_value(&mut region, candidate, candidate.label());
                        }
                        reconstruction_changed |= app.apply(Action::SetSearchRegion(region));
                    })
                    .response
                    .on_hover_text("Search Region (Arrows)");
            },
        );
        let x_changed = sideband_axis_drag(ui, "sideband-x", "X", &mut sideband_x, x_range, locked);
        let y_changed = sideband_axis_drag(ui, "sideband-y", "Y", &mut sideband_y, y_range, locked);
        if (x_changed || y_changed)
            && let Some(sideband) = &mut app.config.reconstruction.locked_sideband
        {
            sideband.x = sideband_x;
            sideband.y = sideband_y;
            reconstruction_changed = true;
        }
    });
    let automatic_radius = app.config.crop.is_automatic();
    let current_sideband = app
        .config
        .reconstruction
        .locked_sideband
        .or_else(|| app.current_reconstruction().map(|result| result.sideband));
    let image_max_radius = max_output_crop_radius(
        app.status.width,
        app.status.height,
        app.config.reconstruction.output_width,
        app.config.reconstruction.output_height,
    );
    let position_max_radius = current_sideband.map_or(image_max_radius, |sideband| {
        sideband
            .max_crop_radius(app.status.width, app.status.height)
            .min(image_max_radius)
            .max(1)
    });
    sideband_row(ui, "sideband-radius", |ui| {
        if square_toggle_button(
            ui,
            button_size,
            SquareToggle::Letter("A"),
            automatic_radius,
            !locked,
        )
        .on_hover_text(if automatic_radius {
            "Manual Radius"
        } else {
            "Automatic Radius"
        })
        .clicked()
        {
            app.config.crop.toggle_mode();
            reconstruction_changed = true;
        }
        let mut radius = app.config.crop.last_manual();
        if sideband_slider(
            ui,
            &mut radius,
            1..=image_max_radius,
            "Radius",
            !automatic_radius,
            0,
            "",
        ) {
            app.config
                .crop
                .set_manual(radius.min(position_max_radius).max(1));
            reconstruction_changed = true;
        }
    });
    sideband_row(ui, "sideband-tukey", |ui| {
        if square_toggle_button(
            ui,
            button_size,
            SquareToggle::Letter("T"),
            app.config.reconstruction.tukey_enabled,
            true,
        )
        .on_hover_text("Toggle Tukey Window (F)")
        .clicked()
        {
            reconstruction_changed |= app.apply(Action::ToggleTukey);
        }
        reconstruction_changed |= sideband_slider(
            ui,
            &mut app.config.reconstruction.tukey_alpha,
            0.0..=1.0,
            "Shape α",
            app.config.reconstruction.tukey_enabled,
            2,
            "",
        );
    });
    sideband_row(ui, "sideband-center-mask", |ui| {
        ui.allocate_exact_size(button_size, Sense::hover());
        reconstruction_changed |= sideband_slider(
            ui,
            &mut app.config.reconstruction.center_mask_percent,
            0.0..=9.9,
            "CB Mask",
            !locked,
            1,
            "%",
        );
    });
    reconstruction_changed
}

fn crop_axis_range(length: usize, radius: usize) -> std::ops::RangeInclusive<usize> {
    let hi = length.saturating_sub(radius);
    let lo = radius.min(hi);
    lo..=hi.max(lo)
}
