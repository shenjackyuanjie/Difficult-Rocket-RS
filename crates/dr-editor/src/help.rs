//! 按需显示操作说明，不让常驻 HUD 占据画布。
use super::*;
use bevy_egui::{EguiContexts, egui};

#[derive(Resource, Default)]
pub(crate) struct HelpState {
    pub open: bool,
    pub suppress_frame: bool,
}

#[derive(Component)]
pub(crate) struct HelpButton;

/// 顶栏按钮标签与最小触控高度；F1 / Esc 的说明留在帮助正文里。
pub(crate) const HELP_LABEL: &str = "? 帮助";
pub(crate) const HELP_MIN_HEIGHT: f32 = 44.0;

pub(crate) fn setup(mut commands: Commands, assets: Res<AssetServer>) {
    let font = assets.load("fonts/HarmonyOS_Sans/HarmonyOS_Sans_SC/HarmonyOS_Sans_SC_Regular.ttf");
    commands
        .spawn((
            native_buttons::press_triggered(),
            HelpButton,
            panels::ToolbarItem::Help,
            panels::EditorPanel,
            Node {
                position_type: PositionType::Absolute,
                min_height: px(HELP_MIN_HEIGHT),
                border: UiRect::all(px(1)),
                border_radius: native_buttons::native_corners(),
                padding: UiRect::axes(px(12), px(7)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(native_buttons::BUTTON),
        ))
        .with_children(|button| {
            button.spawn(native_buttons::label(HELP_LABEL, font));
        });
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(crate) fn input(
    keys: Res<ButtonInput<KeyCode>>,
    mut activated: MessageReader<native_buttons::Activated>,
    buttons: Query<Entity, With<HelpButton>>,
    mut state: ResMut<HelpState>,
    inspector: Res<properties::Inspector>,
    topology: Res<topology_ui::ConnectionEditor>,
    pending: Res<files::PendingFileAction>,
    touch: Option<Res<mobile_input::GestureInput>>,
    mut drag: ResMut<DragState>,
    mut cursor: ResMut<EditorCursor>,
    mut camera_drag: ResMut<CameraDrag>,
    mut pointer: ResMut<panels::UiPointer>,
) {
    let touch = touch.as_deref();
    state.suppress_frame = false;
    if inspector.is_open() || topology.open || pending.is_blocked() {
        activated.clear();
        state.open = false;
        return;
    }
    let mut toggle = keys.just_pressed(KeyCode::F1)
        || touch.is_some_and(|input| {
            matches!(
                input.native_click.as_ref(),
                Some(mobile_input::NativeClick::Help)
            )
        });
    if touch.is_some_and(|input| input.override_mouse) {
        // 本帧触摸独占原生按钮，丢弃 picking 已经桥接的消息，避免兼容鼠标重复触发。
        activated.clear();
    } else {
        for event in activated.read() {
            toggle |= buttons.contains(event.entity);
        }
    }
    let was_open = state.open;
    if toggle {
        state.open = !state.open;
    }
    if state.open && keys.just_pressed(KeyCode::Escape) {
        state.open = false;
    }
    // 关闭的这一帧也不把 Esc/按钮点击漏给画布。
    state.suppress_frame = state.open || was_open || toggle;
    if state.suppress_frame {
        cursor.manual_connection = None;
        drag.cancel();
        cursor.cancel_placement();
        cursor.paste = None;
        camera_drag.0 = None;
        pointer.blocked = true;
    }
}

const SHORTCUTS: &[(&str, &str)] = &[
    ("左键 / Shift+左键", "选择、拖动 / 增减选择"),
    (
        "R · Q/E · Shift+Q/E",
        "90° · ±15° · ±1°；拖拽和预览中也可用",
    ),
    ("侧栏 −15° / +15°", "精细旋转选区；连接点/边随角度旋转"),
    ("自由模式", "双连接点手动连/断；移动旋转不自动断连，不吸附"),
    ("Esc / 右键", "取消预览；重叠落下保留位置并断开外部连接"),
    ("中键拖动 / Ctrl+A", "框选 / 全选；侧栏可切换框选键"),
    ("Tab / Shift+Tab · P", "切换目录部件 · 放置"),
    ("Delete · X / Y", "删除 · 镜像；拖拽中可组合旋转并原子撤销"),
    ("↳ 子节点跟随", "侧栏开关；拖父节点带子孙，不带父节点"),
    ("拖到右侧部件列表", "松手删除整个相连分量；Ctrl+Z 撤销"),
    ("Ctrl+C / X / V", "复制 / 剪切 / 粘贴"),
    ("Ctrl+Z / Y", "撤销 / 重做"),
    ("Ctrl+N / O / S / Shift+S", "新建 / 打开 / 保存 / 另存为"),
    ("滚轮 · 空白拖动", "鼠标位置缩放 · 平移（框选键之外的键）"),
    ("触控 · 单指", "拖部件移动；空白处拖动平移画布"),
    (
        "触控 · 选目录后",
        "按住画布移动，抬指时在该处放置；取消或失焦不提交",
    ),
    (
        "触控 · 双指",
        "第二指加入即取消未提交编辑，并转为双指平移 / 缩放",
    ),
    ("触控 · 全抬指", "所有手指抬起后才能重新单指操作"),
    ("Home · F / Shift+F", "视图复位 · 适配整船 / 选区"),
    ("F2 · F6", "属性与分级 · 连接树 / 图"),
    ("F3 · F4 · F12", "调试轮廓 · 显隐贴图 · 截图"),
    ("F1 / Esc", "开关帮助 / 关闭"),
];

fn show(ctx: &egui::Context, open: &mut bool) {
    if !*open {
        return;
    }
    // 触控/窄屏：窗口收窄到可用区域，两列按剩余宽度换行，正文超出高度时滚动。
    let size = topology_ui::window_size(ctx, egui::vec2(880.0, 560.0));
    // 两列各分一半可用宽度，给窗口装饰留出余量，窄屏宁窄不溢出。
    let column = ((size.x - 80.0) / 2.0).clamp(96.0, 420.0);
    egui::Window::new("? 操作帮助")
        .id(egui::Id::new("editor_help"))
        .open(open)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .max_width(size.x)
        .max_height(size.y)
        .vscroll(true)
        .show(ctx, |ui| {
            // 先给内容一个宽度硬上界（滚动区内容 ui 的可用宽度可能是无限），再强制换行，
            // 否则长行会按自然宽度把自适应窗口撑宽。
            ui.set_max_width(size.x);
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
            egui::Grid::new("editor_shortcuts")
                .striped(true)
                .spacing([20.0, 9.0])
                .num_columns(2)
                .max_col_width(column)
                .show(ui, |ui| {
                    for (key, meaning) in SHORTCUTS {
                        ui.strong(*key);
                        ui.label(*meaning);
                        ui.end_row();
                    }
                });
            ui.separator();
            ui.label("紫色 ○ 连接候选 · 亮紫 ○ 已吸附 · 半透明：未连接");
            ui.label("常用按钮用符号与短标签表示；悬停可查看说明。");
        });
}

pub(crate) fn draw(mut contexts: EguiContexts, mut state: ResMut<HelpState>) {
    if let Ok(ctx) = contexts.ctx_mut() {
        show(ctx, &mut state.open);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ui::Pressed;

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
}
