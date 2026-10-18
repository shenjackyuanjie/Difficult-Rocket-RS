use super::*;

/// 当前鼠标在编辑器世界坐标中的位置，以及目录中待放置的部件。
#[derive(Resource, Default)]
pub(crate) struct EditorCursor {
    pub(crate) manual_connection: Option<free_mode::Endpoint>,
    pub(crate) clipboard: Option<ShipFragment>,
    pub(crate) paste: Option<ShipFragment>,
    pub(crate) world: (f64, f64),
    pub(crate) catalog_index: usize,
    pub(crate) placing: bool,
    pub(crate) palette_drag: bool,
    pub(crate) rotation: i32,
    pub(crate) fine_rotation: f64,
    pub(crate) flip_x: bool,
    pub(crate) flip_y: bool,
    pub(crate) valid: bool,
}

impl EditorCursor {
    pub(crate) fn cancel_placement(&mut self) {
        self.placing = false;
        self.palette_drag = false;
    }
}

pub(crate) fn collides(
    ship: &Ship,
    catalog: &PartCatalog,
    part: &Part,
    source: Option<PartKey>,
) -> bool {
    let Some(kind) = catalog.get(&part.part_type) else {
        return false;
    };
    let original = source.and_then(|key| ship.part_at(key));
    ship.all_parts()
        .filter(|other| !original.is_some_and(|part| std::ptr::eq(part, *other)))
        .any(|other| {
            catalog.get(&other.part_type).is_some_and(|other_kind| {
                dr_core::geometry::editor_overlap_blocked(part, kind, other, other_kind)
            })
        })
}

pub(crate) fn transform(document: &mut EditorDocument, id: PartKey, command: EditorCommand) {
    if document.free_mode {
        document.execute(command.at(id));
        return;
    }
    let Some(part) = document.ship.part_at(id).cloned() else {
        return;
    };
    let mut candidate = Ship {
        parts: vec![part],
        ..Ship::default()
    };
    if command.apply(&mut candidate).is_err() {
        return;
    }
    if collides(
        &document.ship,
        &document.catalog,
        &candidate.parts[0],
        Some(id),
    ) {
        document.sfx.push(audio::SfxEvent::Reject);
        document.status = "变换后的部件与其他部件重叠".into();
        return;
    }
    document.execute(EditorCommand::Batch(vec![
        EditorCommand::Disconnect(id.id).at(id),
        command.at(id),
    ]));
}

/// 连接提示与实际吸附共用占用、歧义和碰撞判断，不显示无法提交的候选。
#[allow(clippy::too_many_arguments)]
pub(crate) fn candidate_allowed(
    ship: &Ship,
    catalog: &PartCatalog,
    proposed: &Part,
    source_key: Option<PartKey>,
    target: &Part,
    target_key: PartKey,
    candidate: &dr_core::SnapCandidate,
    free_mode: bool,
) -> bool {
    let (Some(source_type), Some(target_type)) = (
        catalog.get(&proposed.part_type),
        catalog.get(&target.part_type),
    ) else {
        return false;
    };
    if !dr_core::connections::available_scoped(
        ship,
        catalog,
        proposed,
        source_type,
        source_key,
        target,
        target_type,
        target_key,
        candidate,
    ) {
        return false;
    }
    if !free_mode {
        return !collides(ship, catalog, proposed, source_key);
    }
    // 自由几何只放开与其它部件的整体碰撞；新自动连接仍遵守连接对约束。
    if dr_core::geometry::editor_overlap_blocked(proposed, source_type, target, target_type) {
        return false;
    }
    let Some(source_key) = source_key else {
        return true;
    };
    if source_key.group == target_key.group
        && ship
            .group(source_key.group)
            .is_some_and(|(_, connections)| {
                connections.iter().any(|connection| {
                    connection.touches(source_key.id) && connection.touches(target_key.id)
                })
            })
    {
        return false;
    }
    // 自由拖动保留源部件的旧连接，因此也检查源端占用。
    let reverse = dr_core::SnapCandidate {
        source_index: candidate.target_index,
        target_index: candidate.source_index,
        ..*candidate
    };
    dr_core::connections::available_scoped(
        ship,
        catalog,
        target,
        target_type,
        Some(target_key),
        proposed,
        source_type,
        source_key,
        &reverse,
    )
}

/// 辅助几何下的纯吸附入口，供直接回归复用。
#[cfg(test)]
pub(crate) fn snap(
    ship: &Ship,
    catalog: &PartCatalog,
    source: &mut Part,
    source_key: Option<PartKey>,
) -> Option<EditorCommand> {
    snap_with_mode(ship, catalog, source, source_key, false)
}

/// 预览和落点提交共用的吸附计算，不改变文档。
fn snap_with_mode(
    ship: &Ship,
    catalog: &PartCatalog,
    source: &mut Part,
    source_key: Option<PartKey>,
    free_mode: bool,
) -> Option<EditorCommand> {
    let source_type = catalog.get(&source.part_type)?;
    let mut best: Option<(EditorCommand, dr_core::SnapCandidate)> = None;
    for (target_key, target) in ship
        .keyed_parts()
        .filter(|(key, _)| Some(*key) != source_key)
    {
        let Some(target_type) = catalog.get(&target.part_type) else {
            continue;
        };
        for candidate in dr_core::connections::candidates(
            source,
            source_type,
            target,
            target_type,
            dr_core::connections::CONNECTION_DISTANCE,
        ) {
            if best
                .as_ref()
                .is_some_and(|(_, prior)| prior.distance <= candidate.distance)
            {
                continue;
            }
            let mut proposed = source.clone();
            proposed.x = candidate.position.x;
            proposed.y = candidate.position.y;
            if !candidate_allowed(
                ship, catalog, &proposed, source_key, target, target_key, &candidate, free_mode,
            ) {
                continue;
            }
            let child = source_key.unwrap_or_else(|| PartKey::new(0, source.id, 0));
            let kind = if candidate.dock {
                LinkKind::Dock {
                    connector: if source_type.kind == PartKind::DockConnector {
                        child
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
            let connection = EditorCommand::ConnectParts {
                parent: target_key,
                child,
                kind,
            };
            best = Some((connection, candidate));
        }
    }
    best.map(|(connection, candidate)| {
        source.x = candidate.position.x;
        source.y = candidate.position.y;
        connection
    })
}

pub(crate) fn preview(
    document: &EditorDocument,
    cursor: &EditorCursor,
) -> Option<(Part, Option<EditorCommand>, bool)> {
    let kind = document.catalog.visible().nth(cursor.catalog_index)?;
    let mut part = kind.instantiate(document.ship.next_part_id(), cursor.world);
    part.editor_angle = if kind.disable_editor_rotation {
        0
    } else {
        cursor.rotation
    };
    part.angle = if kind.disable_editor_rotation {
        0.0
    } else {
        (part.editor_angle as f64 * std::f64::consts::FRAC_PI_2 + cursor.fine_rotation)
            .rem_euclid(std::f64::consts::TAU)
    };
    part.flip_x = cursor.flip_x;
    part.flip_y = cursor.flip_y;
    let allowed = kind
        .max_occurrences
        .is_none_or(|limit| document.ship.count_type(&kind.id) < limit as usize);
    let connection = if document.snap_enabled {
        snap_with_mode(
            &document.ship,
            &document.catalog,
            &mut part,
            None,
            document.free_mode,
        )
    } else {
        None
    };
    let allowed = allowed
        && (document.free_mode || !collides(&document.ship, &document.catalog, &part, None));
    Some((part, connection, allowed))
}

pub(crate) fn place(document: &mut EditorDocument, cursor: &EditorCursor) -> bool {
    let Some((part, connection, allowed)) = preview(document, cursor) else {
        return false;
    };
    if !allowed {
        document.sfx.push(audio::SfxEvent::Reject);
        document.status = "无法放置：实体重叠达到 5% 或已达到数量上限".into();
        return false;
    }
    let id = part.id;
    let mut commands = vec![EditorCommand::Place(part.into())];
    if let Some(connection) = connection {
        commands.push(connection);
    }
    if document.execute(EditorCommand::Batch(commands)) {
        document.select_only(document.ship.unique_key(id));
        true
    } else {
        false
    }
}

/// 目录拖拽结束只提交一次，取消和失败均清除虚影，不影响现有文档。
pub fn finish_palette_drag(
    document: &mut EditorDocument,
    cursor: &mut EditorCursor,
    canvas: bool,
) -> bool {
    if !cursor.palette_drag {
        return false;
    }
    let placed = canvas && cursor.placing && cursor.valid && place(document, cursor);
    cursor.cancel_placement();
    if placed {
        document.clear_selection();
    }
    placed
}

fn preview_color(allowed: bool, connected: bool, palette_drag: bool) -> Color {
    let color = if !allowed {
        Color::srgba(1.0, 0.2, 0.2, 0.65)
    } else if connected {
        Color::srgba(0.35, 1.0, 0.65, 0.75)
    } else {
        Color::srgba(1.0, 1.0, 1.0, 0.55)
    };
    if palette_drag {
        color.with_alpha(render::UNLINKED_ALPHA)
    } else {
        color
    }
}

pub(crate) fn cancel_for_file_action(
    mut actions: MessageReader<files::FileAction>,
    mut cursor: ResMut<EditorCursor>,
) {
    if actions.read().next().is_some() {
        cursor.manual_connection = None;
        cursor.cancel_placement();
        cursor.paste = None;
        cursor.valid = false;
        actions.clear();
    }
}

#[derive(Component)]
pub(crate) struct PlacementVisual;

pub(crate) fn draw_preview(
    mut commands: Commands,
    document: Res<EditorDocument>,
    cursor: Res<EditorCursor>,
    pointer: Res<panels::UiPointer>,
    assets: Res<AssetServer>,
    images: Res<Assets<Image>>,
    existing: Query<Entity, With<PlacementVisual>>,
) {
    for entity in &existing {
        commands.entity(entity).despawn();
    }
    if !cursor.placing || !cursor.valid || pointer.blocked {
        return;
    }
    let Some((part, connection, allowed)) = preview(&document, &cursor) else {
        return;
    };
    let kind = document.catalog.get(&part.part_type).unwrap();
    let mut sprite = if kind.sprite.is_empty() {
        Sprite::from_color(
            Color::WHITE,
            Vec2::new(kind.width as f32 * 30.0, kind.height as f32 * 30.0),
        )
    } else {
        Sprite::from_image(assets.load(render::part_sprite_path(&kind.sprite)))
    };
    let (custom_size, anchor) = render::sprite_geometry(Some(kind), &sprite.image, &images, &part);
    sprite.custom_size = custom_size;
    sprite.flip_x = part.flip_x;
    sprite.flip_y = part.flip_y;
    sprite.color = preview_color(allowed, connection.is_some(), cursor.palette_drag);
    commands.spawn((
        sprite,
        anchor,
        Transform {
            translation: Vec3::new(part.x as f32 * 60.0, part.y as f32 * 60.0, 5.0),
            rotation: Quat::from_rotation_z(part.angle as f32),
            ..default()
        },
        PlacementVisual,
    ));
}

#[cfg(test)]
mod tests;
