//! 章节说明的非交互绘制；详细属性样例移到模态外侧，不遮挡错误和按钮。
use bevy_egui::egui;

const TITLE: egui::Color32 = egui::Color32::from_rgb(221, 203, 255);
const BODY: egui::Color32 = egui::Color32::from_rgb(198, 192, 210);
const DETAIL: egui::Color32 = egui::Color32::from_rgb(150, 140, 170);
const FILL: egui::Color32 = egui::Color32::from_rgb(22, 20, 29);
const BORDER: egui::Color32 = egui::Color32::from_rgb(64, 54, 92);

pub(super) fn paint(
    ctx: &egui::Context,
    title: &str,
    description: &str,
    details: Option<&str>,
    left_of: Option<f32>,
) -> egui::Rect {
    let (id, anchor, offset, width) = if let Some(left) = left_of {
        (
            "demo-modal-caption",
            egui::Align2::LEFT_BOTTOM,
            egui::vec2(14., -14.),
            (left - ctx.content_rect().left() - 48.).clamp(1., 300.),
        )
    } else {
        (
            "demo-caption",
            egui::Align2::CENTER_BOTTOM,
            egui::vec2(0., -14.),
            640.,
        )
    };
    egui::Area::new(egui::Id::new(id))
        .anchor(anchor, offset)
        .order(egui::Order::Tooltip)
        .interactable(false)
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(FILL)
                .stroke(egui::Stroke::new(1., BORDER))
                .corner_radius(8.)
                .inner_margin(egui::Margin::symmetric(16, 12))
                .show(ui, |ui| {
                    ui.set_max_width(width);
                    ui.label(egui::RichText::new(title).size(17.).strong().color(TITLE));
                    ui.add_space(2.);
                    ui.label(egui::RichText::new(description).size(14.).color(BODY));
                    if let Some(details) = details {
                        ui.add_space(6.);
                        ui.separator();
                        ui.label(egui::RichText::new(details).size(13.).color(DETAIL));
                    }
                });
        })
        .response
        .rect
}
