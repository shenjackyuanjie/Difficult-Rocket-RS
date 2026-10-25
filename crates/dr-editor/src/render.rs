use super::*;
use bevy::asset::AssetId;
use bevy::sprite::Anchor;
use dr_core::PartType;
use std::collections::{HashMap, HashSet};

/// 未连接部件与从目录拖出的虚影使用同一透明度。
pub const UNLINKED_ALPHA: f32 = 100.0 / 255.0;

#[derive(Component)]
pub(crate) struct EditorHud;

pub(crate) fn setup_hud(mut commands: Commands, assets: Res<AssetServer>) {
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
            panels::ToolbarItem::Status,
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

pub(crate) fn update_hud(
    document: Res<EditorDocument>,
    mut labels: Query<&mut Text, With<EditorHud>>,
) {
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

/// 原版数据存在大写扩展名（PartList.xml 的 dock-1 是 DockingConnector.PNG，磁盘上是 .png）。
/// Bevy 只按小写登记 AssetLoader，原样路径会走兜底并打出多 loader 告警；这里只规范化扩展名，
/// 保持目录与文件名主干原样，避免改动只读样本或目录 XML。
pub(crate) fn part_sprite_path(sprite: &str) -> String {
    match sprite.rsplit_once('.') {
        Some((stem, extension)) => {
            format!("textures/parts/{stem}.{}", extension.to_ascii_lowercase())
        }
        None => format!("textures/parts/{sprite}"),
    }
}

/// 原版贴图使用 PNG 原始像素，不以 PartList 的物理尺寸缩放。
pub(crate) fn image_size(
    texture: &str,
    assets: &AssetServer,
    images: &Assets<Image>,
) -> Option<Vec2> {
    let handle = assets.get_handle::<Image>(part_sprite_path(texture))?;
    images.get(&handle).map(|image| image.size().as_vec2())
}

pub(crate) fn fallback_size(kind: Option<&PartType>) -> Vec2 {
    kind.map(|kind| Vec2::new(kind.width as f32 * 30.0, kind.height as f32 * 30.0))
        .unwrap_or(Vec2::splat(30.0))
}

/// pyglet 的锚点为 floor(width/2)、floor(height/2)。Bevy 的 flip 只翻 UV，
/// 因此奇数像素的半像素锚点偏移也要随镜像翻转，随后再按实例角度旋转。
pub(crate) fn pixel_anchor(size: Vec2, flip_x: bool, flip_y: bool) -> Anchor {
    let center = size * 0.5;
    let offset = (center.floor() - center) / size.max(Vec2::ONE);
    Anchor(
        offset
            * Vec2::new(
                if flip_x { -1.0 } else { 1.0 },
                if flip_y { -1.0 } else { 1.0 },
            ),
    )
}

pub(crate) fn sprite_geometry(
    kind: Option<&PartType>,
    image: &Handle<Image>,
    images: &Assets<Image>,
    part: &Part,
) -> (Option<Vec2>, Anchor) {
    if kind.is_some_and(|kind| !kind.sprite.is_empty()) {
        let anchor = images.get(image).map_or(Anchor::CENTER, |image| {
            pixel_anchor(image.size().as_vec2(), part.flip_x, part.flip_y)
        });
        (None, anchor)
    } else {
        (Some(fallback_size(kind)), Anchor::CENTER)
    }
}

/// 仅供视图适配的可见四角；碰撞和命中仍独立使用 dr_core 的 Shape。
pub(crate) fn image_corners(part: &Part, size: Vec2) -> [Vec2; 4] {
    dr_core::image_corners(part, (size.x as f64, size.y as f64))
        .map(|p| Vec2::new(p.x as f32, p.y as f32))
}

#[derive(Component)]
pub(crate) struct PartVisual {
    pub id: i64,
    pub group: usize,
    pub occurrence: usize,
    texture: String,
}

pub(crate) type VisualKey = (usize, i64, usize);

/// 少量历史样本同组也有重复 ID；渲染保留每个实例，不修正原始 XML。
pub(crate) fn parts(ship: &Ship) -> impl Iterator<Item = (VisualKey, &Part)> {
    ship.keyed_parts()
        .map(|(key, part)| ((key.group, key.id, key.occurrence), part))
}

#[derive(Default)]
pub(crate) struct VisualIndex {
    entities: HashMap<VisualKey, (Entity, f32)>,
    dragged: BTreeSet<PartKey>,
    texture_sizes: HashMap<AssetId<Image>, Option<UVec2>>,
    linked: HashSet<PartKey>,
}

/// 按真实拓扑从主驾驶舱遍历；同组重复编号的引用不猜测归属。
fn linked_parts(ship: &Ship) -> HashSet<PartKey> {
    let main: Vec<_> = ship
        .keyed_parts()
        .filter(|(key, _)| key.group == 0)
        .collect();
    let Some((root, _)) = main
        .iter()
        .find(|(_, part)| part.pod.is_some())
        .or(main.first())
    else {
        return HashSet::new();
    };
    let mut unique = HashMap::<i64, Option<PartKey>>::new();
    for (key, _) in &main {
        unique
            .entry(key.id)
            .and_modify(|key| *key = None)
            .or_insert(Some(*key));
    }
    let mut edges = HashMap::<i64, Vec<i64>>::new();
    for connection in &ship.connections {
        let ids = match *connection {
            Connection::Normal { parent, child, .. } => vec![parent, child],
            Connection::Dock {
                parent,
                child,
                dock,
            } => vec![parent, child, dock],
        };
        if ids
            .iter()
            .any(|id| !unique.get(id).is_some_and(Option::is_some))
        {
            continue;
        }
        for pair in ids.windows(2) {
            edges.entry(pair[0]).or_default().push(pair[1]);
            edges.entry(pair[1]).or_default().push(pair[0]);
        }
    }
    let mut linked = HashSet::from([*root]);
    let mut stack = vec![root.id];
    while let Some(id) = stack.pop() {
        for neighbour in edges.get(&id).into_iter().flatten() {
            if let Some(Some(key)) = unique.get(neighbour)
                && linked.insert(*key)
            {
                stack.push(*neighbour);
            }
        }
    }
    linked
}

fn appearance(
    document: &EditorDocument,
    drag: &DragState,
    key: PartKey,
    part: &Part,
    linked: bool,
) -> (Color, Transform) {
    let preview = drag.pose(key, part);
    let collision = drag.contains(key) && drag.blocked;
    let color = if collision {
        Color::srgb(1.0, 0.2, 0.2)
    } else if document.is_selected(key) {
        Color::srgb(0.95, 0.72, 0.18)
    } else {
        Color::WHITE
    };
    let (x, y) = (preview.x, preview.y);
    (
        color.with_alpha(if linked { 1.0 } else { UNLINKED_ALPHA }),
        Transform {
            translation: Vec3::new(x as f32 * 60.0, y as f32 * 60.0, 0.0),
            rotation: Quat::from_rotation_z(preview.angle as f32),
            ..default()
        },
    )
}

/// 文档变化时对齐 ID；拖动帧只更新拖动部件，复用其余实体及 GPU 数据。
pub(crate) fn sync(
    mut commands: Commands,
    document: Res<EditorDocument>,
    drag: Res<DragState>,
    assets: Res<AssetServer>,
    images: Res<Assets<Image>>,
    mut visuals: Query<(&mut PartVisual, &mut Sprite, &mut Transform, &mut Anchor)>,
    mut index: Local<VisualIndex>,
) {
    // 异步解码完成后补齐半像素锚点；字体图集变化不应重扫整艘船。
    let textures_changed = images.is_changed()
        && index
            .texture_sizes
            .iter()
            .any(|(id, size)| images.get(*id).map(Image::size) != *size);
    let refresh_all = document.is_changed() || textures_changed;
    if !refresh_all && !drag.is_changed() {
        return;
    }
    if document.is_changed() {
        index.linked = linked_parts(&document.ship);
    }
    let parts: Vec<_> = if refresh_all {
        let parts: Vec<_> = parts(&document.ship).collect();
        let ids: HashSet<_> = parts.iter().map(|(key, _)| *key).collect();
        index.entities.retain(|id, (entity, _)| {
            if ids.contains(id) {
                true
            } else {
                commands.entity(*entity).despawn();
                false
            }
        });
        parts
    } else {
        let ids: BTreeSet<_> = index.dragged.iter().copied().chain(drag.keys()).collect();
        if ids.is_empty() {
            vec![]
        } else if ids.len() <= 2 {
            ids.into_iter()
                .filter_map(|key| {
                    document
                        .ship
                        .part_at(key)
                        .map(|part| ((key.group, key.id, key.occurrence), part))
                })
                .collect()
        } else {
            document
                .ship
                .keyed_parts()
                .filter(|(key, _)| ids.contains(key))
                .map(|(key, part)| ((key.group, key.id, key.occurrence), part))
                .collect()
        }
    };
    index.dragged = drag.keys();
    if refresh_all {
        index.texture_sizes.clear();
    }
    let count = parts.len().max(1) as f32;
    for (order, (key, part)) in parts.into_iter().enumerate() {
        let kind = document.catalog.get(&part.part_type);
        let texture = kind.map(|kind| kind.sprite.as_str()).unwrap_or("");
        let size = fallback_size(kind);
        let part_key = PartKey::new(key.0, key.1, key.2);
        let preview = drag.contains(part_key).then(|| drag.pose(part_key, part));
        let sprite_part = preview.as_ref().unwrap_or(part);
        let (color, mut target) = appearance(
            &document,
            &drag,
            part_key,
            part,
            index.linked.contains(&part_key),
        );
        let layer = if refresh_all {
            order as f32 / count * 2.0
        } else {
            index
                .entities
                .get(&key)
                .map(|(_, layer)| *layer)
                .unwrap_or(0.0)
        };
        target.translation.z = layer;
        if let Some(&(entity, _)) = index.entities.get(&key)
            && let Ok((mut visual, mut sprite, mut transform, mut anchor)) = visuals.get_mut(entity)
        {
            debug_assert_eq!(visual.id, part.id);
            debug_assert_eq!(visual.group, key.0);
            if visual.texture != texture {
                sprite.image = if texture.is_empty() {
                    default()
                } else {
                    assets.load(part_sprite_path(texture))
                };
                visual.texture = texture.to_owned();
            }
            let (custom_size, target_anchor) =
                sprite_geometry(kind, &sprite.image, &images, sprite_part);
            if sprite.custom_size != custom_size {
                sprite.custom_size = custom_size;
            }
            if *anchor != target_anchor {
                *anchor = target_anchor;
            }
            if !texture.is_empty() {
                index.texture_sizes.insert(
                    sprite.image.id(),
                    images.get(&sprite.image).map(Image::size),
                );
            }
            if sprite.color != color {
                sprite.color = color;
            }
            if sprite.flip_x != sprite_part.flip_x {
                sprite.flip_x = sprite_part.flip_x;
            }
            if sprite.flip_y != sprite_part.flip_y {
                sprite.flip_y = sprite_part.flip_y;
            }
            if *transform != target {
                *transform = target;
            }
            index.entities.insert(key, (entity, layer));
        } else {
            let mut sprite = if texture.is_empty() {
                Sprite::from_color(color, size)
            } else {
                Sprite::from_image(assets.load(part_sprite_path(texture)))
            };
            let (custom_size, anchor) = sprite_geometry(kind, &sprite.image, &images, sprite_part);
            sprite.custom_size = custom_size;
            if !texture.is_empty() {
                index.texture_sizes.insert(
                    sprite.image.id(),
                    images.get(&sprite.image).map(Image::size),
                );
            }
            sprite.color = color;
            sprite.flip_x = sprite_part.flip_x;
            sprite.flip_y = sprite_part.flip_y;
            let entity = commands
                .spawn((
                    sprite,
                    anchor,
                    target,
                    PartVisual {
                        id: part.id,
                        group: key.0,
                        occurrence: key.2,
                        texture: texture.to_owned(),
                    },
                    Name::new(format!("Part {}", part.id)),
                ))
                .id();
            index.entities.insert(key, (entity, layer));
        }
    }
}

#[derive(Default)]
pub(crate) struct ConnectionLines(Vec<(usize, Connection, [Vec2; 4])>);

fn dragged_connection_path(
    document: &EditorDocument,
    drag: &DragState,
    group: usize,
    connection: &Connection,
) -> Option<[Vec2; 4]> {
    let (parent, child) = match *connection {
        Connection::Normal { parent, child, .. } | Connection::Dock { parent, child, .. } => {
            (parent, child)
        }
    };
    let parent_key = PartKey::new(group, parent, 0);
    let child_key = PartKey::new(group, child, 0);
    let parent = drag.pose(parent_key, document.ship.part_at(parent_key)?);
    let child = drag.pose(child_key, document.ship.part_at(child_key)?);
    let (a, b) =
        dr_core::connections::positions_between(&document.catalog, connection, &parent, &child)?;
    Some([
        Vec2::new(parent.x as f32, parent.y as f32) * 60.0,
        Vec2::new(a.x as f32, a.y as f32) * 60.0,
        Vec2::new(b.x as f32, b.y as f32) * 60.0,
        Vec2::new(child.x as f32, child.y as f32) * 60.0,
    ])
}

pub(crate) fn connections(
    mut gizmos: Gizmos<connection_lines::LineGizmos>,
    document: Res<EditorDocument>,
    drag: Res<DragState>,
    mut lines: Local<ConnectionLines>,
    settings: Res<connection_lines::Settings>,
    time: Res<Time>,
    cameras: Query<&Projection, With<Camera2d>>,
) {
    if document.is_changed() {
        lines.0.clear();
        for (group, group_parts, group_connections) in document.ship.groups() {
            let mut parts = HashMap::new();
            for part in group_parts {
                parts
                    .entry(part.id)
                    .and_modify(|value| *value = None)
                    .or_insert(Some(part));
            }
            for connection in group_connections {
                let (parent, child) = match *connection {
                    Connection::Normal { parent, child, .. }
                    | Connection::Dock { parent, child, .. } => (parent, child),
                };
                let (Some(Some(parent)), Some(Some(child))) =
                    (parts.get(&parent), parts.get(&child))
                else {
                    continue;
                };
                let Some((a, b)) = dr_core::connections::positions_between(
                    &document.catalog,
                    connection,
                    parent,
                    child,
                ) else {
                    continue;
                };
                // 显示中心→连接面→连接面→中心。完全贴合时仍能看见连接，
                // 而不是因接触点重合而把整条连接线省略。
                let points = [
                    Vec2::new(parent.x as f32, parent.y as f32) * 60.0,
                    Vec2::new(a.x as f32, a.y as f32) * 60.0,
                    Vec2::new(b.x as f32, b.y as f32) * 60.0,
                    Vec2::new(child.x as f32, child.y as f32) * 60.0,
                ];
                lines.0.push((group, connection.clone(), points));
            }
        }
    }
    if !settings.enabled {
        return;
    }
    let scale = match cameras.single().ok() {
        Some(Projection::Orthographic(projection)) => projection.scale,
        _ => 1.0,
    };
    for (group, connection, path) in &lines.0 {
        let refs = match *connection {
            Connection::Normal { parent, child, .. } => vec![parent, child],
            Connection::Dock {
                parent,
                child,
                dock,
            } => vec![parent, child, dock],
        };
        let count = refs
            .iter()
            .filter(|id| drag.contains(PartKey::new(*group, **id, 0)))
            .count();
        let points = if count == 0 {
            *path
        } else if count == refs.len() {
            path.map(|point| drag.point(point))
        } else if document.free_mode {
            let Some(points) = dragged_connection_path(&document, &drag, *group, connection) else {
                continue;
            };
            points
        } else {
            continue;
        };
        connection_lines::draw_path(&mut gizmos, &settings, &points, time.elapsed_secs(), scale);
    }
}

#[cfg(test)]
mod tests;
