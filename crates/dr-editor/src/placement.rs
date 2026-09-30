use super::*;

/// 预览和落点提交共用的吸附计算，不改变文档。
pub(crate) fn snap(ship: &Ship, catalog: &PartCatalog, source: &mut Part) -> Option<Connection> {
    let source_type = catalog.get(&source.part_type)?;
    let mut best: Option<(Connection, dr_core::SnapCandidate)> = None;
    for target in ship.all_parts().filter(|target| target.id != source.id) {
        let Some(target_type) = catalog.get(&target.part_type) else {
            continue;
        };
        let Some(candidate) = find_snap(source, source_type, target, target_type, 0.35) else {
            continue;
        };
        if best
            .as_ref()
            .is_some_and(|(_, prior)| prior.distance <= candidate.distance)
        {
            continue;
        }
        let connection = if candidate.dock {
            let dock = if source_type.kind == PartKind::DockConnector {
                source.id
            } else if target_type.kind == PartKind::DockConnector {
                target.id
            } else {
                continue;
            };
            Connection::Dock {
                dock,
                parent: target.id,
                child: source.id,
            }
        } else {
            Connection::Normal {
                parent: target.id,
                child: source.id,
                parent_attach: candidate.target_index as i32 + 1,
                child_attach: candidate.source_index as i32 + 1,
            }
        };
        best = Some((connection, candidate));
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
) -> Option<(Part, Option<Connection>, bool)> {
    let kind = document.catalog.visible().nth(cursor.catalog_index)?;
    let mut part = kind.instantiate(
        document.ship.next_part_id(),
        (
            (cursor.world.0 * 2.0).round() / 2.0,
            (cursor.world.1 * 2.0).round() / 2.0,
        ),
    );
    part.editor_angle = if kind.disable_editor_rotation {
        0
    } else {
        cursor.rotation
    };
    part.angle = part.editor_angle as f64 * std::f64::consts::FRAC_PI_2;
    part.flip_x = cursor.flip_x;
    part.flip_y = cursor.flip_y;
    let allowed = kind
        .max_occurrences
        .is_none_or(|limit| document.ship.count_type(&kind.id) < limit as usize);
    let connection = snap(&document.ship, &document.catalog, &mut part);
    Some((part, connection, allowed))
}

pub(crate) fn place(document: &mut EditorDocument, cursor: &EditorCursor) -> bool {
    let Some((part, connection, allowed)) = preview(document, cursor) else {
        return false;
    };
    if !allowed {
        document.status = "该部件已达到目录规定的数量上限".into();
        return false;
    }
    let id = part.id;
    let mut commands = vec![EditorCommand::Place(part.into())];
    if let Some(connection) = connection {
        commands.push(EditorCommand::Connect(connection));
    }
    if document.execute(EditorCommand::Batch(commands)) {
        document.selected = Some(id);
        true
    } else {
        false
    }
}

pub(crate) fn cancel_for_file_action(
    mut actions: MessageReader<files::FileAction>,
    mut cursor: ResMut<EditorCursor>,
) {
    if actions.read().next().is_some() {
        cursor.placing = false;
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
        Sprite::from_image(assets.load(format!("textures/parts/{}", kind.sprite)))
    };
    sprite.custom_size = Some(Vec2::new(
        kind.width as f32 * 30.0,
        kind.height as f32 * 30.0,
    ));
    sprite.flip_x = part.flip_x;
    sprite.flip_y = part.flip_y;
    sprite.color = if !allowed {
        Color::srgba(1.0, 0.2, 0.2, 0.65)
    } else if connection.is_some() {
        Color::srgba(0.35, 1.0, 0.65, 0.75)
    } else {
        Color::srgba(1.0, 1.0, 1.0, 0.55)
    };
    commands.spawn((
        sprite,
        Transform {
            translation: Vec3::new(part.x as f32 * 60.0, part.y as f32 * 60.0, 5.0),
            rotation: Quat::from_rotation_z(part.angle as f32),
            ..default()
        },
        PlacementVisual,
    ));
}
