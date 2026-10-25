use crate::egui_ui;
use crate::files;
use crate::help::*;
use crate::mobile_input;
use crate::native_buttons;
use crate::panels;
use crate::placement::EditorCursor;
use crate::properties;
use crate::selection::DragState;
use crate::topology_ui;
use crate::view::CameraDrag;
use bevy::ui::Pressed;
use bevy::ui_widgets::Button;
use dr_core::PartKey;

#[derive(Resource, Default)]
struct CanvasTicks(u32);

fn app() -> App {
    let mut app = App::new();
    app.add_plugins(native_buttons::NativeButtonsPlugin)
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<HelpState>()
        .init_resource::<properties::Inspector>()
        .init_resource::<topology_ui::ConnectionEditor>()
        .init_resource::<files::PendingFileAction>()
        .init_resource::<files::FileMenu>()
        .init_resource::<DragState>()
        .init_resource::<EditorCursor>()
        .init_resource::<CameraDrag>()
        .init_resource::<panels::UiPointer>()
        .init_resource::<CanvasTicks>()
        .insert_resource(crate::test_support::document())
        .add_systems(
            Update,
            (
                input,
                egui_ui::prepare_input,
                (|mut ticks: ResMut<CanvasTicks>| ticks.0 += 1)
                    .run_if(egui_ui::canvas_input_available),
            )
                .chain(),
        );
    app
}

#[test]
fn help_setup_is_a_48px_native_icon_entry_without_text_children() {
    let mut app = App::new();
    app.add_systems(Startup, setup);
    app.update();
    let world = app.world_mut();
    let (entity, node) = world
        .query_filtered::<(Entity, &Node), With<HelpButton>>()
        .single(world)
        .unwrap();
    assert_eq!(node.width, px(48));
    assert_eq!(node.height, px(48));
    assert_eq!(node.left, px(96));
    assert_eq!(node.bottom, px(4));
    assert!(world.get::<Button>(entity).is_some());
    assert!(
        world
            .get::<bevy::ui_widgets::ActivateOnPress>(entity)
            .is_some()
    );
    assert_eq!(world.query::<&Text>().iter(world).count(), 0);
}

#[test]
fn opening_help_closes_file_menu_and_clears_stale_menu_hits() {
    let mut app = app();
    {
        let mut menu = app.world_mut().resource_mut::<files::FileMenu>();
        menu.open = true;
        menu.area = Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(100.0, 100.0),
        ));
        menu.hits
            .push((files::FileAction::Save, egui::Rect::EVERYTHING));
    }
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::F1);
    app.update();
    assert!(app.world().resource::<HelpState>().open);
    let menu = app.world().resource::<files::FileMenu>();
    assert!(!menu.open);
    assert!(menu.area.is_none());
    assert!(menu.hits.is_empty());
}

#[test]
fn help_open_and_close_frames_cancel_previews_and_block_canvas() {
    let mut app = app();
    assert!(!app.world().resource::<HelpState>().open);
    app.world_mut().resource_mut::<DragState>().id = Some(PartKey::new(0, 1, 0));
    app.world_mut().resource_mut::<EditorCursor>().placing = true;
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::F1);
    app.update();
    assert!(app.world().resource::<HelpState>().open);
    assert!(app.world().resource::<DragState>().id.is_none());
    assert!(!app.world().resource::<EditorCursor>().placing);
    assert_eq!(app.world().resource::<CanvasTicks>().0, 0);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .reset_all();
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::F2);
    app.update();
    assert!(!app.world().resource::<properties::Inspector>().is_open());
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .reset_all();
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Escape);
    app.update();
    assert!(!app.world().resource::<HelpState>().open);
    assert_eq!(app.world().resource::<CanvasTicks>().0, 0);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .reset_all();
    app.update();
    assert_eq!(app.world().resource::<CanvasTicks>().0, 1);
}

#[test]
fn help_blocks_file_shortcuts_and_drops_and_discards_queued_palette_actions() {
    use bevy::window::{FileDragAndDrop, WindowCloseRequested};
    let mut app = App::new();
    app.init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<HelpState>()
        .init_resource::<panels::Palette>()
        .init_resource::<panels::ShipBrowser>()
        .init_resource::<EditorCursor>()
        .init_resource::<DragState>()
        .insert_resource(crate::test_support::document())
        .add_message::<panels::PanelButton>()
        .add_message::<files::FileAction>()
        .add_message::<FileDragAndDrop>()
        .add_message::<WindowCloseRequested>()
        .init_resource::<crate::properties::Inspector>()
        .add_systems(Update, (panels::panel_actions, files::file_inputs).chain());
    for open in [true, false] {
        *app.world_mut().resource_mut::<HelpState>() = HelpState {
            open,
            suppress_frame: !open,
        };
        app.world_mut()
            .write_message(panels::PanelButton::DragPart(0));
        app.world_mut().write_message(FileDragAndDrop::DroppedFile {
            window: Entity::PLACEHOLDER,
            path_buf: "不会打开.xml".into(),
        });
        let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        keys.press(KeyCode::ControlLeft);
        keys.press(KeyCode::KeyN);
        app.update();
        assert!(!app.world().resource::<EditorCursor>().placing);
        assert!(
            app.world()
                .resource::<Messages<files::FileAction>>()
                .is_empty()
        );
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .reset_all();
    }
    *app.world_mut().resource_mut::<HelpState>() = HelpState::default();
    app.update();
    assert!(!app.world().resource::<EditorCursor>().placing);
    assert!(
        app.world()
            .resource::<Messages<files::FileAction>>()
            .is_empty()
    );
    app.world_mut().resource_mut::<HelpState>().open = true;
    app.world_mut().write_message(WindowCloseRequested {
        window: Entity::PLACEHOLDER,
    });
    app.update();
    assert_eq!(
        app.world().resource::<Messages<files::FileAction>>().len(),
        1
    );
}

#[test]
fn actual_toolbar_activation_toggles_help_without_repeating_while_held() {
    let mut app = app();
    let entity = app
        .world_mut()
        .spawn((
            native_buttons::press_triggered(),
            HelpButton,
            Pressed,
            BackgroundColor(Color::BLACK),
        ))
        .id();
    app.world_mut()
        .trigger(bevy::ui_widgets::Activate { entity });
    app.update();
    assert!(app.world().resource::<HelpState>().open);
    assert_eq!(
        app.world().get::<BackgroundColor>(entity).unwrap().0,
        native_buttons::PRESSED
    );
    app.update();
    assert!(
        app.world().resource::<HelpState>().open,
        "按住态本身不产生第二次激活"
    );
    app.world_mut().entity_mut(entity).remove::<Pressed>();
    app.update();
    assert!(
        app.world().resource::<HelpState>().open,
        "释放只清按下色，不切换帮助"
    );
    assert_eq!(
        app.world().get::<BackgroundColor>(entity).unwrap().0,
        native_buttons::BUTTON
    );
    app.world_mut().entity_mut(entity).insert(Pressed);
    app.world_mut()
        .trigger(bevy::ui_widgets::Activate { entity });
    app.update();
    assert!(!app.world().resource::<HelpState>().open);
    assert_eq!(app.world().resource::<CanvasTicks>().0, 0);
}

#[test]
fn help_drops_modal_blocked_activations_without_delayed_toggle() {
    for inspector_open in [true, false] {
        let mut app = app();
        let entity = app
            .world_mut()
            .spawn((
                native_buttons::press_triggered(),
                HelpButton,
                BackgroundColor(Color::BLACK),
            ))
            .id();
        if inspector_open {
            let mut inspector = properties::Inspector::default();
            let mut document = crate::test_support::document();
            document.selected = Some(PartKey::new(0, 1, 0));
            properties::open(&mut inspector, &document);
            assert!(inspector.is_open());
            app.insert_resource(inspector);
        } else {
            app.world_mut()
                .resource_mut::<topology_ui::ConnectionEditor>()
                .open = true;
        }
        app.world_mut()
            .trigger(bevy::ui_widgets::Activate { entity });
        app.update();
        assert!(!app.world().resource::<HelpState>().open);
        app.insert_resource(properties::Inspector::default());
        app.world_mut()
            .resource_mut::<topology_ui::ConnectionEditor>()
            .open = false;
        app.update();
        assert!(
            !app.world().resource::<HelpState>().open,
            "模态关闭后不补发旧激活"
        );
        let other = app.world_mut().spawn_empty().id();
        app.world_mut()
            .trigger(bevy::ui_widgets::Activate { entity: other });
        app.update();
        assert!(
            !app.world().resource::<HelpState>().open,
            "其它控件激活不切换帮助"
        );
        app.world_mut()
            .trigger(bevy::ui_widgets::Activate { entity });
        app.update();
        assert!(app.world().resource::<HelpState>().open);
    }
}

fn help_frame(
    ctx: &egui::Context,
    open: &mut bool,
    screen: egui::Rect,
    frame: u32,
    events: Vec<egui::Event>,
) -> (egui::FullOutput, egui::Rect) {
    let mut window = egui::Rect::NOTHING;
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(screen),
            time: Some(f64::from(frame)),
            focused: true,
            events,
            ..Default::default()
        },
        |ui| {
            show(ctx, open);
            window = ui
                .ctx()
                .memory(|memory| memory.area_rect(egui::Id::new("editor_help")))
                .unwrap_or(egui::Rect::NOTHING);
        },
    );
    output.textures_delta.clear();
    (output, window)
}

fn text_visible(shapes: &[egui::epaint::ClippedShape], needle: &str) -> bool {
    shapes.iter().any(|clipped| match &clipped.shape {
        egui::epaint::Shape::Text(text) if text.galley.job.text.contains(needle) => clipped
            .shape
            .visual_bounding_rect()
            .intersect(clipped.clip_rect)
            .is_positive(),
        _ => false,
    })
}

/// 触控/窄屏：帮助窗口留在可用区域内，长文本换行不横向溢出，正文可滚到最后一条。
#[test]
fn help_window_fits_touch_screens_and_scrolls_to_the_last_shortcut() {
    for (width, height) in [
        (320.0, 640.0),
        (390.0, 844.0),
        (640.0, 360.0),
        (1280.0, 800.0),
    ] {
        let ctx = crate::egui_ui::test_context();
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height));
        let mut open = true;
        let mut window = egui::Rect::NOTHING;
        let mut output = None;
        for frame in 0..4 {
            let (current, rect) = help_frame(&ctx, &mut open, screen, frame, vec![]);
            window = rect;
            output = Some(current);
        }
        let output = output.unwrap();
        assert!(
            open,
            "{width}×{height} 帮助窗口应保持打开，关闭按钮由标题栏提供"
        );
        assert!(
            window.is_positive() && screen.expand(1.0).contains_rect(window),
            "{width}×{height} 帮助窗口应留在可用区域内：{window:?}"
        );
        // 两列网格在窄屏要换行：正文文本不能横向溢出屏幕。
        // 窗口阴影默认 offset [10, 20] + blur 15，会向右扩 17.5px，属装饰不参与本断言。
        for clipped in &output.shapes {
            if let egui::epaint::Shape::Text(text) = &clipped.shape {
                let rect = text.visual_bounding_rect();
                assert!(
                    rect.min.x >= screen.min.x - 1.0 && rect.max.x <= screen.max.x + 1.0,
                    "{width}×{height} 帮助长行横向溢出：{rect:?}"
                );
            }
        }
        // 正文超出窗口高度时靠滚动读完。
        let pointer = window.center();
        let mut output = None;
        for frame in 4..18 {
            let events = if frame == 4 {
                vec![egui::Event::PointerMoved(pointer)]
            } else {
                vec![egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: egui::vec2(0.0, -240.0),
                    phase: egui::TouchPhase::Move,
                    modifiers: egui::Modifiers::NONE,
                }]
            };
            let (current, rect) = help_frame(&ctx, &mut open, screen, frame, events);
            window = rect;
            output = Some(current);
        }
        let output = output.unwrap();
        assert!(
            text_visible(&output.shapes, "F1 / Esc"),
            "{width}×{height} 滚动后应能看到最后一条快捷键说明"
        );
        assert!(
            window.is_positive() && screen.expand(1.0).contains_rect(window),
            "{width}×{height} 滚动不应把帮助窗口撑出可用区域：{window:?}"
        );
    }
}

fn text_rect(output: &egui::FullOutput, needle: &str) -> egui::Rect {
    output
        .shapes
        .iter()
        .find_map(|clipped| match &clipped.shape {
            egui::epaint::Shape::Text(text) if text.galley.job.text == needle => {
                Some(text.visual_bounding_rect())
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("应真实绘制文本：{needle}"))
}

fn collect_rects(shape: &egui::epaint::Shape, rects: &mut Vec<egui::Rect>) {
    match shape {
        egui::epaint::Shape::Rect(rect) => rects.push(rect.rect),
        egui::epaint::Shape::Vec(shapes) => {
            for shape in shapes {
                collect_rects(shape, rects);
            }
        }
        _ => {}
    }
}

#[test]
fn help_window_title_drag_moves_and_reopening_keeps_the_position() {
    let ctx = crate::egui_ui::test_context();
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1440.0, 900.0));
    let mut open = true;
    for frame in 0..3 {
        help_frame(&ctx, &mut open, screen, frame, vec![]);
    }
    let (output, before) = help_frame(&ctx, &mut open, screen, 3, vec![]);
    assert!(
        (before.center() - screen.center()).length() <= 1.0,
        "首次默认居中：{before:?}"
    );
    let title = text_rect(&output, "? 操作帮助").center();
    help_frame(
        &ctx,
        &mut open,
        screen,
        4,
        vec![egui::Event::PointerMoved(title)],
    );
    help_frame(
        &ctx,
        &mut open,
        screen,
        5,
        vec![egui::Event::PointerButton {
            pos: title,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        }],
    );
    help_frame(
        &ctx,
        &mut open,
        screen,
        6,
        vec![egui::Event::PointerMoved(title + egui::vec2(18.0, 12.0))],
    );
    let delta = egui::vec2(64.0, 40.0);
    help_frame(
        &ctx,
        &mut open,
        screen,
        7,
        vec![egui::Event::PointerMoved(title + delta)],
    );
    help_frame(
        &ctx,
        &mut open,
        screen,
        8,
        vec![egui::Event::PointerButton {
            pos: title + delta,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }],
    );
    let (_, moved) = help_frame(&ctx, &mut open, screen, 9, vec![]);
    assert!(open, "拖动标题栏不应关闭帮助");
    assert!(
        (moved.min - before.min - delta).length() <= 2.0,
        "必须由正式 Window 的真实拖动移动：{before:?} → {moved:?}"
    );
    open = false;
    help_frame(&ctx, &mut open, screen, 10, vec![]);
    open = true;
    let (_, reopened) = help_frame(&ctx, &mut open, screen, 11, vec![]);
    assert!(
        (reopened.min - moved.min).length() <= 1.0,
        "重新打开保留用户位置，不强制再次居中"
    );
}

#[test]
fn help_title_and_body_share_the_same_width_on_wide_and_narrow_screens() {
    for (width, height) in [
        (320.0, 640.0),
        (390.0, 844.0),
        (640.0, 360.0),
        (1440.0, 900.0),
    ] {
        let ctx = crate::egui_ui::test_context();
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height));
        let mut open = true;
        for frame in 0..3 {
            help_frame(&ctx, &mut open, screen, frame, vec![]);
        }
        let (output, window) = help_frame(&ctx, &mut open, screen, 3, vec![]);
        assert!(
            screen.expand(1.0).contains_rect(window),
            "窗口必须在 {width}×{height} 内：{window:?}"
        );
        assert!(
            (window.center() - screen.center()).length() <= 1.0,
            "内容尺寸变化不应偏离初始中心：{window:?}"
        );
        let expected = egui::vec2(720.0_f32.min(width - 40.0), 560.0_f32.min(height - 40.0));
        assert!(
            (window.size() - expected).length() <= 2.0,
            "正文不可撑破共享外框：{window:?}"
        );
        let title_text = text_rect(&output, "? 操作帮助");
        let mut rects = vec![];
        for clipped in &output.shapes {
            collect_rects(&clipped.shape, &mut rects);
        }
        let title = rects
            .into_iter()
            .find(|rect| {
                rect.contains(title_text.center())
                    && rect.height() < 80.0
                    && rect.width() > window.width() * 0.8
            })
            .expect("正式 Window 应绘制完整标题栏背景");
        assert!(
            (title.left() - window.left()).abs() <= 2.0
                && (title.right() - window.right()).abs() <= 2.0,
            "标题栏与 body 左右缘必须一致：{title:?} / {window:?}"
        );
        if width < 420.0 {
            let key = text_rect(&output, SHORTCUTS[0].0);
            assert!(
                (key.center().x - window.center().x).abs() <= 10.0,
                "窄窗条目居中，只给滚动条预留必要空间：{key:?} / {window:?}"
            );
        }
    }
}

#[test]
fn help_legend_renders_actual_candidate_snapped_and_unlinked_colors() {
    let ctx = crate::egui_ui::test_context();
    let output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(390.0, 240.0),
            )),
            ..Default::default()
        },
        legend,
    );
    let expected = [
        egui::Color32::from_rgb(171, 143, 224),
        egui::Color32::from_rgb(217, 189, 255),
        egui::Color32::from_white_alpha(100),
    ];
    assert_eq!(legend_colors(), expected);
    for (label, expected) in ["○ 候选", "○ 吸附", "◌ 未连接"].into_iter().zip(expected)
    {
        let text = output
            .shapes
            .iter()
            .find_map(|clipped| match &clipped.shape {
                egui::epaint::Shape::Text(text) if text.galley.job.text == label => Some(text),
                _ => None,
            })
            .unwrap_or_else(|| panic!("应绘制真实图例：{label}"));
        assert!(
            text.galley
                .job
                .sections
                .iter()
                .all(|section| section.format.color == expected),
            "{label} 使用实际渲染色，而非只写颜色名称"
        );
    }
}

#[test]
fn native_help_click_toggles_while_override_mouse_drops_widget_activation() {
    let mut app = app();
    let entity = app
        .world_mut()
        .spawn((
            native_buttons::press_triggered(),
            HelpButton,
            Pressed,
            BackgroundColor(Color::BLACK),
        ))
        .id();
    // 抬指产生原生点击，同帧兼容鼠标激活不能再切换一次。
    let mut touch = mobile_input::GestureInput::default();
    touch.native_click = Some(mobile_input::NativeClick::Help);
    touch.override_mouse = true;
    app.insert_resource(touch);
    app.world_mut()
        .trigger(bevy::ui_widgets::Activate { entity });
    app.update();
    assert!(app.world().resource::<HelpState>().open);
    assert_eq!(
        app.world().get::<BackgroundColor>(entity).unwrap().0,
        native_buttons::BUTTON
    );
    {
        let mut touch = app.world_mut().resource_mut::<mobile_input::GestureInput>();
        touch.native_click = None;
        touch.native_pressed = Some(entity);
    }
    app.world_mut()
        .trigger(bevy::ui_widgets::Activate { entity });
    app.update();
    assert!(app.world().resource::<HelpState>().open);
    assert_eq!(
        app.world().get::<BackgroundColor>(entity).unwrap().0,
        native_buttons::PRESSED
    );
    app.world_mut()
        .resource_mut::<mobile_input::GestureInput>()
        .native_pressed = None;
    app.world_mut()
        .trigger(bevy::ui_widgets::Activate { entity });
    app.update();
    assert!(app.world().resource::<HelpState>().open);
    assert_eq!(
        app.world().get::<BackgroundColor>(entity).unwrap().0,
        native_buttons::BUTTON
    );
    // 接管解除后的首帧不补旧消息；新鼠标激活仍然可用。
    app.world_mut()
        .resource_mut::<mobile_input::GestureInput>()
        .override_mouse = false;
    app.update();
    assert!(app.world().resource::<HelpState>().open);
    app.world_mut()
        .trigger(bevy::ui_widgets::Activate { entity });
    app.update();
    assert!(!app.world().resource::<HelpState>().open);
    {
        let mut touch = app.world_mut().resource_mut::<mobile_input::GestureInput>();
        touch.override_mouse = true;
        touch.native_click = Some(mobile_input::NativeClick::Help);
    }
    app.update();
    assert!(app.world().resource::<HelpState>().open);
    app.world_mut()
        .resource_mut::<mobile_input::GestureInput>()
        .native_click = None;
    app.update();
    assert!(
        app.world().resource::<HelpState>().open,
        "同一次抬指只切换一次"
    );
}
