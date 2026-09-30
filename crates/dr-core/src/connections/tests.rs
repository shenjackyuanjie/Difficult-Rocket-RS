use super::*;
use crate::{EditorCommand, EditorHistory};

fn catalog() -> PartCatalog {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("PartList.xml");
    std::fs::write(&path, r#"<PartTypes>
      <PartType id="beam" width="8" height="2"><AttachPoints>
        <AttachPoint location="Top"/><AttachPoint location="BottomSide"/>
      </AttachPoints></PartType>
      <PartType id="small" width="2" height="2"><AttachPoints>
        <AttachPoint location="TopCenter"/><AttachPoint location="BottomCenter"/>
        <AttachPoint location="LeftCenter" group="7"/><AttachPoint location="RightCenter" group="7" flipX="true" flipY="true"/>
      </AttachPoints></PartType>
      <PartType id="plug" type="dockconnector" width="2" height="2"><AttachPoints>
        <AttachPoint location="TopCenter" dock="true"/>
      </AttachPoints></PartType>
      <PartType id="port" type="dockport" width="2" height="2"><AttachPoints>
        <AttachPoint location="BottomCenter" dock="true"/>
      </AttachPoints></PartType>
    </PartTypes>"#).unwrap();
    crate::load_catalog(path).unwrap()
}

fn connection(parent: i64, child: i64, pa: i32, ca: i32) -> Connection {
    Connection::Normal {
        parent,
        child,
        parent_attach: pa,
        child_attach: ca,
    }
}

#[test]
fn distant_rejection_preserves_offset_points_and_rotated_surface_ends() {
    let catalog = catalog();
    let mut kind = catalog.get("small").unwrap().clone();
    kind.attach_points.truncate(1);
    kind.attach_points[0].location.clear();
    kind.attach_points[0].x = 100.0;
    kind.attach_points[0].y = 0.0;
    for angle in [0.0_f64, 0.37, std::f64::consts::FRAC_PI_2] {
        let mut source = kind.instantiate(1, (0.0, 0.0));
        source.angle = angle;
        let mut target = kind.instantiate(2, (100.0 * angle.cos(), 100.0 * angle.sin()));
        target.angle = angle;
        target.flip_x = true;
        assert_eq!(candidates(&source, &kind, &target, &kind, 0.001).len(), 1);
        target.x += 1000.0;
        assert!(candidates(&source, &kind, &target, &kind, 0.35).is_empty());
    }
    let beam = catalog.get("beam").unwrap();
    let small = catalog.get("small").unwrap();
    for angle in [0.0_f64, 0.37, std::f64::consts::FRAC_PI_2] {
        let mut source = beam.instantiate(1, (0.0, 0.0));
        source.angle = angle;
        let mut target = small.instantiate(
            2,
            (
                2.0 * angle.cos() - angle.sin(),
                2.0 * angle.sin() + angle.cos(),
            ),
        );
        target.angle = angle;
        assert!(!candidates(&source, beam, &target, small, 0.001).is_empty());
    }
}

#[test]
fn occupancy_only_uses_the_target_group_and_does_not_ignore_a_foreign_duplicate() {
    use crate::{PartKey, ShipGroup};
    let catalog = catalog();
    let kind = catalog.get("small").unwrap();
    let source = kind.instantiate(2, (0.0, 1.0));
    let target = kind.instantiate(1, (0.0, 0.0));
    let mut ship = Ship {
        parts: vec![target.clone()],
        disconnected: vec![ShipGroup {
            parts: vec![kind.instantiate(1, (5.0, 0.0)), source.clone()],
            connections: vec![connection(1, 2, 1, 2)],
        }],
        ..Default::default()
    };
    let candidate = candidates(&source, kind, &target, kind, 0.01)[0];
    assert!(available_scoped(
        &ship,
        &catalog,
        &source,
        kind,
        Some(PartKey::new(1, 2, 0)),
        &target,
        kind,
        PartKey::new(0, 1, 0),
        &candidate
    ));
    ship.parts.push(kind.instantiate(2, (0.0, 1.0)));
    ship.connections.push(connection(1, 2, 1, 2));
    assert!(!available_scoped(
        &ship,
        &catalog,
        &source,
        kind,
        Some(PartKey::new(1, 2, 0)),
        &target,
        kind,
        PartKey::new(0, 1, 0),
        &candidate
    ));
}

#[test]
fn scoped_docking_remaps_equal_endpoint_ids_and_rejects_bad_merges_atomically() {
    use crate::{LinkKind, PartKey, ShipGroup};
    let catalog = catalog();
    let port = PartKey::new(0, 1, 0);
    let plug = PartKey::new(1, 1, 0);
    let mut ship = Ship {
        parts: vec![catalog.get("port").unwrap().instantiate(1, (0.0, 1.0))],
        disconnected: vec![ShipGroup {
            parts: vec![catalog.get("plug").unwrap().instantiate(1, (0.0, 0.0))],
            connections: vec![],
        }],
        ..Default::default()
    };
    let original = ship.clone();
    let mut history = EditorHistory::default();
    history
        .execute_with_catalog(
            &mut ship,
            &catalog,
            EditorCommand::ConnectParts {
                parent: port,
                child: plug,
                kind: LinkKind::Dock { connector: plug },
            },
        )
        .unwrap();
    assert!(ship.disconnected.is_empty());
    assert_eq!(
        ship.connections,
        vec![Connection::Dock {
            parent: 1,
            child: ship.parts[1].id,
            dock: ship.parts[1].id
        }]
    );
    assert!(history.undo(&mut ship));
    assert_eq!(ship, original);
    assert!(
        history
            .execute_with_catalog(
                &mut ship,
                &catalog,
                EditorCommand::ConnectParts {
                    parent: port,
                    child: plug,
                    kind: LinkKind::Normal {
                        parent_attach: 99,
                        child_attach: 1
                    },
                }
            )
            .is_err()
    );
    assert_eq!(ship, original);
    assert!(history.can_redo());
}

#[test]
fn surface_snap_retains_tangent_and_allows_separate_contacts() {
    let catalog = catalog();
    let beam = catalog.get("beam").unwrap();
    let small = catalog.get("small").unwrap();
    let target = beam.instantiate(1, (0.0, 0.0));
    let mut source = small.instantiate(2, (1.5, 1.1));
    let candidate = candidates(&source, small, &target, beam, 0.35)[0];
    assert_eq!(candidate.source_index, 1);
    assert_eq!(candidate.target_index, 0);
    assert!((candidate.position.x - 1.5).abs() < EPSILON);
    assert!((candidate.position.y - 1.0).abs() < EPSILON);
    source.y = candidate.position.y;
    let mut ship = Ship {
        parts: vec![target, source],
        ..Ship::default()
    };
    let mut history = EditorHistory::default();
    history
        .execute_with_catalog(
            &mut ship,
            &catalog,
            EditorCommand::Connect(connection(1, 2, 1, 2)),
        )
        .unwrap();
    history
        .execute_with_catalog(
            &mut ship,
            &catalog,
            EditorCommand::Batch(vec![
                EditorCommand::Place(small.instantiate(3, (-1.5, 1.0)).into()),
                EditorCommand::Connect(connection(1, 3, 1, 2)),
            ]),
        )
        .unwrap();
    assert_eq!(ship.connections.len(), 2);
    let (a, b) = positions(&ship, &catalog, &ship.connections[1]).unwrap();
    assert_eq!(a, Vec2d { x: -1.5, y: 0.5 });
    assert_eq!(a, b);
    assert!(history.undo(&mut ship));
    assert_eq!(ship.parts.len(), 2);
    assert_eq!(ship.connections.len(), 1);
}

#[test]
fn fixed_point_and_shared_group_reject_reuse_atomically() {
    let catalog = catalog();
    let small = catalog.get("small").unwrap();
    let mut ship = Ship {
        parts: vec![
            small.instantiate(1, (0.0, 0.0)),
            small.instantiate(2, (1.0, 0.0)),
        ],
        ..Ship::default()
    };
    let mut history = EditorHistory::default();
    history
        .execute_with_catalog(
            &mut ship,
            &catalog,
            EditorCommand::Connect(connection(1, 2, 4, 3)),
        )
        .unwrap();
    let before = ship.clone();
    // 使用另一侧连接点，仍被 group=7 的现有连接占用。
    assert!(
        history
            .execute_with_catalog(
                &mut ship,
                &catalog,
                EditorCommand::Batch(vec![
                    EditorCommand::Place(small.instantiate(3, (-1.0, 0.0)).into()),
                    EditorCommand::Connect(connection(1, 3, 3, 4)),
                ])
            )
            .is_err()
    );
    assert_eq!(ship, before);
    assert!(history.undo(&mut ship));
    assert!(history.can_redo());
    assert!(
        history
            .execute_with_catalog(
                &mut ship,
                &catalog,
                EditorCommand::Connect(connection(1, 2, 99, 3))
            )
            .is_err()
    );
    assert!(history.can_redo());
    // 两个部件引用同一个固定中心点也不允许重复占用。
    ship.parts.push(small.instantiate(3, (0.0, 1.0)));
    history
        .execute_with_catalog(
            &mut ship,
            &catalog,
            EditorCommand::Connect(connection(1, 3, 1, 2)),
        )
        .unwrap();
    let before = ship.clone();
    assert!(
        history
            .execute_with_catalog(
                &mut ship,
                &catalog,
                EditorCommand::Batch(vec![
                    EditorCommand::Place(small.instantiate(4, (0.0, 1.0)).into()),
                    EditorCommand::Connect(connection(1, 4, 1, 2)),
                ])
            )
            .is_err()
    );
    assert_eq!(ship, before);
}

#[test]
fn moving_source_can_reuse_its_own_connection_but_not_another_contact() {
    let catalog = catalog();
    let beam = catalog.get("beam").unwrap();
    let small = catalog.get("small").unwrap();
    let source = small.instantiate(2, (1.0, 1.0));
    let target = beam.instantiate(1, (0.0, 0.0));
    let candidate = candidates(&source, small, &target, beam, 0.35)[0];
    let ship = Ship {
        parts: vec![target.clone(), source.clone()],
        connections: vec![connection(1, 2, 1, 2)],
        ..Ship::default()
    };
    assert!(available(
        &ship,
        &catalog,
        &source,
        small,
        &target,
        beam,
        &candidate,
        Some(2)
    ));
    let mut another = source.clone();
    another.id = 3;
    assert!(!available(
        &ship,
        &catalog,
        &another,
        small,
        &target,
        beam,
        &candidate,
        Some(3)
    ));
}

#[test]
fn directions_follow_rotation_mirrors_and_docking_requires_matching_types() {
    let catalog = catalog();
    let small = catalog.get("small").unwrap();
    assert!(small.attach_points[3].flip_x && small.attach_points[3].flip_y);
    let source = small.instantiate(1, (0.0, 0.0));
    let mut target = small.instantiate(2, (0.0, 1.0));
    assert!(compatible(
        &source,
        small,
        &small.attach_points[0],
        &target,
        small,
        &small.attach_points[1]
    ));
    assert!(!compatible(
        &source,
        small,
        &small.attach_points[0],
        &target,
        small,
        &small.attach_points[0]
    ));
    target.flip_y = true;
    assert!(compatible(
        &source,
        small,
        &small.attach_points[0],
        &target,
        small,
        &small.attach_points[0]
    ));
    target.flip_y = false;
    target.angle = std::f64::consts::FRAC_PI_2;
    assert!(compatible(
        &source,
        small,
        &small.attach_points[0],
        &target,
        small,
        &small.attach_points[2]
    ));
    let plug = catalog.get("plug").unwrap();
    let port = catalog.get("port").unwrap();
    let source = plug.instantiate(3, (0.0, 0.0));
    let target = port.instantiate(4, (0.0, 1.0));
    assert!(candidates(&source, plug, &target, port, 0.35)[0].dock);
    assert!(
        candidates(
            &source,
            plug,
            &small.instantiate(5, (0.0, 1.0)),
            small,
            0.35
        )
        .is_empty()
    );
    let ship = Ship {
        parts: vec![source, target],
        ..Ship::default()
    };
    assert!(
        validate(
            &ship,
            &catalog,
            &Connection::Dock {
                dock: 3,
                parent: 3,
                child: 4
            }
        )
        .is_ok()
    );
    let mut distant = ship.clone();
    distant.parts[1].y = 10.0;
    assert!(
        validate(
            &distant,
            &catalog,
            &Connection::Dock {
                dock: 3,
                parent: 3,
                child: 4
            }
        )
        .is_err()
    );
}

#[test]
fn surface_geometry_handles_rotated_edges_and_point_intersections() {
    let catalog = catalog();
    let beam = catalog.get("beam").unwrap();
    let mut part = beam.instantiate(1, (2.0, 3.0));
    part.angle = std::f64::consts::FRAC_PI_2;
    let (a, b) = segment(&part, beam, &beam.attach_points[0]);
    assert!(a.distance(Vec2d { x: 1.5, y: 1.0 }) < EPSILON);
    assert!(b.distance(Vec2d { x: 1.5, y: 5.0 }) < EPSILON);
    let (a, b) = closest_points(
        (Vec2d { x: -1.0, y: 0.0 }, Vec2d { x: 1.0, y: 0.0 }),
        (Vec2d { x: 0.0, y: -1.0 }, Vec2d { x: 0.0, y: 1.0 }),
    );
    assert_eq!(a, Vec2d::default());
    assert_eq!(a, b);
}
