use crate::model::{Connection, Part, PartId, Ship};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CommandError {
    #[error("部件不存在: {0}")]
    MissingPart(PartId),
    #[error("部件已存在: {0}")]
    DuplicatePart(PartId),
    #[error("操作不适用")]
    Invalid,
}

#[derive(Debug, Clone)]
pub enum EditorCommand {
    Place(Part),
    Delete(PartId),
    Move {
        id: PartId,
        from: (f64, f64),
        to: (f64, f64),
    },
    Rotate(PartId),
    FlipX(PartId),
    FlipY(PartId),
    Connect(Connection),
    Disconnect(PartId),
    Batch(Vec<EditorCommand>),
}

#[derive(Debug, Clone)]
struct Entry {
    before: Ship,
    after: Ship,
}

#[derive(Debug, Default)]
pub struct EditorHistory {
    undo: Vec<Entry>,
    redo: Vec<Entry>,
    limit: usize,
}

impl EditorHistory {
    pub fn with_limit(limit: usize) -> Self {
        Self {
            limit: limit.max(1),
            ..Self::default()
        }
    }
    pub fn execute(&mut self, ship: &mut Ship, command: EditorCommand) -> Result<(), CommandError> {
        let before = ship.clone();
        command.apply(ship)?;
        if *ship == before {
            return Ok(());
        }
        if self.limit == 0 {
            self.limit = 256;
        }
        self.undo.push(Entry {
            before,
            after: ship.clone(),
        });
        if self.undo.len() > self.limit {
            self.undo.remove(0);
        }
        self.redo.clear();
        Ok(())
    }
    pub fn undo(&mut self, ship: &mut Ship) -> bool {
        if let Some(entry) = self.undo.pop() {
            *ship = entry.before.clone();
            self.redo.push(entry);
            true
        } else {
            false
        }
    }
    pub fn redo(&mut self, ship: &mut Ship) -> bool {
        if let Some(entry) = self.redo.pop() {
            *ship = entry.after.clone();
            self.undo.push(entry);
            true
        } else {
            false
        }
    }
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
}

impl EditorCommand {
    /// 原子执行：任何子命令失败时保留原始船体。
    pub fn apply(&self, ship: &mut Ship) -> Result<(), CommandError> {
        let mut after = ship.clone();
        self.apply_inner(&mut after)?;
        *ship = after;
        Ok(())
    }

    fn apply_inner(&self, ship: &mut Ship) -> Result<(), CommandError> {
        match self {
            Self::Batch(commands) => {
                for command in commands {
                    command.apply_inner(ship)?;
                }
            }
            Self::Disconnect(id) => {
                if ship.part(*id).is_none() {
                    return Err(CommandError::MissingPart(*id));
                }
                ship.disconnect_part(*id);
            }
            Self::Place(part) => {
                if part.id <= 0 || !part.x.is_finite() || !part.y.is_finite() {
                    return Err(CommandError::Invalid);
                }
                if ship.part(part.id).is_some() {
                    return Err(CommandError::DuplicatePart(part.id));
                }
                ship.parts.push(part.clone());
            }
            Self::Delete(id) => {
                ship.remove_part(*id)
                    .ok_or(CommandError::MissingPart(*id))?;
            }
            Self::Move { id, to, .. } => {
                if !to.0.is_finite() || !to.1.is_finite() {
                    return Err(CommandError::Invalid);
                }
                let part = ship.part_mut(*id).ok_or(CommandError::MissingPart(*id))?;
                part.x = to.0;
                part.y = to.1;
            }
            Self::Rotate(id) => {
                let part = ship.part_mut(*id).ok_or(CommandError::MissingPart(*id))?;
                part.editor_angle = (part.editor_angle + 1).rem_euclid(4);
                part.angle = (part.editor_angle as f64) * std::f64::consts::FRAC_PI_2;
            }
            Self::FlipX(id) => {
                let part = ship.part_mut(*id).ok_or(CommandError::MissingPart(*id))?;
                part.flip_x = !part.flip_x;
            }
            Self::FlipY(id) => {
                let part = ship.part_mut(*id).ok_or(CommandError::MissingPart(*id))?;
                part.flip_y = !part.flip_y;
            }
            Self::Connect(connection) => {
                let (parent, child, parent_attach, child_attach) = match connection {
                    Connection::Normal {
                        parent,
                        child,
                        parent_attach,
                        child_attach,
                    } => (*parent, *child, Some(*parent_attach), Some(*child_attach)),
                    Connection::Dock { parent, child, .. } => (*parent, *child, None, None),
                };
                if parent == child || parent <= 0 || child <= 0 {
                    return Err(CommandError::Invalid);
                }
                if ship.part(parent).is_none() || ship.part(child).is_none() {
                    return Err(CommandError::MissingPart(if ship.part(parent).is_none() {
                        parent
                    } else {
                        child
                    }));
                }
                if parent_attach.is_some_and(|index| index <= 0)
                    || child_attach.is_some_and(|index| index <= 0)
                {
                    return Err(CommandError::Invalid);
                }
                if let Connection::Dock { dock, .. } = connection {
                    if ship.part(*dock).is_none() {
                        return Err(CommandError::MissingPart(*dock));
                    }
                }
                if ship
                    .all_connections()
                    .any(|existing| existing.equivalent(connection))
                {
                    return Err(CommandError::Invalid);
                }
                ship.add_connection(connection.clone());
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct EditorState {
    pub ship: Ship,
    pub selected: Option<PartId>,
    pub dirty: bool,
}

impl EditorState {
    pub fn new(ship: Ship) -> Self {
        Self {
            ship,
            selected: None,
            dirty: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn part(id: i64) -> Part {
        Part {
            id,
            part_type: "pod-1".into(),
            x: 0.0,
            y: 0.0,
            angle: 0.0,
            editor_angle: 0,
            angle_v: 0.0,
            flip_x: false,
            flip_y: false,
            active: false,
            exploded: false,
            fuel: None,
            fuel_kind: None,
            extension: None,
            parachute: Default::default(),
            pod: None,
        }
    }
    fn connection(parent: i64, child: i64) -> Connection {
        Connection::Normal {
            parent,
            child,
            parent_attach: 1,
            child_attach: 2,
        }
    }

    #[test]
    fn batch_failure_is_atomic_and_preserves_redo() {
        let mut ship = Ship::default();
        let mut history = EditorHistory::default();
        history
            .execute(&mut ship, EditorCommand::Place(part(1)))
            .unwrap();
        history.undo(&mut ship);
        let before = ship.clone();
        assert!(
            history
                .execute(
                    &mut ship,
                    EditorCommand::Batch(vec![
                        EditorCommand::Place(part(2)),
                        EditorCommand::Delete(99)
                    ])
                )
                .is_err()
        );
        assert_eq!(ship, before);
        assert!(history.can_redo());
        assert!(history.redo(&mut ship));
        assert!(ship.part(1).is_some());
    }

    #[test]
    fn move_and_connect_undo_together() {
        let mut ship = Ship {
            parts: vec![part(1), part(2)],
            ..Ship::default()
        };
        let before = ship.clone();
        let mut history = EditorHistory::default();
        history
            .execute(
                &mut ship,
                EditorCommand::Batch(vec![
                    EditorCommand::Move {
                        id: 2,
                        from: (0.0, 0.0),
                        to: (1.0, 2.0),
                    },
                    EditorCommand::Connect(connection(1, 2)),
                ]),
            )
            .unwrap();
        let after = ship.clone();
        assert!(history.undo(&mut ship));
        assert_eq!(ship, before);
        assert!(!history.can_undo());
        assert!(history.redo(&mut ship));
        assert_eq!(ship, after);
    }

    #[test]
    fn connections_merge_groups_and_reject_reverse_duplicates() {
        use crate::model::ShipGroup;
        let mut ship = Ship {
            parts: vec![part(1)],
            disconnected: vec![ShipGroup {
                parts: vec![part(2), part(3)],
                connections: vec![connection(2, 3)],
            }],
            ..Ship::default()
        };
        let before = ship.clone();
        let reverse = Connection::Normal {
            parent: 3,
            child: 2,
            parent_attach: 2,
            child_attach: 1,
        };
        assert!(EditorCommand::Connect(reverse).apply(&mut ship).is_err());
        assert_eq!(ship, before);
        EditorCommand::Connect(connection(1, 2))
            .apply(&mut ship)
            .unwrap();
        assert!(ship.disconnected.is_empty());
        assert_eq!(ship.parts.len(), 3);
        assert_eq!(ship.connections.len(), 2);
        let bad_dock = Connection::Dock {
            dock: 99,
            parent: 1,
            child: 3,
        };
        assert!(EditorCommand::Connect(bad_dock).apply(&mut ship).is_err());
    }

    #[test]
    fn delete_cleans_dock_and_staging_and_undo_restores_both() {
        use crate::model::{Activation, PodState, ShipGroup, StageStep, StagingState};
        let mut pod = part(1);
        pod.pod = Some(PodState {
            staging: Some(StagingState {
                current_stage: 0,
                steps: vec![StageStep {
                    activations: vec![Activation { id: 3, moved: true }],
                }],
            }),
            ..Default::default()
        });
        let mut ship = Ship {
            parts: vec![pod, part(2), part(3)],
            disconnected: vec![ShipGroup {
                parts: vec![part(4)],
                connections: vec![],
            }],
            connections: vec![Connection::Dock {
                dock: 3,
                parent: 1,
                child: 2,
            }],
            ..Ship::default()
        };
        let before = ship.clone();
        let mut history = EditorHistory::default();
        history
            .execute(&mut ship, EditorCommand::Delete(3))
            .unwrap();
        assert!(ship.connections.is_empty());
        assert!(
            ship.part(1)
                .unwrap()
                .pod
                .as_ref()
                .unwrap()
                .staging
                .as_ref()
                .unwrap()
                .steps[0]
                .activations
                .is_empty()
        );
        assert!(history.undo(&mut ship));
        assert_eq!(ship, before);
    }

    #[test]
    fn no_op_does_not_consume_history_or_clear_redo() {
        let mut ship = Ship {
            parts: vec![part(1)],
            ..Ship::default()
        };
        let mut history = EditorHistory::default();
        history
            .execute(&mut ship, EditorCommand::Place(part(2)))
            .unwrap();
        history.undo(&mut ship);
        history
            .execute(
                &mut ship,
                EditorCommand::Move {
                    id: 1,
                    from: (0.0, 0.0),
                    to: (0.0, 0.0),
                },
            )
            .unwrap();
        assert!(!history.can_undo());
        assert!(history.can_redo());
    }

    #[test]
    fn undo_redo_restores_ship() {
        let mut ship = Ship::default();
        let mut history = EditorHistory::with_limit(8);
        history
            .execute(&mut ship, EditorCommand::Place(part(1)))
            .unwrap();
        assert!(history.undo(&mut ship));
        assert!(ship.parts.is_empty());
        assert!(history.redo(&mut ship));
        assert_eq!(ship.parts.len(), 1);
    }
}
