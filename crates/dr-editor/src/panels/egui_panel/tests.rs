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
                part_row(ui, texture, size, "分离器".to_owned(), false, 240.0);
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

#[derive(Debug)]
struct PaintedRow {
    button: egui::Rect,
    image: egui::Rect,
    text: egui::Rect,
    clicked: bool,
    drag_started: bool,
}

fn row_frame(ctx: &egui::Context, size: Vec2, name: &str, events: Vec<egui::Event>) -> PaintedRow {
    let texture = egui::TextureId::User(321);
    let mut response = None;
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(300.0, 100.0),
            )),
            focused: true,
            events,
            ..default()
        },
        |ui| {
            response = Some(part_row(ui, texture, size, name.to_owned(), false, 240.0));
        },
    );
    output.textures_delta.clear();
    let mut image = None;
    let mut text = None;
    for clipped in output.shapes {
        match clipped.shape {
            egui::epaint::Shape::Rect(rect)
                if rect
                    .brush
                    .as_ref()
                    .is_some_and(|brush| brush.fill_texture_id == texture) =>
            {
                image = Some(rect.rect);
            }
            egui::epaint::Shape::Text(shape) if shape.galley.job.text == name => {
                text = Some(egui::Rect::from_min_size(shape.pos, shape.galley.size()));
            }
            _ => {}
        }
    }
    let response = response.unwrap();
    PaintedRow {
        button: response.rect,
        image: image.expect("图片列未绘制"),
        text: text.expect("名称列未绘制"),
        clicked: response.clicked(),
        drag_started: response.drag_started(),
    }
}

#[test]
fn palette_columns_keep_name_left_aligned_and_image_center_fixed() {
    let mut baseline: Option<(f32, f32)> = None;
    for (size, name) in [
        (Vec2::new(129.0, 30.0), "Detacher"),
        (Vec2::new(30.0, 100.0), "Side Detacher"),
        (Vec2::splat(32.0), "Wheel"),
        (Vec2::new(30.0, 180.0), "Sloshy T6000"),
        (
            Vec2::new(129.0, 30.0),
            "名称很长的部件 Long Long Long Long Long Long Long Long",
        ),
    ] {
        let ctx = egui::Context::default();
        row_frame(&ctx, size, name, vec![]);
        let row = row_frame(&ctx, size, name, vec![]);
        let positions = (row.image.center().x, row.text.left());
        if let Some((image_center, text_left)) = baseline {
            assert!(
                (positions.0 - image_center).abs() < 1.01,
                "{name}: {positions:?}"
            );
            assert!(
                (positions.1 - text_left).abs() < 1.01,
                "{name}: {positions:?}"
            );
        } else {
            baseline = Some(positions);
        }
        assert!(row.image.right() < row.text.left());
        assert!(
            row.text.right() <= row.button.right() + 0.01,
            "名称未截断：{name} {row:?}"
        );
        assert!((row.button.width() - 240.0).abs() < 0.01);
        assert!((row.button.height() - 52.0).abs() < 0.01);
    }
}

#[test]
fn clicking_image_or_name_selects_the_same_whole_row_button() {
    let ctx = egui::Context::default();
    let size = Vec2::new(30.0, 100.0);
    row_frame(&ctx, size, "Side Detacher", vec![]);
    let row = row_frame(&ctx, size, "Side Detacher", vec![]);
    for point in [row.image.center(), row.text.center()] {
        for pressed in [true, false] {
            let result = row_frame(
                &ctx,
                size,
                "Side Detacher",
                vec![
                    egui::Event::PointerMoved(point),
                    egui::Event::PointerButton {
                        pos: point,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            );
            assert_eq!(result.clicked, !pressed);
        }
    }
}

#[test]
fn dragging_from_the_name_column_starts_a_palette_gesture_without_a_click() {
    let ctx = egui::Context::default();
    let size = Vec2::new(129.0, 30.0);
    row_frame(&ctx, size, "Detacher", vec![]);
    let row = row_frame(&ctx, size, "Detacher", vec![]);
    let origin = row.text.center();
    row_frame(
        &ctx,
        size,
        "Detacher",
        vec![
            egui::Event::PointerMoved(origin),
            egui::Event::PointerButton {
                pos: origin,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ],
    );
    let moved = origin + egui::vec2(-80.0, 70.0);
    let row = row_frame(
        &ctx,
        size,
        "Detacher",
        vec![egui::Event::PointerMoved(moved)],
    );
    assert!(row.drag_started);
    assert!(!row.clicked);
    let row = row_frame(
        &ctx,
        size,
        "Detacher",
        vec![egui::Event::PointerButton {
            pos: moved,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }],
    );
    assert!(!row.clicked);
}

#[test]
fn disabled_directory_button_keeps_prerequisite_tooltip_and_no_hitmap() {
    let ctx = egui::Context::default();
    let mut rect = egui::Rect::NOTHING;
    let mut saw_hint = false;
    let mut stable_id = None;
    for frame in 0..6 {
        let mut state = UiState::default();
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(600.0, 240.0),
                )),
                time: Some(f64::from(frame)),
                // 移入一次后推进静止帧；重复 Move 会重置 egui 的悬停等待。
                events: if frame == 2 {
                    vec![egui::Event::PointerMoved(rect.center())]
                } else {
                    vec![]
                },
                ..Default::default()
            },
            |ui| {
                ui.disable();
                // 同一动作改短标签不应改变 egui 控件身份。
                let response = directory_button(
                    ui,
                    &PanelButton::Refresh,
                    false,
                    if frame % 2 == 0 {
                        "刷新"
                    } else {
                        "重新扫描"
                    },
                );
                if let Some(id) = stable_id {
                    assert_eq!(id, response.id);
                }
                stable_id = Some(response.id);
                rect = response.rect;
                assert!(!response.clicked());
                hit(ui, &response, PanelButton::Refresh, &mut state);
            },
        );
        output.textures_delta.clear();
        assert!(state.hits.is_empty());
        fn has_hint(shape: &egui::epaint::Shape) -> bool {
            match shape {
                egui::epaint::Shape::Text(text) => {
                    text.galley.job.text.contains("请先应用或取消属性草稿")
                }
                egui::epaint::Shape::Vec(shapes) => shapes.iter().any(has_hint),
                _ => false,
            }
        }
        saw_hint |= output.shapes.iter().any(|shape| has_hint(&shape.shape));
    }
    assert!(saw_hint, "禁用目录按钮必须解释被草稿 / 文件确认阻塞的条件");
}
