use super::*;
use crate::{Activation, PodState, ShipGroup, StageStep};

fn ship() -> Ship {
    let mut pod = super::tests::part(1);
    pod.pod = Some(PodState {
        staging: Some(StagingState {
            current_stage: 3,
            steps: vec![StageStep {
                activations: vec![
                    Activation { id: 1, moved: true },
                    Activation {
                        id: 1,
                        moved: false,
                    },
                ],
            }],
        }),
        ..Default::default()
    });
    let mut second = super::tests::part(1);
    second.x = 5.0;
    Ship {
        parts: vec![pod, second, super::tests::part(2)],
        connections: vec![
            Connection::Normal {
                parent: 1,
                child: 1,
                parent_attach: 2,
                child_attach: 3,
            },
            Connection::Dock {
                parent: 2,
                child: 1,
                dock: 1,
            },
        ],
        disconnected: vec![ShipGroup {
            parts: vec![super::tests::part(1)],
            connections: vec![],
        }],
        ..Default::default()
    }
}

#[test]
fn each_reference_is_assigned_explicitly_and_every_other_field_roundtrips() {
    let mut ship = ship();
    let before = ship.clone();
    let mut repair = DuplicateRepair::new(&ship, PartKey::new(0, 1, 1)).unwrap();
    assert_eq!(repair.unassigned(), 6);
    let new_id = repair.new_ids()[1];
    for (index, target) in [0, 1, 1, 1, 1, 0].into_iter().enumerate() {
        repair.assign(index, Some(target)).unwrap();
    }
    let mut history = EditorHistory::default();
    history
        .execute(&mut ship, EditorCommand::RepairDuplicates(Box::new(repair)))
        .unwrap();
    let mut expected = before.clone();
    expected.parts[1].id = new_id;
    expected.connections[0] = Connection::Normal {
        parent: 1,
        child: new_id,
        parent_attach: 2,
        child_attach: 3,
    };
    expected.connections[1] = Connection::Dock {
        parent: 2,
        child: new_id,
        dock: new_id,
    };
    expected.parts[0]
        .pod
        .as_mut()
        .unwrap()
        .staging
        .as_mut()
        .unwrap()
        .steps[0]
        .activations[0]
        .id = new_id;
    assert_eq!(ship, expected);
    assert_eq!(
        crate::ship_from_xml(&crate::ship_to_xml(&ship).unwrap()).unwrap(),
        expected
    );
    assert!(history.undo(&mut ship));
    assert_eq!(ship, before);
    assert!(history.redo(&mut ship));
    assert_eq!(ship, expected);
}

#[test]
fn incomplete_or_stale_repair_preserves_document_and_redo() {
    let mut ship = ship();
    let mut history = EditorHistory::default();
    history
        .execute(&mut ship, EditorCommand::SetActive(2, true))
        .unwrap();
    history.undo(&mut ship);
    let before = ship.clone();
    let mut repair = DuplicateRepair::new(&ship, PartKey::new(0, 1, 0)).unwrap();
    assert!(repair.assign(0, Some(2)).is_err());
    assert!(repair.assign(99, Some(0)).is_err());
    repair.assign(0, Some(1)).unwrap();
    assert!(
        history
            .execute(
                &mut ship,
                EditorCommand::RepairDuplicates(Box::new(repair.clone()))
            )
            .is_err()
    );
    assert_eq!(ship, before);
    assert!(history.can_redo());
    for index in 0..repair.references().len() {
        repair.assign(index, Some(0)).unwrap();
    }
    ship.parts[2].x = 99.0;
    let changed = ship.clone();
    assert!(
        EditorCommand::RepairDuplicates(Box::new(repair))
            .apply(&mut ship)
            .is_err()
    );
    assert_eq!(ship, changed);
}

#[test]
fn reserved_dangling_ids_are_skipped_and_concurrent_collisions_are_rejected() {
    let mut ship = ship();
    ship.disconnected[0].connections.push(Connection::Dock {
        parent: 100,
        child: 101,
        dock: 102,
    });
    let mut repair = DuplicateRepair::new(&ship, PartKey::new(0, 1, 0)).unwrap();
    assert!(repair.new_ids()[1] > 102);
    for index in 0..repair.references().len() {
        repair.assign(index, Some(0)).unwrap();
    }
    ship.disconnected[0]
        .parts
        .push(super::tests::part(repair.new_ids()[1]));
    let before = ship.clone();
    assert!(
        EditorCommand::RepairDuplicates(Box::new(repair))
            .apply(&mut ship)
            .is_err()
    );
    assert_eq!(ship, before);
}

#[test]
fn three_unreferenced_instances_become_independent_without_touching_another_group() {
    let mut ship = Ship {
        parts: vec![super::tests::part(1); 3],
        disconnected: vec![ShipGroup {
            parts: vec![super::tests::part(1)],
            connections: vec![],
        }],
        ..Default::default()
    };
    let repair = DuplicateRepair::new(&ship, PartKey::new(0, 1, 2)).unwrap();
    assert!(repair.references().is_empty());
    let ids = repair.new_ids().to_vec();
    EditorCommand::RepairDuplicates(Box::new(repair))
        .apply(&mut ship)
        .unwrap();
    assert_eq!(
        ship.parts.iter().map(|part| part.id).collect::<Vec<_>>(),
        ids
    );
    assert_eq!(ship.disconnected[0].parts[0].id, 1);
    EditorCommand::Delete(ids[1]).apply(&mut ship).unwrap();
    assert_eq!(ship.parts.len(), 2);
    assert!(DuplicateRepair::new(&ship, PartKey::new(0, 1, 0)).is_err());
}
