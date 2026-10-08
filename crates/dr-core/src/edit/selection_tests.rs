use super::*;
use crate::{Activation, PodState, ShipGroup, StageStep};

fn catalog() -> PartCatalog {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("目录.xml");
    std::fs::write(&path, r#"<PartTypes>
        <PartType id="pod" type="pod" width="2" height="2"><AttachPoints><AttachPoint location="TopCenter"/><AttachPoint location="BottomCenter"/></AttachPoints></PartType>
        <PartType id="tank" type="tank" width="2" height="2"><Tank fuel="100"/><AttachPoints><AttachPoint location="TopCenter"/><AttachPoint location="BottomCenter"/></AttachPoints></PartType>
        <PartType id="fixed" width="2" height="2" disableEditorRotation="true"/>
        <PartType id="limited" width="2" height="2" maxOccurrences="1"/>
    </PartTypes>"#).unwrap();
    crate::load_catalog(path).unwrap()
}

fn ship(catalog: &PartCatalog) -> Ship {
    let mut pod = catalog.get("pod").unwrap().instantiate(1, (0.0, 0.0));
    pod.pod = Some(PodState {
        name: "试验 & 船体".into(),
        throttle: 0.4,
        staging: Some(StagingState {
            current_stage: 1,
            steps: vec![StageStep {
                activations: vec![
                    Activation { id: 2, moved: true },
                    Activation {
                        id: 3,
                        moved: false,
                    },
                ],
            }],
        }),
    });
    Ship {
        parts: vec![
            pod,
            catalog.get("tank").unwrap().instantiate(2, (0.0, 1.0)),
            catalog.get("tank").unwrap().instantiate(3, (0.0, 2.0)),
        ],
        connections: vec![connection(1, 2), connection(2, 3)],
        ..Default::default()
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

fn pair() -> Vec<PartKey> {
    vec![PartKey::new(0, 1, 0), PartKey::new(0, 2, 0)]
}

#[test]
fn whole_selection_move_keeps_internal_links_and_detaches_only_its_boundary() {
    let catalog = catalog();
    let mut ship = ship(&catalog);
    let before = ship.clone();
    let mut history = EditorHistory::default();
    history
        .execute_with_catalog(
            &mut ship,
            &catalog,
            EditorCommand::TransformSelection {
                parts: pair(),
                transform: SelectionTransform::Translate { dx: 5.0, dy: -2.0 },
            },
        )
        .unwrap();
    assert_eq!(ship.connections, vec![connection(1, 2)]);
    assert_eq!(ship.parts[0].x, 5.0);
    assert_eq!(ship.parts[1].y, -1.0);
    assert_eq!(ship.parts[2], before.parts[2]);
    assert_eq!(ship.parts[0].pod, before.parts[0].pod);
    assert!(history.undo(&mut ship));
    assert_eq!(ship, before);
    assert!(history.redo(&mut ship));
    assert_eq!(ship.connections, vec![connection(1, 2)]);
}

#[test]
fn rotation_and_world_mirrors_preserve_attachment_geometry() {
    let catalog = catalog();
    let kind = catalog.get("pod").unwrap();
    let mut part = kind.instantiate(1, (2.0, 3.0));
    part.angle = 0.37;
    part.flip_x = true;
    let ship = Ship {
        parts: vec![part.clone()],
        ..Default::default()
    };
    let original = crate::part_world_attach(&part, &kind.attach_points[0]);
    for (transform, expected) in [
        (
            SelectionTransform::Rotate { center: (1.0, 1.0) },
            (2.0 - original.y, original.x),
        ),
        (
            SelectionTransform::FlipX { center: (1.0, 1.0) },
            (2.0 - original.x, original.y),
        ),
        (
            SelectionTransform::FlipY { center: (1.0, 1.0) },
            (original.x, 2.0 - original.y),
        ),
    ] {
        let preview =
            selection::preview(&ship, Some(&catalog), &[PartKey::new(0, 1, 0)], transform).unwrap();
        let actual = crate::part_world_attach(&preview[0].1, &kind.attach_points[0]);
        assert!((actual.x - expected.0).abs() < 1e-9);
        assert!((actual.y - expected.1).abs() < 1e-9);
    }
}

#[test]
fn compound_pose_rotates_every_attachment_and_edge_through_mirrors_and_translation() {
    let catalog = catalog();
    let mut kind = catalog.get("tank").unwrap().clone();
    kind.attach_points[0].location = "Top".into();
    let mut part = kind.instantiate(1, (2.3, -1.7));
    part.angle = 0.37;
    for original_x in [false, true] {
        for original_y in [false, true] {
            part.flip_x = original_x;
            part.flip_y = original_y;
            for mirror_x in [false, true] {
                for mirror_y in [false, true] {
                    for angle in [-2.17_f64, -0.1, 0.0, 0.63, std::f64::consts::FRAC_PI_2] {
                        let pose = SelectionPose {
                            pivot: (0.4, 0.8),
                            radians: angle,
                            flip_x: mirror_x,
                            flip_y: mirror_y,
                        };
                        let delta = (-2.0, 4.7);
                        let result =
                            selection::pose_part(&part, Some(&catalog), pose, delta).unwrap();
                        for attach in &kind.attach_points {
                            let before = crate::part_world_attach(&part, attach);
                            let after = crate::part_world_attach(&result, attach);
                            let expected = pose.point((before.x, before.y), delta);
                            assert!(
                                after.distance(crate::Vec2d {
                                    x: expected.0,
                                    y: expected.1
                                }) < 1e-9
                            );
                            let (a, b) = crate::connections::segment(&part, &kind, attach);
                            let (c, d) = crate::connections::segment(&result, &kind, attach);
                            for (before, after) in [(a, c), (b, d)] {
                                let expected = pose.point((before.x, before.y), delta);
                                assert!(
                                    after.distance(crate::Vec2d {
                                        x: expected.0,
                                        y: expected.1
                                    }) < 1e-9
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn free_pose_is_atomic_preserves_boundary_edges_and_still_checks_rotation_permissions() {
    let catalog = catalog();
    let mut ship = ship(&catalog);
    let original = ship.clone();
    let mut history = EditorHistory::default();
    let command = EditorCommand::FreeEdit(Box::new(EditorCommand::DragPoseSelection {
        parts: pair(),
        pose: SelectionPose {
            pivot: (0.0, 0.0),
            radians: 0.37,
            flip_x: true,
            flip_y: true,
        },
        delta: (0.0, 1.9),
    }));
    history
        .execute_with_catalog(&mut ship, &catalog, command)
        .unwrap();
    assert_eq!(ship.connections, original.connections);
    assert_ne!(ship.parts, original.parts);
    let edited = ship.clone();
    history.undo(&mut ship);
    assert_eq!(ship, original);
    history.redo(&mut ship);
    assert_eq!(ship, edited);
    ship.parts
        .push(catalog.get("fixed").unwrap().instantiate(4, (10.0, 0.0)));
    let before = ship.clone();
    assert!(
        history
            .execute_with_catalog(
                &mut ship,
                &catalog,
                EditorCommand::FreeEdit(Box::new(EditorCommand::TransformSelection {
                    parts: vec![PartKey::new(0, 4, 0)],
                    transform: SelectionTransform::RotateBy {
                        center: (0.0, 0.0),
                        radians: 0.2
                    }
                }))
            )
            .is_err()
    );
    assert_eq!(ship, before);
    let key = PartKey::new(0, 1, 0);
    assert!(
        history
            .execute_with_catalog(
                &mut ship,
                &catalog,
                EditorCommand::FreeEdit(Box::new(EditorCommand::DragPoseSelection {
                    parts: vec![key],
                    pose: SelectionPose {
                        radians: f64::NAN,
                        ..Default::default()
                    },
                    delta: (0.0, 0.0)
                }))
            )
            .is_err()
    );
    assert_eq!(ship, before);
}

#[test]
fn single_quarter_rotation_keeps_non_quarter_offset() {
    let catalog = catalog();
    let mut ship = ship(&catalog);
    ship.parts[0].angle = 0.37;
    EditorCommand::Rotate(1).apply(&mut ship).unwrap();
    assert!((ship.parts[0].angle - 0.37 - std::f64::consts::FRAC_PI_2).abs() < 1e-10);
}

#[test]
fn copy_filters_external_references_but_keeps_internal_staging_flags() {
    let catalog = catalog();
    let ship = ship(&catalog);
    let fragment = ShipFragment::capture(&ship, &pair()).unwrap();
    assert_eq!(fragment.parts().count(), 2);
    assert_eq!(fragment.groups()[0].connections, vec![connection(1, 2)]);
    let pod = fragment.groups()[0].parts[0].pod.as_ref().unwrap();
    assert_eq!(pod.name, "试验 & 船体");
    assert_eq!(pod.throttle, 0.4);
    assert_eq!(pod.staging.as_ref().unwrap().current_stage, 1);
    assert_eq!(
        pod.staging.as_ref().unwrap().steps[0].activations,
        vec![Activation { id: 2, moved: true }]
    );
    assert_eq!(
        ship.parts[0]
            .pod
            .as_ref()
            .unwrap()
            .staging
            .as_ref()
            .unwrap()
            .steps[0]
            .activations
            .len(),
        2
    );
}

#[test]
fn paste_multiple_groups_reusing_ids_remaps_all_internal_references_and_undoes() {
    let catalog = catalog();
    let mut source = ship(&catalog);
    let mut detached = ShipGroup {
        parts: source.parts[..2].to_vec(),
        connections: vec![Connection::Dock {
            parent: 1,
            child: 2,
            dock: 2,
        }],
    };
    for part in &mut detached.parts {
        part.x += 10.0;
    }
    source.disconnected.push(detached);
    let fragment = ShipFragment::capture(
        &source,
        &[pair(), vec![PartKey::new(1, 1, 0), PartKey::new(1, 2, 0)]].concat(),
    )
    .unwrap();
    let mut target = Ship::default();
    let mut history = EditorHistory::default();
    history
        .execute_with_catalog(
            &mut target,
            &catalog,
            EditorCommand::Paste {
                fragment: Box::new(fragment),
                offset: (20.0, 5.0),
            },
        )
        .unwrap();
    assert_eq!(target.all_parts().count(), 4);
    let unique: std::collections::HashSet<_> = target.all_parts().map(|part| part.id).collect();
    assert_eq!(unique.len(), 4);
    assert_eq!(target.disconnected.len(), 1);
    let group = &target.disconnected[0];
    let pod = &group.parts[0];
    let tank = &group.parts[1];
    assert_eq!(
        group.connections,
        vec![Connection::Dock {
            parent: pod.id,
            child: tank.id,
            dock: tank.id
        }]
    );
    assert_eq!(
        pod.pod.as_ref().unwrap().staging.as_ref().unwrap().steps[0].activations,
        vec![Activation {
            id: tank.id,
            moved: true
        }]
    );
    assert_eq!(pod.x, 30.0);
    let after = target.clone();
    assert_eq!(
        crate::ship_from_xml(&crate::ship_to_xml(&target).unwrap()).unwrap(),
        target
    );
    assert!(history.undo(&mut target));
    assert_eq!(target, Ship::default());
    assert!(history.redo(&mut target));
    assert_eq!(target, after);
}

#[test]
fn collision_rotation_limits_and_paste_limits_fail_without_losing_redo() {
    let catalog = catalog();
    let mut ship = ship(&catalog);
    let mut history = EditorHistory::default();
    history
        .execute(&mut ship, EditorCommand::SetActive(1, true))
        .unwrap();
    history.undo(&mut ship);
    let before = ship.clone();
    let fragment = ShipFragment::capture(&ship, &pair()).unwrap();
    for command in [
        EditorCommand::TransformSelection {
            parts: pair(),
            transform: SelectionTransform::Translate { dx: 0.0, dy: 1.0 },
        },
        EditorCommand::Paste {
            fragment: Box::new(fragment),
            offset: (0.0, 0.0),
        },
    ] {
        assert!(
            history
                .execute_with_catalog(&mut ship, &catalog, command)
                .is_err()
        );
        assert_eq!(ship, before);
        assert!(history.can_redo());
    }
    ship.parts = vec![catalog.get("limited").unwrap().instantiate(1, (0.0, 0.0))];
    ship.connections.clear();
    let fragment = ShipFragment::capture(&ship, &[PartKey::new(0, 1, 0)]).unwrap();
    let before = ship.clone();
    assert!(
        history
            .execute_with_catalog(
                &mut ship,
                &catalog,
                EditorCommand::Paste {
                    fragment: Box::new(fragment),
                    offset: (20.0, 0.0)
                }
            )
            .is_err()
    );
    assert_eq!(ship, before);
    ship.parts[0].part_type = "fixed".into();
    let before = ship.clone();
    assert!(
        history
            .execute_with_catalog(
                &mut ship,
                &catalog,
                EditorCommand::TransformSelection {
                    parts: vec![PartKey::new(0, 1, 0)],
                    transform: SelectionTransform::Rotate { center: (0.0, 0.0) }
                }
            )
            .is_err()
    );
    assert_eq!(ship, before);
}

#[test]
fn bulk_delete_is_atomic_and_duplicate_occurrences_do_not_shift_targets() {
    let catalog = catalog();
    let part = catalog.get("tank").unwrap().instantiate(1, (0.0, 0.0));
    let mut ship = Ship {
        parts: vec![part.clone(); 3],
        disconnected: vec![ShipGroup {
            parts: vec![part],
            connections: vec![],
        }],
        ..Default::default()
    };
    ship.parts[1].active = true;
    let before = ship.clone();
    assert!(
        EditorCommand::DeleteSelection(vec![PartKey::new(0, 1, 0), PartKey::new(3, 1, 0)])
            .apply(&mut ship)
            .is_err()
    );
    assert_eq!(ship, before);
    EditorCommand::DeleteSelection(vec![PartKey::new(0, 1, 0), PartKey::new(0, 1, 2)])
        .apply(&mut ship)
        .unwrap();
    assert_eq!(ship.parts, vec![before.parts[1].clone()]);
    assert_eq!(ship.disconnected, before.disconnected);
    assert!(ShipFragment::capture(&before, &[PartKey::new(0, 1, 1)]).is_err());
}

#[test]
fn no_op_transform_preserves_connections_and_redo() {
    let catalog = catalog();
    let mut ship = ship(&catalog);
    let mut history = EditorHistory::default();
    history
        .execute(&mut ship, EditorCommand::SetActive(1, true))
        .unwrap();
    history.undo(&mut ship);
    let before = ship.clone();
    history
        .execute_with_catalog(
            &mut ship,
            &catalog,
            EditorCommand::TransformSelection {
                parts: pair(),
                transform: SelectionTransform::Translate { dx: 0.0, dy: 0.0 },
            },
        )
        .unwrap();
    assert_eq!(ship, before);
    assert!(history.can_redo());
    assert!(!history.can_undo());
}

#[test]
fn bulk_delete_cleans_every_reference_field_only_in_its_selected_group() {
    let catalog = catalog();
    let mut ship = ship(&catalog);
    ship.connections.extend([
        connection(1, 3),
        Connection::Dock {
            parent: 2,
            child: 1,
            dock: 3,
        },
        Connection::Dock {
            parent: 1,
            child: 2,
            dock: 3,
        },
        Connection::Dock {
            parent: 1,
            child: 3,
            dock: 2,
        },
    ]);
    let staging = ship.parts[0]
        .pod
        .as_mut()
        .unwrap()
        .staging
        .as_mut()
        .unwrap();
    staging.steps.push(StageStep {
        activations: vec![
            Activation {
                id: 999,
                moved: true,
            },
            Activation {
                id: 2,
                moved: false,
            },
        ],
    });
    let detached = ShipGroup {
        parts: ship.parts.clone(),
        connections: ship.connections.clone(),
    };
    ship.disconnected = vec![detached.clone(), detached.clone()];
    let before = ship.clone();
    let mut history = EditorHistory::default();
    history
        .execute(
            &mut ship,
            EditorCommand::DeleteSelection(vec![PartKey::new(0, 2, 0), PartKey::new(1, 3, 0)]),
        )
        .unwrap();
    assert_eq!(
        ship.parts.iter().map(|part| part.id).collect::<Vec<_>>(),
        vec![1, 3]
    );
    assert_eq!(ship.connections, vec![connection(1, 3)]);
    let staging = ship.parts[0]
        .pod
        .as_ref()
        .unwrap()
        .staging
        .as_ref()
        .unwrap();
    assert_eq!(staging.current_stage, 1);
    assert_eq!(
        staging.steps[0].activations,
        vec![Activation {
            id: 3,
            moved: false
        }]
    );
    assert_eq!(
        staging.steps[1].activations,
        vec![Activation {
            id: 999,
            moved: true
        }]
    );
    let edited = &ship.disconnected[0];
    assert_eq!(
        edited.parts.iter().map(|part| part.id).collect::<Vec<_>>(),
        vec![1, 2]
    );
    assert_eq!(edited.connections, vec![connection(1, 2)]);
    let staging = edited.parts[0]
        .pod
        .as_ref()
        .unwrap()
        .staging
        .as_ref()
        .unwrap();
    assert_eq!(
        staging.steps[0].activations,
        vec![Activation { id: 2, moved: true }]
    );
    assert_eq!(
        staging.steps[1],
        before.disconnected[0].parts[0]
            .pod
            .as_ref()
            .unwrap()
            .staging
            .as_ref()
            .unwrap()
            .steps[1]
    );
    assert_eq!(ship.disconnected[1], detached);
    let after = ship.clone();
    assert!(history.undo(&mut ship));
    assert_eq!(ship, before);
    assert!(!history.can_undo());
    assert!(history.redo(&mut ship));
    assert_eq!(ship, after);
}

#[test]
fn bulk_delete_interleaved_occurrences_use_original_keys_and_deduplicate_input() {
    let catalog = catalog();
    let kind = catalog.get("tank").unwrap();
    let mut ship = Ship {
        parts: [1, 2, 1, 2, 1]
            .into_iter()
            .enumerate()
            .map(|(index, id)| kind.instantiate(id, (index as f64, 0.0)))
            .collect(),
        ..Default::default()
    };
    let before = ship.clone();
    EditorCommand::DeleteSelection(vec![
        PartKey::new(0, 1, 0),
        PartKey::new(0, 1, 2),
        PartKey::new(0, 2, 1),
        PartKey::new(0, 1, 0),
    ])
    .apply(&mut ship)
    .unwrap();
    assert_eq!(
        ship.parts,
        vec![before.parts[1].clone(), before.parts[2].clone()]
    );
}

#[test]
fn bulk_delete_rejects_ambiguous_original_references_even_when_owners_are_selected() {
    let catalog = catalog();
    for connection in [
        Some(connection(2, 99)),
        Some(connection(99, 2)),
        Some(Connection::Dock {
            parent: 2,
            child: 3,
            dock: 99,
        }),
        Some(Connection::Dock {
            parent: 3,
            child: 2,
            dock: 99,
        }),
        Some(Connection::Dock {
            parent: 3,
            child: 99,
            dock: 2,
        }),
        None,
    ] {
        let mut ship = ship(&catalog);
        ship.parts[0].id = 99;
        ship.parts.push(ship.parts[1].clone());
        ship.connections = connection.into_iter().collect();
        if !ship.connections.is_empty() {
            ship.parts[0].pod.as_mut().unwrap().staging = None;
        }
        let mut history = EditorHistory::default();
        history
            .execute(&mut ship, EditorCommand::SetActive(3, true))
            .unwrap();
        assert!(history.undo(&mut ship));
        let before = ship.clone();
        for selection in [
            vec![PartKey::new(0, 99, 0), PartKey::new(0, 2, 1)],
            vec![
                PartKey::new(0, 99, 0),
                PartKey::new(0, 2, 0),
                PartKey::new(0, 2, 1),
            ],
        ] {
            assert!(matches!(
                history.execute(&mut ship, EditorCommand::DeleteSelection(selection)),
                Err(CommandError::AmbiguousReference(key)) if key.group == 0 && key.id == 2
            ));
            assert_eq!(ship, before);
            assert!(!history.can_undo());
            assert!(history.can_redo());
        }
    }
}

#[test]
fn bulk_delete_defers_empty_group_compaction_until_the_batch_finishes() {
    let catalog = catalog();
    let part = catalog.get("tank").unwrap().instantiate(1, (0.0, 0.0));
    let mut ship = Ship {
        disconnected: vec![
            ShipGroup {
                parts: vec![part.clone()],
                connections: vec![],
            },
            ShipGroup {
                parts: vec![part],
                connections: vec![],
            },
            ShipGroup::default(),
        ],
        ..Default::default()
    };
    let before = ship.clone();
    let mut history = EditorHistory::default();
    history
        .execute(
            &mut ship,
            EditorCommand::Batch(vec![
                EditorCommand::DeleteSelection(vec![PartKey::new(1, 1, 0)]),
                EditorCommand::Scoped {
                    part: PartKey::new(2, 1, 0),
                    command: Box::new(EditorCommand::SetActive(1, true)),
                },
            ]),
        )
        .unwrap();
    assert_eq!(ship.disconnected.len(), 2);
    assert!(ship.disconnected[0].parts[0].active);
    assert_eq!(ship.disconnected[1], ShipGroup::default());
    assert!(history.undo(&mut ship));
    assert_eq!(ship, before);
    assert!(!history.can_undo());
}

#[test]
fn bulk_delete_large_selection_preserves_remaining_order_and_one_undo() {
    let catalog = catalog();
    let kind = catalog.get("tank").unwrap();
    let mut ship = Ship {
        parts: (1..=12_000)
            .map(|id| kind.instantiate(id, (id as f64, 0.0)))
            .collect(),
        connections: (1..12_000).map(|id| connection(id, id + 1)).collect(),
        ..Default::default()
    };
    let before = ship.clone();
    let selection = ship
        .keyed_parts()
        .filter(|(key, _)| key.id % 4 != 0)
        .map(|(key, _)| key)
        .collect();
    let mut history = EditorHistory::default();
    history
        .execute(&mut ship, EditorCommand::DeleteSelection(selection))
        .unwrap();
    assert_eq!(
        ship.parts,
        before
            .parts
            .iter()
            .filter(|part| part.id % 4 == 0)
            .cloned()
            .collect::<Vec<_>>()
    );
    assert!(ship.connections.is_empty());
    assert!(history.undo(&mut ship));
    assert_eq!(ship, before);
    assert!(!history.can_undo());
}

#[test]
fn selection_collision_preview_matches_direct_scan_for_duplicate_and_group_instances() {
    let catalog = catalog();
    let kind = catalog.get("tank").unwrap();
    let mut ship = Ship {
        parts: vec![
            kind.instantiate(1, (-4.0, 0.0)),
            kind.instantiate(1, (-4.0, 1.0)),
            kind.instantiate(2, (2.0, 0.0)),
        ],
        disconnected: vec![ShipGroup {
            parts: vec![
                kind.instantiate(1, (-4.0, 2.0)),
                kind.instantiate(3, (2.0, 2.0)),
            ],
            connections: vec![],
        }],
        ..Default::default()
    };
    let mut unknown = kind.instantiate(9, (0.0, 0.0));
    unknown.part_type = "没有目录定义的历史部件".into();
    ship.parts.push(unknown);
    let selected = [PartKey::new(0, 1, 1), PartKey::new(1, 1, 0)];
    let before = ship.clone();
    for x in -20..=20 {
        for y in -8..=8 {
            let (dx, dy) = (x as f64 / 2.0, y as f64 / 2.0);
            let proposed: Vec<_> = ship
                .keyed_parts()
                .filter(|(key, _)| selected.contains(key))
                .map(|(key, part)| {
                    let mut part = part.clone();
                    part.x += dx;
                    part.y += dy;
                    (key, part)
                })
                .collect();
            let collision = proposed.iter().any(|(_, part)| {
                ship.keyed_parts()
                    .filter(|(key, _)| !selected.contains(key))
                    .any(|(_, other)| {
                        catalog.get(&other.part_type).is_some_and(|other_kind| {
                            crate::intersects(part, kind, other, other_kind)
                        })
                    })
            });
            let preview = selection::preview(
                &ship,
                Some(&catalog),
                &selected,
                SelectionTransform::Translate { dx, dy },
            );
            assert_eq!(preview.is_err(), collision, "{dx} {dy}");
            if let Ok(actual) = preview {
                assert_eq!(actual, proposed);
            }
            assert_eq!(ship, before);
        }
    }
}
