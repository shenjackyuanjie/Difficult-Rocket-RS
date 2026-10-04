use super::*;
use bevy::asset::AssetId;
use bevy::sprite::Anchor;
use dr_core::PartType;
use std::collections::{HashMap, HashSet};

/// 原版贴图使用 PNG 原始像素，不以 PartList 的物理尺寸缩放。
pub(crate) fn image_size(
    texture: &str,
    assets: &AssetServer,
    images: &Assets<Image>,
) -> Option<Vec2> {
    let handle = assets.get_handle::<Image>(format!("textures/parts/{texture}"))?;
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
    let anchor = pixel_anchor(size, part.flip_x, part.flip_y).as_vec();
    let rotation = Mat2::from_angle(part.angle as f32);
    let position = Vec2::new(part.x as f32, part.y as f32) * 60.0;
    [Vec2::ZERO, Vec2::X, Vec2::ONE, Vec2::Y]
        .map(|corner| position + rotation * ((corner - Vec2::splat(0.5) - anchor) * size))
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
}

fn appearance(
    document: &EditorDocument,
    drag: &DragState,
    key: PartKey,
    part: &Part,
) -> (Color, Transform) {
    let collision = if drag.contains(key) {
        if drag.members.len() > 1 {
            drag.blocked
        } else {
            let mut preview = part.clone();
            preview.x = drag.preview.0;
            preview.y = drag.preview.1;
            placement::collides(&document.ship, &document.catalog, &preview, Some(key))
        }
    } else {
        false
    };
    let color = if collision {
        Color::srgb(1.0, 0.2, 0.2)
    } else if document.is_selected(key) {
        Color::srgb(0.95, 0.72, 0.18)
    } else {
        Color::WHITE
    };
    let (x, y) = if drag.contains(key) {
        let delta = drag.delta();
        (part.x + delta.0, part.y + delta.1)
    } else {
        (part.x, part.y)
    };
    (
        color.with_alpha(if key.group == 0 { 1.0 } else { 100.0 / 255.0 }),
        Transform {
            translation: Vec3::new(x as f32 * 60.0, y as f32 * 60.0, 0.0),
            rotation: Quat::from_rotation_z(part.angle as f32),
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
        let (color, mut target) =
            appearance(&document, &drag, PartKey::new(key.0, key.1, key.2), part);
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
                    assets.load(format!("textures/parts/{texture}"))
                };
                visual.texture = texture.to_owned();
            }
            let (custom_size, target_anchor) = sprite_geometry(kind, &sprite.image, &images, part);
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
            if sprite.flip_x != part.flip_x {
                sprite.flip_x = part.flip_x;
            }
            if sprite.flip_y != part.flip_y {
                sprite.flip_y = part.flip_y;
            }
            if *transform != target {
                *transform = target;
            }
            index.entities.insert(key, (entity, layer));
        } else {
            let mut sprite = if texture.is_empty() {
                Sprite::from_color(color, size)
            } else {
                Sprite::from_image(assets.load(format!("textures/parts/{texture}")))
            };
            let (custom_size, anchor) = sprite_geometry(kind, &sprite.image, &images, part);
            sprite.custom_size = custom_size;
            if !texture.is_empty() {
                index.texture_sizes.insert(
                    sprite.image.id(),
                    images.get(&sprite.image).map(Image::size),
                );
            }
            sprite.color = color;
            sprite.flip_x = part.flip_x;
            sprite.flip_y = part.flip_y;
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
pub(crate) struct ConnectionLines(Vec<(usize, Connection, Vec2, Vec2)>);

pub(crate) fn connections(
    mut gizmos: Gizmos,
    document: Res<EditorDocument>,
    drag: Res<DragState>,
    mut lines: Local<ConnectionLines>,
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
                // 两端完全接触时线段没有长度，无需向 GPU 重复提交。
                if a.distance(b) < 1e-6 {
                    continue;
                }
                lines.0.push((
                    group,
                    connection.clone(),
                    Vec2::new(a.x as f32 * 60.0, a.y as f32 * 60.0),
                    Vec2::new(b.x as f32 * 60.0, b.y as f32 * 60.0),
                ));
            }
        }
    }
    for (group, connection, a, b) in &lines.0 {
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
        if count == 0 {
            gizmos.line_2d(*a, *b, Color::srgb(0.25, 0.9, 0.55));
        } else if count == refs.len() {
            let delta = drag.delta();
            let offset = Vec2::new(delta.0 as f32 * 60.0, delta.1 as f32 * 60.0);
            gizmos.line_2d(*a + offset, *b + offset, Color::srgb(0.25, 0.9, 0.55));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Image>()
            .insert_resource(crate::tests::document())
            .init_resource::<DragState>()
            .add_systems(Update, sync);
        app
    }

    fn visual(app: &mut App, id: i64) -> (Entity, Transform, Color) {
        app.world_mut()
            .query::<(Entity, &PartVisual, &Transform, &Sprite)>()
            .iter(app.world())
            .find(|(_, visual, _, _)| visual.id == id)
            .map(|(entity, _, transform, sprite)| (entity, *transform, sprite.color))
            .unwrap()
    }

    #[test]
    fn natural_image_size_and_odd_pixel_anchor_are_independent_of_physics() {
        let mut kind = panels::tests::catalog().get("pod").unwrap().clone();
        kind.sprite = "DockingConnector.png".into();
        kind.width = 4;
        kind.height = 1;
        let part = kind.instantiate(1, (0.0, 0.0));
        let mut images = Assets::<Image>::default();
        let mut image = Image::default();
        image.texture_descriptor.size.width = 61;
        image.texture_descriptor.size.height = 29;
        let image = images.add(image);
        let (size, anchor) = sprite_geometry(Some(&kind), &image, &images, &part);
        assert_eq!(size, None);
        assert!(
            anchor
                .as_vec()
                .abs_diff_eq(Vec2::new(-0.5 / 61.0, -0.5 / 29.0), 1e-7)
        );
        assert_eq!(kind.half_extents(), (1.0, 0.25));
        assert_eq!(
            sprite_geometry(None, &image, &images, &part),
            (Some(Vec2::splat(30.0)), Anchor::CENTER)
        );
        images.remove(image.id());
        assert_eq!(
            sprite_geometry(Some(&kind), &image, &images, &part),
            (None, Anchor::CENTER)
        );
    }

    #[test]
    fn mirrored_odd_pixels_rotate_about_original_floor_anchor() {
        let mut part = crate::tests::document().ship.parts.remove(0);
        part.x = 3.0;
        part.y = -2.0;
        for size in [
            Vec2::new(61.0, 29.0),
            Vec2::new(84.0, 207.0),
            Vec2::new(60.0, 30.0),
        ] {
            for flip_x in [false, true] {
                for flip_y in [false, true] {
                    for angle in [0.0, 0.37, std::f64::consts::FRAC_PI_2, -2.1] {
                        part.flip_x = flip_x;
                        part.flip_y = flip_y;
                        part.angle = angle;
                        let corners = image_corners(&part, size);
                        for uv in [Vec2::ZERO, Vec2::X, Vec2::ONE, Vec2::Y] {
                            let local = (uv * size - (size * 0.5).floor())
                                * Vec2::new(
                                    if flip_x { -1.0 } else { 1.0 },
                                    if flip_y { -1.0 } else { 1.0 },
                                );
                            let (sin, cos) = angle.sin_cos();
                            let expected = Vec2::new(
                                (local.x as f64 * cos - local.y as f64 * sin) as f32 + 180.0,
                                (local.x as f64 * sin + local.y as f64 * cos) as f32 - 120.0,
                            );
                            // 世界坐标约 291 时一个 f32 ULP 为 0.000030517578125，
                            // 原固定阈值 0.00003 小于可表示的最小步进。每轴最多补足
                            // 一个 ULP，不扩大为统一宽松阈值；半像素锚点符号错误仍会
                            // 产生约 1 像素差异。参考值继续独立使用 floor 锚点及 f64 旋转。
                            let one_ulp = |value: f32| {
                                let value = value.abs();
                                value.next_up() - value
                            };
                            let tolerance = Vec2::new(
                                3e-5_f32.max(one_ulp(expected.x)),
                                3e-5_f32.max(one_ulp(expected.y)),
                            );
                            assert!(
                                corners.iter().any(|corner| {
                                    (*corner - expected).abs().cmple(tolerance).all()
                                }),
                                "size={size:?}, angle={angle}, flip=({flip_x},{flip_y}), uv={uv:?}, corners={corners:?}, expected={expected:?}, tolerance={tolerance:?}"
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn disconnected_alpha_survives_selection_and_collision_tints() {
        let mut document = crate::tests::document();
        let part = document.ship.parts[0].clone();
        let key = PartKey::new(1, 1, 0);
        document.ship.disconnected.push(dr_core::ShipGroup {
            parts: vec![part.clone()],
            connections: vec![],
        });
        let mut drag = DragState::default();
        let ordinary = appearance(&document, &drag, key, &part).0;
        assert_eq!(ordinary, Color::WHITE.with_alpha(100.0 / 255.0));
        document.select_only(Some(key));
        let selected = appearance(&document, &drag, key, &part).0;
        assert_eq!(selected.alpha(), ordinary.alpha());
        assert_ne!(selected, ordinary);
        drag.id = Some(key);
        drag.members = [key, PartKey::new(0, 1, 0)].into_iter().collect();
        drag.blocked = true;
        let blocked = appearance(&document, &drag, key, &part).0;
        assert_eq!(blocked, Color::srgba(1.0, 0.2, 0.2, 100.0 / 255.0));
    }

    #[test]
    fn selection_drag_cancel_and_document_changes_reuse_visual_entities() {
        let mut app = app();
        app.update();
        let initial = visual(&mut app, 1);
        app.world_mut().resource_mut::<EditorDocument>().selected = Some(PartKey::new(0, 1, 0));
        app.update();
        assert_eq!(visual(&mut app, 1).0, initial.0);
        assert_ne!(visual(&mut app, 1).2, initial.2);
        {
            let mut drag = app.world_mut().resource_mut::<DragState>();
            drag.id = Some(PartKey::new(0, 1, 0));
            drag.preview = (5.0, 3.0);
        }
        app.update();
        let moved = visual(&mut app, 1);
        assert_eq!(moved.0, initial.0);
        assert_eq!(moved.1.translation, Vec3::new(300.0, 180.0, 0.0));
        assert_eq!(
            app.world()
                .resource::<EditorDocument>()
                .ship
                .part(1)
                .unwrap()
                .x,
            0.0
        );
        app.world_mut().resource_mut::<DragState>().id = None;
        app.update();
        assert_eq!(visual(&mut app, 1).1, initial.1);
        app.world_mut()
            .resource_mut::<EditorDocument>()
            .execute(EditorCommand::FlipX(1));
        app.update();
        assert_eq!(visual(&mut app, 1).0, initial.0);
        assert!(app.world().get::<Sprite>(initial.0).unwrap().flip_x);
    }

    #[test]
    fn deletion_only_removes_missing_ids_and_undo_restores_them() {
        let mut app = app();
        let mut part = app.world().resource::<EditorDocument>().ship.parts[0].clone();
        part.id = 2;
        part.x = 2.0;
        app.world_mut()
            .resource_mut::<EditorDocument>()
            .ship
            .parts
            .push(part);
        app.update();
        let keep = visual(&mut app, 1).0;
        let removed = visual(&mut app, 2).0;
        app.world_mut()
            .resource_mut::<EditorDocument>()
            .execute(EditorCommand::Delete(2));
        app.update();
        assert_eq!(visual(&mut app, 1).0, keep);
        assert!(app.world().get_entity(removed).is_err());
        app.world_mut().resource_mut::<EditorDocument>().undo();
        app.update();
        assert_eq!(visual(&mut app, 1).0, keep);
        assert_ne!(visual(&mut app, 2).0, removed);
        assert_eq!(visual(&mut app, 2).1.translation.x, 120.0);
    }

    #[test]
    fn reordered_document_keeps_entities_but_updates_draw_order() {
        let mut app = app();
        let mut part = app.world().resource::<EditorDocument>().ship.parts[0].clone();
        part.id = 2;
        app.world_mut()
            .resource_mut::<EditorDocument>()
            .ship
            .parts
            .push(part);
        app.update();
        let first = visual(&mut app, 1);
        let second = visual(&mut app, 2);
        assert!(first.1.translation.z < second.1.translation.z);
        app.world_mut()
            .resource_mut::<EditorDocument>()
            .ship
            .parts
            .reverse();
        app.update();
        let a = visual(&mut app, 1);
        let b = visual(&mut app, 2);
        assert_eq!(a.0, first.0);
        assert_eq!(b.0, second.0);
        assert!(a.1.translation.z > b.1.translation.z);
    }

    #[test]
    fn disconnected_groups_reusing_ids_keep_separate_visuals() {
        let mut app = app();
        let mut part = app.world().resource::<EditorDocument>().ship.parts[0].clone();
        part.x = 5.0;
        app.world_mut()
            .resource_mut::<EditorDocument>()
            .ship
            .disconnected
            .push(dr_core::ShipGroup {
                parts: vec![part],
                connections: vec![],
            });
        app.update();
        let snapshots = |app: &mut App| {
            app.world_mut()
                .query::<(Entity, &PartVisual, &Transform)>()
                .iter(app.world())
                .map(|(entity, visual, transform)| {
                    ((visual.group, visual.id), (entity, transform.translation))
                })
                .collect::<HashMap<_, _>>()
        };
        let before = snapshots(&mut app);
        assert_eq!(before.len(), 2);
        assert_eq!(before[&(0, 1)].1.x, 0.0);
        assert_eq!(before[&(1, 1)].1.x, 300.0);
        app.world_mut().resource_mut::<EditorDocument>().status = "刷新状态".into();
        app.update();
        assert_eq!(snapshots(&mut app), before);
    }

    #[test]
    fn repeated_ids_inside_one_group_are_rendered_without_leaking_entities() {
        let mut app = app();
        let mut part = app.world().resource::<EditorDocument>().ship.parts[0].clone();
        part.x = 5.0;
        app.world_mut()
            .resource_mut::<EditorDocument>()
            .ship
            .parts
            .push(part);
        app.update();
        assert_eq!(
            app.world_mut()
                .query::<&PartVisual>()
                .iter(app.world())
                .count(),
            2
        );
        app.world_mut().resource_mut::<EditorDocument>().status = "刷新".into();
        app.update();
        assert_eq!(
            app.world_mut()
                .query::<&PartVisual>()
                .iter(app.world())
                .count(),
            2
        );
        let positions: Vec<_> = app
            .world_mut()
            .query::<(&PartVisual, &Transform)>()
            .iter(app.world())
            .map(|(visual, transform)| (visual.occurrence, transform.translation.x))
            .collect();
        assert!(positions.contains(&(0, 0.0)));
        assert!(positions.contains(&(1, 300.0)));
    }

    #[test]
    fn selection_and_drag_only_affect_the_exact_duplicate_instance() {
        let mut app = app();
        let part = app.world().resource::<EditorDocument>().ship.parts[0].clone();
        {
            let mut document = app.world_mut().resource_mut::<EditorDocument>();
            document.ship.parts.push(part.clone());
            document.ship.disconnected.push(dr_core::ShipGroup {
                parts: vec![part],
                connections: vec![],
            });
            document.selected = Some(PartKey::new(0, 1, 1));
        }
        app.update();
        {
            let mut drag = app.world_mut().resource_mut::<DragState>();
            drag.id = Some(PartKey::new(0, 1, 1));
            drag.preview = (5.0, 3.0);
        }
        app.update();
        for (visual, transform, sprite) in app
            .world_mut()
            .query::<(&PartVisual, &Transform, &Sprite)>()
            .iter(app.world())
        {
            if visual.group == 0 && visual.occurrence == 1 {
                assert_eq!(transform.translation.x, 300.0);
                assert_ne!(sprite.color, Color::WHITE);
            } else {
                assert_eq!(transform.translation.x, 0.0);
                assert_eq!(
                    sprite.color,
                    Color::WHITE.with_alpha(if visual.group == 0 {
                        1.0
                    } else {
                        100.0 / 255.0
                    })
                );
            }
        }
        app.world_mut().resource_mut::<DragState>().id = None;
        app.update();
        assert!(
            app.world_mut()
                .query::<&Transform>()
                .iter(app.world())
                .all(|transform| transform.translation.x == 0.0)
        );
    }
}
