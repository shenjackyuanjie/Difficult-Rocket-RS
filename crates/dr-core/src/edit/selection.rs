use super::*;
use crate::ShipGroup;
use std::collections::{BTreeSet, HashMap, HashSet};

#[derive(Debug, Clone, Copy)]
pub enum SelectionTransform {
    Translate { dx: f64, dy: f64 },
    Rotate { center: (f64, f64) },
    FlipX { center: (f64, f64) },
    FlipY { center: (f64, f64) },
}

impl SelectionTransform {
    fn apply(self, part: &mut Part, catalog: Option<&PartCatalog>) -> Result<(), CommandError> {
        match self {
            Self::Translate { dx, dy } => {
                part.x += dx;
                part.y += dy;
            }
            Self::Rotate { center: (x, y) } => {
                if catalog
                    .and_then(|catalog| catalog.get(&part.part_type))
                    .is_some_and(|kind| kind.disable_editor_rotation)
                {
                    return Err(CommandError::InvalidSelection(format!(
                        "部件 {} 不允许旋转",
                        part.part_type
                    )));
                }
                (part.x, part.y) = (x - (part.y - y), y + (part.x - x));
                part.angle =
                    (part.angle + std::f64::consts::FRAC_PI_2).rem_euclid(std::f64::consts::TAU);
                part.editor_angle = (part.editor_angle.rem_euclid(4) + 1).rem_euclid(4);
            }
            Self::FlipX { center: (x, _) } => {
                part.x = 2.0 * x - part.x;
                part.angle = (-part.angle).rem_euclid(std::f64::consts::TAU);
                part.editor_angle = (-part.editor_angle.rem_euclid(4)).rem_euclid(4);
                part.flip_x = !part.flip_x;
            }
            Self::FlipY { center: (_, y) } => {
                part.y = 2.0 * y - part.y;
                part.angle = (-part.angle).rem_euclid(std::f64::consts::TAU);
                part.editor_angle = (-part.editor_angle.rem_euclid(4)).rem_euclid(4);
                part.flip_y = !part.flip_y;
            }
        }
        if !part.x.is_finite() || !part.y.is_finite() || !part.angle.is_finite() {
            return Err(CommandError::Invalid);
        }
        Ok(())
    }
}

fn keys(ship: &Ship, parts: &[PartKey]) -> Result<BTreeSet<PartKey>, CommandError> {
    let keys: BTreeSet<_> = parts.iter().copied().collect();
    if keys.is_empty() {
        return Err(CommandError::Invalid);
    }
    let present: HashSet<_> = ship.keyed_parts().map(|(key, _)| key).collect();
    for key in &keys {
        if !present.contains(key) {
            return Err(CommandError::MissingInstance(*key));
        }
    }
    Ok(keys)
}

fn references(connection: &Connection) -> Vec<PartId> {
    match *connection {
        Connection::Normal { parent, child, .. } => vec![parent, child],
        Connection::Dock {
            parent,
            child,
            dock,
        } => vec![parent, child, dock],
    }
}

/// 预览和提交共用的整体变换；内部相对形状不变，只检查与选择之外的碰撞。
pub fn preview(
    ship: &Ship,
    catalog: Option<&PartCatalog>,
    parts: &[PartKey],
    transform: SelectionTransform,
) -> Result<Vec<(PartKey, Part)>, CommandError> {
    let keys = keys(ship, parts)?;
    let mut result = Vec::with_capacity(keys.len());
    for (key, original) in ship.keyed_parts().filter(|(key, _)| keys.contains(key)) {
        let mut part = original.clone();
        transform.apply(&mut part, catalog)?;
        result.push((key, part));
    }
    if let Some(catalog) = catalog {
        let others: Vec<_> = ship
            .keyed_parts()
            .filter(|(key, _)| !keys.contains(key))
            .map(|(_, part)| part)
            .collect();
        for (_, part) in &result {
            collides(catalog, part, others.iter().copied())?;
        }
    }
    Ok(result)
}

fn collides<'a>(
    catalog: &PartCatalog,
    part: &Part,
    others: impl Iterator<Item = &'a Part>,
) -> Result<(), CommandError> {
    if let Some(kind) = catalog.get(&part.part_type)
        && others.into_iter().any(|other| {
            catalog
                .get(&other.part_type)
                .is_some_and(|other_kind| crate::intersects(part, kind, other, other_kind))
        })
    {
        return Err(CommandError::InvalidSelection(
            "部件与选择之外的部件重叠".into(),
        ));
    }
    Ok(())
}

pub(super) fn transform(
    ship: &mut Ship,
    catalog: Option<&PartCatalog>,
    parts: &[PartKey],
    transform: SelectionTransform,
) -> Result<(), CommandError> {
    let proposed = preview(ship, catalog, parts, transform)?;
    if proposed
        .iter()
        .all(|(key, part)| ship.part_at(*key) == Some(part))
    {
        return Ok(());
    }
    let mut selected = HashMap::<usize, HashSet<PartId>>::new();
    for (key, _) in &proposed {
        let (parts, connections) = ship.group(key.group).unwrap();
        if parts.iter().filter(|part| part.id == key.id).count() > 1
            && connections.iter().any(|c| c.touches(key.id))
        {
            return Err(CommandError::AmbiguousReference(*key));
        }
        selected.entry(key.group).or_default().insert(key.id);
    }
    for (group, selected) in selected {
        ship.group_mut(group).unwrap().1.retain(|connection| {
            let ids = references(connection);
            !ids.iter().any(|id| selected.contains(id))
                || ids.iter().all(|id| selected.contains(id))
        });
    }
    for (key, part) in proposed {
        *ship.part_at_mut(key).unwrap() = part;
    }
    Ok(())
}

pub(super) fn delete(ship: &mut Ship, parts: &[PartKey]) -> Result<(), CommandError> {
    // 同组同 ID 按出现次数倒序删除，避免前面的删除改变后面实例的位置。
    for key in keys(ship, parts)?.into_iter().rev() {
        scoped::delete(ship, key)?;
    }
    Ok(())
}

/// 应用内剪贴板；分组、内部连接及分级动作随选中部件一起复制。
#[derive(Debug, Clone)]
pub struct ShipFragment {
    groups: Vec<ShipGroup>,
}

impl ShipFragment {
    pub fn capture(ship: &Ship, selection: &[PartKey]) -> Result<Self, CommandError> {
        let keys = keys(ship, selection)?;
        let mut selected = HashMap::<usize, HashSet<PartId>>::new();
        for key in keys {
            if ship.group_part(key.group, key.id).is_none() {
                return Err(CommandError::AmbiguousReference(key));
            }
            selected.entry(key.group).or_default().insert(key.id);
        }
        let mut groups = vec![];
        for (group, parts, connections) in ship.groups() {
            let Some(ids) = selected.get(&group) else {
                continue;
            };
            let mut parts: Vec<_> = parts
                .iter()
                .filter(|part| ids.contains(&part.id))
                .cloned()
                .collect();
            for part in &mut parts {
                if let Some(staging) = part.pod.as_mut().and_then(|pod| pod.staging.as_mut()) {
                    for step in &mut staging.steps {
                        step.activations
                            .retain(|activation| ids.contains(&activation.id));
                    }
                }
            }
            let connections = connections
                .iter()
                .filter(|connection| references(connection).iter().all(|id| ids.contains(id)))
                .cloned()
                .collect();
            groups.push(ShipGroup { parts, connections });
        }
        Ok(Self { groups })
    }

    pub fn groups(&self) -> &[ShipGroup] {
        &self.groups
    }
    pub fn parts(&self) -> impl Iterator<Item = &Part> {
        self.groups.iter().flat_map(|group| &group.parts)
    }
    pub fn center(&self) -> (f64, f64) {
        let mut low = (f64::INFINITY, f64::INFINITY);
        let mut high = (f64::NEG_INFINITY, f64::NEG_INFINITY);
        for part in self.parts() {
            low = (low.0.min(part.x), low.1.min(part.y));
            high = (high.0.max(part.x), high.1.max(part.y));
        }
        ((low.0 + high.0) / 2.0, (low.1 + high.1) / 2.0)
    }
    pub fn transformed(
        &self,
        catalog: &PartCatalog,
        transform: SelectionTransform,
    ) -> Result<Self, CommandError> {
        let mut result = self.clone();
        for part in result.groups.iter_mut().flat_map(|group| &mut group.parts) {
            transform.apply(part, Some(catalog))?;
        }
        Ok(result)
    }
}

pub(super) fn paste(
    ship: &mut Ship,
    catalog: Option<&PartCatalog>,
    fragment: &ShipFragment,
    offset: (f64, f64),
) -> Result<(), CommandError> {
    let mut groups = fragment.groups.clone();
    if let Some(catalog) = catalog {
        let mut counts = HashMap::<&str, usize>::new();
        for part in fragment.parts() {
            *counts.entry(&part.part_type).or_default() += 1;
        }
        for (id, count) in counts {
            if catalog
                .get(id)
                .and_then(|kind| kind.max_occurrences)
                .is_some_and(|limit| ship.count_type(id) + count > limit as usize)
            {
                return Err(CommandError::InvalidSelection(format!(
                    "部件 {id} 已达到数量上限"
                )));
            }
        }
    }
    let mut reserved: BTreeSet<_> = ship
        .groups()
        .flat_map(|(_, parts, connections)| scoped::namespace(parts, connections))
        .collect();
    let mut next = reserved
        .last()
        .copied()
        .unwrap_or(0)
        .checked_add(1)
        .unwrap_or(1)
        .max(1);
    for group in &mut groups {
        let mut ids = HashMap::new();
        for part in &mut group.parts {
            while reserved.contains(&next) {
                next = next.checked_add(1).ok_or(CommandError::Invalid)?;
            }
            ids.insert(part.id, next);
            reserved.insert(next);
            part.id = next;
            SelectionTransform::Translate {
                dx: offset.0,
                dy: offset.1,
            }
            .apply(part, catalog)?;
            if let Some(catalog) = catalog {
                collides(catalog, part, ship.all_parts())?;
            }
        }
        for part in &mut group.parts {
            if let Some(staging) = part.pod.as_mut().and_then(|pod| pod.staging.as_mut()) {
                for activation in staging
                    .steps
                    .iter_mut()
                    .flat_map(|step| &mut step.activations)
                {
                    activation.id = ids[&activation.id];
                }
            }
        }
        for connection in &mut group.connections {
            scoped::remap_connection(connection, &ids);
        }
    }
    for group in groups {
        if ship.parts.is_empty() && ship.connections.is_empty() {
            ship.parts = group.parts;
            ship.connections = group.connections;
        } else {
            ship.disconnected.push(group);
        }
    }
    Ok(())
}
