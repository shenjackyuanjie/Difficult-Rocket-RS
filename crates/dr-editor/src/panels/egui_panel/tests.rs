use super::*;

fn painted_preview_size(size: Vec2) -> egui::Vec2 {
    let ctx = egui::Context::default();
    let texture = egui::TextureId::User(321);
    let mut rendered = None;
    // 首次布局的测量 pass 不作为最终可见绘制依据。
    for _ in 0..2 {
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(300.0, 100.0),
                )),
                ..default()
            },
            |ui| {
                ui.add_sized(
                    [240.0, 52.0],
                    egui::Button::new((preview_image(texture, size), "分离器".to_owned())),
                );
            },
        );
        output.textures_delta.clear();
        for clipped in output.shapes {
            if let egui::epaint::Shape::Rect(rect) = clipped.shape
                && rect
                    .brush
                    .as_ref()
                    .is_some_and(|brush| brush.fill_texture_id == texture)
            {
                rendered = Some(rect.rect.size());
            }
        }
    }
    rendered.expect("按钮未绘制部件纹理")
}

#[test]
fn palette_image_paint_preserves_horizontal_vertical_and_square_texture_ratios() {
    for size in [
        Vec2::new(120.0, 30.0),
        Vec2::new(30.0, 120.0),
        Vec2::new(129.0, 30.0), // 原版 DetacherVertical.png
        Vec2::new(30.0, 100.0), // 原版 DetacherRadial.png
        Vec2::new(121.0, 31.0),
        Vec2::new(31.0, 121.0),
        Vec2::new(37.0, 53.0),
        Vec2::splat(32.0),
    ] {
        let painted = painted_preview_size(size);
        let expected = size * (40.0 / size.max_element());
        // egui 会将绘制边界对齐到像素，允许不足一像素的量化误差。
        assert!(
            (painted.x - expected.x).abs() < 1.01,
            "{size:?} -> {painted:?}"
        );
        assert!(
            (painted.y - expected.y).abs() < 1.01,
            "{size:?} -> {painted:?}"
        );
        assert!(painted.x <= 40.01 && painted.y <= 40.01);
    }
}

#[test]
fn unloaded_texture_fallback_uses_catalog_proportions_instead_of_a_square() {
    let mut kind = crate::panels::tests::catalog().get("pod").unwrap().clone();
    kind.width = 4;
    kind.height = 1;
    let size = render::fallback_size(Some(&kind));
    assert_eq!(painted_preview_size(size), egui::vec2(40.0, 10.0));
    kind.width = 1;
    kind.height = 4;
    let size = render::fallback_size(Some(&kind));
    assert_eq!(painted_preview_size(size), egui::vec2(10.0, 40.0));
}
