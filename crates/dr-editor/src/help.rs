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

pub(crate) fn setup(mut commands: Commands, assets: Res<AssetServer>) {
    let font = assets.load("fonts/HarmonyOS_Sans/HarmonyOS_Sans_SC/HarmonyOS_Sans_SC_Regular.ttf");
    commands
        .spawn((
            Button,
            HelpButton,
            panels::EditorPanel,
            Node {
                position_type: PositionType::Absolute,
                top: px(12),
                right: px(16),
                padding: UiRect::axes(px(12), px(7)),
                ..default()
            },
            BackgroundColor(Color::srgb(0.13, 0.18, 0.24)),
        ))
        .with_children(|button| {
            button.spawn((
                Text::new("? 帮助 · F1"),
                TextFont {
                    font: bevy::text::FontSource::Handle(font),
                    font_size: FontSize::Px(16.0),
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
        });
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(crate) fn input(
    keys: Res<ButtonInput<KeyCode>>,
    mut buttons: Query<
        (&Interaction, &mut BackgroundColor),
        (With<HelpButton>, Changed<Interaction>),
    >,
    mut state: ResMut<HelpState>,
    inspector: Res<properties::Inspector>,
    topology: Res<topology_ui::ConnectionEditor>,
    pending: Res<files::PendingFileAction>,
    mut drag: ResMut<DragState>,
    mut cursor: ResMut<EditorCursor>,
    mut camera_drag: ResMut<CameraDrag>,
    mut pointer: ResMut<panels::UiPointer>,
) {
    state.suppress_frame = false;
    if inspector.is_open() || topology.open || pending.is_blocked() {
        state.open = false;
        return;
    }
    let mut toggle = keys.just_pressed(KeyCode::F1);
    for (interaction, mut color) in &mut buttons {
        color.0 = match interaction {
            Interaction::Pressed => {
                toggle = true;
                Color::srgb(0.3, 0.5, 0.65)
            }
            Interaction::Hovered => Color::srgb(0.22, 0.32, 0.43),
            Interaction::None => Color::srgb(0.13, 0.18, 0.24),
        };
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
        drag.cancel();
        cursor.cancel_placement();
        cursor.paste = None;
        camera_drag.0 = None;
        pointer.blocked = true;
    }
}

const SHORTCUTS: &[(&str, &str)] = &[
    ("左键 / Shift+左键", "选择、拖动 / 增减选择"),
    ("R（拖拽时也可用）", "旋转；拖拽期间不镜像、不复制"),
    ("Esc / 右键", "取消预览；重叠落下保留位置并断开外部连接"),
    ("中键拖动 / Ctrl+A", "框选 / 全选；侧栏可切换框选键"),
    ("Tab / Shift+Tab · P", "切换目录部件 · 放置"),
    ("Delete · X / Y", "删除 · 镜像（非拖拽状态）"),
    ("↳ 子节点跟随", "侧栏开关；拖父节点带子孙，不带父节点"),
    ("拖到右侧部件列表", "松手删除整个相连分量；Ctrl+Z 撤销"),
    ("Ctrl+C / X / V", "复制 / 剪切 / 粘贴"),
    ("Ctrl+Z / Y", "撤销 / 重做"),
    ("Ctrl+N / O / S / Shift+S", "新建 / 打开 / 保存 / 另存为"),
    ("滚轮 · 空白拖动", "鼠标位置缩放 · 平移（框选键之外的键）"),
    ("Home · F / Shift+F", "视图复位 · 适配整船 / 选区"),
    ("F2 · F6", "属性与分级 · 连接树 / 图"),
    ("F3 · F4 · F12", "调试轮廓 · 显隐贴图 · 截图"),
    ("F1 / Esc", "开关帮助 / 关闭"),
];

fn show(ctx: &egui::Context, open: &mut bool) {
    if !*open {
        return;
    }
    egui::Window::new("? 操作帮助")
        .id(egui::Id::new("editor_help"))
        .open(open)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .show(ctx, |ui| {
            egui::Grid::new("editor_shortcuts")
                .striped(true)
                .spacing([20.0, 9.0])
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

    #[derive(Resource, Default)]
    struct CanvasTicks(u32);

    fn app() -> App {
        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<HelpState>()
            .init_resource::<properties::Inspector>()
            .init_resource::<topology_ui::ConnectionEditor>()
            .init_resource::<files::PendingFileAction>()
            .init_resource::<DragState>()
            .init_resource::<EditorCursor>()
            .init_resource::<CameraDrag>()
            .init_resource::<panels::UiPointer>()
            .init_resource::<CanvasTicks>()
            .insert_resource(crate::tests::document())
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
            .insert_resource(crate::tests::document())
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
    fn actual_toolbar_interaction_toggles_help() {
        let mut app = app();
        let entity = app
            .world_mut()
            .spawn((
                HelpButton,
                Interaction::Pressed,
                BackgroundColor(Color::BLACK),
            ))
            .id();
        app.update();
        assert!(app.world().resource::<HelpState>().open);
        app.world_mut().entity_mut(entity).insert(Interaction::None);
        app.update();
        app.world_mut()
            .entity_mut(entity)
            .insert(Interaction::Pressed);
        app.update();
        assert!(!app.world().resource::<HelpState>().open);
        assert_eq!(app.world().resource::<CanvasTicks>().0, 0);
    }

    #[test]
    fn help_is_opt_in_and_keeps_drag_restrictions_visible() {
        assert!(!HelpState::default().open);
        assert!(
            SHORTCUTS
                .iter()
                .any(|(_, text)| text.contains("不镜像、不复制"))
        );
        assert!(
            SHORTCUTS
                .iter()
                .any(|(_, text)| text.contains("断开外部连接"))
        );
        let ctx = egui::Context::default();
        let mut open = true;
        let mut output = ctx.run_ui(egui::RawInput::default(), |_| show(&ctx, &mut open));
        output.textures_delta.clear();
        assert!(!output.shapes.is_empty());
        assert!(open);
    }
}
