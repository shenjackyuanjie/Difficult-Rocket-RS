//! 编辑器连接面的几何、兼容性及占用校验。加载的历史连接不在这里改写。
use crate::geometry::{SnapCandidate, part_world_attach};
use crate::{AttachPoint, Connection, Part, PartCatalog, PartKind, PartType, Ship, Vec2d};

const EPSILON: f64 = 1e-6;

fn dot(a: Vec2d, b: Vec2d) -> f64 {
    a.x * b.x + a.y * b.y
}
fn sub(a: Vec2d, b: Vec2d) -> Vec2d {
    Vec2d {
        x: a.x - b.x,
        y: a.y - b.y,
    }
}
fn add(a: Vec2d, b: Vec2d) -> Vec2d {
    Vec2d {
        x: a.x + b.x,
        y: a.y + b.y,
    }
}
fn scale(a: Vec2d, k: f64) -> Vec2d {
    Vec2d {
        x: a.x * k,
        y: a.y * k,
    }
}

fn direction(part: &Part, x: f64, y: f64) -> Vec2d {
    let x = if part.flip_x { -x } else { x };
    let y = if part.flip_y { -y } else { y };
    let (sin, cos) = part.angle.sin_cos();
    Vec2d {
        x: x * cos - y * sin,
        y: x * sin + y * cos,
    }
}

fn normal(part: &Part, attach: &AttachPoint) -> Option<Vec2d> {
    let (x, y) = match attach.location.as_str() {
        "Top" | "TopSide" | "TopCenter" => (0.0, 1.0),
        "Bottom" | "BottomSide" | "BottomCenter" => (0.0, -1.0),
        "Left" | "LeftSide" | "LeftCenter" => (-1.0, 0.0),
        "Right" | "RightSide" | "RightCenter" => (1.0, 0.0),
        _ => return None,
    };
    Some(direction(part, x, y))
}

pub fn segment(part: &Part, kind: &PartType, attach: &AttachPoint) -> (Vec2d, Vec2d) {
    let middle = part_world_attach(part, attach);
    let tangent = match attach.location.as_str() {
        "Top" | "Bottom" | "TopSide" | "BottomSide" => {
            direction(part, kind.width as f64 / 4.0, 0.0)
        }
        "Left" | "Right" | "LeftSide" | "RightSide" => {
            direction(part, 0.0, kind.height as f64 / 4.0)
        }
        _ => Vec2d::default(),
    };
    (sub(middle, tangent), add(middle, tangent))
}

fn project(point: Vec2d, segment: (Vec2d, Vec2d)) -> Vec2d {
    let edge = sub(segment.1, segment.0);
    let length = dot(edge, edge);
    if length < EPSILON * EPSILON {
        return segment.0;
    }
    add(
        segment.0,
        scale(
            edge,
            (dot(sub(point, segment.0), edge) / length).clamp(0.0, 1.0),
        ),
    )
}

/// 最近接触点；相互平行且重叠时优先保留源部件的切向位置。
pub fn closest_points(a: (Vec2d, Vec2d), b: (Vec2d, Vec2d)) -> (Vec2d, Vec2d) {
    let target = project(scale(add(a.0, a.1), 0.5), b);
    let mut best = (project(target, a), target);
    let mut consider = |pair: (Vec2d, Vec2d)| {
        if pair.0.distance(pair.1) + EPSILON < best.0.distance(best.1) {
            best = pair;
        }
    };
    for point in [a.0, a.1] {
        consider((point, project(point, b)));
    }
    for point in [b.0, b.1] {
        consider((project(point, a), point));
    }
    let u = sub(a.1, a.0);
    let v = sub(b.1, b.0);
    let cross = |a: Vec2d, b: Vec2d| a.x * b.y - a.y * b.x;
    let determinant = cross(u, v);
    if determinant.abs() > EPSILON {
        let offset = sub(b.0, a.0);
        let t = cross(offset, v) / determinant;
        let s = cross(offset, u) / determinant;
        if (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&s) {
            let point = add(a.0, scale(u, t));
            return (point, point);
        }
    }
    best
}

pub fn compatible(
    source: &Part,
    st: &PartType,
    sa: &AttachPoint,
    target: &Part,
    tt: &PartType,
    ta: &AttachPoint,
) -> bool {
    if sa.dock != ta.dock {
        return false;
    }
    if sa.dock
        && !matches!(
            (st.kind, tt.kind),
            (PartKind::DockConnector, PartKind::DockPort)
                | (PartKind::DockPort, PartKind::DockConnector)
        )
    {
        return false;
    }
    match (normal(source, sa), normal(target, ta)) {
        (Some(a), Some(b)) => dot(a, b) < -1.0 + EPSILON,
        _ => true,
    }
}

pub fn candidates(
    source: &Part,
    st: &PartType,
    target: &Part,
    tt: &PartType,
    threshold: f64,
) -> Vec<SnapCandidate> {
    if source.id == target.id || !threshold.is_finite() || threshold < 0.0 {
        return vec![];
    }
    let mut result = vec![];
    for (source_index, sa) in st.attach_points.iter().enumerate() {
        for (target_index, ta) in tt.attach_points.iter().enumerate() {
            if !compatible(source, st, sa, target, tt, ta) {
                continue;
            }
            let (a, b) = closest_points(segment(source, st, sa), segment(target, tt, ta));
            let distance = a.distance(b);
            if !distance.is_finite() || distance > threshold {
                continue;
            }
            result.push(SnapCandidate {
                source_index,
                target_index,
                distance,
                dock: sa.dock,
                position: Vec2d {
                    x: source.x + b.x - a.x,
                    y: source.y + b.y - a.y,
                },
            });
        }
    }
    result.sort_by(|a, b| {
        a.distance
            .total_cmp(&b.distance)
            .then_with(|| {
                st.attach_points[a.source_index]
                    .order
                    .unwrap_or(i32::MAX)
                    .cmp(&st.attach_points[b.source_index].order.unwrap_or(i32::MAX))
            })
            .then_with(|| {
                tt.attach_points[a.target_index]
                    .order
                    .unwrap_or(i32::MAX)
                    .cmp(&tt.attach_points[b.target_index].order.unwrap_or(i32::MAX))
            })
    });
    result
}

/// 同一接触面可有多个接触位置；固定点和共享 group 独占。
fn occupied(
    ship: &Ship,
    catalog: &PartCatalog,
    part: &Part,
    kind: &PartType,
    index: usize,
    contact: Vec2d,
    ignored: Option<i64>,
) -> bool {
    let attach = &kind.attach_points[index];
    let surface = segment(part, kind, attach);
    let is_point = surface.0.distance(surface.1) < EPSILON;
    ship.all_connections()
        .filter(|connection| !ignored.is_some_and(|id| connection.touches(id)))
        .any(|connection| match connection {
            Connection::Dock { .. } => attach.dock && connection.touches(part.id),
            Connection::Normal {
                parent,
                child,
                parent_attach,
                child_attach,
            } => {
                let (used, peer, peer_index) = if *parent == part.id {
                    (*parent_attach, *child, *child_attach)
                } else if *child == part.id {
                    (*child_attach, *parent, *parent_attach)
                } else {
                    return false;
                };
                let Some(used) = used
                    .checked_sub(1)
                    .and_then(|i| kind.attach_points.get(i as usize))
                else {
                    return false;
                };
                if attach.group.is_some() && attach.group == used.group {
                    return true;
                }
                if !std::ptr::eq(attach, used) {
                    return false;
                }
                if is_point {
                    return true;
                }
                let Some(peer) = ship.part(peer) else {
                    return true;
                };
                let Some(peer_kind) = catalog.get(&peer.part_type) else {
                    return true;
                };
                let Some(peer_attach) = peer_index
                    .checked_sub(1)
                    .and_then(|i| peer_kind.attach_points.get(i as usize))
                else {
                    return true;
                };
                let (_, existing_contact) =
                    closest_points(segment(peer, peer_kind, peer_attach), surface);
                existing_contact.distance(contact) < EPSILON
            }
        })
}

/// 检查吸附候选；拖动时忽略即将断开的旧连接。
#[allow(clippy::too_many_arguments)]
pub fn available(
    ship: &Ship,
    catalog: &PartCatalog,
    source: &Part,
    st: &PartType,
    target: &Part,
    tt: &PartType,
    candidate: &SnapCandidate,
    ignored: Option<i64>,
) -> bool {
    let Some(sa) = st.attach_points.get(candidate.source_index) else {
        return false;
    };
    let Some(ta) = tt.attach_points.get(candidate.target_index) else {
        return false;
    };
    let (a, b) = closest_points(segment(source, st, sa), segment(target, tt, ta));
    !occupied(
        ship,
        catalog,
        source,
        st,
        candidate.source_index,
        a,
        ignored,
    ) && !occupied(
        ship,
        catalog,
        target,
        tt,
        candidate.target_index,
        b,
        ignored,
    )
}

/// 有目录的编辑提交使用此校验；读取已有 XML 时保留原连接。
pub fn validate(ship: &Ship, catalog: &PartCatalog, connection: &Connection) -> Result<(), String> {
    let Connection::Normal {
        parent,
        child,
        parent_attach,
        child_attach,
    } = connection
    else {
        let Connection::Dock {
            dock,
            parent,
            child,
        } = connection
        else {
            unreachable!()
        };
        let type_of = |id| {
            ship.part(id)
                .and_then(|part| catalog.get(&part.part_type))
                .map(|kind| kind.kind)
        };
        if *dock != *parent && *dock != *child {
            return Err("对接插头必须属于连接的两端".into());
        }
        let peer = if *dock == *parent { *child } else { *parent };
        if type_of(*dock) != Some(PartKind::DockConnector)
            || type_of(peer) != Some(PartKind::DockPort)
        {
            return Err("对接连接需要插头与端口".into());
        }
        if ship
            .all_connections()
            .any(|c| matches!(c, Connection::Dock { .. }) && (c.touches(*dock) || c.touches(peer)))
        {
            return Err("对接插头或端口已占用".into());
        }
        let source = ship.part(*child).ok_or("连接部件不存在")?;
        let target = ship.part(*parent).ok_or("连接部件不存在")?;
        let st = catalog.get(&source.part_type).ok_or("连接部件不在目录中")?;
        let tt = catalog.get(&target.part_type).ok_or("连接部件不在目录中")?;
        if !candidates(source, st, target, tt, EPSILON)
            .iter()
            .any(|candidate| {
                candidate.dock && available(ship, catalog, source, st, target, tt, candidate, None)
            })
        {
            return Err("没有接触且可用的对接连接点".into());
        }
        return Ok(());
    };
    let source = ship.part(*child).ok_or("连接部件不存在")?;
    let target = ship.part(*parent).ok_or("连接部件不存在")?;
    let st = catalog.get(&source.part_type).ok_or("连接部件不在目录中")?;
    let tt = catalog.get(&target.part_type).ok_or("连接部件不在目录中")?;
    let si = child_attach
        .checked_sub(1)
        .and_then(|i| usize::try_from(i).ok())
        .ok_or("连接点编号无效")?;
    let ti = parent_attach
        .checked_sub(1)
        .and_then(|i| usize::try_from(i).ok())
        .ok_or("连接点编号无效")?;
    let sa = st.attach_points.get(si).ok_or("连接点超出目录范围")?;
    let ta = tt.attach_points.get(ti).ok_or("连接点超出目录范围")?;
    if sa.dock || ta.dock || !compatible(source, st, sa, target, tt, ta) {
        return Err("连接点类型或方向不兼容".into());
    }
    let (a, b) = closest_points(segment(source, st, sa), segment(target, tt, ta));
    if !a.distance(b).is_finite() || a.distance(b) > EPSILON {
        return Err("连接点未接触".into());
    }
    if occupied(ship, catalog, source, st, si, a, None)
        || occupied(ship, catalog, target, tt, ti, b, None)
    {
        return Err("连接点或共享连接组已占用".into());
    }
    Ok(())
}

/// 读取已有连接的显示端点，不要求历史文件满足当前编辑规则。
pub fn positions(
    ship: &Ship,
    catalog: &PartCatalog,
    connection: &Connection,
) -> Option<(Vec2d, Vec2d)> {
    match *connection {
        Connection::Normal {
            parent,
            child,
            parent_attach,
            child_attach,
        } => {
            let parent = ship.part(parent)?;
            let child = ship.part(child)?;
            let pt = catalog.get(&parent.part_type)?;
            let ct = catalog.get(&child.part_type)?;
            let pa = pt
                .attach_points
                .get(parent_attach.checked_sub(1)? as usize)?;
            let ca = ct
                .attach_points
                .get(child_attach.checked_sub(1)? as usize)?;
            let (child, parent) = closest_points(segment(child, ct, ca), segment(parent, pt, pa));
            Some((parent, child))
        }
        Connection::Dock { parent, child, .. } => {
            let parent = ship.part(parent)?;
            let child = ship.part(child)?;
            Some((
                Vec2d {
                    x: parent.x,
                    y: parent.y,
                },
                Vec2d {
                    x: child.x,
                    y: child.y,
                },
            ))
        }
    }
}

#[cfg(test)]
mod tests;
