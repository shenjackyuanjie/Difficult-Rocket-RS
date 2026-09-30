use bevy::prelude::*;
use bevy::window::{PresentMode, WindowResolution};
use dr_core::{EditorCommand, EditorHistory, PartCatalog, Ship, load_catalog, load_ship};

#[derive(Resource, Clone)]
pub struct EditorPaths {
    pub ship: Option<String>,
    pub catalog: String,
}

#[derive(Resource)]
pub struct EditorDocument {
    pub ship: Ship,
    pub catalog: PartCatalog,
    pub selected: Option<i64>,
    pub dirty: bool,
    pub history: EditorHistory,
}

impl EditorDocument {
    fn undo(&mut self) -> bool {
        self.history.undo(&mut self.ship)
    }
    fn redo(&mut self) -> bool {
        self.history.redo(&mut self.ship)
    }
    fn execute(&mut self, command: EditorCommand) -> bool {
        self.history.execute(&mut self.ship, command).is_ok()
    }
}

#[derive(Resource, Default)]
struct DragState {
    id: Option<i64>,
    origin: (f64, f64),
}

#[derive(Component)]
struct ShipPartVisual {
    id: i64,
}
#[derive(Component)]
struct EditorHud;

fn main() {
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
    let document = load_document(ship.as_deref(), &catalog_path);
    App::new()
        .insert_resource(EditorPaths {
            ship,
            catalog: catalog_path,
        })
        .insert_resource(document)
        .init_resource::<DragState>()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Difficult Rocket Editor".into(),
                resolution: WindowResolution::new(1440, 900),
                present_mode: PresentMode::AutoVsync,
                ..default()
            }),
            ..default()
        }))
        .add_systems(Startup, (setup_camera, setup_hud))
        .add_systems(
            Update,
            (
                keyboard_commands,
                mouse_editor,
                sync_ship_visuals,
                update_hud,
            ),
        )
        .run();
}

fn load_document(ship_path: Option<&str>, catalog_path: &str) -> EditorDocument {
    let catalog = load_catalog(catalog_path).unwrap_or_else(|error| {
        eprintln!("部件表加载失败: {error}");
        PartCatalog::default()
    });
    let ship = ship_path
        .and_then(|path| load_ship(path).ok())
        .unwrap_or_default();
    EditorDocument {
        ship,
        catalog,
        selected: None,
        dirty: false,
        history: EditorHistory::with_limit(256),
    }
}

fn setup_camera(mut commands: Commands) {
    commands.spawn((Camera2d, Name::new("Editor camera")));
}

fn setup_hud(mut commands: Commands) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(16),
                top: px(14),
                padding: UiRect::all(px(8)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.03, 0.04, 0.06, 0.86)),
            EditorHud,
            Name::new("Editor HUD"),
        ))
        .with_children(|root| {
            root.spawn((
                Text::new("DR Editor | O:打开示例  Delete:删除  R:旋转  X/Y:镜像  Ctrl+S:保存"),
                TextFont {
                    font_size: FontSize::Px(16.0),
                    ..default()
                },
                TextColor(Color::srgb(0.9, 0.93, 0.98)),
            ));
        });
}

fn keyboard_commands(keys: Res<ButtonInput<KeyCode>>, mut document: ResMut<EditorDocument>) {
    let control = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    if control && keys.just_pressed(KeyCode::KeyZ) {
        let changed = document.undo();
        document.dirty = changed;
        return;
    }
    if control && keys.just_pressed(KeyCode::KeyY) {
        let changed = document.redo();
        document.dirty = changed;
        return;
    }
    if keys.just_pressed(KeyCode::Delete) {
        if let Some(id) = document.selected.take() {
            document.ship.remove_part(id);
            document.dirty = true;
        }
    }
    if let Some(id) = document.selected {
        if keys.just_pressed(KeyCode::KeyR) {
            if let Some(part) = document.ship.part_mut(id) {
                part.editor_angle = (part.editor_angle + 1).rem_euclid(4);
                part.angle = part.editor_angle as f64 * std::f64::consts::FRAC_PI_2;
                document.dirty = true;
            }
        }
        if keys.just_pressed(KeyCode::KeyX) {
            if let Some(part) = document.ship.part_mut(id) {
                part.flip_x = !part.flip_x;
                document.dirty = true;
            }
        }
        if keys.just_pressed(KeyCode::KeyY) {
            if let Some(part) = document.ship.part_mut(id) {
                part.flip_y = !part.flip_y;
                document.dirty = true;
            }
        }
    }
}

fn mouse_editor(
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    cameras: Query<(&Camera, &GlobalTransform)>,
    mut drag: ResMut<DragState>,
    mut document: ResMut<EditorDocument>,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    let Some(cursor) = window.cursor_position() else {
        return;
    };
    let Ok((camera, transform)) = cameras.single() else {
        return;
    };
    let Ok(world) = camera.viewport_to_world_2d(transform, cursor) else {
        return;
    };
    let logical = (world.x as f64 / 60.0, world.y as f64 / 60.0);
    if mouse.just_pressed(MouseButton::Left) {
        document.selected = document
            .ship
            .parts
            .iter()
            .chain(
                document
                    .ship
                    .disconnected
                    .iter()
                    .flat_map(|g| g.parts.iter()),
            )
            .rev()
            .find(|part| {
                let ty = document.catalog.get(&part.part_type);
                ty.map(|t| {
                    let (w, h) = t.half_extents();
                    (logical.0 - part.x).abs() <= w && (logical.1 - part.y).abs() <= h
                })
                .unwrap_or(false)
            })
            .map(|part| part.id);
        if let Some(id) = document.selected {
            if let Some(part) = document.ship.part(id) {
                drag.id = Some(id);
                drag.origin = (part.x, part.y);
            }
        }
    }
    if mouse.pressed(MouseButton::Left) {
        if let Some(id) = drag.id {
            if let Some(part) = document.ship.part_mut(id) {
                part.x = (logical.0 * 2.0).round() / 2.0;
                part.y = (logical.1 * 2.0).round() / 2.0;
                document.dirty = true;
            }
        }
    }
    if mouse.just_released(MouseButton::Left) {
        if let Some(id) = drag.id.take() {
            if let Some(part) = document.ship.part(id) {
                let to = (part.x, part.y);
                let from = drag.origin;
                part_mut_set(&mut document.ship, id, from);
                let _ = document.execute(EditorCommand::Move { id, from, to });
                document.dirty = true;
            }
        }
    }
}

fn part_mut_set(ship: &mut Ship, id: i64, position: (f64, f64)) {
    if let Some(part) = ship.part_mut(id) {
        part.x = position.0;
        part.y = position.1;
    }
}

fn sync_ship_visuals(
    mut commands: Commands,
    document: Res<EditorDocument>,
    existing: Query<Entity, With<ShipPartVisual>>,
) {
    if !document.is_changed() {
        return;
    }
    for entity in &existing {
        commands.entity(entity).despawn();
    }
    for part in document.ship.parts.iter().chain(
        document
            .ship
            .disconnected
            .iter()
            .flat_map(|group| group.parts.iter()),
    ) {
        let (width, height) = document
            .catalog
            .get(&part.part_type)
            .map(|ty| (ty.width as f32 * 30.0, ty.height as f32 * 30.0))
            .unwrap_or((30.0, 30.0));
        let color = if Some(part.id) == document.selected {
            Color::srgb(0.95, 0.72, 0.18)
        } else {
            Color::srgb(0.32, 0.68, 0.88)
        };
        commands.spawn((
            Sprite::from_color(color, Vec2::new(width, height)),
            Transform {
                translation: Vec3::new(part.x as f32 * 60.0, part.y as f32 * 60.0, 0.0),
                rotation: Quat::from_rotation_z(part.angle as f32),
                ..default()
            },
            ShipPartVisual { id: part.id },
            Name::new(format!("Part {}", part.id)),
        ));
    }
}

fn update_hud(document: Res<EditorDocument>, mut labels: Query<&mut Text, With<EditorHud>>) {
    if !document.is_changed() {
        return;
    }
    for mut text in &mut labels {
        **text = format!(
            "DR Editor | 部件: {} | 质量: {:.2} | {}",
            document.ship.parts.len(),
            document.ship.total_mass(&document.catalog),
            if document.dirty {
                "未保存"
            } else {
                "已保存"
            }
        );
    }
}
