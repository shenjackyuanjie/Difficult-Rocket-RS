use super::*;

fn painted_preview_size(size: Vec2) -> egui::Vec2 {
    let ctx = crate::egui_ui::test_context();
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
        let expected = size * (48.0 / size.max_element());
        // egui 会将绘制边界对齐到像素，允许不足一像素的量化误差。
        assert!(
            (painted.x - expected.x).abs() < 1.01,
            "{size:?} -> {painted:?}"
        );
        assert!(
            (painted.y - expected.y).abs() < 1.01,
            "{size:?} -> {painted:?}"
        );
        assert!(painted.x <= 48.01 && painted.y <= 48.01);
    }
}

#[test]
fn unloaded_texture_fallback_uses_catalog_proportions_instead_of_a_square() {
    let mut kind = crate::panels::tests::catalog().get("pod").unwrap().clone();
    kind.width = 4;
    kind.height = 1;
    let size = render::fallback_size(Some(&kind));
    assert_eq!(painted_preview_size(size), egui::vec2(48.0, 12.0));
    kind.width = 1;
    kind.height = 4;
    let size = render::fallback_size(Some(&kind));
    assert_eq!(painted_preview_size(size), egui::vec2(12.0, 48.0));
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
        let ctx = crate::egui_ui::test_context();
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
        assert!((row.button.height() - 64.0).abs() < 0.01);
    }
}

#[test]
fn clicking_image_or_name_selects_the_same_whole_row_button() {
    let ctx = crate::egui_ui::test_context();
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
    let ctx = crate::egui_ui::test_context();
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
    let ctx = crate::egui_ui::test_context();
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

#[test]
fn follow_toggle_real_pointer_clicks_toggle_and_disabled_input_is_ignored_with_direction_hint() {
    fn frame(
        ctx: &egui::Context,
        follow: &mut bool,
        enabled: bool,
        time: f64,
        events: Vec<egui::Event>,
    ) -> (egui::Rect, bool, bool) {
        let mut rect = egui::Rect::NOTHING;
        let mut changed = false;
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(640.0, 240.0),
                )),
                time: Some(time),
                focused: true,
                events,
                ..Default::default()
            },
            |ui| {
                ui.add_enabled_ui(enabled, |ui| {
                    let response = follow_toggle(ui, follow);
                    rect = response.rect;
                    changed |= response.changed();
                    assert_eq!(response.enabled(), enabled);
                });
            },
        );
        output.textures_delta.clear();
        fn has_direction_hint(shape: &egui::epaint::Shape) -> bool {
            match shape {
                egui::epaint::Shape::Text(text) => {
                    text.galley.job.text.contains("后代")
                        && text.galley.job.text.contains("按连接方向")
                }
                egui::epaint::Shape::Vec(shapes) => shapes.iter().any(has_direction_hint),
                _ => false,
            }
        }
        let hint = output
            .shapes
            .iter()
            .any(|shape| has_direction_hint(&shape.shape));
        (rect, changed, hint)
    }
    fn pointer(pos: egui::Pos2, pressed: bool) -> Vec<egui::Event> {
        vec![
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            },
        ]
    }

    let ctx = crate::egui_ui::test_context();
    let mut follow = false;
    let mut rect = egui::Rect::NOTHING;
    for index in 0..3 {
        rect = frame(&ctx, &mut follow, true, f64::from(index) * 0.016, vec![]).0;
    }
    for (index, expected) in [true, false].into_iter().enumerate() {
        let time = 0.1 + index as f64 * 0.1;
        let before = follow;
        let pos = rect.center();
        let (_, changed, _) = frame(&ctx, &mut follow, true, time, pointer(pos, true));
        assert!(!changed);
        assert_eq!(follow, before, "按下不能提前切换开关");
        let (next_rect, changed, _) =
            frame(&ctx, &mut follow, true, time + 0.016, pointer(pos, false));
        rect = next_rect;
        assert!(changed, "真实按下 / 松开应修改开关");
        assert_eq!(follow, expected);
    }
    for (index, initial) in [false, true].into_iter().enumerate() {
        follow = initial;
        let time = 1.0 + index as f64;
        rect = frame(&ctx, &mut follow, false, time, vec![]).0;
        for (offset, pressed) in [(0.016, true), (0.032, false)] {
            let (_, changed, _) = frame(
                &ctx,
                &mut follow,
                false,
                time + offset,
                pointer(rect.center(), pressed),
            );
            assert!(!changed);
            assert_eq!(follow, initial, "禁用时点击不能改变跟随设置");
        }
    }

    // 新上下文只做真实悬停，避免点击抑制 tooltip 的时序影响。
    let hover_ctx = crate::egui_ui::test_context();
    let mut saw_hint = false;
    for index in 0..6 {
        let events = if index == 2 {
            vec![egui::Event::PointerMoved(rect.center())]
        } else {
            vec![]
        };
        let (next_rect, changed, hint) =
            frame(&hover_ctx, &mut follow, true, f64::from(index), events);
        rect = next_rect;
        assert!(!changed);
        saw_hint |= hint;
    }
    assert!(
        saw_hint,
        "跟随开关实际 tooltip 必须说明后代及按连接方向计算"
    );
}

#[test]
fn deletion_hint_reuses_preview_frames_and_refreshes_roots_and_undo_redo() {
    let mut document = crate::tests::document();
    document.catalog = crate::panels::tests::catalog();
    let kind = document.catalog.get("pod").unwrap();
    document.ship.parts = (1..=3)
        .map(|id| kind.instantiate(id, (id as f64 * 2.0, 0.0)))
        .collect();
    document.ship.connections = vec![Connection::Normal {
        parent: 1,
        child: 2,
        parent_attach: 1,
        child_attach: 1,
    }];
    document.refresh();
    let mut state = UiState::default();
    let mut drag = DragState {
        id: Some(PartKey::new(0, 1, 0)),
        ..default()
    };
    let expected = "× 拖入此处删除 2 个相连部件";
    assert_eq!(state.deletion_text(&document, &drag), expected);
    let cached_text = state.deletion_text(&document, &drag).as_ptr();
    for index in 0..30 {
        drag.preview = (index as f64 * 0.1, 0.2);
        drag.turns = index % 4;
        assert_eq!(state.deletion_text(&document, &drag), expected);
        assert_eq!(state.deletion_text(&document, &drag).as_ptr(), cached_text);
    }
    drag.members.insert(PartKey::new(0, 3, 0));
    assert_eq!(
        state.deletion_text(&document, &drag),
        "× 拖入此处删除 3 个相连部件"
    );
    drag.members.clear();
    assert_eq!(state.deletion_text(&document, &drag), expected);
    assert!(document.execute(EditorCommand::Disconnect(1)));
    assert_eq!(
        state.deletion_text(&document, &drag),
        "× 拖入此处删除 1 个相连部件"
    );
    assert!(document.undo());
    assert_eq!(state.deletion_text(&document, &drag), expected);
    assert!(document.redo());
    assert_eq!(
        state.deletion_text(&document, &drag),
        "× 拖入此处删除 1 个相连部件"
    );
}

#[test]
fn deletion_hint_caches_errors_and_refreshes_after_repair_or_document_replacement() {
    let mut document = crate::tests::document();
    document.ship.connections.push(Connection::Normal {
        parent: 1,
        child: 99,
        parent_attach: 1,
        child_attach: 1,
    });
    document.refresh();
    let mut state = UiState::default();
    let drag = DragState {
        id: Some(PartKey::new(0, 1, 0)),
        ..default()
    };
    assert_eq!(
        state.deletion_text(&document, &drag),
        "× 连接引用有歧义，无法删除"
    );
    let cached_text = state.deletion_text(&document, &drag).as_ptr();
    assert_eq!(state.deletion_text(&document, &drag).as_ptr(), cached_text);
    document.ship.connections.clear();
    document.refresh();
    assert_eq!(
        state.deletion_text(&document, &drag),
        "× 拖入此处删除 1 个相连部件"
    );
    let mut replacement = document.ship.clone();
    let mut part = replacement.parts[0].clone();
    part.id = 2;
    replacement.parts.push(part);
    replacement.connections.push(Connection::Dock {
        parent: 1,
        child: 2,
        dock: 2,
    });
    document.ship = replacement;
    document.refresh();
    assert_eq!(
        state.deletion_text(&document, &drag),
        "× 拖入此处删除 2 个相连部件"
    );
}

/// 与 `draw` 相同的顶部工具栏 / 头部 Panel / 单侧 drawer 容器布局。
/// drawer 正文只放占位标签：本回归隔离头部与容器，不假装覆盖完整侧栏内容。
#[derive(Debug)]
struct LayoutFrame {
    toolbar: egui::Rect,
    header: egui::Rect,
    left: Option<egui::Rect>,
    right: Option<egui::Rect>,
    canvas: egui::Rect,
    areas: Vec<egui::Rect>,
    actions: Vec<PanelButton>,
}

/// 头部入口全部可用的布局 pass：可撤销 / 可重做 / 有选中 / 剪贴板有内容 / 有待提交编辑。
fn header_pass(root: &mut egui::Ui, state: &mut UiState, screen: egui::Vec2) -> LayoutFrame {
    let ctx = root.ctx().clone();
    let viewport = ctx.viewport_rect();
    let width = screen.x;
    let compact = crate::panels::compact(width);
    // 与 draw 相同：窄屏把交互尺寸提升到 44；头部 Panel 首帧高度也取自这里。
    let original = *state
        .default_interact_size
        .get_or_insert(ctx.style_of(ctx.theme()).spacing.interact_size);
    let interact_size = if compact {
        original.max(egui::Vec2::splat(44.0))
    } else {
        original
    };
    if ctx.style_of(ctx.theme()).spacing.interact_size != interact_size {
        ctx.style_mut_of(ctx.theme(), |style| {
            style.spacing.interact_size = interact_size
        });
    }
    state.hits.clear();
    state.areas.clear();
    state.palette_area = None;
    state.canvas_area = None;
    state.ship_toggle = None;
    state.part_toggle = None;
    let mut viewport_ui = egui::Ui::new(
        ctx,
        egui::Id::new("directory_viewport"),
        egui::UiBuilder::new()
            .layer_id(egui::LayerId::background())
            .max_rect(viewport),
    );
    let toolbar = reserve_native_toolbar(&mut viewport_ui, width);
    state.areas.push(toolbar.rect);
    let header = egui::Panel::top("directory_navigation").show(&mut viewport_ui, |ui| {
        directory_header(ui, state, compact, true, true, true, true, true, true)
    });
    state.areas.push(header.response.rect);
    let mut left = None;
    if !compact || state.drawer == Some(Drawer::Ships) {
        let panel = egui::Panel::left("ship_browser_sidebar")
            .exact_size(sidebar_width(width, 248.0))
            .resizable(false)
            .show(&mut viewport_ui, |ui| {
                ui.label("船体目录占位");
            });
        state.areas.push(panel.response.rect);
        left = Some(panel.response.rect);
    }
    let mut right = None;
    if !compact || state.drawer == Some(Drawer::Parts) {
        let panel = egui::Panel::right("part_palette_sidebar")
            .exact_size(sidebar_width(width, 296.0))
            .resizable(false)
            .show(&mut viewport_ui, |ui| {
                ui.label("部件目录占位");
            });
        state.areas.push(panel.response.rect);
        state.palette_area = Some(panel.response.rect);
        right = state.palette_area;
    }
    let mut canvas = viewport_ui.available_rect_before_wrap();
    if !compact {
        canvas.min.y = canvas.min.y.max(220.0);
    }
    state.canvas_area = Some(canvas);
    LayoutFrame {
        toolbar: toolbar.rect,
        header: header.response.rect,
        left,
        right,
        canvas,
        areas: state.areas.clone(),
        actions: header.inner,
    }
}

/// 一帧头部 / 容器布局，返回该帧矩形与绘制输出。
fn header_frame(
    ctx: &egui::Context,
    state: &mut UiState,
    screen: egui::Vec2,
    events: Vec<egui::Event>,
    time: f64,
) -> (LayoutFrame, egui::FullOutput) {
    let mut frame = None;
    let output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, screen)),
            focused: true,
            time: Some(time),
            events,
            ..default()
        },
        |root| {
            frame = Some(header_pass(root, state, screen));
        },
    );
    (frame.expect("头部布局必须执行"), output)
}

/// 真实指针按下与抬起两帧；返回抬起帧的布局。
fn click_header(
    ctx: &egui::Context,
    state: &mut UiState,
    screen: egui::Vec2,
    position: egui::Pos2,
    time: f64,
) -> (LayoutFrame, egui::FullOutput) {
    header_frame(
        ctx,
        state,
        screen,
        vec![
            egui::Event::PointerMoved(position),
            egui::Event::PointerButton {
                pos: position,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ],
        time,
    );
    header_frame(
        ctx,
        state,
        screen,
        vec![egui::Event::PointerButton {
            pos: position,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }],
        time + 0.016,
    )
}

/// 绘制输出里某个标签文字的矩形；控件矩形必定包含它。
fn label_rect(output: &egui::FullOutput, want: &str) -> Option<egui::Rect> {
    fn visit(shape: &egui::epaint::Shape, want: &str, found: &mut Option<egui::Rect>) {
        match shape {
            egui::epaint::Shape::Text(text) if text.galley.job.text == want => {
                *found = Some(egui::Rect::from_min_size(text.pos, text.galley.size()));
            }
            egui::epaint::Shape::Vec(shapes) => {
                for shape in shapes {
                    visit(shape, want, found);
                }
            }
            _ => {}
        }
    }
    let mut found = None;
    for clipped in &output.shapes {
        visit(&clipped.shape, want, &mut found);
    }
    found
}

#[test]
fn directory_header_controls_fit_four_viewports_and_narrow_layouts_keep_a_full_width_canvas() {
    for (width, height) in [
        (320.0, 640.0),
        (390.0, 844.0),
        (640.0, 360.0),
        (1440.0, 900.0),
    ] {
        let screen = egui::vec2(width, height);
        let viewport = egui::Rect::from_min_size(egui::Pos2::ZERO, screen);
        let compact = crate::panels::compact(width);
        let size = format!("{width}×{height}");
        let ctx = crate::egui_ui::test_context();
        let mut state = UiState::default();
        // 头部 Panel 首帧按交互尺寸估高，先学到真实内容高度再取控件矩形。
        for pass in 0..2 {
            header_frame(&ctx, &mut state, screen, vec![], f64::from(pass) * 0.016);
        }
        let (frame, output) = header_frame(&ctx, &mut state, screen, vec![], 0.1);

        // 原生顶栏与头部各占满宽度，头部紧接其下且够容纳一行 44 高控件。
        assert!(
            viewport.contains_rect(frame.toolbar),
            "{size} 工具栏越界：{:?}",
            frame.toolbar
        );
        assert!(
            (frame.toolbar.height() - crate::panels::toolbar_height(width)).abs() < 0.51,
            "{size} 工具栏高度 {} 与 panels::toolbar_height 不一致",
            frame.toolbar.height()
        );
        assert!(
            viewport.contains_rect(frame.header),
            "{size} 头部越界：{:?}",
            frame.header
        );
        assert!(
            frame.header.top() >= frame.toolbar.bottom() - 0.51,
            "{size} 头部必须紧接工具栏下方"
        );
        assert!(
            frame.header.height() >= 44.0,
            "{size} 头部高度 {} 不足一行 44 高控件",
            frame.header.height()
        );

        // 有 hit 记录的头部控件都占满 44 高并落在屏幕内。
        // 「适配视野」是本行里唯一没有 hit 记录的按钮，其交互由下面的真实点击确认。
        for (action, rect) in &state.hits {
            assert!(
                rect.height() >= 44.0 - 0.01,
                "{size} {action:?} 控件高度 {} 不足 44",
                rect.height()
            );
            assert!(rect.width() > 0.0, "{size} {action:?} 控件没有宽度");
            assert!(
                frame.header.contains_rect(*rect),
                "{size} {action:?} 控件超出头部：{rect:?}"
            );
            assert!(
                viewport.contains_rect(*rect),
                "{size} {action:?} 控件超出屏幕：{rect:?}"
            );
        }
        // 导航、事务与剪贴板入口都注册了可点击矩形。
        for action in [
            PanelButton::Undo,
            PanelButton::Redo,
            PanelButton::Delete,
            PanelButton::SelectAll,
            PanelButton::Copy,
            PanelButton::Cut,
            PanelButton::Paste,
            PanelButton::CancelPlacement,
        ] {
            assert!(
                state.hits.iter().any(|(hit, _)| *hit == action),
                "{size} 头部缺少 {action:?} 入口"
            );
        }

        if compact {
            // 窄屏默认收起两侧目录，画布保留全宽。
            assert!(
                frame.left.is_none() && frame.right.is_none(),
                "{size} 窄屏默认不应展开目录"
            );
            assert!(
                state.palette_area.is_none(),
                "{size} 窄屏默认不应有部件目录区域"
            );
            assert_eq!(frame.areas.len(), 2, "{size} 只应有工具栏与头部两个区域");
            assert!((frame.canvas.left() - viewport.left()).abs() < 0.51);
            assert!((frame.canvas.right() - viewport.right()).abs() < 0.51);
            assert!(
                (frame.canvas.width() - width).abs() < 0.51,
                "{size} 画布未保留全宽：{:?}",
                frame.canvas
            );
            assert!(frame.canvas.min.y >= frame.header.bottom() - 0.51);
        } else {
            // 宽屏默认展开两侧目录，画布落在两者之间。
            let left = frame.left.expect("宽屏默认展开船体目录");
            let right = frame.right.expect("宽屏默认展开部件目录");
            assert!(
                (left.width() - 248.0).abs() < 0.51,
                "{size} 船体目录宽度 {}",
                left.width()
            );
            assert!(
                (right.width() - 296.0).abs() < 0.51,
                "{size} 部件目录宽度 {}",
                right.width()
            );
            assert!((left.left() - viewport.left()).abs() < 0.51);
            assert!((right.right() - viewport.right()).abs() < 0.51);
            assert!((frame.canvas.left() - left.right()).abs() < 0.51);
            assert!((frame.canvas.right() - right.left()).abs() < 0.51);
            assert!(
                frame.canvas.min.y >= 220.0 - 0.51,
                "{size} 宽屏画布顶部不得高于 220"
            );
            assert_eq!(
                state.palette_area,
                Some(right),
                "{size} 部件目录区域必须与右侧容器一致"
            );
            assert_eq!(
                frame.areas.len(),
                4,
                "{size} 应为工具栏、头部与两侧目录四个区域"
            );
        }
        assert!(
            frame.canvas.height() > 0.0 && viewport.contains_rect(frame.canvas),
            "{size} 画布越界：{:?}",
            frame.canvas
        );

        // 点击「适配视野」的标签必须请求适配视野，且不产生目录动作。
        let label = label_rect(&output, "适配视野")
            .unwrap_or_else(|| panic!("{size} 头部未绘制适配视野标签"));
        assert!(
            frame.header.contains_rect(label) && viewport.contains_rect(label),
            "{size} 适配视野标签越界：{label:?}"
        );
        state.fit_requested = false;
        let (frame, _) = click_header(&ctx, &mut state, screen, label.center(), 0.4);
        assert!(state.fit_requested, "{size} 点击适配视野必须请求适配视野");
        assert!(frame.actions.is_empty(), "{size} 适配视野不产生目录动作");
    }
}

#[test]
fn single_drawer_toggle_keeps_one_sidebar_and_restores_the_full_width_canvas() {
    for (width, height) in [(320.0, 640.0), (390.0, 844.0), (640.0, 360.0)] {
        let screen = egui::vec2(width, height);
        let viewport = egui::Rect::from_min_size(egui::Pos2::ZERO, screen);
        let size = format!("{width}×{height}");
        let ctx = crate::egui_ui::test_context();
        let mut state = UiState::default();
        // 同上：先让头部 Panel 学到真实内容高度，再读抽屉入口矩形。
        for pass in 0..2 {
            header_frame(&ctx, &mut state, screen, vec![], f64::from(pass) * 0.016);
        }
        let (frame, _) = header_frame(&ctx, &mut state, screen, vec![], 0.1);
        assert!(crate::panels::compact(width) && state.drawer.is_none());
        assert!(frame.left.is_none() && frame.right.is_none());
        let ship = state.ship_toggle.expect("窄屏头部提供船体抽屉入口");
        let parts = state.part_toggle.expect("窄屏头部提供部件抽屉入口");
        for rect in [ship, parts] {
            assert!(
                rect.height() >= 44.0 - 0.01,
                "{size} 抽屉入口高度 {} 不足 44",
                rect.height()
            );
            assert!(
                viewport.contains_rect(rect) && frame.header.contains_rect(rect),
                "{size} 抽屉入口越界：{rect:?}"
            );
        }

        // 开船体：只出现左侧目录，画布从它的右边缘开始。
        let (frame, _) = click_header(&ctx, &mut state, screen, ship.center(), 0.4);
        let left = frame.left.expect("点击船体必须展开左侧目录");
        assert_eq!(state.drawer, Some(Drawer::Ships), "{size} 船体抽屉未打开");
        assert!(
            frame.right.is_none() && state.palette_area.is_none(),
            "{size} 船体抽屉不得同时展开部件目录"
        );
        assert_eq!(frame.areas.len(), 3, "{size} 单侧抽屉应只有三个区域");
        assert!((frame.canvas.left() - left.right()).abs() < 0.51);
        assert!(
            (frame.canvas.width() - (width - left.width())).abs() < 0.51,
            "{size} 单侧 drawer 的画布宽度错误：{:?}",
            frame.canvas
        );

        // 切部件：船体目录收起，只剩右侧目录。
        let (frame, _) = click_header(&ctx, &mut state, screen, parts.center(), 0.9);
        let right = frame.right.expect("点击部件必须展开右侧目录");
        assert_eq!(state.drawer, Some(Drawer::Parts), "{size} 部件抽屉未打开");
        assert!(frame.left.is_none(), "{size} 部件抽屉不得同时保留船体目录");
        assert_eq!(state.palette_area, Some(right));
        assert!((frame.canvas.right() - right.left()).abs() < 0.51);
        assert!((frame.canvas.left() - viewport.left()).abs() < 0.51);

        // 再点船体：仍然只有一个抽屉。
        let (frame, _) = click_header(&ctx, &mut state, screen, ship.center(), 1.4);
        assert_eq!(state.drawer, Some(Drawer::Ships));
        assert!(frame.left.is_some() && frame.right.is_none() && state.palette_area.is_none());

        // 再点一次收起：画布恢复全宽。
        let (frame, _) = click_header(&ctx, &mut state, screen, ship.center(), 1.9);
        assert_eq!(state.drawer, None, "{size} 再次点击应收起抽屉");
        assert!(frame.left.is_none() && frame.right.is_none() && state.palette_area.is_none());
        assert!(
            (frame.canvas.width() - width).abs() < 0.51,
            "{size} 收起抽屉后画布未恢复全宽：{:?}",
            frame.canvas
        );
    }
}

/// 沿用目录行现成的 `part_row` + ScrollArea 最小框架。
#[derive(Debug, Default)]
struct PaletteGesture {
    clicked: bool,
    drag_started: bool,
    contains_pointer: bool,
    offset: f32,
    first_row: Option<(egui::Id, egui::Rect)>,
}

/// 300×240 屏幕、200 高视口里的 12 行 52 高目录行。
fn palette_scroll_frame(
    ctx: &egui::Context,
    events: Vec<egui::Event>,
    time: f64,
) -> PaletteGesture {
    let texture = egui::TextureId::User(321);
    let mut gesture = PaletteGesture::default();
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(300.0, 240.0),
            )),
            focused: true,
            time: Some(time),
            events,
            ..default()
        },
        |ui| {
            let scroll_id = ui.make_persistent_id(egui::IdSalt::new("touch_palette_rows"));
            let scroll = egui::ScrollArea::vertical()
                .id_salt("touch_palette_rows")
                .max_height(200.0)
                .auto_shrink([false, false]);
            let output = scroll.show(ui, |ui| {
                touch_scroll_area(ui, scroll_id);
                for index in 0..12 {
                    let response = part_row(
                        ui,
                        texture,
                        Vec2::new(30.0, 100.0),
                        format!("部件 {index}"),
                        false,
                        240.0,
                    );
                    if index == 0 {
                        gesture.first_row =
                            Some((response.id, response.rect.intersect(ui.clip_rect())));
                    }
                    gesture.clicked |= response.clicked();
                    gesture.drag_started |= response.drag_started();
                    gesture.contains_pointer |= response.contains_pointer();
                }
            });
            gesture.offset = output.state.offset.y;
        },
    );
    output.textures_delta.clear();
    gesture
}

/// bevy_egui 的 `write_touch_message` 先发 `Event::Touch`，再同步 PointerMoved / PointerButton。
fn touch_event(phase: egui::TouchPhase, pos: egui::Pos2) -> egui::Event {
    egui::Event::Touch {
        device_id: egui::TouchDeviceId(1),
        id: egui::TouchId(7),
        phase,
        pos,
        force: None,
    }
}

fn touch_press(pos: egui::Pos2) -> Vec<egui::Event> {
    vec![
        touch_event(egui::TouchPhase::Start, pos),
        egui::Event::PointerMoved(pos),
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        },
    ]
}

fn touch_release(pos: egui::Pos2) -> Vec<egui::Event> {
    vec![
        touch_event(egui::TouchPhase::End, pos),
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        },
        egui::Event::PointerGone,
    ]
}

#[test]
fn touch_scroll_swipe_on_palette_rows_never_clicks_or_starts_a_drag() {
    let ctx = crate::egui_ui::test_context();
    let mut time = 0.0;
    for _ in 0..3 {
        palette_scroll_frame(&ctx, vec![], time);
        time += 0.016;
    }
    let start = egui::pos2(150.0, 180.0);
    let mut gesture = PaletteGesture::default();
    let mut frames = vec![touch_press(start)];
    for index in 1..=5 {
        let pos = egui::pos2(150.0, 180.0 - index as f32 * 20.0);
        frames.push(vec![
            touch_event(egui::TouchPhase::Move, pos),
            egui::Event::PointerMoved(pos),
        ]);
    }
    frames.push(touch_release(egui::pos2(150.0, 80.0)));
    for events in frames {
        time += 0.016;
        let frame = palette_scroll_frame(&ctx, events, time);
        gesture.clicked |= frame.clicked;
        gesture.drag_started |= frame.drag_started;
        gesture.contains_pointer |= frame.contains_pointer;
        gesture.offset = frame.offset;
    }
    // 与 bevy_egui 同序的触摸指针必须让行收到指针，同时把滑动交给 ScrollArea。
    assert!(
        gesture.contains_pointer,
        "同步的 PointerMoved 必须让目录行收到指针"
    );
    assert!(!gesture.clicked, "触摸滚动不能产生目录行点击");
    assert!(!gesture.drag_started, "触摸滚动不能从目录行开始拖拽");
    assert!(
        gesture.offset > 20.0,
        "首轮触摸滑动必须实际滚动列表，偏移 {}",
        gesture.offset
    );
}

#[test]
fn touch_events_without_synchronized_pointer_events_neither_scroll_nor_click() {
    let ctx = crate::egui_ui::test_context();
    let mut time = 0.0;
    for _ in 0..3 {
        palette_scroll_frame(&ctx, vec![], time);
        time += 0.016;
    }
    let start = egui::pos2(150.0, 180.0);
    let mut gesture = PaletteGesture::default();
    let mut frames = vec![vec![touch_event(egui::TouchPhase::Start, start)]];
    for index in 1..=5 {
        frames.push(vec![touch_event(
            egui::TouchPhase::Move,
            egui::pos2(150.0, 180.0 - index as f32 * 20.0),
        )]);
    }
    frames.push(vec![touch_event(
        egui::TouchPhase::End,
        egui::pos2(150.0, 80.0),
    )]);
    for events in frames {
        time += 0.016;
        let frame = palette_scroll_frame(&ctx, events, time);
        gesture.clicked |= frame.clicked;
        gesture.drag_started |= frame.drag_started;
        gesture.offset = frame.offset;
    }
    assert!(!gesture.clicked && !gesture.drag_started);
    assert!(
        !gesture.contains_pointer,
        "只有 Event::Touch 时 egui 没有指针位置，目录行收不到指针"
    );
    assert!(
        gesture.offset.abs() < 0.5,
        "缺少同步指针事件的触摸序列不会滚动，实际偏移 {}",
        gesture.offset
    );
}

/// 复现真实回放的秒级 pass 间隔；固定控件身份和命中位置，不放宽长按阈值。
#[test]
fn palette_short_replay_click_selects_once_but_one_second_hold_is_not_a_click() {
    for (hold, should_click) in [(0.045, true), (1.0, false)] {
        let ctx = crate::egui_ui::test_context();
        let mut row = PaletteGesture::default();
        for time in [0.0, 0.016, 0.032] {
            row = palette_scroll_frame(&ctx, vec![], time);
        }
        let identity = row.first_row.expect("预热后必须存在可见的首个目录行");
        assert!(identity.1.is_positive());
        let pos = identity.1.center();
        let pointer_button = |pressed| {
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ]
        };
        let pressed = palette_scroll_frame(&ctx, pointer_button(true), 0.1);
        assert_eq!(
            pressed.first_row,
            Some(identity),
            "按下时控件身份和命中矩形必须稳定"
        );
        assert!(pressed.contains_pointer);
        assert!(!pressed.clicked && !pressed.drag_started);

        // 与失败日志同构：按下/释放分属两次 pass，两次之间没有额外指针移动。
        let released = palette_scroll_frame(&ctx, pointer_button(false), 0.1 + hold);
        assert_eq!(
            released.first_row,
            Some(identity),
            "释放时控件不能移动或更换 ID"
        );
        assert_eq!(
            released.clicked, should_click,
            "按住 {hold}s 的实际目录行点击判定"
        );
        if should_click {
            assert!(!released.drag_started, "短点击不能同时触发目录拖出");
        }
        assert!(released.offset.abs() < 0.5, "原位点击/长按不应滚动目录");

        let settled = palette_scroll_frame(&ctx, vec![], 0.116 + hold);
        assert!(!settled.clicked, "释放后的等待 pass 不能再次选择目录行");
    }
}

#[test]
fn palette_shared_short_click_packet_selects_once_after_a_slow_frame() {
    let ctx = crate::egui_ui::test_context();
    let mut row = PaletteGesture::default();
    for time in [0.0, 0.016, 0.032] {
        row = palette_scroll_frame(&ctx, vec![], time);
    }
    let identity = row.first_row.expect("预热后的目录行");
    let mut state = UiState::default();
    state.hits.push((PanelButton::Part(0), identity.1));
    let mut driver = crate::egui_ui::UiTestInput::default();
    let mut input = bevy_egui::EguiInput::default();
    assert!(driver.click_panel(PanelButton::Part(0), &state, &mut input));
    let clicked = palette_scroll_frame(&ctx, std::mem::take(&mut input.0.events), 1.0);
    assert!(clicked.clicked && !clicked.drag_started);
    assert_eq!(clicked.first_row, Some(identity));
    for time in [2.0, 3.0] {
        assert!(driver.tick(&mut input));
        assert!(input.0.events.is_empty());
        assert!(!palette_scroll_frame(&ctx, vec![], time).clicked);
    }
    assert!(!driver.tick(&mut input));
}

#[test]
fn palette_touch_tap_selects_once_and_mouse_drag_survives_touch_input() {
    for tap_first in [false, true] {
        let ctx = crate::egui_ui::test_context();
        let mut time = 0.0;
        for _ in 0..3 {
            palette_scroll_frame(&ctx, vec![], time);
            time += 0.016;
        }
        let start = egui::pos2(150.0, 30.0);
        if tap_first {
            let mut clicks = 0;
            for events in [touch_press(start), touch_release(start), vec![]] {
                time += 0.016;
                let frame = palette_scroll_frame(&ctx, events, time);
                clicks += usize::from(frame.clicked);
                assert!(!frame.drag_started, "轻点目录行不应开始部件拖拽");
                assert!(frame.offset.abs() < 0.5, "轻点不应滚动目录");
            }
            assert_eq!(clicks, 1, "触摸轻点必须恰好选择一次目录行");
        }
        time += 0.016;
        palette_scroll_frame(
            &ctx,
            vec![
                egui::Event::PointerMoved(start),
                egui::Event::PointerButton {
                    pos: start,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            time,
        );
        time += 0.016;
        let moved = palette_scroll_frame(
            &ctx,
            vec![egui::Event::PointerMoved(egui::pos2(190.0, 100.0))],
            time,
        );
        assert!(moved.drag_started, "触摸前后都应保留鼠标从目录拖出路径");
        assert!(!moved.clicked, "鼠标拖出不应同时产生目录点击");
        assert!(moved.offset.abs() < 0.5, "鼠标拖部件不能变为触摸滚动");
        time += 0.016;
        palette_scroll_frame(
            &ctx,
            vec![egui::Event::PointerButton {
                pos: egui::pos2(190.0, 100.0),
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }],
            time,
        );
    }
}

#[test]
fn reserved_toolbar_never_paints_over_native_buttons() {
    fn covers(shape: &egui::epaint::Shape, toolbar: egui::Rect) -> bool {
        match shape {
            egui::epaint::Shape::Rect(rect) => {
                rect.fill.a() > 0 && rect.rect.contains_rect(toolbar)
            }
            egui::epaint::Shape::Vec(shapes) => shapes.iter().any(|shape| covers(shape, toolbar)),
            _ => false,
        }
    }
    for size in [egui::vec2(320.0, 640.0), egui::vec2(1440.0, 900.0)] {
        let ctx = crate::egui_ui::test_context();
        let mut toolbar = egui::Rect::NOTHING;
        let output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
                ..default()
            },
            |ui| toolbar = reserve_native_toolbar(ui, size.x).rect,
        );
        assert_eq!(toolbar.height(), crate::panels::toolbar_height(size.x));
        assert!(
            !output
                .shapes
                .iter()
                .any(|shape| covers(&shape.shape, toolbar)),
            "egui 不得用不透明背景遮住原生顶栏"
        );
    }
}

#[test]
fn virtual_browser_uses_measured_extent_and_keeps_the_last_row_reachable() {
    for text_size in [12.5, 16.0, 22.0] {
        let ctx = crate::egui_ui::test_context();
        ctx.style_mut_of(ctx.theme(), |style| {
            style.text_styles.insert(
                egui::TextStyle::Button,
                egui::FontId::proportional(text_size),
            );
        });
        let mut offset = 0.0;
        let mut last = egui::Rect::NOTHING;
        let mut viewport = egui::Rect::NOTHING;
        for frame in 0..4 {
            let _ = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(248.0, 640.0),
                    )),
                    time: Some(f64::from(frame) * 0.02),
                    ..default()
                },
                |ui| {
                    let row_height = browser_row_height(ui);
                    let output = egui::ScrollArea::vertical()
                        .vertical_scroll_offset(offset)
                        .show_rows(ui, row_height, 1000, |ui, rows| {
                            for index in rows {
                                let response = ui.add_sized(
                                    [ui.available_width(), row_height],
                                    egui::Button::new(format!("ship-{index:04}")).truncate(),
                                );
                                assert!(response.rect.height() <= row_height + 0.01);
                                if index == 999 {
                                    last = response.rect;
                                }
                            }
                        });
                    viewport = output.inner_rect;
                    offset = (output.content_size.y - viewport.height()).max(0.0);
                },
            );
        }
        assert!(
            viewport.expand(0.01).contains_rect(last),
            "字号 {text_size} 的末行必须完全可见：{last:?} / {viewport:?}"
        );
    }
}

#[test]
fn category_header_truncates_long_custom_names_and_keeps_touch_height() {
    let ctx = crate::egui_ui::test_context();
    for width in [240.0, 296.0] {
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(width, 480.0),
                )),
                ..default()
            },
            |ui| {
                ui.spacing_mut().interact_size.y = 44.0;
                let left = ui.cursor().left();
                let group = category_group(ui, &"很长的自定义部件类别".repeat(20), false, |_| {});
                assert!(group.header_response.rect.height() >= 44.0);
                assert!(
                    group.header_response.rect.right() <= left + width + 0.01,
                    "标题不得超出目录宽度：{:?} / {left} + {width}",
                    group.header_response.rect
                );
            },
        );
        output.textures_delta.clear();
    }
}

#[test]
fn actual_group_clicks_share_palette_state_with_keyboard_and_expand_all() {
    let mut document = crate::tests::document();
    document.catalog = crate::panels::tests::catalog();
    let catalog = document.catalog.clone();
    let before = document.ship.clone();
    let mut app = App::new();
    app.insert_resource(document)
        .init_resource::<Palette>()
        .init_resource::<ShipBrowser>()
        .init_resource::<EditorCursor>()
        .init_resource::<DragState>()
        .add_message::<files::FileAction>()
        .add_message::<PanelButton>()
        .add_systems(Update, panel_actions);
    let ctx = crate::egui_ui::test_context();
    let mut hits = Vec::<(PanelButton, egui::Rect)>::new();
    let mut driver = crate::egui_ui::UiTestInput::default();
    for frame in 0..9 {
        let mut input = bevy_egui::EguiInput::default();
        if frame == 3 || frame == 6 {
            let action = if frame == 3 {
                PanelButton::ToggleCategory("Gizmos".into())
            } else {
                PanelButton::ExpandCategories
            };
            let rect = hits
                .iter()
                .find(|(candidate, _)| *candidate == action)
                .unwrap()
                .1;
            driver.click_rect(rect, &mut input);
        }
        let mut actions = Vec::new();
        let mut enabled_parts = Vec::new();
        hits.clear();
        let palette = app.world().resource::<Palette>();
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(400.0, 480.0),
                )),
                time: Some(f64::from(frame) * 0.2),
                events: input.0.events,
                ..default()
            },
            |ui| {
                let action = PanelButton::ExpandCategories;
                let response =
                    directory_button(ui, &action, palette.collapsed.is_empty(), "全部展开");
                hits.push((action.clone(), response.rect));
                if response.clicked() {
                    actions.push(action);
                }
                for (name, indices) in palette.groups(&catalog) {
                    let group = category_group(ui, name, !palette.collapsed.contains(name), |ui| {
                        for index in indices {
                            let response = part_row(
                                ui,
                                egui::TextureId::User(321),
                                Vec2::splat(30.0),
                                format!("部件 {index}"),
                                false,
                                ui.available_width(),
                            );
                            if response.enabled() {
                                enabled_parts.push(index);
                            }
                        }
                    });
                    hits.push((
                        PanelButton::ToggleCategory(name.into()),
                        group.header_response.rect,
                    ));
                    if group.header_response.clicked() {
                        actions.push(PanelButton::ToggleCategory(name.into()));
                    }
                }
            },
        );
        output.textures_delta.clear();
        if frame == 5 {
            assert_eq!(enabled_parts, vec![1, 2]);
        }
        if frame == 8 {
            assert_eq!(enabled_parts, vec![0, 1, 2]);
        }
        for action in actions {
            app.world_mut().write_message(action);
        }
        app.update();
        if frame == 3 {
            assert!(
                app.world()
                    .resource::<Palette>()
                    .collapsed
                    .contains("Gizmos")
            );
            assert_eq!(
                app.world().resource::<Palette>().indices(&catalog),
                vec![1, 2]
            );
        }
    }
    assert!(app.world().resource::<Palette>().collapsed.is_empty());
    let document = app.world().resource::<EditorDocument>();
    assert_eq!(document.ship, before);
    assert!(!document.dirty && !document.history.can_undo());
}
