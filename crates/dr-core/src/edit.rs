use crate::model::{Connection, Part, PartCatalog, PartId, Ship, StagingState};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CommandError {
    #[error("部件不存在: {0}")]
    MissingPart(PartId),
    #[error("部件已存在: {0}")]
    DuplicatePart(PartId),
    #[error("操作不适用")]
    Invalid,
    #[error("无法连接: {0}")]
    InvalidConnection(String),
}

#[derive(Debug, Clone)]
pub enum EditorCommand {
    Place(Box<Part>),
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
    SetActive(PartId, bool),
    SetFuel(PartId, f64),
    RenamePod(PartId, String),
    SetThrottle(PartId, f64),
    SetStaging(PartId, Option<StagingState>),
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
        self.execute_inner(ship, None, command)
    }
    pub fn execute_with_catalog(
        &mut self,
        ship: &mut Ship,
        catalog: &PartCatalog,
        command: EditorCommand,
    ) -> Result<(), CommandError> {
        self.execute_inner(ship, Some(catalog), command)
    }
    fn execute_inner(
        &mut self,
        ship: &mut Ship,
        catalog: Option<&PartCatalog>,
        command: EditorCommand,
    ) -> Result<(), CommandError> {
        let before = ship.clone();
        command.apply_checked(ship, catalog)?;
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
        self.apply_checked(ship, None)
    }

    fn apply_checked(
        &self,
        ship: &mut Ship,
        catalog: Option<&PartCatalog>,
    ) -> Result<(), CommandError> {
        let mut after = ship.clone();
        self.apply_inner(&mut after, catalog)?;
        *ship = after;
        Ok(())
    }

    fn apply_inner(
        &self,
        ship: &mut Ship,
        catalog: Option<&PartCatalog>,
    ) -> Result<(), CommandError> {
        match self {
            Self::Batch(commands) => {
                for command in commands {
                    command.apply_inner(ship, catalog)?;
                }
            }
            Self::SetActive(id, active) => {
                ship.part_mut(*id)
                    .ok_or(CommandError::MissingPart(*id))?
                    .active = *active;
            }
            Self::SetFuel(id, fuel) => {
                let part = ship.part_mut(*id).ok_or(CommandError::MissingPart(*id))?;
                if !fuel.is_finite() || *fuel < 0.0 || part.fuel_kind.is_none() {
                    return Err(CommandError::Invalid);
                }
                part.fuel = Some(*fuel);
            }
            Self::RenamePod(id, name) => {
                let part = ship.part_mut(*id).ok_or(CommandError::MissingPart(*id))?;
                part.pod.as_mut().ok_or(CommandError::Invalid)?.name = name.clone();
            }
            Self::SetThrottle(id, throttle) => {
                if !throttle.is_finite() || !(0.0..=1.0).contains(throttle) {
                    return Err(CommandError::Invalid);
                }
                let part = ship.part_mut(*id).ok_or(CommandError::MissingPart(*id))?;
                part.pod.as_mut().ok_or(CommandError::Invalid)?.throttle = *throttle;
            }
            Self::SetStaging(id, staging) => {
                if let Some(staging) = staging {
                    if staging.current_stage < 0 {
                        return Err(CommandError::Invalid);
                    }
                    for step in &staging.steps {
                        let mut ids = std::collections::HashSet::new();
                        for activation in &step.activations {
                            if ship.part(activation.id).is_none() {
                                return Err(CommandError::MissingPart(activation.id));
                            }
                            if !ids.insert(activation.id) {
                                return Err(CommandError::Invalid);
                            }
                        }
                    }
                }
                let part = ship.part_mut(*id).ok_or(CommandError::MissingPart(*id))?;
                part.pod.as_mut().ok_or(CommandError::Invalid)?.staging = staging.clone();
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
                ship.parts.push((**part).clone());
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
                if let Connection::Dock { dock, .. } = connection
                    && ship.part(*dock).is_none()
                {
                    return Err(CommandError::MissingPart(*dock));
                }
                if ship
                    .all_connections()
                    .any(|existing| existing.equivalent(connection))
                {
                    return Err(CommandError::Invalid);
                }
                if let Some(catalog) = catalog {
                    crate::connections::validate(ship, catalog, connection)
                        .map_err(CommandError::InvalidConnection)?;
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
            lander: Default::default(),
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
            .execute(&mut ship, EditorCommand::Place(part(1).into()))
            .unwrap();
        history.undo(&mut ship);
        let before = ship.clone();
        assert!(
            history
                .execute(
                    &mut ship,
                    EditorCommand::Batch(vec![
                        EditorCommand::Place(part(2).into()),
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
            .execute(&mut ship, EditorCommand::Place(part(2).into()))
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
            .execute(&mut ship, EditorCommand::Place(part(1).into()))
            .unwrap();
        assert!(history.undo(&mut ship));
        assert!(ship.parts.is_empty());
        assert!(history.redo(&mut ship));
        assert_eq!(ship.parts.len(), 1);
    }

    #[test]
    fn properties_and_staging_are_one_transaction_and_roundtrip_xml() {
        use crate::{Activation, FuelKind, PodState, StageStep};
        let mut pod = part(1);
        pod.pod = Some(PodState::default());
        let mut tank = part(2);
        tank.fuel_kind = Some(FuelKind::Tank);
        tank.fuel = Some(10.0);
        let mut ship = Ship {
            parts: vec![pod, tank],
            ..Ship::default()
        };
        let before = ship.clone();
        let staging = StagingState {
            current_stage: 0,
            steps: vec![StageStep {
                activations: vec![Activation { id: 2, moved: true }],
            }],
        };
        let mut history = EditorHistory::default();
        history
            .execute(
                &mut ship,
                EditorCommand::Batch(vec![
                    EditorCommand::RenamePod(1, "中文 & 火箭".into()),
                    EditorCommand::SetThrottle(1, 0.25),
                    EditorCommand::SetActive(2, true),
                    EditorCommand::SetFuel(2, 3.5),
                    EditorCommand::SetStaging(1, Some(staging)),
                ]),
            )
            .unwrap();
        let after = ship.clone();
        assert_eq!(
            crate::ship_from_xml(&crate::ship_to_xml(&ship).unwrap()).unwrap(),
            ship
        );
        assert!(history.undo(&mut ship));
        assert_eq!(ship, before);
        assert!(!history.can_undo());
        assert!(history.redo(&mut ship));
        assert_eq!(ship, after);
    }

    #[test]
    fn invalid_properties_do_not_mutate_ship_or_clear_redo() {
        use crate::{Activation, FuelKind, PodState, StageStep};
        let mut pod = part(1);
        pod.pod = Some(PodState::default());
        let mut tank = part(2);
        tank.fuel_kind = Some(FuelKind::Tank);
        tank.fuel = Some(10.0);
        let mut ship = Ship {
            parts: vec![pod, tank],
            ..Ship::default()
        };
        let mut history = EditorHistory::default();
        history
            .execute(&mut ship, EditorCommand::SetActive(2, true))
            .unwrap();
        history.undo(&mut ship);
        let before = ship.clone();
        for invalid in [
            EditorCommand::SetFuel(2, f64::NAN),
            EditorCommand::SetFuel(2, -1.0),
            EditorCommand::SetFuel(1, 1.0),
            EditorCommand::SetThrottle(1, f64::INFINITY),
            EditorCommand::SetThrottle(1, 1.1),
            EditorCommand::RenamePod(2, "不是驾驶舱".into()),
            EditorCommand::SetStaging(
                1,
                Some(StagingState {
                    current_stage: -1,
                    steps: vec![],
                }),
            ),
            EditorCommand::SetStaging(
                1,
                Some(StagingState {
                    current_stage: 0,
                    steps: vec![StageStep {
                        activations: vec![Activation {
                            id: 99,
                            moved: false,
                        }],
                    }],
                }),
            ),
            EditorCommand::SetStaging(
                1,
                Some(StagingState {
                    current_stage: 0,
                    steps: vec![StageStep {
                        activations: vec![
                            Activation {
                                id: 2,
                                moved: false
                            };
                            2
                        ],
                    }],
                }),
            ),
        ] {
            assert!(
                history
                    .execute(
                        &mut ship,
                        EditorCommand::Batch(vec![
                            EditorCommand::RenamePod(1, "不应保留".into()),
                            invalid,
                        ])
                    )
                    .is_err()
            );
            assert_eq!(ship, before);
            assert!(history.can_redo());
        }
    }
}
