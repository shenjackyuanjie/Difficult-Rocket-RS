use crate::edit::*;
use crate::{ShipGroup, ship_from_xml, ship_to_xml};

fn boundary_ship(maximum: i64) -> Ship {
    ship_from_xml(&format!(
        r#"<Ship><Parts><Part id="1"/><Part id="2"/><Part id="{maximum}"/></Parts>
        <Connections>
          <Connection parentPart="3" childPart="4" parentAttachPoint="1" childAttachPoint="1"/>
          <DockConnection parentPart="6" childPart="7" dockPart="5"/>
        </Connections><DisconnectedParts><DisconnectedPart><Parts>
          <Part id="8"><Pod><Staging><Step><Activate Id="9"/></Step></Staging></Pod></Part>
        </Parts></DisconnectedPart></DisconnectedParts></Ship>"#
    ))
    .unwrap()
}

fn source_ship() -> Ship {
    ship_from_xml(
        r#"<Ship><Parts>
          <Part id="1"><Pod><Staging><Step><Activate Id="2" moved="1"/></Step></Staging></Pod></Part>
          <Part id="2" y="1"/>
        </Parts><Connections>
          <Connection parentPart="1" childPart="2" parentAttachPoint="1" childAttachPoint="2"/>
        </Connections></Ship>"#,
    )
    .unwrap()
}

fn fragment() -> ShipFragment {
    ShipFragment::capture(
        &source_ship(),
        &[PartKey::new(0, 1, 0), PartKey::new(0, 2, 0)],
    )
    .unwrap()
}

fn assert_staging_target(part: &Part, expected: i64) {
    let activation = &part.pod.as_ref().unwrap().staging.as_ref().unwrap().steps[0].activations[0];
    assert_eq!(activation.id, expected);
    assert!(activation.moved);
}

#[test]
fn next_part_id_wraps_at_upper_boundary_without_capturing_dangling_references() {
    let mut ship = boundary_ship(i64::MAX - 1);
    assert_eq!(ship.next_part_id(), i64::MAX);
    EditorCommand::Place(Box::new(super::tests::part(i64::MAX)))
        .apply(&mut ship)
        .unwrap();
    assert_eq!(ship.next_part_id(), 10);

    // 上界可来自实际部件或任一类历史引用，预留范围与普通路径保持一致。
    for source in [
        format!(r#"<Parts><Part id="{}"/></Parts>"#, i64::MAX),
        format!(
            r#"<Connections><Connection parentPart="{}" childPart="1" parentAttachPoint="1" childAttachPoint="1"/></Connections>"#,
            i64::MAX
        ),
        format!(
            r#"<Connections><DockConnection parentPart="1" childPart="1" dockPart="{}"/></Connections>"#,
            i64::MAX
        ),
        format!(
            r#"<Parts><Part id="1"><Pod><Staging><Step><Activate Id="{}"/></Step></Staging></Pod></Part></Parts>"#,
            i64::MAX
        ),
    ] {
        let ship = ship_from_xml(&format!("<Ship>{source}</Ship>")).unwrap();
        assert!(ship.next_part_id() > 0);
        assert_ne!(ship.next_part_id(), i64::MAX);
    }
}

#[test]
fn paste_allocates_multiple_ids_across_upper_boundary_and_roundtrips_history() {
    for (maximum, expected) in [(i64::MAX - 1, [i64::MAX, 10]), (i64::MAX, [10, 11])] {
        let mut ship = boundary_ship(maximum);
        let before = ship.clone();
        let mut history = EditorHistory::default();
        history
            .execute(
                &mut ship,
                EditorCommand::Paste {
                    fragment: Box::new(fragment()),
                    offset: (10.0, 0.0),
                },
            )
            .unwrap();
        let pasted = &ship.disconnected[1];
        assert_eq!(
            pasted.parts.iter().map(|part| part.id).collect::<Vec<_>>(),
            expected
        );
        assert_staging_target(&pasted.parts[0], expected[1]);
        assert_eq!(
            pasted.connections,
            vec![Connection::Normal {
                parent: expected[0],
                child: expected[1],
                parent_attach: 1,
                child_attach: 2,
            }]
        );
        assert_eq!(ship.parts, before.parts);
        assert_eq!(ship.connections, before.connections);
        assert_eq!(ship.disconnected[0], before.disconnected[0]);
        let after = ship.clone();
        assert_eq!(ship_from_xml(&ship_to_xml(&ship).unwrap()).unwrap(), after);
        assert!(history.undo(&mut ship));
        assert_eq!(ship, before);
        assert!(history.redo(&mut ship));
        assert_eq!(ship, after);
    }
}

#[test]
fn duplicate_repair_allocates_multiple_ids_across_upper_boundary() {
    let mut ship = boundary_ship(i64::MAX - 1);
    ship.parts
        .extend([super::tests::part(2), super::tests::part(2)]);
    let before = ship.clone();
    let repair = DuplicateRepair::new(&ship, PartKey::new(0, 2, 2)).unwrap();
    assert!(repair.references().is_empty());
    assert_eq!(repair.new_ids(), [2, i64::MAX, 10]);
    let mut history = EditorHistory::default();
    history
        .execute(&mut ship, EditorCommand::RepairDuplicates(Box::new(repair)))
        .unwrap();
    assert_eq!(
        ship.parts.iter().map(|part| part.id).collect::<Vec<_>>(),
        [1, 2, i64::MAX - 1, i64::MAX, 10]
    );
    assert_eq!(ship.connections, before.connections);
    assert_eq!(ship.disconnected, before.disconnected);
    let after = ship.clone();
    assert!(history.undo(&mut ship));
    assert_eq!(ship, before);
    assert!(history.redo(&mut ship));
    assert_eq!(ship, after);
}

#[test]
fn group_merge_remaps_multiple_conflicts_across_upper_boundary() {
    let mut ship = boundary_ship(i64::MAX - 1);
    let source = source_ship();
    ship.disconnected.push(ShipGroup {
        parts: source.parts,
        connections: source.connections,
    });
    let before = ship.clone();
    let mut history = EditorHistory::default();
    history
        .execute(
            &mut ship,
            EditorCommand::ConnectParts {
                parent: PartKey::new(0, 1, 0),
                child: PartKey::new(2, 2, 0),
                kind: LinkKind::Normal {
                    parent_attach: 2,
                    child_attach: 1,
                },
            },
        )
        .unwrap();
    assert_eq!(
        ship.parts.iter().map(|part| part.id).collect::<Vec<_>>(),
        [1, 2, i64::MAX - 1, i64::MAX, 10]
    );
    assert_staging_target(&ship.parts[3], 10);
    assert_eq!(&ship.connections[..2], &before.connections);
    assert_eq!(
        &ship.connections[2..],
        &[
            Connection::Normal {
                parent: i64::MAX,
                child: 10,
                parent_attach: 1,
                child_attach: 2,
            },
            Connection::Normal {
                parent: 1,
                child: 10,
                parent_attach: 2,
                child_attach: 1,
            },
        ]
    );
    assert_eq!(ship.disconnected, before.disconnected[..1]);
    let after = ship.clone();
    assert!(history.undo(&mut ship));
    assert_eq!(ship, before);
    assert!(history.redo(&mut ship));
    assert_eq!(ship, after);
}

#[test]
fn boundary_allocation_is_rolled_back_when_a_later_batch_command_fails() {
    let mut ship = boundary_ship(i64::MAX - 1);
    let mut history = EditorHistory::default();
    history
        .execute(&mut ship, EditorCommand::SetActive(1, true))
        .unwrap();
    assert!(history.undo(&mut ship));
    let before = ship.clone();
    let bytes = history.reserved_bytes();
    assert!(matches!(
        history.execute(
            &mut ship,
            EditorCommand::Batch(vec![
                EditorCommand::Paste {
                    fragment: Box::new(fragment()),
                    offset: (10.0, 0.0),
                },
                EditorCommand::Delete(42),
            ])
        ),
        Err(CommandError::MissingPart(42))
    ));
    assert_eq!(ship, before);
    assert_eq!(history.undo_len(), 0);
    assert_eq!(history.redo_len(), 1);
    assert_eq!(history.reserved_bytes(), bytes);
    assert!(history.redo(&mut ship));
    assert!(ship.parts[0].active);
}

#[test]
fn single_rotate_accepts_extreme_xml_editor_angles_and_preserves_history() {
    for editor_angle in [i32::MIN, i32::MIN + 1, -5, -1, 0, 3, 4, i32::MAX] {
        let mut ship = ship_from_xml(&format!(
            r#"<Ship><Parts><Part id="1" angle="0.3" editorAngle="{editor_angle}"/></Parts></Ship>"#
        ))
        .unwrap();
        let before = ship.clone();
        let mut expected = before.clone();
        EditorCommand::TransformSelection {
            parts: vec![PartKey::new(0, 1, 0)],
            transform: SelectionTransform::Rotate { center: (0.0, 0.0) },
        }
        .apply(&mut expected)
        .unwrap();
        let mut history = EditorHistory::default();
        history
            .execute(&mut ship, EditorCommand::Rotate(1))
            .unwrap();
        assert_eq!(ship, expected);
        assert_eq!(
            ship.parts[0].editor_angle,
            (editor_angle.rem_euclid(4) + 1).rem_euclid(4)
        );
        assert!(history.undo(&mut ship));
        assert_eq!(ship, before);
        assert!(history.redo(&mut ship));
        assert_eq!(ship, expected);
    }
}
