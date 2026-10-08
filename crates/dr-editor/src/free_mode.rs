//! 自由模式的画布连接工具：两次点击决定连接/断开，移动不推断拓扑。
use super::*;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Endpoint {
    pub key: PartKey,
    pub attach: usize,
    revision: u64,
    point: Vec2d,
}

pub(crate) fn discard_stale(document: &EditorDocument, cursor: &mut EditorCursor) {
    if cursor
        .manual_connection
        .is_some_and(|endpoint| !document.free_mode || endpoint.revision != document.revision)
    {
        cursor.manual_connection = None;
    }
}

pub(crate) fn set_enabled(
    document: &mut EditorDocument,
    cursor: &mut EditorCursor,
    drag: &mut DragState,
    enabled: bool,
) {
    document.free_mode = enabled;
    cursor.manual_connection = None;
    cursor.cancel_placement();
    cursor.paste = None;
    drag.cancel();
    document.status = if enabled {
        "自由模式：依次点击两个连接点/边，建立或断开连接；移动不自动断连，Esc 取消".into()
    } else {
        "辅助模式：自动吸附连接；实体重叠必须小于 5%".into()
    };
}

fn hit(
    document: &EditorDocument,
    cursor: &EditorCursor,
    position: (f64, f64),
    radius: f64,
) -> Option<Endpoint> {
    let point = Vec2d {
        x: position.0,
        y: position.1,
    };
    let mut best: Option<(f64, bool, Endpoint)> = None;
    for (key, part) in document.ship.keyed_parts() {
        let Some(kind) = document.catalog.get(&part.part_type) else {
            continue;
        };
        for (attach, specification) in kind.attach_points.iter().enumerate() {
            let segment = dr_core::connections::segment(part, kind, specification);
            let (contact, _) = dr_core::connections::closest_points(segment, (point, point));
            let distance = contact.distance(point);
            if !distance.is_finite() || distance > radius {
                continue;
            }
            let same_part = cursor
                .manual_connection
                .is_some_and(|start| start.key == key);
            if best.as_ref().is_none_or(|(prior, same, _)| {
                distance < prior - 1e-9
                    || ((distance - prior).abs() < 1e-9 && (!same_part || *same))
            }) {
                best = Some((
                    distance,
                    same_part,
                    Endpoint {
                        key,
                        attach,
                        revision: document.revision,
                        point: contact,
                    },
                ));
            }
        }
    }
    best.map(|(_, _, endpoint)| endpoint)
}

fn command(document: &EditorDocument, a: Endpoint, b: Endpoint) -> Result<EditorCommand, String> {
    if a.key == b.key {
        return Err("不能将部件连接到自身，请点击另一个部件的连接点".into());
    }
    let endpoint = |endpoint: Endpoint| {
        let (parts, _) = document
            .ship
            .group(endpoint.key.group)
            .ok_or("连接所属组已失效")?;
        if parts
            .iter()
            .filter(|part| part.id == endpoint.key.id)
            .count()
            != 1
        {
            return Err("同组编号不唯一，请先修复引用归属");
        }
        let part = document
            .ship
            .part_at(endpoint.key)
            .ok_or("连接部件已失效")?;
        let kind = document
            .catalog
            .get(&part.part_type)
            .ok_or("连接部件不在目录中")?;
        let attach = kind
            .attach_points
            .get(endpoint.attach)
            .ok_or("连接点已失效")?;
        Ok((kind, attach))
    };
    let (at, aa) = endpoint(a)?;
    let (bt, ba) = endpoint(b)?;
    let (kind, expected) = if aa.dock || ba.dock {
        if !aa.dock
            || !ba.dock
            || !matches!(
                (at.kind, bt.kind),
                (PartKind::DockConnector, PartKind::DockPort)
                    | (PartKind::DockPort, PartKind::DockConnector)
            )
        {
            return Err("对接连接需要插头与端口的对接点；普通点不能接到对接点".into());
        }
        let connector = if at.kind == PartKind::DockConnector {
            a.key
        } else {
            b.key
        };
        (
            LinkKind::Dock { connector },
            Connection::Dock {
                parent: a.key.id,
                child: b.key.id,
                dock: connector.id,
            },
        )
    } else {
        let (parent_attach, child_attach) = (a.attach as i32 + 1, b.attach as i32 + 1);
        (
            LinkKind::Normal {
                parent_attach,
                child_attach,
            },
            Connection::Normal {
                parent: a.key.id,
                child: b.key.id,
                parent_attach,
                child_attach,
            },
        )
    };
    if a.key.group == b.key.group {
        let (_, connections) = document.ship.group(a.key.group).unwrap();
        if let Some((index, connection)) = connections
            .iter()
            .enumerate()
            .find(|(_, connection)| connection.equivalent(&expected))
        {
            return Ok(EditorCommand::RemoveConnection(dr_core::ConnectionRef {
                group: a.key.group,
                index,
                expected: connection.clone(),
            }));
        }
    }
    Ok(EditorCommand::ConnectParts {
        parent: a.key,
        child: b.key,
        kind,
    })
}

/// 返回 true 时消费画布点击，不能同时开始选择拖拽或框选。
pub(crate) fn click(
    document: &mut EditorDocument,
    cursor: &mut EditorCursor,
    position: (f64, f64),
    radius: f64,
) -> bool {
    if !document.free_mode {
        return false;
    }
    discard_stale(document, cursor);
    let Some(end) = hit(document, cursor, position, radius) else {
        cursor.manual_connection = None;
        return false;
    };
    if let Some(start) = cursor.manual_connection {
        if start.key == end.key && start.attach == end.attach {
            cursor.manual_connection = None;
            document.status = "已取消手动连接".into();
        } else {
            match command(document, start, end) {
                Ok(command) => {
                    let unlink = matches!(command, EditorCommand::RemoveConnection(_));
                    if document.execute(command) {
                        cursor.manual_connection = None;
                        document.status = if unlink {
                            "已手动断开连接 · Ctrl+Z 撤销"
                        } else {
                            "已手动连接 · Ctrl+Z 撤销"
                        }
                        .into();
                    }
                }
                Err(message) => document.status = message,
            }
        }
    } else {
        cursor.manual_connection = Some(end);
        document.select_only(Some(end.key));
        document.status = format!(
            "已选连接点 {}，请点击另一部件的连接点；Esc / 右键取消",
            end.attach + 1
        );
    }
    true
}

pub(crate) fn draw(
    gizmos: &mut Gizmos,
    document: &EditorDocument,
    drag: &DragState,
    cursor: &EditorCursor,
    radius: f32,
    canvas: bool,
) {
    let pending = cursor
        .manual_connection
        .filter(|start| start.revision == document.revision);
    let normal = Color::srgb(0.67, 0.56, 0.88);
    let selected = Color::srgb(1.0, 0.83, 0.35);
    let point = |p: Vec2d| (Vec2::new(p.x as f32, p.y as f32) * 60.0).extend(8.0);
    for (key, part) in document.ship.keyed_parts() {
        let part = drag.pose(key, part);
        let Some(kind) = document.catalog.get(&part.part_type) else {
            continue;
        };
        for (index, attach) in kind.attach_points.iter().enumerate() {
            let (a, b) = dr_core::connections::segment(&part, kind, attach);
            let active = pending.filter(|start| start.key == key && start.attach == index);
            let color = if active.is_some() { selected } else { normal };
            gizmos.line(point(a), point(b), color);
            let center = active.map_or(
                Vec2d {
                    x: (a.x + b.x) / 2.0,
                    y: (a.y + b.y) / 2.0,
                },
                |start| start.point,
            );
            gizmos.circle(Isometry3d::from_translation(point(center)), radius, color);
        }
    }
    if let Some(start) = pending
        && canvas
        && cursor.valid
    {
        gizmos.line(
            point(start.point),
            point(Vec2d {
                x: cursor.world.0,
                y: cursor.world.1,
            }),
            selected,
        );
    }
}

#[cfg(test)]
mod tests;
