use crate::icons::*;

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
