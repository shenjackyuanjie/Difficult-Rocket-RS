use super::*;
use std::collections::HashSet;

#[cfg(test)]
mod tests;

pub(crate) fn click(document: &mut EditorDocument, hit: Option<PartKey>, shift: bool) {
    if shift {
        document.selection = document.selected_keys().into_iter().collect();
        if let Some(key) = hit {
            if !document.selection.remove(&key) {
                document.selection.insert(key);
                document.selected = Some(key);
            } else {
                document.selected = document.selection.last().copied();
            }
        }
    } else if let Some(key) = hit {
        if !document.is_selected(key) {
            document.select_only(Some(key));
        } else {
            document.selected = Some(key);
        }
    } else {
        document.clear_selection();
    }
}

pub(crate) fn rectangle(
    document: &mut EditorDocument,
    a: (f64, f64),
    b: (f64, f64),
    additive: bool,
) {
    let mut selected: BTreeSet<_> = if additive {
        document.selected_keys().into_iter().collect()
    } else {
        BTreeSet::new()
    };
    selected.extend(
        document
            .ship
            .keyed_parts()
            .filter(|(_, part)| {
                document.catalog.get(&part.part_type).is_some_and(|kind| {
                    dr_core::geometry::intersects_rect(
                        part,
                        kind,
                        Vec2d { x: a.0, y: a.1 },
                        Vec2d { x: b.0, y: b.1 },
                    )
                })
            })
            .map(|(key, _)| key),
    );
    document.selected = selected.last().copied();
    document.selection = selected;
}

pub(crate) fn center(document: &EditorDocument, keys: &[PartKey]) -> (f64, f64) {
    let mut low = (f64::INFINITY, f64::INFINITY);
    let mut high = (f64::NEG_INFINITY, f64::NEG_INFINITY);
    let keys: HashSet<_> = keys.iter().copied().collect();
    for (_, part) in document
        .ship
        .keyed_parts()
        .filter(|(key, _)| keys.contains(key))
    {
        low = (low.0.min(part.x), low.1.min(part.y));
        high = (high.0.max(part.x), high.1.max(part.y));
    }
    ((low.0 + high.0) / 2.0, (low.1 + high.1) / 2.0)
}

/// 返回实际连接候选，最终由同一套原子命令校验内部占用和整体碰撞。
fn snaps(
    ship: &Ship,
    catalog: &PartCatalog,
    keys: &[PartKey],
    delta: (f64, f64),
) -> Vec<(f64, (f64, f64), EditorCommand)> {
    snaps_pose(ship, catalog, keys, SelectionPose::default(), delta)
}

fn snaps_pose(
    ship: &Ship,
    catalog: &PartCatalog,
    keys: &[PartKey],
    pose: SelectionPose,
    delta: (f64, f64),
) -> Vec<(f64, (f64, f64), EditorCommand)> {
    let selected: HashSet<_> = keys.iter().copied().collect();
    let parts: Vec<_> = ship.keyed_parts().collect();
    let targets: Vec<_> = parts
        .iter()
        .copied()
        .filter(|(key, _)| !selected.contains(key))
        .filter_map(|(key, part)| {
            let kind = catalog.get(&part.part_type)?;
            Some((
                key,
                part,
                kind,
                dr_core::connections::attachment_radius(kind),
            ))
        })
        .collect();
    // 只扫描 X 范围内的目标，随后仍用原始圆半径和连接面精确判断。
    // 目标索引保留文档顺序，不能让空间排序改变等距离吸附的优先项。
    let mut by_x: Vec<_> = (0..targets.len()).collect();
    by_x.sort_by(|a, b| targets[*a].1.x.total_cmp(&targets[*b].1.x));
    let max_radius = targets
        .iter()
        .map(|(_, _, _, radius)| *radius)
        .fold(0.0, f64::max);
    let mut result = vec![];
    for &(source_key, source) in parts.iter().filter(|(key, _)| selected.contains(key)) {
        let Some(st) = catalog.get(&source.part_type) else {
            continue;
        };
        let Ok(source) = dr_core::edit::selection::pose_part(source, Some(catalog), pose, delta)
        else {
            continue;
        };
        let source_radius = dr_core::connections::attachment_radius(st);
        let distance = dr_core::connections::CONNECTION_DISTANCE;
        let reach = source_radius + max_radius + distance + 1e-6;
        let start = by_x.partition_point(|index| targets[*index].1.x < source.x - reach);
        let end = by_x.partition_point(|index| targets[*index].1.x <= source.x + reach);
        let mut nearby = by_x[start..end].to_vec();
        nearby.sort_unstable();
        for index in nearby {
            let (target_key, target, tt, target_radius) = targets[index];
            let distance_squared = (source.x - target.x).powi(2) + (source.y - target.y).powi(2);
            if distance_squared > (source_radius + target_radius + distance + 1e-6).powi(2) {
                continue;
            }
            for candidate in dr_core::connections::candidates(&source, st, target, tt, distance) {
                let kind = if candidate.dock {
                    LinkKind::Dock {
                        connector: if st.kind == PartKind::DockConnector {
                            source_key
                        } else {
                            target_key
                        },
                    }
                } else {
                    LinkKind::Normal {
                        parent_attach: candidate.target_index as i32 + 1,
                        child_attach: candidate.source_index as i32 + 1,
                    }
                };
                result.push((
                    candidate.distance,
                    (
                        delta.0 + candidate.position.x - source.x,
                        delta.1 + candidate.position.y - source.y,
                    ),
                    EditorCommand::ConnectParts {
                        parent: target_key,
                        child: source_key,
                        kind,
                    },
                ));
            }
        }
    }
    result.sort_by(|a, b| a.0.total_cmp(&b.0));
    result
}

/// 按组内唯一父子引用扩展后代；循环安全，不把对接连接器当作子节点。
pub(crate) fn drag_keys(ship: &Ship, roots: &[PartKey], follow: bool) -> BTreeSet<PartKey> {
    let mut result: BTreeSet<_> = roots.iter().copied().collect();
    if !follow {
        return result;
    }
    let mut pending = roots.to_vec();
    while let Some(parent_key) = pending.pop() {
        let Some((parts, connections)) = ship.group(parent_key.group) else {
            continue;
        };
        if parts.iter().filter(|part| part.id == parent_key.id).count() != 1 {
            continue;
        }
        for connection in connections {
            let (parent, child) = match *connection {
                Connection::Normal { parent, child, .. }
                | Connection::Dock { parent, child, .. } => (parent, child),
            };
            if parent != parent_key.id || parts.iter().filter(|part| part.id == child).count() != 1
            {
                continue;
            }
            let key = PartKey::new(parent_key.group, child, 0);
            if result.insert(key) {
                pending.push(key);
            }
        }
    }
    result
}

/// 删除按无向连接分量扩展；同组重号引用有歧义时拒绝，不猜归属。
pub(crate) fn connected_keys(ship: &Ship, roots: &[PartKey]) -> Result<BTreeSet<PartKey>, String> {
    let mut result: BTreeSet<_> = roots.iter().copied().collect();
    let mut pending = roots.to_vec();
    while let Some(key) = pending.pop() {
        let Some((parts, connections)) = ship.group(key.group) else {
            return Err("拖拽部件已失效".into());
        };
        for connection in connections.iter().filter(|edge| edge.touches(key.id)) {
            let ids = match *connection {
                Connection::Normal { parent, child, .. } => vec![parent, child],
                Connection::Dock {
                    parent,
                    child,
                    dock,
                } => vec![parent, child, dock],
            };
            for id in ids {
                if parts.iter().filter(|part| part.id == id).count() != 1 {
                    return Err("连接引用有重复编号或缺失部件，请先修复后删除".into());
                }
                let next = PartKey::new(key.group, id, 0);
                if result.insert(next) {
                    pending.push(next);
                }
            }
        }
    }
    Ok(result)
}

#[cfg(test)]
pub(crate) fn movement(
    document: &EditorDocument,
    keys: &[PartKey],
    delta: (f64, f64),
) -> ((f64, f64), EditorCommand, bool) {
    movement_pose(document, keys, (0.0, 0.0), 0, delta)
}

#[cfg(test)]
pub(crate) fn movement_pose(
    document: &EditorDocument,
    keys: &[PartKey],
    pivot: (f64, f64),
    turns: u8,
    delta: (f64, f64),
) -> ((f64, f64), EditorCommand, bool) {
    movement_transformed(
        document,
        keys,
        SelectionPose {
            pivot,
            radians: (turns % 4) as f64 * std::f64::consts::FRAC_PI_2,
            ..default()
        },
        delta,
    )
}

pub(crate) fn movement_transformed(
    document: &EditorDocument,
    keys: &[PartKey],
    pose: SelectionPose,
    delta: (f64, f64),
) -> ((f64, f64), EditorCommand, bool) {
    if document.free_mode {
        let command = drag_command(keys, pose, delta);
        let valid = document
            .edit_command(command.clone())
            .preview_with_catalog(&document.ship, &document.catalog)
            .is_ok();
        return (delta, command, valid);
    }
    movement_transformed_for(&document.ship, &document.catalog, keys, pose, delta)
}

#[cfg(test)]
pub(crate) fn movement_for(
    ship: &Ship,
    catalog: &PartCatalog,
    keys: &[PartKey],
    pivot: (f64, f64),
    turns: u8,
    delta: (f64, f64),
) -> ((f64, f64), EditorCommand, bool) {
    movement_transformed_for(
        ship,
        catalog,
        keys,
        SelectionPose {
            pivot,
            radians: (turns % 4) as f64 * std::f64::consts::FRAC_PI_2,
            ..default()
        },
        delta,
    )
}

pub(crate) fn drag_command(
    keys: &[PartKey],
    pose: SelectionPose,
    delta: (f64, f64),
) -> EditorCommand {
    let quarters = pose.radians / std::f64::consts::FRAC_PI_2;
    if !pose.flip_x && !pose.flip_y && (quarters - quarters.round()).abs() < 1e-10 {
        EditorCommand::DragSelection {
            parts: keys.to_vec(),
            pivot: pose.pivot,
            turns: quarters.round().rem_euclid(4.0) as u8,
            delta,
        }
    } else {
        EditorCommand::DragPoseSelection {
            parts: keys.to_vec(),
            pose,
            delta,
        }
    }
}

fn movement_transformed_for(
    ship: &Ship,
    catalog: &PartCatalog,
    keys: &[PartKey],
    pose: SelectionPose,
    delta: (f64, f64),
) -> ((f64, f64), EditorCommand, bool) {
    let command = |offset| drag_command(keys, pose, offset);
    let selected: HashSet<_> = keys.iter().copied().collect();
    let others = dr_core::geometry::CollisionSet::new(
        ship.keyed_parts()
            .filter(|(key, _)| !selected.contains(key))
            .map(|(_, part)| part),
        catalog,
    );
    let moving = ship
        .keyed_parts()
        .filter(|(key, _)| selected.contains(key))
        .filter_map(|(_, part)| catalog.get(&part.part_type).map(|kind| (part, kind)))
        .map(|(part, kind)| {
            dr_core::edit::selection::pose_part(part, Some(catalog), pose, (0.0, 0.0))
                .map(|part| (part, kind))
        })
        .collect::<Result<Vec<_>, _>>();
    let Ok(moving) = moving else {
        return (delta, command(delta), false);
    };
    let collides = |offset: (f64, f64)| {
        moving.iter().any(|(part, kind)| {
            let mut proposed = (*part).clone();
            proposed.x += offset.0;
            proposed.y += offset.1;
            others.editor_overlap_blocked(&proposed, kind)
        })
    };
    if delta != (0.0, 0.0)
        || pose.radians.rem_euclid(std::f64::consts::TAU).abs() > 1e-10
        || pose.flip_x
        || pose.flip_y
    {
        let mut invalid = HashSet::new();
        for (_, offset, connection) in snaps_pose(ship, catalog, keys, pose, delta) {
            let bits = (offset.0.to_bits(), offset.1.to_bits());
            if invalid.contains(&bits) {
                continue;
            }
            if collides(offset) {
                invalid.insert(bits);
                continue;
            }
            let movement = command(offset);
            if let Ok(moved) = movement.preview_with_catalog(ship, catalog)
                && connection.preview_with_catalog(&moved, catalog).is_ok()
            {
                return (
                    offset,
                    EditorCommand::Batch(vec![movement, connection]),
                    true,
                );
            }
        }
    }
    // 没有合法吸附时，即使重叠也保留落点，不新增连接。
    (delta, command(delta), !collides(delta))
}

pub(crate) fn keyboard(
    document: &mut EditorDocument,
    cursor: &mut EditorCursor,
    keys: &ButtonInput<KeyCode>,
    blocked: bool,
) -> bool {
    let control = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    if control && keys.just_pressed(KeyCode::KeyA) {
        document.selection = document.ship.keyed_parts().map(|(key, _)| key).collect();
        document.selected = document.selection.last().copied();
        cursor.paste = None;
        cursor.cancel_placement();
        return true;
    }
    if control && (keys.just_pressed(KeyCode::KeyC) || keys.just_pressed(KeyCode::KeyX)) {
        let selected = document.selected_keys();
        match ShipFragment::capture(&document.ship, &selected) {
            Ok(fragment) => {
                let cut = keys.just_pressed(KeyCode::KeyX);
                if !cut || document.execute(EditorCommand::DeleteSelection(selected.clone())) {
                    cursor.clipboard = Some(fragment);
                    cursor.paste = None;
                    document.status = format!(
                        "已{} {} 个部件",
                        if cut { "剪切" } else { "复制" },
                        selected.len()
                    );
                }
            }
            Err(error) => document.status = error.to_string(),
        }
        return true;
    }
    if control && keys.just_pressed(KeyCode::KeyV) {
        cursor.paste = cursor.clipboard.clone();
        cursor.cancel_placement();
        document.status = if cursor.paste.is_some() {
            "粘贴预览：移动鼠标定位，左键/P 放置，Esc 取消".into()
        } else {
            "剪贴板为空，请先选择并复制部件".into()
        };
        return true;
    }
    if control {
        return false;
    }
    if keys.just_pressed(KeyCode::Tab) {
        cursor.paste = None;
        return false;
    }
    if let Some(fragment) = &cursor.paste {
        let center = fragment.center();
        let transform = if let Some(radians) = rotation_input(keys) {
            Some(SelectionTransform::RotateBy { center, radians })
        } else if keys.just_pressed(KeyCode::KeyR) {
            Some(SelectionTransform::Rotate { center })
        } else if keys.just_pressed(KeyCode::KeyX) {
            Some(SelectionTransform::FlipX { center })
        } else if keys.just_pressed(KeyCode::KeyY) {
            Some(SelectionTransform::FlipY { center })
        } else {
            None
        };
        if let Some(transform) = transform {
            match fragment.transformed(&document.catalog, transform) {
                Ok(fragment) => cursor.paste = Some(fragment),
                Err(error) => document.status = error.to_string(),
            }
        }
        if keys.just_pressed(KeyCode::KeyP) && cursor.valid && !blocked {
            commit_paste(document, cursor);
        }
        return true;
    }
    if cursor.placing {
        return false;
    }
    let selected = document.selected_keys();
    if selected.len() < 2 {
        return false;
    }
    if keys.just_pressed(KeyCode::Delete) {
        document.execute(EditorCommand::DeleteSelection(selected));
        return true;
    }
    let center = center(document, &selected);
    let transform = if let Some(radians) = rotation_input(keys) {
        Some(SelectionTransform::RotateBy { center, radians })
    } else if keys.just_pressed(KeyCode::KeyR) {
        Some(SelectionTransform::Rotate { center })
    } else if keys.just_pressed(KeyCode::KeyX) {
        Some(SelectionTransform::FlipX { center })
    } else if keys.just_pressed(KeyCode::KeyY) {
        Some(SelectionTransform::FlipY { center })
    } else {
        None
    };
    if let Some(transform) = transform {
        document.execute(EditorCommand::TransformSelection {
            parts: selected,
            transform,
        });
        return true;
    }
    false
}

/// 参考 KSP 的步进/精调：Q/E 为 ±15°，Shift+Q/E 为 ±1°。
pub(crate) fn rotation_input(keys: &ButtonInput<KeyCode>) -> Option<f64> {
    if keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight) {
        return None;
    }
    let direction = if keys.just_pressed(KeyCode::KeyQ) {
        -1.0_f64
    } else if keys.just_pressed(KeyCode::KeyE) {
        1.0_f64
    } else {
        return None;
    };
    let fine = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    Some((direction * if fine { 1.0 } else { 15.0 }).to_radians())
}

struct PastePreview {
    parts: Vec<Part>,
    command: EditorCommand,
    valid: bool,
    snapped: bool,
}

fn paste_preview(document: &EditorDocument, cursor: &EditorCursor) -> Option<PastePreview> {
    let fragment = cursor.paste.as_ref()?;
    let center = fragment.center();
    let mut offset = if document.free_mode {
        (cursor.world.0 - center.0, cursor.world.1 - center.1)
    } else {
        (
            (cursor.world.0 * 2.0).round() / 2.0 - center.0,
            (cursor.world.1 * 2.0).round() / 2.0 - center.1,
        )
    };
    let paste = |offset| {
        document.edit_command(EditorCommand::Paste {
            fragment: Box::new(fragment.clone()),
            offset,
        })
    };
    let mut command = paste(offset);
    let mut temporary = document.ship.clone();
    let mut snapped = false;
    if !document.free_mode && command.apply(&mut temporary).is_ok() {
        let existing: HashSet<_> = document.ship.all_parts().map(|part| part.id).collect();
        let added: Vec<_> = temporary
            .keyed_parts()
            .filter(|(_, part)| !existing.contains(&part.id))
            .map(|(key, _)| key)
            .collect();
        let mut invalid_offsets = HashSet::new();
        let collision_set =
            dr_core::geometry::CollisionSet::new(document.ship.all_parts(), &document.catalog);
        for (_, delta, connection) in snaps(&temporary, &document.catalog, &added, (0.0, 0.0)) {
            let proposed = (offset.0 + delta.0, offset.1 + delta.1);
            let offset_key = (proposed.0.to_bits(), proposed.1.to_bits());
            if invalid_offsets.contains(&offset_key) {
                continue;
            }
            if fragment.parts().any(|part| {
                let Some(kind) = document.catalog.get(&part.part_type) else {
                    return false;
                };
                let mut part = part.clone();
                part.x += proposed.0;
                part.y += proposed.1;
                collision_set.editor_overlap_blocked(&part, kind)
            }) {
                invalid_offsets.insert(offset_key);
                continue;
            }
            let paste = paste(proposed);
            match paste.preview_with_catalog(&document.ship, &document.catalog) {
                Ok(pasted) => {
                    if connection
                        .preview_with_catalog(&pasted, &document.catalog)
                        .is_ok()
                    {
                        offset = proposed;
                        command = EditorCommand::Batch(vec![paste, connection]);
                        snapped = true;
                        break;
                    }
                }
                Err(_) => {
                    invalid_offsets.insert(offset_key);
                }
            }
        }
    }
    let valid = command
        .preview_with_catalog(&document.ship, &document.catalog)
        .is_ok();
    let parts = fragment
        .parts()
        .map(|part| {
            let mut part = part.clone();
            part.x += offset.0;
            part.y += offset.1;
            part
        })
        .collect();
    Some(PastePreview {
        parts,
        command,
        valid,
        snapped,
    })
}

pub(crate) fn commit_paste(document: &mut EditorDocument, cursor: &mut EditorCursor) -> bool {
    let Some(preview) = paste_preview(document, cursor) else {
        return false;
    };
    let existing: HashSet<_> = document.ship.all_parts().map(|part| part.id).collect();
    if document.execute(preview.command) {
        document.selection = document
            .ship
            .keyed_parts()
            .filter(|(_, part)| !existing.contains(&part.id))
            .map(|(key, _)| key)
            .collect();
        document.selected = document.selection.last().copied();
        cursor.paste = None;
        true
    } else {
        false
    }
}

#[derive(Component)]
pub(crate) struct PasteVisual;

#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_preview(
    mut commands: Commands,
    document: Res<EditorDocument>,
    cursor: Res<EditorCursor>,
    drag: Res<DragState>,
    pointer: Res<panels::UiPointer>,
    assets: Res<AssetServer>,
    images: Res<Assets<Image>>,
    mut visuals: Query<(&mut Sprite, &mut Transform, &mut bevy::sprite::Anchor), With<PasteVisual>>,
    mut entities: Local<Vec<Entity>>,
    mut gizmos: Gizmos,
) {
    if let Some(start) = drag.rectangle {
        let end = drag.rect_end;
        let a = Vec2::new(start.0 as f32 * 60.0, start.1 as f32 * 60.0);
        let b = Vec2::new(end.0 as f32 * 60.0, end.1 as f32 * 60.0);
        for (a, b) in [
            (a, Vec2::new(b.x, a.y)),
            (Vec2::new(b.x, a.y), b),
            (b, Vec2::new(a.x, b.y)),
            (Vec2::new(a.x, b.y), a),
        ] {
            gizmos.line_2d(a, b, Color::srgb(0.3, 0.85, 1.0));
        }
    }
    if !cursor.is_changed()
        && !document.is_changed()
        && !pointer.is_changed()
        && !images.is_changed()
    {
        return;
    }
    let preview = if cursor.valid && !pointer.blocked {
        paste_preview(&document, &cursor)
    } else {
        None
    };
    let count = preview.as_ref().map_or(0, |preview| preview.parts.len());
    while entities.len() > count {
        commands.entity(entities.pop().unwrap()).despawn();
    }
    let Some(preview) = preview else {
        return;
    };
    let color = if !preview.valid {
        Color::srgba(1.0, 0.2, 0.2, 0.65)
    } else if preview.snapped {
        Color::srgba(0.35, 1.0, 0.65, 0.75)
    } else {
        Color::srgba(1.0, 1.0, 1.0, 0.6)
    };
    for (index, part) in preview.parts.iter().enumerate() {
        let kind = document.catalog.get(&part.part_type);
        let sprite_path = kind.map(|kind| kind.sprite.as_str()).unwrap_or("");
        let image = if sprite_path.is_empty() {
            default()
        } else {
            assets.load(format!("textures/parts/{sprite_path}"))
        };
        let (custom_size, anchor) = render::sprite_geometry(kind, &image, &images, part);
        let sprite = Sprite {
            image,
            custom_size,
            color,
            flip_x: part.flip_x,
            flip_y: part.flip_y,
            ..default()
        };
        let transform = Transform {
            translation: Vec3::new(
                part.x as f32 * 60.0,
                part.y as f32 * 60.0,
                5.0 + index as f32 / count.max(1) as f32,
            ),
            rotation: Quat::from_rotation_z(part.angle as f32),
            ..default()
        };
        if let Some(entity) = entities.get(index)
            && let Ok((mut existing_sprite, mut existing_transform, mut existing_anchor)) =
                visuals.get_mut(*entity)
        {
            *existing_sprite = sprite;
            *existing_transform = transform;
            *existing_anchor = anchor;
        } else {
            entities.push(
                commands
                    .spawn((sprite, anchor, transform, PasteVisual))
                    .id(),
            );
        }
    }
}
