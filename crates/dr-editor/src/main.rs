mod connection_smoke;
mod files;
mod panels;
mod performance;
mod placement;
mod properties;
mod render;
mod scoped_smoke;

use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::prelude::*;
use bevy::window::{PresentMode, WindowResolution};
use dr_core::geometry::{Vec2d, contains_point};
use dr_core::{
    Connection, EditorCommand, EditorHistory, LinkKind, Part, PartCatalog, PartKey, PartKind, Ship,
    load_catalog, load_ship, save_ship,
};

#[derive(Resource, Clone)]
pub struct EditorPaths {
    pub ship: Option<String>,
    pub catalog: String,
    pub assets: String,
}

#[derive(Resource)]
pub struct EditorDocument {
    pub ship: Ship,
    pub catalog: PartCatalog,
    pub selected: Option<PartKey>,
    pub dirty: bool,
    pub history: EditorHistory,
    saved_ship: Ship,
    status: String,
}

impl EditorDocument {
    fn refresh(&mut self) {
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
            self.selected = None;
        }
        self.refresh();
        changed
    }
    fn redo(&mut self) -> bool {
        let changed = self.history.redo(&mut self.ship);
        if changed {
            self.selected = None;
        }
        self.refresh();
        changed
    }
    fn execute(&mut self, command: EditorCommand) -> bool {
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
                    self.selected = None;
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
}

#[derive(Resource, Default)]
struct DragState {
    id: Option<PartKey>,
    origin: (f64, f64),
    offset: (f64, f64),
    preview: (f64, f64),
}

/// 当前鼠标在编辑器世界坐标中的位置，以及目录中待放置的部件。
#[derive(Resource, Default)]
struct EditorCursor {
    world: (f64, f64),
    catalog_index: usize,
    placing: bool,
    rotation: i32,
    flip_x: bool,
    flip_y: bool,
    valid: bool,
}

#[derive(Component)]
struct EditorHud;

#[derive(Resource, Default)]
struct CameraDrag(Option<Vec2>);

#[derive(Resource)]
struct SmokeTest {
    enabled: bool,
    native_dialogs: bool,
    panels: bool,
    properties: bool,
    connections: bool,
    performance: bool,
    scoped: bool,
    repair: bool,
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
    App::new()
        .insert_resource(panels::ShipBrowser::new(
            std::path::Path::new(&assets).join("ships"),
        ))
        .init_resource::<panels::Palette>()
        .init_resource::<panels::UiPointer>()
        .init_resource::<properties::Inspector>()
        .insert_resource(SmokeTest {
            enabled: args.iter().any(|arg| arg == "--smoke-test"),
            native_dialogs: args.iter().any(|arg| arg == "--native-dialog-test"),
            panels: args.iter().any(|arg| arg == "--panel-smoke-test"),
            properties: args.iter().any(|arg| arg == "--properties-smoke-test"),
            connections: args.iter().any(|arg| arg == "--connection-smoke-test"),
            performance: args.iter().any(|arg| arg == "--performance-test"),
            scoped: args.iter().any(|arg| arg == "--scoped-smoke-test"),
            repair: args.iter().any(|arg| arg == "--repair-smoke-test"),
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
        .add_plugins(
            DefaultPlugins
                .set(AssetPlugin {
                    file_path: assets,
                    ..default()
                })
                .set(WindowPlugin {
                    close_when_requested: false,
                    primary_window: Some(Window {
                        title: "Difficult Rocket Editor".into(),
                        resolution: WindowResolution::new(1440, 900),
                        present_mode: PresentMode::AutoVsync,
                        ..default()
                    }),
                    ..default()
                }),
        )
        .add_message::<files::FileAction>()
        .add_systems(
            Startup,
            (
                setup_camera,
                setup_hud,
                files::setup_file_toolbar,
                properties::setup,
            ),
        )
        .add_systems(
            Update,
            (
                (
                    panels::smoke::run,
                    properties::smoke::run,
                    connection_smoke::run,
                    performance::run,
                    scoped_smoke::run,
                    properties::repair_smoke::run,
                    panels::pointer_over_ui,
                    properties::actions,
                    properties::input,
                    panels::panel_actions.run_if(properties::closed),
                    files::toolbar_actions.run_if(properties::closed),
                    files::file_inputs,
                    properties::cancel_for_file_action,
                    placement::cancel_for_file_action,
                    files::file_actions,
                    mouse_editor.run_if(properties::closed),
                    keyboard_commands.run_if(properties::closed),
                    camera_controls.run_if(properties::closed),
                )
                    .chain(),
                panels::scroll_panels,
                render::sync,
                placement::draw_preview,
                panels::render_palette,
                panels::render_browser,
                properties::render,
                render::connections,
                update_hud,
                files::update_window_title,
                capture_screenshot,
                files::native_dialog_test,
            )
                .chain(),
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
        saved_ship: ship.clone(),
        status: String::new(),
        ship,
        catalog,
        selected: None,
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
    if keys.just_pressed(KeyCode::Home) {
        transform.translation.x = 0.0;
        transform.translation.y = 0.0;
        projection.scale = 1.0;
    }
    let Some(cursor) = window.cursor_position().filter(|_| window.focused) else {
        drag.0 = None;
        wheels.clear();
        return;
    };
    if pointer.blocked {
        drag.0 = None;
        wheels.clear();
        return;
    }
    if mouse.pressed(MouseButton::Middle) {
        if let Some(previous) = drag.0 {
            let delta = cursor - previous;
            transform.translation.x -= delta.x * projection.scale;
            transform.translation.y += delta.y * projection.scale;
        }
        drag.0 = Some(cursor);
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
                left: px(16),
                top: px(60),
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
                    font_size: FontSize::Px(16.0),
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
    if drag.id.is_some() {
        return;
    }
    let control = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    if control && keys.just_pressed(KeyCode::KeyZ) {
        document.undo();
        return;
    }
    if control && keys.just_pressed(KeyCode::KeyY) {
        document.redo();
        return;
    }
    if keys.just_pressed(KeyCode::Tab) {
        let reverse = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
        panels::cycle_part(&document, &palette, &mut cursor, reverse);
        return;
    }
    if cursor.placing {
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
        placement::place(&mut document, &cursor);
        return;
    }
    if cursor.placing {
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
    cameras: Query<(&Camera, &GlobalTransform)>,
    mut drag: ResMut<DragState>,
    mut document: ResMut<EditorDocument>,
    mut cursor: ResMut<EditorCursor>,
    pointer: Res<panels::UiPointer>,
) {
    if keys.just_pressed(KeyCode::Escape) || mouse.just_pressed(MouseButton::Right) {
        drag.id = None;
        cursor.placing = false;
        return;
    }
    let Ok(window) = windows.single() else {
        return;
    };
    // 失焦或窗口外释放必须取消预览，不能让部件继续粘在鼠标上。
    if !window.focused || window.cursor_position().is_none() {
        cursor.valid = false;
        if !window.focused || !mouse.pressed(MouseButton::Left) {
            drag.id = None;
        }
        return;
    }
    if pointer.blocked {
        cursor.valid = false;
        if mouse.just_released(MouseButton::Left) {
            drag.id = None;
        }
        return;
    }
    let Ok((camera, transform)) = cameras.single() else {
        return;
    };
    let Ok(world) = camera.viewport_to_world_2d(transform, window.cursor_position().unwrap())
    else {
        return;
    };
    let logical = (world.x as f64 / 60.0, world.y as f64 / 60.0);
    cursor.world = logical;
    cursor.valid = true;
    if cursor.placing {
        if mouse.just_pressed(MouseButton::Left) {
            placement::place(&mut document, &cursor);
        }
        return;
    }
    if mouse.just_pressed(MouseButton::Left) {
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
        document.selected = selected;
        if let Some(part) = document.selected.and_then(|id| document.ship.part_at(id)) {
            drag.id = document.selected;
            drag.origin = (part.x, part.y);
            drag.preview = drag.origin;
            drag.offset = (part.x - logical.0, part.y - logical.1);
        }
    }
    if drag.id.is_some()
        && (mouse.pressed(MouseButton::Left) || mouse.just_released(MouseButton::Left))
    {
        // 以按下位置为锚点，单击不会把原有非网格坐标改写。
        let dx = (logical.0 + drag.offset.0 - drag.origin.0) * 2.0;
        let dy = (logical.1 + drag.offset.1 - drag.origin.1) * 2.0;
        drag.preview = (
            drag.origin.0 + dx.round() / 2.0,
            drag.origin.1 + dy.round() / 2.0,
        );
    }
    if let Some(id) = drag.id
        && let Some(mut part) = document.ship.part_at(id).cloned()
    {
        part.x = drag.preview.0;
        part.y = drag.preview.1;
        placement::snap(&document.ship, &document.catalog, &mut part, Some(id));
        drag.preview = (part.x, part.y);
    }
    if mouse.just_released(MouseButton::Left)
        && let Some(id) = drag.id.take()
        && drag.preview != drag.origin
    {
        let command = move_with_snap(&document.ship, &document.catalog, id, drag.preview);
        if let Some(command) = command {
            document.execute(command);
        } else {
            document.status = "无法移动：该位置与其他部件重叠".into();
        }
    }
}

/// 将预览落点、断开旧连接和新吸附作为一个可撤销操作。
fn move_with_snap(
    ship: &Ship,
    catalog: &PartCatalog,
    id: PartKey,
    position: (f64, f64),
) -> Option<EditorCommand> {
    let mut source = ship.part_at(id)?.clone();
    let origin = (source.x, source.y);
    source.x = position.0;
    source.y = position.1;
    let connection = placement::snap(ship, catalog, &mut source, Some(id));
    if placement::collides(ship, catalog, &source, Some(id)) {
        return None;
    }
    let to = (source.x, source.y);
    let mut commands = vec![
        EditorCommand::Disconnect(id.id).at(id),
        EditorCommand::Move {
            id: id.id,
            from: origin,
            to,
        }
        .at(id),
    ];
    if let Some(connection) = connection {
        commands.push(connection);
    }
    Some(EditorCommand::Batch(commands))
}

fn update_hud(
    document: Res<EditorDocument>,
    cursor: Res<EditorCursor>,
    mut labels: Query<&mut Text, With<EditorHud>>,
) {
    if !document.is_changed() && !cursor.is_changed() {
        return;
    }
    let chosen = document
        .catalog
        .visible()
        .nth(cursor.catalog_index)
        .map(|ty| ty.name.as_str())
        .unwrap_or("无可用部件");
    for mut text in &mut labels {
        **text = format!(
            "DR Editor | 部件: {} | 质量: {:.2} | {}\nTab: 切换部件（{}） P: 放置 | 拖动: 移动并吸附 | Esc/右键: 取消\nDelete: 删除 R: 旋转 X/Y: 镜像 | Ctrl+Z/Y: 撤销/重做 Ctrl+S: 保存 Ctrl+Shift+S: 另存为\nCtrl+N: 新建 Ctrl+O: 打开（也可拖入 XML）\n滚轮: 缩放 中键: 平移 Home: 复位 F12: 截图\n{}",
            document.ship.all_parts().count(),
            document.ship.total_mass(&document.catalog),
            if document.dirty {
                "未保存"
            } else {
                "已保存"
            },
            chosen,
            document.status
        );
    }
}

#[cfg(test)]
mod tests;
