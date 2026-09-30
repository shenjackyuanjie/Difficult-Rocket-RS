use super::*;
use std::collections::{HashMap, HashSet};

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
    dragged: Option<PartKey>,
}

fn appearance(
    document: &EditorDocument,
    drag: &DragState,
    key: PartKey,
    part: &Part,
) -> (Color, Transform) {
    let collision = if drag.id == Some(key) {
        let mut preview = part.clone();
        preview.x = drag.preview.0;
        preview.y = drag.preview.1;
        placement::collides(&document.ship, &document.catalog, &preview, Some(key))
    } else {
        false
    };
    let color = if collision {
        Color::srgb(1.0, 0.2, 0.2)
    } else if Some(key) == document.selected {
        Color::srgb(0.95, 0.72, 0.18)
    } else {
        Color::WHITE
    };
    let (x, y) = if drag.id == Some(key) {
        drag.preview
    } else {
        (part.x, part.y)
    };
    (
        color,
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
    mut visuals: Query<(&mut PartVisual, &mut Sprite, &mut Transform)>,
    mut index: Local<VisualIndex>,
) {
    if !document.is_changed() && !drag.is_changed() {
        return;
    }
    let parts: Vec<_> = if document.is_changed() {
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
        let mut ids: Vec<_> = [index.dragged, drag.id].into_iter().flatten().collect();
        ids.sort_unstable();
        ids.dedup();
        if ids.is_empty() {
            vec![]
        } else {
            index
                .entities
                .keys()
                .filter(|key| ids.contains(&PartKey::new(key.0, key.1, key.2)))
                .filter_map(|&key| {
                    let parts = if key.0 == 0 {
                        &document.ship.parts
                    } else {
                        &document.ship.disconnected.get(key.0 - 1)?.parts
                    };
                    parts
                        .iter()
                        .filter(|part| part.id == key.1)
                        .nth(key.2)
                        .map(|part| (key, part))
                })
                .collect()
        }
    };
    index.dragged = drag.id;
    let count = parts.len().max(1) as f32;
    for (order, (key, part)) in parts.into_iter().enumerate() {
        let kind = document.catalog.get(&part.part_type);
        let texture = kind.map(|kind| kind.sprite.as_str()).unwrap_or("");
        let size = kind
            .map(|kind| Vec2::new(kind.width as f32 * 30.0, kind.height as f32 * 30.0))
            .unwrap_or(Vec2::splat(30.0));
        let (color, mut target) =
            appearance(&document, &drag, PartKey::new(key.0, key.1, key.2), part);
        let layer = if document.is_changed() {
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
            && let Ok((mut visual, mut sprite, mut transform)) = visuals.get_mut(entity)
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
            if sprite.custom_size != Some(size) {
                sprite.custom_size = Some(size);
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
            sprite.custom_size = Some(size);
            sprite.color = color;
            sprite.flip_x = part.flip_x;
            sprite.flip_y = part.flip_y;
            let entity = commands
                .spawn((
                    sprite,
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
        if !drag
            .id
            .is_some_and(|id| id.group == *group && connection.touches(id.id))
        {
            gizmos.line_2d(*a, *b, Color::srgb(0.25, 0.9, 0.55));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
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
                assert_eq!(sprite.color, Color::WHITE);
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
