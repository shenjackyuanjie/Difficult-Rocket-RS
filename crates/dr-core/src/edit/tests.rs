use crate::edit::*;
#[test]
fn history_byte_budget_evicts_oldest_and_swaps_without_cloning() {
    let mut ship = Ship {
        parts: vec![part(1)],
        ..Ship::default()
    };
    let mut history = EditorHistory::with_limits(20, ship.retained_bytes() * 2);
    for x in 1..=4 {
        history
            .execute(
                &mut ship,
                EditorCommand::Move {
                    id: 1,
                    from: (0.0, 0.0),
                    to: (x as f64, 0.0),
                },
            )
            .unwrap();
    }
    assert_eq!(history.undo_len(), 2);
    let bytes = history.reserved_bytes();
    let current = ship.parts.as_ptr();
    let previous = history.undo.last().unwrap().snapshot.parts.as_ptr();
    assert!(history.undo(&mut ship));
    assert_eq!(ship.parts.as_ptr(), previous);
    assert_eq!(ship.parts[0].x, 3.0);
    assert_eq!(history.reserved_bytes(), bytes);
    assert!(history.undo(&mut ship));
    assert_eq!(ship.parts[0].x, 2.0);
    assert!(!history.undo(&mut ship));
    assert!(history.redo(&mut ship));
    assert!(history.redo(&mut ship));
    assert_eq!(ship.parts.as_ptr(), current);
    assert_eq!(ship.parts[0].x, 4.0);
    assert_eq!(history.reserved_bytes(), bytes);
    history.undo(&mut ship);
    history
        .execute(&mut ship, EditorCommand::SetActive(1, true))
        .unwrap();
    assert_eq!(history.redo_len(), 0);
    assert_eq!(history.undo_len(), 2);
}

#[test]
fn history_keeps_last_oversized_edit_and_counts_nested_staging_capacity() {
    let mut ship = Ship {
        parts: vec![part(1)],
        ..Ship::default()
    };
    let plain_bytes = ship.retained_bytes();
    ship.parts[0].pod = Some(crate::PodState {
        name: "长名称".repeat(100),
        staging: Some(StagingState {
            steps: vec![crate::StageStep {
                activations: vec![
                    crate::Activation {
                        id: 1,
                        moved: false
                    };
                    500
                ],
            }],
            ..Default::default()
        }),
        ..Default::default()
    });
    assert!(
        ship.retained_bytes()
            >= plain_bytes + 100 * "长名称".len() + 500 * std::mem::size_of::<crate::Activation>()
    );
    let original = ship.clone();
    let mut history = EditorHistory::with_limits(1, 1);
    history
        .execute(&mut ship, EditorCommand::SetActive(1, true))
        .unwrap();
    assert_eq!(history.undo_len(), 1);
    assert!(history.reserved_bytes() > 1);
    history.undo(&mut ship);
    assert_eq!(ship, original);
    let bytes = history.reserved_bytes();
    assert!(
        history
            .execute(&mut ship, EditorCommand::Delete(999))
            .is_err()
    );
    history
        .execute(
            &mut ship,
            EditorCommand::SetActive(1, original.parts[0].active),
        )
        .unwrap();
    assert_eq!(history.reserved_bytes(), bytes);
    assert!(history.can_redo());
}

pub fn part(id: i64) -> Part {
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
