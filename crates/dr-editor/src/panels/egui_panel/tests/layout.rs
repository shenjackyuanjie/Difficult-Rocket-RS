use super::*;

/// 复用生产底栏、抽屉标题及导航；正文占位仅隔离几何和真实输入合同。
#[derive(Debug)]
struct LayoutFrame {
    toolbar: egui::Rect,
    left: Option<egui::Rect>,
    right: Option<egui::Rect>,
    tools: Option<egui::Rect>,
    canvas: egui::Rect,
    areas: Vec<egui::Rect>,
    actions: Vec<PanelButton>,
}

fn header_pass(root: &mut egui::Ui, state: &mut UiState, screen: egui::Vec2) -> LayoutFrame {
    state.hits.clear();
    state.areas.clear();
    state.palette_area = None;
    state.browser_area = None;
    state.tools_area = None;
    state.canvas_area = None;
    state.ship_toggle = None;
    state.part_toggle = None;
    state.more_toggle = None;
    state.settings_toggle = None;
    state.fit_button = None;
    let mut viewport_ui = egui::Ui::new(
        root.ctx().clone(),
        egui::Id::new("directory_viewport"),
        egui::UiBuilder::new()
            .layer_id(egui::LayerId::background())
            .max_rect(root.ctx().viewport_rect()),
    );
    let (toolbar, actions) = bottom_toolbar(&mut viewport_ui, state, true, true, true, true);
    state.areas.push(toolbar);
    let mut left = None;
    if state.drawer == Some(Drawer::Ships) {
        let panel = egui::Panel::left("ship_browser_sidebar")
            .exact_size(sidebar_width(screen.x, 248.0))
            .resizable(false)
            .show(&mut viewport_ui, |ui| drawer_header(ui, "船体目录", state));
        state.areas.push(panel.response.rect);
        state.browser_area = Some(panel.response.rect);
        left = state.browser_area;
    }
    let mut right = None;
    if state.drawer == Some(Drawer::Parts) {
        let panel = egui::Panel::right("part_palette_sidebar")
            .exact_size(sidebar_width(screen.x, 296.0))
            .resizable(false)
            .show(&mut viewport_ui, |ui| drawer_header(ui, "部件目录", state));
        state.areas.push(panel.response.rect);
        state.palette_area = Some(panel.response.rect);
        right = state.palette_area;
    }
    let mut tools = None;
    if state.drawer == Some(Drawer::Tools) {
        let panel = egui::Panel::right("editor_tools_drawer")
            .exact_size(sidebar_width(screen.x, 296.0))
            .resizable(false)
            .show(&mut viewport_ui, |ui| {
                drawer_header(ui, "编辑工具", state);
                tools_navigation(ui, state);
            });
        state.areas.push(panel.response.rect);
        state.tools_area = Some(panel.response.rect);
        tools = state.tools_area;
    }
    let canvas = viewport_ui.available_rect_before_wrap();
    state.canvas_area = Some(canvas);
    LayoutFrame {
        toolbar,
        left,
        right,
        tools,
        canvas,
        areas: state.areas.clone(),
        actions,
    }
}

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
    (frame.expect("工具栏布局必须执行"), output)
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

#[test]
fn bottom_toolbar_keeps_a_full_width_canvas_and_reachable_touch_icons() {
    for (width, height) in [
        (320.0, 640.0),
        (390.0, 844.0),
        (640.0, 360.0),
        (1440.0, 900.0),
    ] {
        let screen = egui::vec2(width, height);
        let viewport = egui::Rect::from_min_size(egui::Pos2::ZERO, screen);
        let ctx = crate::egui_ui::test_context();
        let mut state = UiState::default();
        for pass in 0..2 {
            header_frame(&ctx, &mut state, screen, vec![], f64::from(pass) * 0.02);
        }
        let (frame, _) = header_frame(&ctx, &mut state, screen, vec![], 0.1);
        assert_eq!(frame.toolbar.height(), panels::TOOLBAR_HEIGHT);
        assert!((frame.toolbar.bottom() - height).abs() < 0.51);
        assert!(frame.left.is_none() && frame.right.is_none() && frame.tools.is_none());
        assert_eq!(frame.areas.len(), 1, "默认只有一条底部命中区域");
        assert_eq!(frame.canvas.min, egui::Pos2::ZERO, "顶部不再预留文字栏空白");
        assert!((frame.canvas.width() - width).abs() < 0.51);
        assert!((frame.canvas.bottom() - (height - panels::TOOLBAR_HEIGHT)).abs() < 0.51);
        assert!(viewport.contains_rect(frame.canvas));
        let undo = state
            .hits
            .iter()
            .find(|(action, _)| *action == PanelButton::Undo)
            .expect("320 宽也保留撤销入口")
            .1;
        for rect in [
            state.part_toggle.expect("部件入口"),
            state.more_toggle.expect("更多入口"),
            undo,
        ] {
            assert!(rect.width() >= 44.0 && rect.height() >= 44.0);
            assert!(
                rect.left() >= panels::NATIVE_TOOLBAR_WIDTH,
                "原生入口区域不放 egui 控件"
            );
            assert!(frame.toolbar.contains_rect(rect) && viewport.contains_rect(rect));
        }
        assert_eq!(state.settings_toggle, state.more_toggle);
        assert!(
            !state
                .hits
                .iter()
                .any(|(action, _)| *action == PanelButton::Delete),
            "低频工具收进抽屉，不生成第二行工具条"
        );
        let (clicked, _) = click_header(&ctx, &mut state, screen, undo.center(), 0.4);
        assert_eq!(
            clicked.actions,
            vec![PanelButton::Undo],
            "正式按钮保留语义动作"
        );
    }
}

#[test]
fn drawers_are_mutually_exclusive_on_mobile_and_desktop() {
    for (width, height) in [
        (320.0, 640.0),
        (390.0, 844.0),
        (640.0, 360.0),
        (1440.0, 900.0),
    ] {
        let screen = egui::vec2(width, height);
        let ctx = crate::egui_ui::test_context();
        let mut state = UiState::default();
        for pass in 0..2 {
            header_frame(&ctx, &mut state, screen, vec![], f64::from(pass) * 0.02);
        }
        let parts = state.part_toggle.expect("部件入口").center();
        let (frame, _) = click_header(&ctx, &mut state, screen, parts, 0.3);
        let palette = frame.right.expect("部件抽屉");
        assert!(frame.left.is_none() && frame.tools.is_none());
        assert_eq!(frame.areas.len(), 2);
        assert_eq!(state.drawer, Some(Drawer::Parts));
        assert!((frame.canvas.right() - palette.left()).abs() < 0.51);
        let more = state.more_toggle.expect("更多入口").center();
        let (frame, _) = click_header(&ctx, &mut state, screen, more, 0.6);
        assert_eq!(state.drawer, Some(Drawer::Tools));
        assert!(frame.tools.is_some() && frame.right.is_none() && frame.left.is_none());
        assert!(state.palette_area.is_none());
        let ships = state.ship_toggle.expect("更多抽屉提供船体入口").center();
        click_header(&ctx, &mut state, screen, ships, 0.9);
        let (frame, _) = header_frame(&ctx, &mut state, screen, vec![], 1.0);
        assert_eq!(state.drawer, Some(Drawer::Ships));
        let browser = frame.left.expect("船体抽屉");
        assert!(frame.right.is_none() && frame.tools.is_none());
        assert!((frame.canvas.left() - browser.right()).abs() < 0.51);
        let parts = state.part_toggle.expect("底栏部件入口").center();
        let (frame, _) = click_header(&ctx, &mut state, screen, parts, 1.3);
        assert!(frame.right.is_some() && frame.left.is_none() && frame.tools.is_none());
        let parts = state.part_toggle.expect("底栏部件入口").center();
        let (frame, _) = click_header(&ctx, &mut state, screen, parts, 1.6);
        assert_eq!(state.drawer, None);
        assert!(frame.left.is_none() && frame.right.is_none() && frame.tools.is_none());
        assert!((frame.canvas.width() - width).abs() < 0.51);
    }
}

#[test]
fn fit_remains_a_formal_button_inside_more_on_narrow_screens() {
    for screen in [egui::vec2(320.0, 640.0), egui::vec2(1440.0, 900.0)] {
        let ctx = crate::egui_ui::test_context();
        let mut state = UiState::default();
        for pass in 0..2 {
            header_frame(&ctx, &mut state, screen, vec![], f64::from(pass) * 0.02);
        }
        let more = state.more_toggle.expect("更多入口").center();
        click_header(&ctx, &mut state, screen, more, 0.3);
        let fit = state.fit_button.expect("更多内的适配按钮");
        state.fit_requested = false;
        let (frame, _) = click_header(&ctx, &mut state, screen, fit.center(), 0.6);
        assert!(state.fit_requested);
        assert!(frame.actions.is_empty(), "适配视野不伪造目录动作");
    }
}

#[test]
fn toolbar_never_paints_over_native_buttons_and_disabled_icons_have_no_hits() {
    fn overlaps(shape: &egui::epaint::Shape, native: egui::Rect) -> bool {
        match shape {
            egui::epaint::Shape::Rect(rect) => {
                rect.fill.a() > 0 && rect.rect.intersect(native).is_positive()
            }
            egui::epaint::Shape::Vec(shapes) => shapes.iter().any(|shape| overlaps(shape, native)),
            _ => false,
        }
    }
    for screen in [egui::vec2(320.0, 640.0), egui::vec2(1440.0, 900.0)] {
        let ctx = crate::egui_ui::test_context();
        let mut state = UiState::default();
        let mut toolbar = egui::Rect::NOTHING;
        let output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, screen)),
                ..default()
            },
            |ui| {
                toolbar = bottom_toolbar(ui, &mut state, false, true, true, true).0;
            },
        );
        let native = egui::Rect::from_min_max(
            toolbar.min,
            egui::pos2(
                toolbar.left() + panels::NATIVE_TOOLBAR_WIDTH,
                toolbar.bottom(),
            ),
        )
        .shrink(0.1);
        assert!(
            !output
                .shapes
                .iter()
                .any(|shape| overlaps(&shape.shape, native)),
            "egui 的不透明背景不得盖住原生三个入口"
        );
        assert!(state.hits.is_empty());
        assert!(state.part_toggle.is_none() && state.more_toggle.is_none());
    }
}

#[test]
fn native_entries_share_the_fixed_bottom_layout_contract() {
    let mut app = App::new();
    app.world_mut()
        .spawn((Window::default(), bevy::window::PrimaryWindow));
    let entries: Vec<_> = [
        panels::ToolbarItem::Files,
        panels::ToolbarItem::Properties,
        panels::ToolbarItem::Help,
    ]
    .into_iter()
    .map(|item| {
        app.world_mut()
            .spawn((
                item,
                Node {
                    top: px(100),
                    ..default()
                },
            ))
            .id()
    })
    .collect();
    let status = app
        .world_mut()
        .spawn((panels::ToolbarItem::Status, Node::default()))
        .id();
    app.add_systems(Update, panels::layout_toolbars);
    app.update();
    for (index, entity) in entries.into_iter().enumerate() {
        let node = app.world().get::<Node>(entity).expect("原生入口 Node");
        assert_eq!(node.left, px(index as f32 * 48.0));
        assert_eq!(node.top, Val::Auto);
        assert_eq!(node.bottom, px(4));
        assert_eq!(node.width, px(48));
        assert_eq!(node.height, px(48));
    }
    assert_eq!(
        app.world().get::<Node>(status).unwrap().display,
        Display::None
    );
}

#[test]
fn panel_driver_opens_hidden_ship_navigation_with_real_pointer_events() {
    let screen = egui::vec2(320.0, 640.0);
    let ctx = crate::egui_ui::test_context();
    let mut state = UiState::default();
    for pass in 0..2 {
        header_frame(&ctx, &mut state, screen, vec![], f64::from(pass) * 0.02);
    }
    let mut driver = crate::egui_ui::UiTestInput::default();
    let mut input = bevy_egui::EguiInput::default();
    let target = PanelButton::Open(PathBuf::from("example.xml"));
    assert!(!driver.click_panel(target.clone(), &state, &mut input));
    assert_eq!(state.drawer, None, "测试驱动不直接改 drawer 或发送语义动作");
    assert!(
        input
            .0
            .events
            .iter()
            .any(|event| matches!(event, egui::Event::PointerButton { pressed: true, .. }))
    );
    let events = std::mem::take(&mut input.0.events);
    header_frame(&ctx, &mut state, screen, events, 0.2);
    header_frame(&ctx, &mut state, screen, vec![], 0.25);
    assert_eq!(state.drawer, Some(Drawer::Tools), "先通过真实 More 入口");
    assert!(!driver.click_panel(target, &state, &mut input));
    assert_eq!(state.drawer, Some(Drawer::Tools));
    let events = std::mem::take(&mut input.0.events);
    header_frame(&ctx, &mut state, screen, events, 0.4);
    let (frame, _) = header_frame(&ctx, &mut state, screen, vec![], 0.45);
    assert_eq!(state.drawer, Some(Drawer::Ships), "再通过真实船体入口");
    assert!(frame.left.is_some() && frame.right.is_none() && frame.tools.is_none());
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
