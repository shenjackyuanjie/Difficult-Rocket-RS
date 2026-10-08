use super::*;
use std::collections::{BTreeSet, HashMap, HashSet};

pub(super) fn resolve(
    ship: &Ship,
    id: PartId,
    scope: Option<PartKey>,
) -> Result<PartKey, CommandError> {
    if let Some(key) = scope {
        if key.id != id || ship.part_at(key).is_none() {
            return Err(CommandError::MissingInstance(key));
        }
        return Ok(key);
    }
    ship.unique_key(id).ok_or_else(|| {
        if ship.all_parts().any(|p| p.id == id) {
            CommandError::AmbiguousPart(id)
        } else {
            CommandError::MissingPart(id)
        }
    })
}

pub(super) fn resolve_in_group(
    ship: &Ship,
    group: usize,
    id: PartId,
) -> Result<PartKey, CommandError> {
    let key = PartKey::new(group, id, 0);
    let parts = ship
        .group(group)
        .ok_or(CommandError::MissingInstance(key))?
        .0;
    match parts.iter().filter(|p| p.id == id).count() {
        0 => Err(CommandError::MissingPart(id)),
        1 => Ok(key),
        _ => Err(CommandError::AmbiguousReference(key)),
    }
}

pub(crate) fn disconnect(ship: &mut Ship, key: PartKey) -> Result<(), CommandError> {
    let (parts, connections) = ship
        .group_mut(key.group)
        .ok_or(CommandError::MissingInstance(key))?;
    if parts.iter().filter(|p| p.id == key.id).count() > 1
        && connections.iter().any(|c| c.touches(key.id))
    {
        return Err(CommandError::AmbiguousReference(key));
    }
    connections.retain(|c| !c.touches(key.id));
    Ok(())
}

pub(crate) fn delete(ship: &mut Ship, key: PartKey) -> Result<(), CommandError> {
    let (parts, connections) = ship
        .group_mut(key.group)
        .ok_or(CommandError::MissingInstance(key))?;
    let duplicate = parts.iter().filter(|p| p.id == key.id).count() > 1;
    if duplicate
        && (connections.iter().any(|c| c.touches(key.id))
            || parts.iter().any(|part| {
                part.pod
                    .as_ref()
                    .and_then(|pod| pod.staging.as_ref())
                    .is_some_and(|staging| {
                        staging
                            .steps
                            .iter()
                            .any(|step| step.activations.iter().any(|a| a.id == key.id))
                    })
            }))
    {
        return Err(CommandError::AmbiguousReference(key));
    }
    let index = parts
        .iter()
        .enumerate()
        .filter(|(_, p)| p.id == key.id)
        .nth(key.occurrence)
        .map(|(index, _)| index)
        .ok_or(CommandError::MissingInstance(key))?;
    parts.remove(index);
    if !duplicate {
        connections.retain(|c| !c.touches(key.id));
        for part in parts.iter_mut() {
            if let Some(staging) = part.pod.as_mut().and_then(|pod| pod.staging.as_mut()) {
                for step in &mut staging.steps {
                    step.activations.retain(|a| a.id != key.id);
                }
            }
        }
    }
    // 不在批量命令中删除空组，防止后续命令的快照位置指向另一个组。
    Ok(())
}

pub(super) fn remap_connection(connection: &mut Connection, ids: &HashMap<PartId, PartId>) {
    let remap = |id: &mut PartId| {
        if let Some(new) = ids.get(id) {
            *id = *new;
        }
    };
    match connection {
        Connection::Normal { parent, child, .. } => {
            remap(parent);
            remap(child);
        }
        Connection::Dock {
            parent,
            child,
            dock,
        } => {
            remap(parent);
            remap(child);
            remap(dock);
        }
    }
}

pub(super) fn namespace(parts: &[Part], connections: &[Connection]) -> BTreeSet<PartId> {
    let mut ids: BTreeSet<_> = parts.iter().map(|part| part.id).collect();
    for connection in connections {
        match *connection {
            Connection::Normal { parent, child, .. } => {
                ids.extend([parent, child]);
            }
            Connection::Dock {
                parent,
                child,
                dock,
            } => {
                ids.extend([parent, child, dock]);
            }
        }
    }
    for part in parts {
        if let Some(staging) = part.pod.as_ref().and_then(|pod| pod.staging.as_ref()) {
            ids.extend(
                staging
                    .steps
                    .iter()
                    .flat_map(|step| step.activations.iter().map(|a| a.id)),
            );
        }
    }
    ids
}

pub(super) fn connect(
    ship: &mut Ship,
    catalog: Option<&PartCatalog>,
    parent: PartKey,
    child: PartKey,
    kind: &LinkKind,
    free: bool,
) -> Result<(), CommandError> {
    if parent == child || parent.id <= 0 || child.id <= 0 {
        return Err(CommandError::Invalid);
    }
    let mut keys = vec![parent, child];
    if let LinkKind::Dock { connector } = kind {
        keys.push(*connector);
    }
    for key in &keys {
        if ship.part_at(*key).is_none() {
            return Err(CommandError::MissingInstance(*key));
        }
        resolve_in_group(ship, key.group, key.id)?;
    }
    let mut groups: Vec<_> = keys.iter().map(|key| key.group).collect();
    groups.sort_unstable();
    groups.dedup();
    let target = groups[0];
    let mut merged = Ship::default();
    let mut used = HashSet::new();
    // 保留历史悬空引用的编号，避免重新编号意外让它们指向新部件。
    let mut reserved: HashSet<_> = ship
        .groups()
        .flat_map(|(_, parts, connections)| namespace(parts, connections))
        .collect();
    let mut next = reserved
        .iter()
        .copied()
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .unwrap_or(1)
        .max(1);
    let mut remapped_keys = HashMap::new();
    for group in &groups {
        let (parts, connections) = ship.group(*group).unwrap();
        let mut ids = HashMap::new();
        let group_ids = namespace(parts, connections);
        for id in &group_ids {
            if used.contains(id) {
                while reserved.contains(&next) {
                    next = next.checked_add(1).ok_or(CommandError::Invalid)?;
                }
                ids.insert(*id, next);
                reserved.insert(next);
            }
        }
        for key in keys.iter().filter(|key| key.group == *group) {
            remapped_keys.insert(*key, ids.get(&key.id).copied().unwrap_or(key.id));
        }
        for part in parts {
            let mut part = part.clone();
            part.id = ids.get(&part.id).copied().unwrap_or(part.id);
            if let Some(staging) = part.pod.as_mut().and_then(|pod| pod.staging.as_mut()) {
                for activation in staging
                    .steps
                    .iter_mut()
                    .flat_map(|step| &mut step.activations)
                {
                    if let Some(id) = ids.get(&activation.id) {
                        activation.id = *id;
                    }
                }
            }
            merged.parts.push(part);
        }
        for connection in connections {
            let mut connection = connection.clone();
            remap_connection(&mut connection, &ids);
            merged.connections.push(connection);
        }
        used.extend(
            group_ids
                .into_iter()
                .map(|id| ids.get(&id).copied().unwrap_or(id)),
        );
    }
    let connection = match *kind {
        LinkKind::Normal {
            parent_attach,
            child_attach,
        } => {
            if parent_attach <= 0 || child_attach <= 0 {
                return Err(CommandError::Invalid);
            }
            Connection::Normal {
                parent: remapped_keys[&parent],
                child: remapped_keys[&child],
                parent_attach,
                child_attach,
            }
        }
        LinkKind::Dock { connector } => Connection::Dock {
            parent: remapped_keys[&parent],
            child: remapped_keys[&child],
            dock: remapped_keys[&connector],
        },
    };
    if merged.connections.iter().any(|c| c.equivalent(&connection)) {
        return Err(CommandError::Invalid);
    }
    if let Some(catalog) = catalog {
        let validate = if free {
            crate::connections::validate_manual
        } else {
            crate::connections::validate
        };
        validate(&merged, catalog, &connection).map_err(CommandError::InvalidConnection)?;
    }
    merged.connections.push(connection);
    let (parts, connections) = ship.group_mut(target).unwrap();
    *parts = merged.parts;
    *connections = merged.connections;
    for group in groups.into_iter().skip(1) {
        ship.disconnected[group - 1] = Default::default();
    }
    Ok(())
}
