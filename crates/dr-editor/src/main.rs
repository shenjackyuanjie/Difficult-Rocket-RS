mod attachment_hints;
mod connection_lines;
mod connection_smoke;
mod demo;
mod demo_cases;
mod egui_ui;
mod files;
mod free_mode;
mod help;
mod interaction_smoke;
mod native_input;
mod native_keys;
mod panels;
mod performance;
mod placement;
mod properties;
mod render;
mod scoped_smoke;
mod selection;
mod selection_smoke;
mod topology_ui;
mod transform_smoke;
mod view;
mod view_smoke;

use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::prelude::*;
use bevy::window::{PresentMode, WindowResolution};
use dr_core::geometry::{Vec2d, contains_point};
use dr_core::{
    Connection, EditorCommand, EditorHistory, LinkKind, Part, PartCatalog, PartKey, PartKind, Ship,
    load_catalog, load_ship, save_ship,
};
use dr_core::{SelectionPose, SelectionTransform, ShipFragment};
use std::collections::BTreeSet;

#[derive(Resource, Clone)]
pub struct EditorPaths {
    pub ship: Option<String>,
    pub catalog: String,
    pub assets: String,
}

#[derive(Resource)]
pub struct EditorDocument {
    pub free_mode: bool,
    pub revision: u64,
    pub ship: Ship,
    pub catalog: PartCatalog,
    pub selected: Option<PartKey>,
    selection: BTreeSet<PartKey>,
    pub dirty: bool,
    pub history: EditorHistory,
    saved_ship: Ship,
    status: String,
}

impl EditorDocument {
    fn selected_keys(&self) -> Vec<PartKey> {
        self.selection
            .iter()
            .copied()
            .chain(self.selected)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }
    fn is_selected(&self, key: PartKey) -> bool {
        self.selected == Some(key) || self.selection.contains(&key)
    }
    fn clear_selection(&mut self) {
        self.selected = None;
        self.selection.clear();
    }
    fn select_only(&mut self, key: Option<PartKey>) {
        self.clear_selection();
        self.selected = key;
    }
    fn refresh(&mut self) {
        self.revision = self.revision.wrapping_add(1);
        self.dirty = self.ship != self.saved_ship;
        if self
            .selected
            .is_some_and(|id| self.ship.part_at(id).is_none())
        {
            self.selected = None;
        }
    }
    fn undo(&mut self) -> bool {
        let changed = self.history.undo(&mut self.ship);
        if changed {
            self.clear_selection();
        }
        self.refresh();
        changed
    }
    fn redo(&mut self) -> bool {
        let changed = self.history.redo(&mut self.ship);
        if changed {
            self.clear_selection();
        }
        self.refresh();
        changed
    }
    fn execute(&mut self, command: EditorCommand) -> bool {
        let command = self.edit_command(command);
        let before: Vec<_> = self.ship.keyed_parts().map(|(key, _)| key).collect();
        match self
            .history
            .execute_with_catalog(&mut self.ship, &self.catalog, command)
        {
            Ok(()) => {
                if !before
                    .into_iter()
                    .eq(self.ship.keyed_parts().map(|(key, _)| key))
                {
                    self.clear_selection();
                }
                self.refresh();
                self.status.clear();
                true
            }
            Err(error) => {
                self.status = error.to_string();
                false
            }
        }
    }

    fn edit_command(&self, command: EditorCommand) -> EditorCommand {
        if self.free_mode {
            EditorCommand::FreeEdit(Box::new(command))
        } else {
            command
        }
    }
}

#[derive(Resource, Default)]
struct DragState {
    id: Option<PartKey>,
    members: BTreeSet<PartKey>,
    blocked: bool,
    command: Option<EditorCommand>,
    rectangle: Option<(f64, f64)>,
    rect_end: (f64, f64),
    additive: bool,
    origin: (f64, f64),
    offset: (f64, f64),
    preview: (f64, f64),
    turns: u8,
    fine_rotation: f64,
    flip_x: bool,
    flip_y: bool,
}

impl DragState {
    fn cancel(&mut self) {
        self.id = None;
        self.members.clear();
        self.rectangle = None;
        self.command = None;
        self.blocked = false;
        self.turns = 0;
        self.fine_rotation = 0.0;
        self.flip_x = false;
        self.flip_y = false;
    }
    /// 连续跟随指针，连接面吸附另行计算，不把拖动量化为半格。
    fn follow(&mut self, logical: (f64, f64)) {
        self.preview = (logical.0 + self.offset.0, logical.1 + self.offset.1);
    }

    fn moved(&self) -> bool {
        let delta = self.delta();
        !self.turns.is_multiple_of(4)
            || self.fine_rotation.abs() > 1e-10
            || self.flip_x
            || self.flip_y
            || delta.0.hypot(delta.1) > 1e-6
    }

    /// 松手后清理所有临时状态；单击仍保留选中，实际拖动不留下高亮。
    fn finish(&mut self, document: &mut EditorDocument) {
        if self.id.is_some() && self.moved() {
            let command = self.command.take().unwrap_or_else(|| {
                selection::drag_command(
                    &self.keys().into_iter().collect::<Vec<_>>(),
                    self.selection_pose(),
                    self.delta(),
                )
            });
            document.execute(command);
            document.clear_selection();
        }
        self.cancel();
    }

    fn contains(&self, key: PartKey) -> bool {
        self.id.is_some() && (self.id == Some(key) || self.members.contains(&key))
    }
    fn keys(&self) -> BTreeSet<PartKey> {
        if self.id.is_none() {
            BTreeSet::new()
        } else {
            self.members.iter().copied().chain(self.id).collect()
        }
    }
    fn point(&self, point: Vec2) -> Vec2 {
        let (x, y) = self
            .selection_pose()
            .point((point.x as f64 / 60.0, point.y as f64 / 60.0), self.delta());
        Vec2::new(x as f32, y as f32) * 60.0
    }
    fn pose(&self, key: PartKey, part: &Part) -> Part {
        if self.contains(key) {
            dr_core::edit::selection::pose_part(part, None, self.selection_pose(), self.delta())
                .unwrap_or_else(|_| part.clone())
        } else {
            part.clone()
        }
    }
    fn delta(&self) -> (f64, f64) {
        (
            self.preview.0 - self.origin.0,
            self.preview.1 - self.origin.1,
        )
    }
}

impl DragState {
    fn selection_pose(&self) -> SelectionPose {
        SelectionPose {
            pivot: self.origin,
            radians: self.turns as f64 * std::f64::consts::FRAC_PI_2 + self.fine_rotation,
            flip_x: self.flip_x,
            flip_y: self.flip_y,
        }
    }

    fn mirror(&mut self, x: bool) {
        if x {
            self.flip_x = !self.flip_x;
        } else {
            self.flip_y = !self.flip_y;
        }
        self.turns = (4 - self.turns) % 4;
        self.fine_rotation = -self.fine_rotation;
    }
}

/// 当前鼠标在编辑器世界坐标中的位置，以及目录中待放置的部件。
#[derive(Resource, Default)]
struct EditorCursor {
    manual_connection: Option<free_mode::Endpoint>,
    clipboard: Option<ShipFragment>,
    paste: Option<ShipFragment>,
    world: (f64, f64),
    catalog_index: usize,
    placing: bool,
    palette_drag: bool,
    rotation: i32,
    fine_rotation: f64,
    flip_x: bool,
    flip_y: bool,
    valid: bool,
}

impl EditorCursor {
    fn cancel_placement(&mut self) {
        self.placing = false;
        self.palette_drag = false;
    }
}

#[derive(Component)]
struct EditorHud;

#[derive(Resource, Default)]
struct CameraDrag(Option<Vec2>);

#[derive(Resource)]
struct SmokeTest {
    enabled: bool,
    native_dialogs: bool,
    native_file_dialogs: bool,
    interaction: bool,
    topology: bool,
    panels: bool,
    properties: bool,
    connections: bool,
    transforms: bool,
    performance: bool,
    performance_selection_count: usize,
    scoped: bool,
    repair: bool,
    selection: bool,
    view: bool,
    browser: bool,
    staging: bool,
    native_ime: bool,
    native_input: Option<std::path::PathBuf>,
    native_keys: Option<std::path::PathBuf>,
    egui: bool,
    started: std::time::Instant,
}

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let ship = args
        .windows(2)
        .find(|w| w[0] == "--ship" || w[0] == "-s")
        .map(|w| w[1].clone());
    let catalog_path = args
        .windows(2)
        .find(|w| w[0] == "--catalog" || w[0] == "-c")
        .map(|w| w[1].clone())
        .unwrap_or_else(|| "../Difficult-Rocket/assets/builtin/PartList.xml".into());
    let document = load_document(ship.as_deref(), &catalog_path)?;
    let assets = args
        .windows(2)
        .find(|w| w[0] == "--assets")
        .map(|w| w[1].clone())
        .unwrap_or_else(|| "../Difficult-Rocket/assets".into());
    let assets = std::path::absolute(&assets)
        .expect("资源路径无效")
        .to_string_lossy()
        .into_owned();
    let showcase = demo::Showcase::from_args(&args)?;
    let presentation = showcase.is_some() && args.iter().any(|arg| arg == "--demo-presentation");
    let mut app = App::new();
    if let Some(showcase) = showcase {
        app.insert_resource(showcase);
    }
    app.insert_resource(panels::ShipBrowser::new(
        std::path::Path::new(&assets).join("ships"),
    ))
    .init_resource::<panels::Palette>()
    .init_resource::<panels::UiPointer>()
    .init_resource::<properties::Inspector>()
    .insert_resource(SmokeTest {
        enabled: args.iter().any(|arg| arg == "--smoke-test"),
        topology: args.iter().any(|arg| arg == "--topology-smoke-test"),
        native_dialogs: args.iter().any(|arg| arg == "--native-dialog-test"),
        native_file_dialogs: args.iter().any(|arg| arg == "--native-file-dialog-test"),
        interaction: args.iter().any(|arg| arg == "--interaction-smoke-test"),
        panels: args.iter().any(|arg| arg == "--panel-smoke-test"),
        properties: args.iter().any(|arg| arg == "--properties-smoke-test"),
        connections: args.iter().any(|arg| arg == "--connection-smoke-test"),
        transforms: args.iter().any(|arg| arg == "--transforms-smoke-test"),
        performance: args.iter().any(|arg| arg == "--performance-test"),
        performance_selection_count: args
            .windows(2)
            .find(|pair| pair[0] == "--performance-selection-count")
            .and_then(|pair| pair[1].parse().ok())
            .unwrap_or(1),
        scoped: args.iter().any(|arg| arg == "--scoped-smoke-test"),
        repair: args.iter().any(|arg| arg == "--repair-smoke-test"),
        selection: args.iter().any(|arg| arg == "--selection-smoke-test"),
        view: args.iter().any(|arg| arg == "--view-smoke-test"),
        browser: args.iter().any(|arg| arg == "--browser-smoke-test"),
        staging: args.iter().any(|arg| arg == "--staging-smoke-test"),
        native_ime: args.iter().any(|arg| arg == "--native-ime-test"),
        native_keys: args
            .windows(2)
            .find(|args| args[0] == "--native-keys-test")
            .map(|args| std::path::PathBuf::from(&args[1])),
        native_input: args
            .windows(2)
            .find(|args| args[0] == "--native-input-test")
            .map(|args| std::path::PathBuf::from(&args[1])),
        egui: args.iter().any(|arg| arg == "--egui-smoke-test"),
        started: std::time::Instant::now(),
    })
    .insert_resource(EditorPaths {
        ship,
        catalog: catalog_path,
        assets: assets.clone(),
    })
    .insert_resource(document)
    .init_resource::<DragState>()
    .init_resource::<EditorCursor>()
    .init_resource::<CameraDrag>()
    .init_resource::<connection_lines::Settings>()
    .init_resource::<connection_lines::Controls>()
    .init_resource::<transform_smoke::Captured>()
    .init_resource::<demo_cases::Run>()
    .init_resource::<demo_cases::Captured>()
    .init_resource::<interaction_smoke::Probe>()
    .init_resource::<view::ViewOptions>()
    .add_plugins(
        DefaultPlugins
            .set(bevy::log::LogPlugin {
                filter: format!("{},icu_provider=error", bevy::log::DEFAULT_FILTER),
                ..default()
            })
            // 编辑器优先输入延迟，不让第 N 帧输入等到第 N+1 帧渲染。
            .disable::<bevy::render::pipelined_rendering::PipelinedRenderingPlugin>()
            .set(AssetPlugin {
                file_path: assets,
                ..default()
            })
            .set(WindowPlugin {
                close_when_requested: false,
                primary_window: Some(Window {
                    title: "Difficult Rocket Editor".into(),
                    // 录制使用固定物理像素和无边框窗口，不随桌面 DPI 改变成片尺寸。
                    resolution: if presentation {
                        WindowResolution::new(1920, 1080).with_scale_factor_override(1.)
                    } else {
                        WindowResolution::new(1440, 900)
                    },
                    decorations: !presentation,
                    resize_constraints: bevy::window::WindowResizeConstraints {
                        min_width: 960.0,
                        min_height: 640.0,
                        ..default()
                    },
                    present_mode: PresentMode::AutoVsync,
                    // 保留无撕裂显示，避免交换链再积压多帧旧输入。
                    desired_maximum_frame_latency: std::num::NonZeroU32::new(1),
                    ..default()
                }),
                ..default()
            }),
    )
    .init_gizmo_group::<connection_lines::LineGizmos>()
    .add_plugins(egui_ui::EditorEguiPlugin)
    .add_message::<files::FileAction>()
    .add_message::<panels::PanelButton>()
    .add_systems(
        Startup,
        (
            setup_camera,
            setup_hud,
            files::setup_file_toolbar,
            help::setup,
            properties::setup,
        ),
    )
    .add_systems(
        Update,
        (
            (
                demo::begin,
                demo::restore_pointer,
                demo_cases::run.run_if(demo::advance_ready),
                (
                    panels::smoke::run,
                    topology_ui::smoke::run,
                    properties::egui_smoke::run,
                    connection_smoke::run,
                    transform_smoke::run,
                    interaction_smoke::input,
                    performance::run,
                    native_input::run,
                    native_keys::run,
                    scoped_smoke::run,
                    properties::repair_smoke::run,
                    selection_smoke::run,
                    view_smoke::run,
                    panels::browser_smoke::run,
                    properties::staging_smoke::run,
                    properties::native_ime::run,
                    files::native_dialog_test,
                )
                    .chain()
                    .run_if(demo::advance_ready),
                demo::animate,
                panels::pointer_over_ui,
                help::input,
                properties::actions,
                egui_ui::prepare_input,
                panels::panel_actions.run_if(properties::closed),
                files::toolbar_actions.run_if(properties::closed),
                files::file_inputs,
                properties::cancel_for_file_action,
                placement::cancel_for_file_action,
                files::file_actions,
                view::controls.run_if(egui_ui::canvas_input_available),
                camera_controls,
                mouse_editor.run_if(egui_ui::canvas_input_available),
                keyboard_commands.run_if(egui_ui::canvas_input_available),
            )
                .chain(),
            render::sync,
            placement::draw_preview,
            selection::draw_preview,
            connection_lines::configure_system,
            render::connections,
            attachment_hints::draw,
            demo_cases::draw,
            demo_cases::draw_degenerate,
            view::draw_debug,
            update_hud,
            files::update_window_title,
            capture_screenshot,
            files::native_file_dialog_test,
            demo::pace,
        )
            .chain(),
    )
    .add_systems(
        PostUpdate,
        interaction_smoke::verify.after(bevy::transform::TransformSystems::Propagate),
    )
    .add_systems(
        PostUpdate,
        demo::isolate_pointer.after(bevy_egui::EguiPostUpdateSet::EndPass),
    )
    .add_systems(Last, demo::finish)
    .add_systems(
        bevy_egui::EguiPrimaryContextPass,
        demo::overlay.after(topology_ui::draw),
    )
    .run();
    Ok(())
}

fn capture_screenshot(
    mut commands: Commands,
    mode: Res<SmokeTest>,
    keys: Res<ButtonInput<KeyCode>>,
    mut captured: Local<bool>,
) {
    use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk};
    if mode.enabled && mode.started.elapsed().as_secs() > 30 {
        panic!("窗口截图自测超时");
    }
    let smoke = mode.enabled && !*captured && mode.started.elapsed().as_secs() >= 5;
    if smoke || keys.just_pressed(KeyCode::F12) {
        *captured = true;
        let path = if smoke {
            "target/editor-smoke.png".to_owned()
        } else {
            format!(
                "editor-{}.png",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis()
            )
        };
        let mut entity = commands.spawn(Screenshot::primary_window());
        entity.observe(save_to_disk(path));
        if smoke {
            entity.observe(
                |_: On<ScreenshotCaptured>, mut exit: MessageWriter<AppExit>| {
                    exit.write(AppExit::Success);
                },
            );
        }
    }
}

fn load_document(ship_path: Option<&str>, catalog_path: &str) -> anyhow::Result<EditorDocument> {
    let catalog = load_catalog(catalog_path)?;
    let ship = ship_path
        .map(load_ship)
        .transpose()?
        .unwrap_or_else(|| new_ship(&catalog));
    Ok(EditorDocument {
        free_mode: false,
        revision: 0,
        saved_ship: ship.clone(),
        status: String::new(),
        ship,
        catalog,
        selected: None,
        selection: default(),
        dirty: false,
        history: EditorHistory::with_limit(256),
    })
}

/// 与原版 assets/builtin/none_ship.xml 一致，新船体自带隐藏的驾驶舱。
fn new_ship(catalog: &PartCatalog) -> Ship {
    let Some(pod) = catalog.types.iter().find(|kind| kind.kind == PartKind::Pod) else {
        return Ship::default();
    };
    Ship {
        touching_ground: false,
        parts: vec![pod.instantiate(1, (0.0, pod.half_extents().1))],
        ..Ship::default()
    }
}

fn setup_camera(mut commands: Commands) {
    commands.spawn((Camera2d, Name::new("Editor camera")));
}

#[allow(clippy::too_many_arguments)]
fn camera_controls(
    mut wheels: MessageReader<MouseWheel>,
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mut cameras: Query<(&mut Transform, &mut Projection), With<Camera2d>>,
    mut drag: ResMut<CameraDrag>,
    editor_drag: Res<DragState>,
    editor_cursor: Res<EditorCursor>,
    options: Res<view::ViewOptions>,
    pointer: Res<panels::UiPointer>,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    let Ok((mut transform, mut projection)) = cameras.single_mut() else {
        return;
    };
    let Projection::Orthographic(projection) = &mut *projection else {
        return;
    };
    if pointer.blocked {
        drag.0 = None;
        wheels.clear();
        return;
    }
    if keys.just_pressed(KeyCode::Home) && window.focused {
        transform.translation.x = 0.0;
        transform.translation.y = 0.0;
        projection.scale = 1.0;
    }
    let Some(cursor) = window.cursor_position().filter(|_| window.focused) else {
        drag.0 = None;
        wheels.clear();
        return;
    };
    let can_pan = editor_drag.id.is_none()
        && editor_cursor.manual_connection.is_none()
        && editor_drag.rectangle.is_none()
        && !editor_cursor.placing
        && editor_cursor.paste.is_none();
    let pan_button = options.box_select_button.pan();
    if keys.just_pressed(KeyCode::Escape) || mouse.just_pressed(pan_button) {
        drag.0 = None;
    }
    let pan_active =
        mouse.pressed(pan_button) || (mouse.just_released(pan_button) && drag.0.is_some());
    if can_pan && pan_active && !keys.just_pressed(KeyCode::Escape) {
        if let Some(previous) = drag.0 {
            let delta = cursor - previous;
            transform.translation.x -= delta.x * projection.scale;
            transform.translation.y += delta.y * projection.scale;
        }
        drag.0 = mouse.pressed(pan_button).then_some(cursor);
    } else {
        drag.0 = None;
    }
    let offset = Vec2::new(
        cursor.x - window.width() / 2.0,
        window.height() / 2.0 - cursor.y,
    );
    for wheel in wheels.read() {
        let delta = match wheel.unit {
            MouseScrollUnit::Line => wheel.y,
            MouseScrollUnit::Pixel => wheel.y / 40.0,
        };
        let old = projection.scale;
        projection.scale = (old * 2.0_f32.powf(-delta * 0.25)).clamp(0.1, 100.0);
        let shift = offset * (old - projection.scale);
        transform.translation.x += shift.x;
        transform.translation.y += shift.y;
    }
}

fn setup_hud(mut commands: Commands, assets: Res<AssetServer>) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(264),
                top: px(60),
                max_width: Val::Percent(40.0),
                overflow: Overflow::clip(),
                padding: UiRect::all(px(8)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.03, 0.04, 0.06, 0.86)),
            Name::new("Editor HUD"),
            panels::EditorPanel,
        ))
        .with_children(|root| {
            root.spawn((
                EditorHud,
                TextLayout::default().with_no_wrap(),
                Text::new("DR Editor"),
                TextFont {
                    font: bevy::text::FontSource::Handle(assets.load(
                        "fonts/HarmonyOS_Sans/HarmonyOS_Sans_SC/HarmonyOS_Sans_SC_Regular.ttf",
                    )),
                    font_size: FontSize::Px(14.0),
                    ..default()
                },
                TextColor(Color::srgb(0.9, 0.93, 0.98)),
            ));
        });
}

/// 处理全局快捷键，并将所有会改变船体的操作记录到历史栈。
fn keyboard_commands(
    keys: Res<ButtonInput<KeyCode>>,
    mut document: ResMut<EditorDocument>,
    mut cursor: ResMut<EditorCursor>,
    drag: Res<DragState>,
    palette: Res<panels::Palette>,
    pointer: Res<panels::UiPointer>,
) {
    if drag.id.is_some() || drag.rectangle.is_some() {
        return;
    }
    let control = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    if control && keys.just_pressed(KeyCode::KeyZ) {
        if cursor.palette_drag {
            cursor.cancel_placement();
        }
        cursor.paste = None;
        document.undo();
        return;
    }
    if control && keys.just_pressed(KeyCode::KeyY) {
        if cursor.palette_drag {
            cursor.cancel_placement();
        }
        cursor.paste = None;
        document.redo();
        return;
    }
    if selection::keyboard(&mut document, &mut cursor, &keys, pointer.blocked) {
        return;
    }
    if control {
        return;
    }
    if keys.just_pressed(KeyCode::Tab) {
        let reverse = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
        panels::cycle_part(&document, &palette, &mut cursor, reverse);
        return;
    }
    if cursor.placing {
        if let Some(radians) = selection::rotation_input(&keys) {
            cursor.fine_rotation += radians;
        }
        if keys.just_pressed(KeyCode::KeyR) {
            cursor.rotation = (cursor.rotation + 1).rem_euclid(4);
        }
        if keys.just_pressed(KeyCode::KeyX) {
            cursor.flip_x = !cursor.flip_x;
        }
        if keys.just_pressed(KeyCode::KeyY) {
            cursor.flip_y = !cursor.flip_y;
        }
    }
    if keys.just_pressed(KeyCode::KeyP) && cursor.valid && !pointer.blocked {
        if cursor.palette_drag {
            placement::finish_palette_drag(&mut document, &mut cursor, true);
        } else {
            placement::place(&mut document, &cursor);
        }
        return;
    }
    if cursor.placing {
        return;
    }
    if let Some(radians) = selection::rotation_input(&keys) {
        let selected = document.selected_keys();
        if !selected.is_empty() {
            let center = selection::center(&document, &selected);
            document.execute(EditorCommand::TransformSelection {
                parts: selected,
                transform: SelectionTransform::RotateBy { center, radians },
            });
        }
        return;
    }
    if keys.just_pressed(KeyCode::Delete)
        && let Some(id) = document.selected
    {
        let _ = document.execute(EditorCommand::Delete(id.id).at(id));
    }
    if let Some(id) = document.selected {
        if keys.just_pressed(KeyCode::KeyR) {
            let can_rotate = document
                .ship
                .part_at(id)
                .and_then(|part| document.catalog.get(&part.part_type))
                .map(|part_type| !part_type.disable_editor_rotation)
                .unwrap_or(false);
            if can_rotate {
                placement::transform(&mut document, id, EditorCommand::Rotate(id.id));
            }
        }
        if keys.just_pressed(KeyCode::KeyX) {
            placement::transform(&mut document, id, EditorCommand::FlipX(id.id));
        }
        if keys.just_pressed(KeyCode::KeyY) {
            placement::transform(&mut document, id, EditorCommand::FlipY(id.id));
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn mouse_editor(
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    cameras: Query<(&Camera, &Transform, &Projection), With<Camera2d>>,
    mut drag: ResMut<DragState>,
    mut document: ResMut<EditorDocument>,
    mut cursor: ResMut<EditorCursor>,
    options: Res<view::ViewOptions>,
    pointer: Res<panels::UiPointer>,
    mut camera_drag: ResMut<CameraDrag>,
) {
    if keys.just_pressed(KeyCode::Escape) || mouse.just_pressed(MouseButton::Right) {
        drag.cancel();
        cursor.cancel_placement();
        cursor.paste = None;
        cursor.manual_connection = None;
        return;
    }
    let Ok(window) = windows.single() else {
        return;
    };
    // 失焦或窗口外释放必须取消预览，不能让部件继续粘在鼠标上。
    if !window.focused || window.cursor_position().is_none() {
        cursor.valid = false;
        cursor.manual_connection = None;
        if !window.focused || !mouse.pressed(MouseButton::Left) {
            drag.cancel();
            if cursor.palette_drag {
                cursor.cancel_placement();
            }
        }
        return;
    }
    let box_button = options.box_select_button.select();
    if pointer.palette_drop && drag.id.is_some() {
        cursor.valid = false;
        if !mouse.pressed(MouseButton::Left) {
            let roots: Vec<_> = drag.keys().into_iter().collect();
            match selection::connected_keys(&document.ship, &roots) {
                Ok(parts) => {
                    let count = parts.len();
                    if document.execute(EditorCommand::DeleteSelection(parts.into_iter().collect()))
                    {
                        document.status = format!("已删除 {count} 个相连部件 · Ctrl+Z 撤销");
                        document.clear_selection();
                    }
                }
                Err(message) => document.status = message,
            }
            drag.cancel();
        }
        return;
    }
    if pointer.blocked {
        cursor.valid = false;
        if (drag.id.is_some() && !mouse.pressed(MouseButton::Left))
            || (drag.rectangle.is_some() && !mouse.pressed(box_button))
        {
            drag.cancel();
        }
        if cursor.palette_drag && !mouse.pressed(MouseButton::Left) {
            cursor.cancel_placement();
        }
        return;
    }
    let Ok((camera, transform, projection)) = cameras.single() else {
        return;
    };
    let Some(world) = view::cursor_world(window, camera, transform, projection) else {
        return;
    };
    let logical = (world.x as f64 / 60.0, world.y as f64 / 60.0);
    cursor.world = logical;
    cursor.valid = true;
    free_mode::discard_stale(&document, &mut cursor);
    if cursor.paste.is_some() {
        if mouse.just_pressed(MouseButton::Left) {
            selection::commit_paste(&mut document, &mut cursor);
        }
        return;
    }
    if cursor.placing {
        if cursor.palette_drag {
            // egui 的拖动消息在下一帧消费，松手可能已不在 just_released 帧。
            if !mouse.pressed(MouseButton::Left) {
                placement::finish_palette_drag(&mut document, &mut cursor, true);
            }
        } else if mouse.just_pressed(MouseButton::Left) {
            placement::place(&mut document, &cursor);
        }
        return;
    }
    if document.free_mode && mouse.just_pressed(MouseButton::Left) && drag.id.is_none() {
        let scale = match projection {
            Projection::Orthographic(p) => p.scale,
            _ => 1.0,
        };
        if free_mode::click(
            &mut document,
            &mut cursor,
            logical,
            8.0 * scale as f64 / 60.0,
        ) {
            camera_drag.0 = None;
            return;
        }
    }
    let left_pressed = mouse.just_pressed(MouseButton::Left);
    let box_pressed = mouse.just_pressed(box_button);
    if left_pressed || box_pressed {
        let selected = document
            .ship
            .keyed_parts()
            .filter(|(_, part)| {
                document.catalog.get(&part.part_type).is_some_and(|ty| {
                    contains_point(
                        part,
                        ty,
                        Vec2d {
                            x: logical.0,
                            y: logical.1,
                        },
                    )
                })
            })
            .map(|(key, _)| key)
            .last();
        let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
        if left_pressed && selected.is_some() {
            drag.cancel();
            selection::click(&mut document, selected, shift);
            if !shift && let Some(part) = selected.and_then(|key| document.ship.part_at(key)) {
                drag.id = selected;
                drag.members = selection::drag_keys(
                    &document.ship,
                    &document.selected_keys(),
                    options.follow_children,
                );
                drag.origin = (part.x, part.y);
                drag.preview = drag.origin;
                drag.offset = (part.x - logical.0, part.y - logical.1);
            }
        } else if left_pressed && selected.is_none() {
            drag.cancel();
            selection::click(&mut document, None, shift);
            if box_pressed {
                drag.rectangle = Some(logical);
                drag.rect_end = logical;
                drag.additive = shift;
            }
        } else if box_pressed && selected.is_none() {
            drag.cancel();
            drag.rectangle = Some(logical);
            drag.rect_end = logical;
            drag.additive = shift;
            if !shift {
                document.clear_selection();
            }
        }
    }
    if let Some(start) = drag.rectangle {
        drag.rect_end = logical;
        if mouse.just_released(box_button) {
            selection::rectangle(&mut document, start, logical, drag.additive);
            drag.cancel();
        }
        return;
    }
    if drag.id.is_some()
        && (mouse.pressed(MouseButton::Left) || mouse.just_released(MouseButton::Left))
    {
        // 保留抓取偏移和非网格坐标，小幅移动也要立即响应。
        drag.follow(logical);
    }
    if drag.id.is_some() {
        let selected: Vec<_> = drag.keys().into_iter().collect();
        if keys.just_pressed(KeyCode::KeyR) {
            if selected.iter().any(|key| {
                document
                    .ship
                    .part_at(*key)
                    .and_then(|part| document.catalog.get(&part.part_type))
                    .is_some_and(|kind| kind.disable_editor_rotation)
            }) {
                document.status = "所拖部件中有禁止旋转的部件".into();
            } else {
                drag.turns = (drag.turns + 1) % 4;
            }
        }
        if let Some(radians) = selection::rotation_input(&keys) {
            if selected.iter().any(|key| {
                document
                    .ship
                    .part_at(*key)
                    .and_then(|part| document.catalog.get(&part.part_type))
                    .is_some_and(|kind| kind.disable_editor_rotation)
            }) {
                document.status = "所拖部件中有禁止旋转的部件".into();
            } else {
                drag.fine_rotation += radians;
            }
        }
        let control = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
        if !control && keys.just_pressed(KeyCode::KeyX) {
            drag.mirror(true);
        }
        if !control && keys.just_pressed(KeyCode::KeyY) {
            drag.mirror(false);
        }
        let (delta, command, clear) = selection::movement_transformed(
            &document,
            &selected,
            drag.selection_pose(),
            drag.delta(),
        );
        drag.preview = (drag.origin.0 + delta.0, drag.origin.1 + delta.1);
        drag.command = Some(command);
        drag.blocked = !clear;
    }
    if drag.id.is_some() && !mouse.pressed(MouseButton::Left) {
        drag.finish(&mut document);
    }
}

/// 将预览落点、断开旧连接和新吸附作为一个可撤销操作。
#[cfg(test)]
fn move_with_snap(
    ship: &Ship,
    catalog: &PartCatalog,
    id: PartKey,
    position: (f64, f64),
) -> Option<EditorCommand> {
    let source = ship.part_at(id)?;
    let origin = (source.x, source.y);
    let delta = (position.0 - origin.0, position.1 - origin.1);
    Some(selection::movement_for(ship, catalog, &[id], origin, 0, delta).1)
}

fn update_hud(document: Res<EditorDocument>, mut labels: Query<&mut Text, With<EditorHud>>) {
    if !document.is_changed() {
        return;
    }
    let value = format!(
        "部件 {} · 已选 {} · {}\n质量 主/全 {:.2}/{:.2}{}",
        document.ship.all_parts().count(),
        document.selected_keys().len(),
        if document.dirty {
            "● 未保存"
        } else {
            "✓ 已保存"
        },
        document
            .ship
            .mass(&document.catalog, dr_core::ShipScope::Main),
        document
            .ship
            .mass(&document.catalog, dr_core::ShipScope::All),
        if document.status.is_empty() {
            String::new()
        } else {
            format!("\n! {}", document.status)
        },
    );
    for mut text in &mut labels {
        if text.0 != value {
            text.0.clone_from(&value);
        }
    }
}

#[cfg(test)]
mod interaction_tests;
#[cfg(test)]
mod tests;
