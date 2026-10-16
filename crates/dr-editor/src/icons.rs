//! 原版灰黑 / 青柠风格的矢量图标；交互始终交给 egui 的正式 Button。
use bevy_egui::egui;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Icon {
    Menu,
    Staging,
    Help,
    Parts,
    Ships,
    Undo,
    Redo,
    More,
    Settings,
    Fit,
    Delete,
    SelectAll,
    Copy,
    Cut,
    Paste,
    Rotate,
    FlipX,
    FlipY,
    Cancel,
    Expand,
    New,
    Open,
    Save,
    SaveAs,
    Exit,
    Add,
    ChevronUp,
    ChevronDown,
    Drag,
    Target,
    Check,
    Follow,
    Snap,
    Free,
}

pub(crate) fn button(ui: &mut egui::Ui, icon: Icon, tooltip: &str) -> egui::Response {
    selected_button(ui, icon, tooltip, false)
}

pub(crate) fn selected_button(
    ui: &mut egui::Ui,
    icon: Icon,
    tooltip: &str,
    selected: bool,
) -> egui::Response {
    ui.push_id(("editor_icon", icon, tooltip), |ui| {
        let response = ui.add_sized([44.0, 44.0], egui::Button::new("").selected(selected));
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), tooltip)
        });
        let color = if response.enabled() {
            crate::egui_ui::ui_color(crate::native_buttons::ACCENT)
        } else {
            ui.visuals().weak_text_color()
        };
        paint(ui.painter(), response.rect, icon, color);
        response
    })
    .inner
    .on_hover_text(tooltip)
    .on_disabled_hover_text(tooltip)
}

/// 仅绘图，不创建命中区；调用方复用 Button / Response 的点击、焦点和可用状态。
pub(crate) fn paint(painter: &egui::Painter, rect: egui::Rect, icon: Icon, color: egui::Color32) {
    let side = rect.width().min(rect.height()) * 0.68;
    let glyph = egui::Rect::from_center_size(rect.center(), egui::Vec2::splat(side));
    let p = |x: f32, y: f32| glyph.min + egui::vec2(x, y) * (side / 24.0);
    let stroke = egui::Stroke::new((side / 15.0).max(1.2), color);
    let line = |a: (f32, f32), b: (f32, f32)| {
        painter.line_segment([p(a.0, a.1), p(b.0, b.1)], stroke);
    };
    let path = |points: &[(f32, f32)]| {
        painter.add(egui::Shape::line(
            points.iter().map(|&(x, y)| p(x, y)).collect(),
            stroke,
        ));
    };
    let outline = |a: (f32, f32), b: (f32, f32)| {
        painter.rect_stroke(
            egui::Rect::from_min_max(p(a.0, a.1), p(b.0, b.1)),
            0.5,
            stroke,
            egui::StrokeKind::Inside,
        );
    };
    match icon {
        Icon::Follow => {
            outline((9.0, 2.0), (15.0, 8.0));
            line((12.0, 8.0), (12.0, 12.0));
            path(&[(5.0, 16.0), (5.0, 12.0), (19.0, 12.0), (19.0, 16.0)]);
            outline((2.0, 16.0), (8.0, 22.0));
            outline((16.0, 16.0), (22.0, 22.0));
        }
        Icon::Snap => {
            path(&[
                (4.0, 3.0),
                (4.0, 14.0),
                (6.0, 19.0),
                (10.0, 21.0),
                (14.0, 21.0),
                (18.0, 19.0),
                (20.0, 14.0),
                (20.0, 3.0),
                (15.0, 3.0),
                (15.0, 14.0),
                (14.0, 16.0),
                (10.0, 16.0),
                (9.0, 14.0),
                (9.0, 3.0),
                (4.0, 3.0),
            ]);
            line((4.0, 8.0), (9.0, 8.0));
            line((15.0, 8.0), (20.0, 8.0));
        }
        Icon::Free => {
            line((12.0, 2.0), (12.0, 22.0));
            line((2.0, 12.0), (22.0, 12.0));
            path(&[(8.0, 6.0), (12.0, 2.0), (16.0, 6.0)]);
            path(&[(8.0, 18.0), (12.0, 22.0), (16.0, 18.0)]);
            path(&[(6.0, 8.0), (2.0, 12.0), (6.0, 16.0)]);
            path(&[(18.0, 8.0), (22.0, 12.0), (18.0, 16.0)]);
        }
        Icon::Menu => {
            for y in [5.0, 12.0, 19.0] {
                outline((1.0, y - 1.5), (4.0, y + 1.5));
                line((7.0, y), (23.0, y));
            }
        }
        Icon::Staging => {
            path(&[(2.0, 16.0), (2.0, 2.0), (16.0, 2.0)]);
            path(&[(6.0, 20.0), (6.0, 6.0), (20.0, 6.0)]);
            outline((10.0, 10.0), (23.0, 23.0));
            line((16.5, 13.0), (16.5, 20.0));
        }
        Icon::Help => {
            painter.circle_stroke(p(12.0, 12.0), side * 0.44, stroke);
            path(&[
                (8.0, 8.0),
                (9.5, 5.5),
                (14.5, 5.5),
                (16.0, 8.0),
                (15.0, 10.0),
                (12.0, 12.0),
                (12.0, 14.5),
            ]);
            painter.circle_filled(p(12.0, 18.0), stroke.width * 0.7, color);
        }
        Icon::Parts | Icon::New | Icon::Add => {
            if icon == Icon::Parts {
                outline((3.0, 3.0), (21.0, 21.0));
            }
            line((7.0, 12.0), (17.0, 12.0));
            line((12.0, 7.0), (12.0, 17.0));
            if icon == Icon::New {
                path(&[
                    (5.0, 20.0),
                    (3.0, 20.0),
                    (3.0, 2.0),
                    (17.0, 2.0),
                    (21.0, 6.0),
                    (21.0, 20.0),
                    (19.0, 20.0),
                ]);
            }
        }
        Icon::Ships => {
            path(&[
                (8.0, 20.0),
                (8.0, 8.0),
                (12.0, 2.0),
                (16.0, 8.0),
                (16.0, 20.0),
                (8.0, 20.0),
            ]);
            path(&[(8.0, 13.0), (4.0, 19.0), (4.0, 22.0), (8.0, 20.0)]);
            path(&[(16.0, 13.0), (20.0, 19.0), (20.0, 22.0), (16.0, 20.0)]);
            line((11.0, 22.0), (13.0, 22.0));
        }
        Icon::Undo | Icon::Redo => {
            let mirror = |x: f32| if icon == Icon::Redo { 24.0 - x } else { x };
            path(&[(mirror(8.0), 3.0), (mirror(2.0), 9.0), (mirror(8.0), 15.0)]);
            path(&[
                (mirror(3.0), 9.0),
                (mirror(14.0), 9.0),
                (mirror(20.0), 13.0),
                (mirror(20.0), 19.0),
                (mirror(16.0), 22.0),
            ]);
        }
        Icon::More => {
            for x in [4.0, 12.0, 20.0] {
                painter.circle_filled(p(x, 12.0), side / 12.0, color);
            }
        }
        Icon::Settings => {
            for (x, y) in [(6.0, 8.0), (12.0, 16.0), (18.0, 6.0)] {
                line((x, 2.0), (x, y - 2.0));
                line((x, y + 2.0), (x, 22.0));
                outline((x - 2.5, y - 2.0), (x + 2.5, y + 2.0));
            }
        }
        Icon::Fit | Icon::SelectAll => {
            for (x, y, dx, dy) in [
                (2.0, 2.0, 1.0, 1.0),
                (22.0, 2.0, -1.0, 1.0),
                (2.0, 22.0, 1.0, -1.0),
                (22.0, 22.0, -1.0, -1.0),
            ] {
                path(&[(x, y + 5.0 * dy), (x, y), (x + 5.0 * dx, y)]);
            }
            if icon == Icon::Fit {
                outline((8.0, 8.0), (16.0, 16.0));
            } else if icon == Icon::SelectAll {
                path(&[(7.0, 12.0), (10.0, 15.0), (17.0, 8.0)]);
            }
        }
        Icon::Delete => {
            line((3.0, 5.0), (21.0, 5.0));
            path(&[(8.0, 5.0), (8.0, 2.0), (16.0, 2.0), (16.0, 5.0)]);
            path(&[(5.0, 8.0), (6.0, 22.0), (18.0, 22.0), (19.0, 8.0)]);
            line((10.0, 9.0), (10.0, 18.0));
            line((14.0, 9.0), (14.0, 18.0));
        }
        Icon::Copy => {
            outline((3.0, 2.0), (16.0, 17.0));
            outline((8.0, 7.0), (21.0, 22.0));
        }
        Icon::Cut => {
            painter.circle_stroke(p(5.0, 18.0), side / 7.0, stroke);
            painter.circle_stroke(p(19.0, 18.0), side / 7.0, stroke);
            line((7.0, 15.0), (20.0, 2.0));
            line((17.0, 15.0), (4.0, 2.0));
        }
        Icon::Paste => {
            outline((5.0, 4.0), (20.0, 22.0));
            outline((9.0, 1.0), (16.0, 7.0));
            line((9.0, 12.0), (16.0, 12.0));
            line((9.0, 17.0), (16.0, 17.0));
        }
        Icon::Rotate | Icon::Expand => {
            path(&[
                (20.0, 9.0),
                (18.0, 4.0),
                (12.0, 2.0),
                (6.0, 4.0),
                (3.0, 10.0),
                (4.0, 17.0),
                (10.0, 21.0),
                (17.0, 20.0),
                (21.0, 16.0),
            ]);
            path(&[(21.0, 3.0), (21.0, 10.0), (14.0, 10.0)]);
            if icon == Icon::Expand {
                line((9.0, 12.0), (15.0, 12.0));
                line((12.0, 9.0), (12.0, 15.0));
            }
        }
        Icon::FlipX => {
            line((12.0, 1.0), (12.0, 23.0));
            path(&[(3.0, 19.0), (8.0, 5.0), (8.0, 19.0), (3.0, 19.0)]);
            path(&[(21.0, 19.0), (16.0, 5.0), (16.0, 19.0), (21.0, 19.0)]);
        }
        Icon::FlipY => {
            line((1.0, 12.0), (23.0, 12.0));
            path(&[(5.0, 3.0), (19.0, 8.0), (5.0, 8.0), (5.0, 3.0)]);
            path(&[(5.0, 21.0), (19.0, 16.0), (5.0, 16.0), (5.0, 21.0)]);
        }
        Icon::Cancel | Icon::Exit => {
            line((5.0, 5.0), (19.0, 19.0));
            line((19.0, 5.0), (5.0, 19.0));
            if icon == Icon::Exit {
                path(&[(21.0, 2.0), (23.0, 2.0), (23.0, 22.0), (21.0, 22.0)]);
            }
        }
        Icon::Open => {
            path(&[
                (2.0, 8.0),
                (2.0, 4.0),
                (10.0, 4.0),
                (13.0, 7.0),
                (22.0, 7.0),
                (22.0, 20.0),
                (2.0, 20.0),
                (2.0, 8.0),
                (22.0, 8.0),
            ]);
        }
        Icon::Save | Icon::SaveAs => {
            path(&[
                (3.0, 2.0),
                (18.0, 2.0),
                (22.0, 6.0),
                (22.0, 22.0),
                (3.0, 22.0),
                (3.0, 2.0),
            ]);
            outline((7.0, 2.0), (16.0, 9.0));
            outline((7.0, 14.0), (18.0, 22.0));
            if icon == Icon::SaveAs {
                line((15.0, 16.0), (21.0, 10.0));
            }
        }
        Icon::ChevronUp => path(&[(4.0, 16.0), (12.0, 8.0), (20.0, 16.0)]),
        Icon::ChevronDown => path(&[(4.0, 8.0), (12.0, 16.0), (20.0, 8.0)]),
        Icon::Drag => {
            for x in [8.0, 16.0] {
                for y in [5.0, 12.0, 19.0] {
                    painter.circle_filled(p(x, y), side / 20.0, color);
                }
            }
        }
        Icon::Target => {
            painter.circle_stroke(p(12.0, 12.0), side / 3.0, stroke);
            line((12.0, 0.0), (12.0, 7.0));
            line((12.0, 17.0), (12.0, 24.0));
            line((0.0, 12.0), (7.0, 12.0));
            line((17.0, 12.0), (24.0, 12.0));
        }
        Icon::Check => path(&[(3.0, 12.0), (9.0, 18.0), (21.0, 5.0)]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn original_style_toolbar_glyphs_are_vector_shapes_not_text() {
        let ctx = crate::egui_ui::test_context();
        let output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(240.0, 80.0),
                )),
                ..Default::default()
            },
            |ui| {
                for (index, icon) in [Icon::Menu, Icon::Staging, Icon::Rotate]
                    .into_iter()
                    .enumerate()
                {
                    paint(
                        ui.painter(),
                        egui::Rect::from_min_size(
                            egui::pos2(8.0 + index as f32 * 56.0, 8.0),
                            egui::Vec2::splat(44.0),
                        ),
                        icon,
                        egui::Color32::from_rgb(179, 219, 5),
                    );
                }
            },
        );
        fn is_text(shape: &egui::Shape) -> bool {
            match shape {
                egui::Shape::Text(_) => true,
                egui::Shape::Vec(shapes) => shapes.iter().any(is_text),
                _ => false,
            }
        }
        assert!(output.shapes.len() >= 9, "三种入口必须有实际矢量笔画");
        assert!(
            !output.shapes.iter().any(|shape| is_text(&shape.shape)),
            "图标不依赖 Unicode 字符或字体覆盖率"
        );
    }
}
