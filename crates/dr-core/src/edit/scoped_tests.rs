use super::*;
use crate::{Activation, PodState, ShipGroup, StageStep};

fn part(id: i64) -> Part {
    super::tests::part(id)
}

fn connection(parent: i64, child: i64) -> Connection {
    Connection::Normal {
        parent,
        child,
        parent_attach: 1,
        child_attach: 2,
    }
}

fn pod(id: i64, target: i64) -> Part {
    let mut part = part(id);
    part.pod = Some(PodState {
        staging: Some(StagingState {
            current_stage: 0,
            steps: vec![StageStep {
                activations: vec![Activation {
                    id: target,
                    moved: true,
                }],
            }],
        }),
        ..Default::default()
    });
    part
}

fn repeated_groups() -> Ship {
    Ship {
        parts: vec![pod(1, 2), part(2)],
        connections: vec![connection(1, 2)],
        disconnected: vec![ShipGroup {
            parts: vec![pod(1, 2), part(2)],
            connections: vec![Connection::Dock {
                parent: 1,
                child: 2,
                dock: 2,
            }],
        }],
        ..Default::default()
    }
}

fn link(parent: PartKey, child: PartKey) -> EditorCommand {
    EditorCommand::ConnectParts {
        parent,
        child,
        kind: LinkKind::Normal {
            parent_attach: 2,
            child_attach: 1,
        },
    }
}

#[test]
fn duplicate_global_ids_require_scope_and_failure_preserves_redo() {
    let mut ship = repeated_groups();
    let original = ship.clone();
    let mut history = EditorHistory::default();
    history
        .execute(
            &mut ship,
            EditorCommand::SetActive(2, true).at(PartKey::new(1, 2, 0)),
        )
        .unwrap();
    assert!(!ship.parts[1].active);
    assert!(ship.disconnected[0].parts[1].active);
    assert!(history.undo(&mut ship));
    assert_eq!(ship, original);
    for command in [
        EditorCommand::SetActive(2, true),
        EditorCommand::Delete(2),
        EditorCommand::Disconnect(2),
        EditorCommand::Connect(connection(1, 2)),
    ] {
        assert!(matches!(
            history.execute(&mut ship, command),
            Err(CommandError::AmbiguousPart(_))
        ));
        assert_eq!(ship, original);
        assert!(history.can_redo());
    }
    assert!(ship.part(2).is_none());
    assert!(ship.part_mut(2).is_none());
    assert!(ship.remove_part(2).is_none());
    ship.disconnect_part(2);
    assert_eq!(ship, original);
}

#[test]
fn scoped_delete_only_cleans_its_own_connections_and_staging() {
    let mut ship = repeated_groups();
    let before = ship.clone();
    let mut history = EditorHistory::default();
    history
        .execute(
            &mut ship,
            EditorCommand::Delete(2).at(PartKey::new(1, 2, 0)),
        )
        .unwrap();
    assert_eq!(ship.parts, before.parts);
    assert_eq!(ship.connections, before.connections);
    let group = &ship.disconnected[0];
    assert_eq!(group.parts.len(), 1);
    assert!(group.connections.is_empty());
    assert!(
        group.parts[0]
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
fn occurrence_edits_are_independent_and_ambiguous_references_are_atomic() {
    let mut ship = Ship {
        parts: vec![part(1), part(1), pod(2, 1)],
        connections: vec![connection(1, 2)],
        ..Default::default()
    };
    let key = PartKey::new(0, 1, 1);
    EditorCommand::SetActive(1, true)
        .at(key)
        .apply(&mut ship)
        .unwrap();
    assert!(!ship.parts[0].active);
    assert!(ship.parts[1].active);
    let before = ship.clone();
    for command in [
        EditorCommand::Delete(1).at(key),
        EditorCommand::Disconnect(1).at(key),
        link(key, PartKey::new(0, 2, 0)),
    ] {
        assert!(matches!(
            command.apply(&mut ship),
            Err(CommandError::AmbiguousReference(_))
        ));
        assert_eq!(ship, before);
    }
    ship.connections.clear();
    ship.parts[2].pod = None;
    EditorCommand::Delete(1).at(key).apply(&mut ship).unwrap();
    assert_eq!(ship.parts.len(), 2);
    assert_eq!(ship.parts[0], before.parts[0]);
}

#[test]
fn staging_cannot_resolve_a_target_from_another_group() {
    let mut ship = repeated_groups();
    ship.parts.push(part(99));
    let before = ship.clone();
    let staging = StagingState {
        current_stage: 0,
        steps: vec![StageStep {
            activations: vec![Activation {
                id: 99,
                moved: false,
            }],
        }],
    };
    assert!(matches!(
        EditorCommand::SetStaging(1, Some(staging))
            .at(PartKey::new(1, 1, 0))
            .apply(&mut ship),
        Err(CommandError::MissingPart(99))
    ));
    assert_eq!(ship, before);
}

#[test]
fn merging_groups_remaps_normal_dock_and_staging_without_touching_other_groups() {
    let mut ship = repeated_groups();
    ship.disconnected[0].connections.push(connection(1, 2));
    ship.disconnected.push(ship.disconnected[0].clone());
    let before = ship.clone();
    let mut history = EditorHistory::default();
    history
        .execute(
            &mut ship,
            link(PartKey::new(0, 1, 0), PartKey::new(1, 1, 0)),
        )
        .unwrap();
    assert_eq!(ship.parts.len(), 4);
    assert_eq!(&ship.parts[..2], before.parts.as_slice());
    assert_eq!(ship.disconnected, vec![before.disconnected[1].clone()]);
    let imported_pod = ship.parts[2].id;
    let imported_part = ship.parts[3].id;
    assert_ne!(imported_pod, 1);
    assert_ne!(imported_part, 2);
    assert!(ship.connections.contains(&Connection::Dock {
        parent: imported_pod,
        child: imported_part,
        dock: imported_part
    }));
    assert!(
        ship.connections
            .contains(&connection(imported_pod, imported_part))
    );
    assert_eq!(
        ship.parts[2]
            .pod
            .as_ref()
            .unwrap()
            .staging
            .as_ref()
            .unwrap()
            .steps[0]
            .activations[0],
        Activation {
            id: imported_part,
            moved: true
        }
    );
    let after = ship.clone();
    assert_eq!(
        crate::ship_from_xml(&crate::ship_to_xml(&ship).unwrap()).unwrap(),
        ship
    );
    assert!(history.undo(&mut ship));
    assert_eq!(ship, before);
    assert!(history.redo(&mut ship));
    assert_eq!(ship, after);
}

#[test]
fn merged_dangling_references_do_not_bind_to_parts_from_the_other_group() {
    let mut ship = Ship {
        parts: vec![pod(1, 2), part(4)],
        disconnected: vec![ShipGroup {
            parts: vec![pod(2, 4)],
            connections: vec![],
        }],
        ..Default::default()
    };
    link(PartKey::new(0, 1, 0), PartKey::new(1, 2, 0))
        .apply(&mut ship)
        .unwrap();
    let incoming = &ship.parts[2];
    assert_ne!(incoming.id, 2);
    let target = incoming
        .pod
        .as_ref()
        .unwrap()
        .staging
        .as_ref()
        .unwrap()
        .steps[0]
        .activations[0]
        .id;
    assert_ne!(target, 4);
    assert!(ship.part(target).is_none());
    assert!(ship.part(2).is_none());
}

#[test]
fn empty_groups_are_compacted_after_batch_without_retargeting_later_commands() {
    let mut ship = Ship {
        disconnected: vec![
            ShipGroup {
                parts: vec![part(1)],
                connections: vec![],
            },
            ShipGroup {
                parts: vec![part(1)],
                connections: vec![],
            },
            ShipGroup::default(),
        ],
        ..Default::default()
    };
    EditorCommand::Batch(vec![
        EditorCommand::Delete(1).at(PartKey::new(1, 1, 0)),
        EditorCommand::SetActive(1, true).at(PartKey::new(2, 1, 0)),
    ])
    .apply(&mut ship)
    .unwrap();
    assert_eq!(ship.disconnected.len(), 2);
    assert!(ship.disconnected[0].parts[0].active);
    assert!(ship.disconnected[1].parts.is_empty());
}

#[test]
fn joining_disconnected_groups_preserves_main_and_later_scoped_edits() {
    let mut ship = repeated_groups();
    ship.disconnected.push(ship.disconnected[0].clone());
    ship.disconnected.push(ship.disconnected[0].clone());
    let before = ship.clone();
    EditorCommand::Batch(vec![
        link(PartKey::new(1, 1, 0), PartKey::new(2, 1, 0)),
        EditorCommand::SetActive(1, true).at(PartKey::new(3, 1, 0)),
    ])
    .apply(&mut ship)
    .unwrap();
    assert_eq!(ship.parts, before.parts);
    assert_eq!(ship.connections, before.connections);
    assert_eq!(ship.disconnected.len(), 2);
    assert_eq!(ship.disconnected[0].parts.len(), 4);
    assert!(ship.disconnected[1].parts[0].active);
}
