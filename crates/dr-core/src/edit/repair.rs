use super::*;
use crate::ShipGroup;
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionRole {
    Parent,
    Child,
    Dock,
}

/// 一个明确的 XML 引用字段；同一条连接的两端也可分别指定实例。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferenceSite {
    Connection {
        index: usize,
        role: ConnectionRole,
    },
    Activation {
        owner: PartKey,
        stage: usize,
        index: usize,
    },
}

#[derive(Debug, Clone)]
pub struct DuplicateRepair {
    group: usize,
    id: PartId,
    original: ShipGroup,
    ids: Vec<PartId>,
    references: Vec<(ReferenceSite, Option<usize>)>,
}

impl DuplicateRepair {
    pub fn new(ship: &Ship, key: PartKey) -> Result<Self, CommandError> {
        if ship.part_at(key).is_none() {
            return Err(CommandError::MissingInstance(key));
        }
        let (parts, connections) = ship.group(key.group).unwrap();
        let count = parts.iter().filter(|part| part.id == key.id).count();
        if count < 2 {
            return Err(CommandError::InvalidRepair("本组没有重复编号".into()));
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
        let mut ids = vec![key.id];
        for _ in 1..count {
            while reserved.contains(&next) {
                next = next.checked_add(1).ok_or(CommandError::Invalid)?;
            }
            ids.push(next);
            reserved.insert(next);
        }
        let mut references = vec![];
        for (index, connection) in connections.iter().enumerate() {
            let (parent, child) = match *connection {
                Connection::Normal { parent, child, .. }
                | Connection::Dock { parent, child, .. } => (parent, child),
            };
            if parent == key.id {
                references.push((
                    ReferenceSite::Connection {
                        index,
                        role: ConnectionRole::Parent,
                    },
                    None,
                ));
            }
            if child == key.id {
                references.push((
                    ReferenceSite::Connection {
                        index,
                        role: ConnectionRole::Child,
                    },
                    None,
                ));
            }
            if let Connection::Dock { dock, .. } = *connection
                && dock == key.id
            {
                references.push((
                    ReferenceSite::Connection {
                        index,
                        role: ConnectionRole::Dock,
                    },
                    None,
                ));
            }
        }
        for (owner, part) in ship
            .keyed_parts()
            .filter(|(owner, _)| owner.group == key.group)
        {
            if let Some(staging) = part.pod.as_ref().and_then(|pod| pod.staging.as_ref()) {
                for (stage, step) in staging.steps.iter().enumerate() {
                    for (index, activation) in step.activations.iter().enumerate() {
                        if activation.id == key.id {
                            references.push((
                                ReferenceSite::Activation {
                                    owner,
                                    stage,
                                    index,
                                },
                                None,
                            ));
                        }
                    }
                }
            }
        }
        Ok(Self {
            group: key.group,
            id: key.id,
            original: ShipGroup {
                parts: parts.to_vec(),
                connections: connections.to_vec(),
            },
            ids,
            references,
        })
    }

    pub fn group(&self) -> usize {
        self.group
    }
    pub fn old_id(&self) -> PartId {
        self.id
    }
    pub fn new_ids(&self) -> &[PartId] {
        &self.ids
    }
    pub fn original(&self) -> &ShipGroup {
        &self.original
    }
    pub fn references(&self) -> &[(ReferenceSite, Option<usize>)] {
        &self.references
    }
    pub fn unassigned(&self) -> usize {
        self.references
            .iter()
            .filter(|(_, target)| target.is_none())
            .count()
    }

    pub fn assign(
        &mut self,
        reference: usize,
        occurrence: Option<usize>,
    ) -> Result<(), CommandError> {
        if occurrence.is_some_and(|target| target >= self.ids.len()) {
            return Err(CommandError::Invalid);
        }
        self.references
            .get_mut(reference)
            .ok_or(CommandError::Invalid)?
            .1 = occurrence;
        Ok(())
    }

    pub(super) fn apply_inner(&self, ship: &mut Ship) -> Result<(), CommandError> {
        let Some((parts, connections)) = ship.group(self.group) else {
            return Err(CommandError::InvalidRepair(
                "部件组已不存在，请重新打开修复界面".into(),
            ));
        };
        if parts != self.original.parts || connections != self.original.connections {
            return Err(CommandError::InvalidRepair(
                "本组数据已变更，请重新打开修复界面".into(),
            ));
        }
        if self.unassigned() != 0 {
            return Err(CommandError::InvalidRepair(format!(
                "还有 {} 条引用未指定实例",
                self.unassigned()
            )));
        }
        let reserved: BTreeSet<_> = ship
            .groups()
            .flat_map(|(_, parts, connections)| scoped::namespace(parts, connections))
            .collect();
        if self.ids.iter().skip(1).any(|id| reserved.contains(id)) {
            return Err(CommandError::InvalidRepair(
                "预分配的新编号已被使用，请重新打开修复界面".into(),
            ));
        }
        for (site, target) in &self.references {
            let new_id = self.ids[target.unwrap()];
            match *site {
                ReferenceSite::Connection { index, role } => {
                    let connection = &mut ship.group_mut(self.group).unwrap().1[index];
                    let slot = match (connection, role) {
                        (
                            Connection::Normal { parent, .. } | Connection::Dock { parent, .. },
                            ConnectionRole::Parent,
                        ) => parent,
                        (
                            Connection::Normal { child, .. } | Connection::Dock { child, .. },
                            ConnectionRole::Child,
                        ) => child,
                        (Connection::Dock { dock, .. }, ConnectionRole::Dock) => dock,
                        _ => unreachable!("引用来自未变更的原始组"),
                    };
                    *slot = new_id;
                }
                ReferenceSite::Activation {
                    owner,
                    stage,
                    index,
                } => {
                    ship.part_at_mut(owner)
                        .unwrap()
                        .pod
                        .as_mut()
                        .unwrap()
                        .staging
                        .as_mut()
                        .unwrap()
                        .steps[stage]
                        .activations[index]
                        .id = new_id;
                }
            }
        }
        for (part, new_id) in ship
            .group_mut(self.group)
            .unwrap()
            .0
            .iter_mut()
            .filter(|part| part.id == self.id)
            .zip(&self.ids)
        {
            part.id = *new_id;
        }
        Ok(())
    }
}
