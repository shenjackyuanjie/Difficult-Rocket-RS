//! 章节说明的非交互绘制；详细属性样例移到模态外侧，不遮挡错误和按钮。
use super::*;
use bevy_egui::egui;

pub(super) fn paint(
    ctx: &egui::Context,
    title: &str,
    description: &str,
    details: Option<&str>,
    step: Duration,
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
            620.,
        )
    };
    egui::Area::new(egui::Id::new(id))
        .anchor(anchor, offset)
        .order(egui::Order::Tooltip)
        .interactable(false)
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(egui::Color32::from_rgb(25, 23, 34))
                .stroke(egui::Stroke::new(
                    1.,
                    egui::Color32::from_rgb(142, 116, 199),
                ))
                .corner_radius(6.)
                .inner_margin(10.)
                .show(ui, |ui| {
                    ui.set_max_width(width);
                    ui.label(
                        egui::RichText::new(title)
                            .size(18.)
                            .color(egui::Color32::from_rgb(221, 203, 255)),
                    );
                    ui.label(description);
                    if let Some(details) = details {
                        ui.separator();
                        ui.label(details);
                    }
                    ui.label(
                        egui::RichText::new(format!(
                            "节奏基准 {}ms · 内部输入注入 · 窗口单次启动",
                            step.as_millis()
                        ))
                        .size(12.)
                        .weak(),
                    );
                });
        })
        .response
        .rect
}
