use super::*;

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
