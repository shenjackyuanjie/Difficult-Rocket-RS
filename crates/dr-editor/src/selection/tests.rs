use super::*;
use crate::test_support::{movement, movement_pose};

fn document() -> EditorDocument {
    let mut document = crate::test_support::document();
    document.catalog = crate::test_support::catalog();
    let kind = document.catalog.get("pod").unwrap();
    document.ship.parts = vec![
        kind.instantiate(1, (0.0, 0.0)),
        kind.instantiate(2, (1.0, 0.0)),
        kind.instantiate(3, (5.0, 0.0)),
    ];
    document.ship.connections = vec![Connection::Normal {
        parent: 1,
        child: 2,
        parent_attach: 2,
        child_attach: 1,
    }];
    document.saved_ship = document.ship.clone();
    document
}
fn key(id: i64) -> PartKey {
    PartKey::new(0, id, 0)
}

#[test]
fn tree_subtree_component_and_rectangle_share_rigid_precision_rotation_and_mirrors() {
    for free_mode in [false, true] {
        for source in 0..3 {
            let mut document = document();
            let kind = document.catalog.get("pod").unwrap();
            document.ship.parts = vec![
                kind.instantiate(1, (-2.0, 1.0)),
                kind.instantiate(2, (1.5, -2.0)),
                kind.instantiate(3, (3.0, -2.0)),
                kind.instantiate(4, (9.0, 9.0)),
            ];
            document.ship.connections.push(Connection::Normal {
                parent: 2,
                child: 3,
                parent_attach: 2,
                child_attach: 1,
            });
            document.free_mode = free_mode;
            document.saved_ship = document.ship.clone();
            document.refresh();
            let original = document.ship.clone();
            let mut topology = topology_ui::ConnectionEditor::default();
            match source {
                0 => {
                    topology_ui::act(
                        topology_ui::Action::Node(key(1)),
                        &mut topology,
                        &mut document,
                    );
                    topology_ui::act(topology_ui::Action::Subtree, &mut topology, &mut document);
                }
                1 => {
                    topology_ui::act(
                        topology_ui::Action::Node(key(2)),
                        &mut topology,
                        &mut document,
                    );
                    topology_ui::act(topology_ui::Action::Component, &mut topology, &mut document);
                }
                _ => rectangle(&mut document, (-4.0, -4.0), (4.0, 3.0), false),
            }
            assert_eq!(document.selected_keys(), vec![key(1), key(2), key(3)]);
            let mut cursor = EditorCursor::default();
            for (code, fine) in [
                (KeyCode::KeyE, false),
                (KeyCode::KeyQ, true),
                (KeyCode::KeyX, false),
                (KeyCode::KeyY, false),
                (KeyCode::KeyR, false),
            ] {
                let parts = document.selected_keys();
                let center = center(&document, &parts);
                let transform = match code {
                    KeyCode::KeyE => SelectionTransform::RotateBy {
                        center,
                        radians: 15.0_f64.to_radians(),
                    },
                    KeyCode::KeyQ => SelectionTransform::RotateBy {
                        center,
                        radians: (-1.0_f64).to_radians(),
                    },
                    KeyCode::KeyX => SelectionTransform::FlipX { center },
                    KeyCode::KeyY => SelectionTransform::FlipY { center },
                    _ => SelectionTransform::Rotate { center },
                };
                let expected = document
                    .edit_command(EditorCommand::TransformSelection { parts, transform })
                    .preview_with_catalog(&document.ship, &document.catalog)
                    .unwrap();
                let undo = document.history.undo_len();
                let mut keys = ButtonInput::default();
                keys.press(code);
                if fine {
                    keys.press(KeyCode::ShiftLeft);
                }
                assert!(keyboard(&mut document, &mut cursor, &keys, false));
                assert_eq!(
                    document.ship, expected,
                    "source={source}, free={free_mode}, key={code:?}"
                );
                assert_eq!(document.history.undo_len(), undo + 1);
                assert_eq!(document.ship.connections, original.connections);
                assert_eq!(document.ship.part(4), original.part(4));
                let edited = document.ship.clone();
                let selected = document.selected;
                let selection = document.selection.clone();
                assert!(document.undo());
                assert!(document.redo());
                assert_eq!(document.ship, edited);
                document.selected = selected;
                document.selection = selection;
            }
        }
    }
}
fn press(document: &mut EditorDocument, cursor: &mut EditorCursor, code: KeyCode, control: bool) {
    let mut keys = ButtonInput::default();
    keys.press(code);
    if control {
        keys.press(KeyCode::ControlLeft);
    }
    keyboard(document, cursor, &keys, false);
}

#[test]
fn shift_click_and_rectangle_keep_precise_selection_without_editing() {
    let mut document = document();
    let before = document.ship.clone();
    click(&mut document, Some(key(1)), false);
    click(&mut document, Some(key(2)), true);
    assert_eq!(document.selected_keys(), vec![key(1), key(2)]);
    click(&mut document, Some(key(1)), false);
    assert_eq!(document.selected_keys().len(), 2);
    click(&mut document, Some(key(2)), true);
    assert_eq!(document.selected_keys(), vec![key(1)]);
    rectangle(&mut document, (5.2, 0.2), (4.8, -0.2), true);
    assert_eq!(document.selected_keys(), vec![key(1), key(3)]);
    rectangle(&mut document, (-0.75, -0.75), (1.75, 0.75), false);
    assert_eq!(document.selected_keys(), vec![key(1), key(2)]);
    assert_eq!(document.ship, before);
    assert!(!document.dirty);
    assert!(!document.history.can_undo());
}

#[test]
fn whole_selection_snap_keeps_internal_connection_and_rejects_whole_body_collisions() {
    let mut document = document();
    let before = document.ship.clone();
    let (_, _, valid) = movement(&document, &[key(1), key(2)], (4.0, 0.0));
    assert!(!valid);
    let (delta, command, valid) = movement(&document, &[key(1), key(2)], (3.2, 0.0));
    assert!(valid);
    assert!((delta.0 - 3.0).abs() < 1e-8);
    assert_eq!(document.ship, before);
    assert!(document.execute(command));
    assert_eq!(document.ship.connections.len(), 2);
    assert!((document.ship.parts[0].x - 3.0).abs() < 1e-8);
    assert!((document.ship.parts[1].x - 4.0).abs() < 1e-8);
    assert!(document.undo());
    assert_eq!(document.ship, before);
}

#[test]
fn copy_paste_preview_isolated_collision_rejection_and_atomic_undo() {
    let mut document = document();
    document.selection = [key(1), key(2)].into_iter().collect();
    let before = document.ship.clone();
    let mut cursor = EditorCursor {
        valid: true,
        world: (0.5, 0.0),
        ..default()
    };
    press(&mut document, &mut cursor, KeyCode::KeyC, true);
    assert_eq!(cursor.clipboard.as_ref().unwrap().parts().count(), 2);
    press(&mut document, &mut cursor, KeyCode::KeyV, true);
    let preview = paste_preview(&document, &cursor).unwrap();
    assert!(!preview.valid);
    assert_eq!(preview.parts.len(), 2);
    assert!(!commit_paste(&mut document, &mut cursor));
    assert_eq!(document.ship, before);
    assert!(cursor.paste.is_some());
    cursor.world = (0.5, 3.0);
    let preview = paste_preview(&document, &cursor).unwrap();
    assert!(preview.valid);
    assert_eq!(document.ship, before);
    assert!(commit_paste(&mut document, &mut cursor));
    assert!(cursor.paste.is_none());
    assert_eq!(document.selected_keys().len(), 2);
    assert_eq!(document.ship.all_parts().count(), 5);
    assert_eq!(document.ship.all_connections().count(), 2);
    assert!(document.undo());
    assert_eq!(document.ship, before);
    assert!(document.redo());
    assert_eq!(document.ship.all_parts().count(), 5);
}

#[test]
fn cut_select_all_rotation_and_delete_use_single_history_entries() {
    let mut document = document();
    let before = document.ship.clone();
    let mut cursor = EditorCursor::default();
    press(&mut document, &mut cursor, KeyCode::KeyA, true);
    assert_eq!(document.selected_keys().len(), 3);
    press(&mut document, &mut cursor, KeyCode::KeyR, false);
    assert_eq!(document.ship.connections, before.connections);
    assert!(document.undo());
    assert_eq!(document.ship, before);
    press(&mut document, &mut cursor, KeyCode::KeyA, true);
    press(&mut document, &mut cursor, KeyCode::KeyX, true);
    assert_eq!(cursor.clipboard.as_ref().unwrap().parts().count(), 3);
    assert_eq!(document.ship.all_parts().count(), 0);
    assert!(document.undo());
    assert_eq!(document.ship, before);
    press(&mut document, &mut cursor, KeyCode::KeyA, true);
    press(&mut document, &mut cursor, KeyCode::Delete, false);
    assert_eq!(document.ship.all_parts().count(), 0);
    assert!(document.undo());
    assert_eq!(document.ship, before);
}

#[test]
fn failed_cut_keeps_previous_clipboard_and_file_cancel_keeps_copy() {
    let mut document = document();
    let mut cursor = EditorCursor::default();
    document.select_only(Some(key(3)));
    press(&mut document, &mut cursor, KeyCode::KeyC, true);
    document.ship.parts.push(document.ship.parts[0].clone());
    document.select_only(Some(key(1)));
    let before = document.ship.clone();
    press(&mut document, &mut cursor, KeyCode::KeyX, true);
    assert_eq!(document.ship, before);
    assert_eq!(
        cursor
            .clipboard
            .as_ref()
            .unwrap()
            .parts()
            .next()
            .unwrap()
            .id,
        3
    );
    press(&mut document, &mut cursor, KeyCode::KeyV, true);
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_resource(cursor)
        .add_message::<files::FileAction>()
        .add_systems(Update, placement::cancel_for_file_action);
    app.world_mut().write_message(files::FileAction::New);
    app.update();
    let cursor = app.world().resource::<EditorCursor>();
    assert!(cursor.paste.is_none());
    assert!(cursor.clipboard.is_some());
}

// 保留原始全扫描作为宽阶段剔除和同距离候选顺序的独立对照。
fn brute_force_snaps(
    ship: &Ship,
    catalog: &PartCatalog,
    keys: &[PartKey],
    delta: (f64, f64),
) -> Vec<(f64, (f64, f64), EditorCommand)> {
    let selected: HashSet<_> = keys.iter().copied().collect();
    let parts: Vec<_> = ship.keyed_parts().collect();
    let targets: Vec<_> = parts
        .iter()
        .copied()
        .filter(|(key, _)| !selected.contains(key))
        .filter_map(|(key, part)| {
            let kind = catalog.get(&part.part_type)?;
            Some((
                key,
                part,
                kind,
                dr_core::connections::attachment_radius(kind),
            ))
        })
        .collect();
    let mut result = vec![];
    for &(source_key, source) in parts.iter().filter(|(key, _)| selected.contains(key)) {
        let Some(st) = catalog.get(&source.part_type) else {
            continue;
        };
        let mut source = source.clone();
        source.x += delta.0;
        source.y += delta.1;
        let source_radius = dr_core::connections::attachment_radius(st);
        for &(target_key, target, tt, target_radius) in &targets {
            let distance_squared = (source.x - target.x).powi(2) + (source.y - target.y).powi(2);
            if distance_squared > (source_radius + target_radius + 0.350001).powi(2) {
                continue;
            }
            for candidate in dr_core::connections::candidates(&source, st, target, tt, 0.35) {
                let kind = if candidate.dock {
                    LinkKind::Dock {
                        connector: if st.kind == PartKind::DockConnector {
                            source_key
                        } else {
                            target_key
                        },
                    }
                } else {
                    LinkKind::Normal {
                        parent_attach: candidate.target_index as i32 + 1,
                        child_attach: candidate.source_index as i32 + 1,
                    }
                };
                result.push((
                    candidate.distance,
                    (
                        delta.0 + candidate.position.x - source.x,
                        delta.1 + candidate.position.y - source.y,
                    ),
                    EditorCommand::ConnectParts {
                        parent: target_key,
                        child: source_key,
                        kind,
                    },
                ));
            }
        }
    }
    result.sort_by(|a, b| a.0.total_cmp(&b.0));
    result
}

#[test]
fn spatial_snap_candidates_match_full_scan_and_preserve_equal_distance_order() {
    let mut document = document();
    let mut offset = document.catalog.get("pod").unwrap().clone();
    offset.id = "offset".into();
    offset.width = 8;
    offset.height = 2;
    offset.attach_points[0].location = "Top".into();
    offset.attach_points[0].x = 40.0;
    offset.attach_points[0].y = 1.0;
    offset.attach_points[1].location = "BottomSide".into();
    offset.attach_points[1].x = -40.0;
    offset.attach_points[1].y = -1.0;
    document.catalog = PartCatalog::new(
        "测试",
        vec![document.catalog.get("pod").unwrap().clone(), offset],
    );
    document.ship.parts = (0..120)
        .map(|i| {
            let kind = &document.catalog.types[i % 2];
            let mut part = kind.instantiate(
                i as i64 % 40,
                (((i * 29) % 121) as f64 - 60.0, (i % 7) as f64 - 3.0),
            );
            part.angle = (i % 4) as f64 * std::f64::consts::FRAC_PI_2;
            part.flip_x = i % 3 == 0;
            part.flip_y = i % 5 == 0;
            part
        })
        .collect();
    document.ship.connections.clear();
    document.ship.disconnected = vec![dr_core::ShipGroup {
        parts: document.ship.parts[..5].to_vec(),
        connections: vec![],
    }];
    let keys = [
        PartKey::new(0, 0, 0),
        PartKey::new(0, 1, 1),
        PartKey::new(1, 0, 0),
    ];
    let mut candidate_count = 0;
    for x in -10..=10 {
        for y in -4..=4 {
            let delta = (x as f64 + 0.2, y as f64 + 0.1);
            let indexed = snaps(&document.ship, &document.catalog, &keys, delta);
            let direct = brute_force_snaps(&document.ship, &document.catalog, &keys, delta);
            candidate_count += direct.len();
            assert_eq!(format!("{indexed:?}"), format!("{direct:?}"), "{delta:?}");
        }
    }
    assert!(candidate_count > 0);

    // 故意以右、左的顺序放入两个等距离目标；不能被 X 排序反转。
    let kind = document.catalog.get("pod").unwrap();
    document.ship.parts = vec![
        kind.instantiate(1, (0.0, 0.0)),
        kind.instantiate(2, (1.2, 0.0)),
        kind.instantiate(3, (-1.2, 0.0)),
    ];
    document.ship.disconnected.clear();
    let candidates = snaps(&document.ship, &document.catalog, &[key(1)], (0.0, 0.0));
    assert_eq!(candidates.len(), 2);
    for (candidate, expected_parent) in candidates.iter().zip([key(2), key(3)]) {
        match &candidate.2 {
            EditorCommand::ConnectParts { parent, .. } => assert_eq!(*parent, expected_parent),
            other => panic!("不是连接候选：{other:?}"),
        }
    }
}

#[test]
fn overlapping_rigid_drag_preserves_internal_edges_detaches_external_and_undoes_once() {
    let mut document = document();
    let kind = document.catalog.get("pod").unwrap();
    document.ship.parts.push(kind.instantiate(4, (-5.0, 0.0)));
    document.ship.connections.push(Connection::Normal {
        parent: 4,
        child: 1,
        parent_attach: 1,
        child_attach: 1,
    });
    let before = document.ship.clone();
    let (delta, command, clear) =
        movement_pose(&document, &[key(1), key(2)], (0.0, 0.0), 1, (5.0, 0.0));
    assert!(!clear);
    assert_eq!(delta, (5.0, 0.0));
    assert_eq!(document.ship, before);
    assert!(document.execute(command));
    assert_eq!(
        (document.ship.parts[0].x, document.ship.parts[0].y),
        (5.0, 0.0)
    );
    assert_eq!(
        (document.ship.parts[1].x, document.ship.parts[1].y),
        (5.0, 1.0)
    );
    assert_eq!(document.ship.connections, before.connections[..1]);
    assert!(document.undo());
    assert_eq!(document.ship, before);
    assert!(!document.history.can_undo());
}

#[test]
fn drag_scope_is_exact_and_ambiguous_references_are_atomic_errors() {
    let mut document = document();
    let kind = document.catalog.get("pod").unwrap();
    document.ship.disconnected.push(dr_core::ShipGroup {
        parts: vec![kind.instantiate(1, (9.0, 0.0))],
        connections: vec![],
    });
    let before = document.ship.clone();
    let command = EditorCommand::DragSelection {
        parts: vec![PartKey::new(1, 1, 0)],
        pivot: (9.0, 0.0),
        turns: 1,
        delta: (-9.0, 0.0),
    };
    assert!(document.execute(command));
    assert_eq!(document.ship.parts, before.parts);
    assert_eq!(document.ship.connections, before.connections);
    assert_eq!(document.ship.disconnected[0].parts[0].editor_angle, 1);
    assert!(document.undo());
    document.ship.parts.push(document.ship.parts[0].clone());
    let before = document.ship.clone();
    let command = EditorCommand::DragSelection {
        parts: vec![key(1)],
        pivot: (0.0, 0.0),
        turns: 1,
        delta: (2.0, 0.0),
    };
    assert!(!document.execute(command));
    assert_eq!(document.ship, before);
}

#[test]
fn drag_permissions_and_finite_values_are_checked_without_weakening_normal_transforms() {
    let mut document = document();
    let before = document.ship.clone();
    assert!(!document.execute(EditorCommand::TransformSelection {
        parts: vec![key(1)],
        transform: SelectionTransform::Translate { dx: 5.0, dy: 0.0 }
    }));
    assert_eq!(document.ship, before);
    for (pivot, delta) in [
        ((f64::NAN, 0.0), (0.0, 0.0)),
        ((0.0, 0.0), (f64::INFINITY, 0.0)),
    ] {
        assert!(!document.execute(EditorCommand::DragSelection {
            parts: vec![key(1)],
            pivot,
            turns: 0,
            delta
        }));
        assert_eq!(document.ship, before);
    }
    document.ship.parts[0] = document
        .catalog
        .get("tank")
        .unwrap()
        .instantiate(1, (0.0, 0.0));
    let before = document.ship.clone();
    assert!(!document.execute(EditorCommand::DragSelection {
        parts: vec![key(1)],
        pivot: (0.0, 0.0),
        turns: 1,
        delta: (0.0, 0.0)
    }));
    assert_eq!(document.ship, before);
}

#[test]
fn descendant_scope_is_directional_scoped_cycle_safe_and_dock_aware() {
    let mut document = document();
    document.ship.connections.push(Connection::Dock {
        parent: 2,
        child: 3,
        dock: 1,
    });
    assert_eq!(
        drag_keys(&document.ship, &[key(2)], false),
        [key(2)].into_iter().collect()
    );
    assert_eq!(
        drag_keys(&document.ship, &[key(2)], true),
        [key(2), key(3)].into_iter().collect()
    );
    document.ship.connections.push(Connection::Normal {
        parent: 3,
        child: 1,
        parent_attach: 1,
        child_attach: 1,
    });
    assert_eq!(drag_keys(&document.ship, &[key(2)], true).len(), 3);
    let kind = document.catalog.get("pod").unwrap();
    document.ship.disconnected.push(dr_core::ShipGroup {
        parts: vec![
            kind.instantiate(1, (8.0, 0.0)),
            kind.instantiate(2, (9.0, 0.0)),
        ],
        connections: vec![Connection::Normal {
            parent: 1,
            child: 2,
            parent_attach: 1,
            child_attach: 1,
        }],
    });
    assert_eq!(
        drag_keys(&document.ship, &[PartKey::new(1, 1, 0)], true),
        [PartKey::new(1, 1, 0), PartKey::new(1, 2, 0)]
            .into_iter()
            .collect()
    );
}

#[test]
fn delete_connected_component_includes_parents_children_and_dock_but_not_other_groups() {
    let mut document = document();
    document.ship.connections.push(Connection::Dock {
        parent: 2,
        child: 3,
        dock: 1,
    });
    let keys = connected_keys(&document.ship, &[key(2)]).unwrap();
    assert_eq!(keys, [key(1), key(2), key(3)].into_iter().collect());
    document.ship.parts.push(document.ship.parts[0].clone());
    assert!(connected_keys(&document.ship, &[key(2)]).is_err());
}

#[test]
fn connected_component_handles_large_chain_cycles_and_repeated_roots_iteratively() {
    let mut document = document();
    let kind = document.catalog.get("pod").unwrap();
    let count = 10_000;
    document.ship.parts = (1..=count)
        .map(|id| kind.instantiate(id, (id as f64, 0.0)))
        .collect();
    document.ship.connections = (1..count)
        .map(|parent| Connection::Normal {
            parent,
            child: parent + 1,
            parent_attach: 1,
            child_attach: 1,
        })
        .collect();
    document.ship.connections.push(Connection::Normal {
        parent: count,
        child: 1,
        parent_attach: 1,
        child_attach: 1,
    });
    let before = document.ship.clone();
    assert_eq!(
        connected_keys(&document.ship, &[key(1), key(count), key(1)]).unwrap(),
        (1..=count).map(key).collect()
    );
    assert_eq!(document.ship, before);
}

#[test]
fn connected_component_indexes_each_group_separately_and_reaches_dock_connector() {
    let mut document = document();
    let kind = document.catalog.get("pod").unwrap();
    document.ship.disconnected.push(dr_core::ShipGroup {
        parts: (1..=3)
            .map(|id| kind.instantiate(id, (id as f64, 5.0)))
            .collect(),
        connections: vec![Connection::Dock {
            parent: 1,
            child: 2,
            dock: 3,
        }],
    });
    let dock = PartKey::new(1, 3, 0);
    let group: BTreeSet<_> = (1..=3).map(|id| PartKey::new(1, id, 0)).collect();
    assert_eq!(connected_keys(&document.ship, &[dock]).unwrap(), group);
    assert_eq!(
        connected_keys(&document.ship, &[key(2), dock]).unwrap(),
        group.into_iter().chain([key(1), key(2)]).collect()
    );
    assert!(connected_keys(&document.ship, &[]).unwrap().is_empty());
    assert!(connected_keys(&document.ship, &[PartKey::new(2, 1, 0)]).is_err());
}

#[test]
fn connected_component_rejects_only_reachable_duplicate_or_missing_references() {
    let mut document = document();
    let kind = document.catalog.get("pod").unwrap();
    document.ship.parts.push(kind.instantiate(4, (8.0, 0.0)));
    document.ship.parts.push(kind.instantiate(4, (9.0, 0.0)));
    document.ship.connections.push(Connection::Normal {
        parent: 3,
        child: 4,
        parent_attach: 1,
        child_attach: 1,
    });
    let component = [key(1), key(2)].into_iter().collect();
    assert_eq!(
        connected_keys(&document.ship, &[key(1)]).unwrap(),
        component
    );
    assert!(connected_keys(&document.ship, &[key(3)]).is_err());
    document.ship.parts.pop();
    assert_eq!(
        connected_keys(&document.ship, &[key(3)]).unwrap(),
        [key(3), key(4)].into_iter().collect()
    );
    document.ship.connections.push(Connection::Normal {
        parent: 3,
        child: 99,
        parent_attach: 1,
        child_attach: 1,
    });
    assert_eq!(
        connected_keys(&document.ship, &[key(1)]).unwrap(),
        component
    );
    assert!(connected_keys(&document.ship, &[key(4)]).is_err());
    document.ship.connections.push(Connection::Normal {
        parent: 2,
        child: 3,
        parent_attach: 1,
        child_attach: 1,
    });
    assert!(connected_keys(&document.ship, &[key(1)]).is_err());
}

#[test]
fn selection_drag_snaps_independently_and_keeps_internal_edges_in_all_four_combinations() {
    for free_mode in [false, true] {
        for snap_enabled in [false, true] {
            let mut document = document();
            document.free_mode = free_mode;
            document.snap_enabled = snap_enabled;
            let before = document.ship.clone();
            let (delta, command, valid) = movement(&document, &[key(1), key(2)], (2.8, 0.0));
            assert!(valid, "free={free_mode}, snap={snap_enabled}");
            let expected = if snap_enabled { 3.0 } else { 2.8 };
            assert!((delta.0 - expected).abs() < 1e-10 && delta.1 == 0.0);
            assert_eq!(document.ship, before);
            assert!(document.execute(command));
            assert!((document.ship.part(1).unwrap().x - expected).abs() < 1e-10);
            assert!((document.ship.part(2).unwrap().x - expected - 1.0).abs() < 1e-10);
            assert!(document.ship.connections.contains(&before.connections[0]));
            assert_eq!(
                document.ship.connections.len(),
                1 + usize::from(snap_enabled)
            );
            assert_eq!(document.history.undo_len(), 1);
            let after = document.ship.clone();
            assert!(document.undo());
            assert_eq!(document.ship, before);
            assert!(document.redo());
            assert_eq!(document.ship, after);
        }
    }
}

#[test]
fn copy_paste_uses_snap_preference_for_grid_and_new_edges_in_all_four_combinations() {
    for free_mode in [false, true] {
        for snap_enabled in [false, true] {
            let mut document = document();
            document.free_mode = free_mode;
            document.snap_enabled = snap_enabled;
            document.ship.parts[2].x = 5.2;
            document.selection = [key(1), key(2)].into_iter().collect();
            let before = document.ship.clone();
            let mut cursor = EditorCursor {
                valid: true,
                world: (3.63, 0.0),
                ..default()
            };
            press(&mut document, &mut cursor, KeyCode::KeyC, true);
            press(&mut document, &mut cursor, KeyCode::KeyV, true);
            let preview = paste_preview(&document, &cursor).unwrap();
            assert!(preview.valid, "free={free_mode}, snap={snap_enabled}");
            assert_eq!(preview.snapped, snap_enabled);
            let expected = if snap_enabled { 3.2 } else { 3.13 };
            assert!((preview.parts[0].x - expected).abs() < 1e-10);
            assert!((preview.parts[1].x - expected - 1.0).abs() < 1e-10);
            assert_eq!(document.ship, before);
            assert!(commit_paste(&mut document, &mut cursor));
            let pasted: Vec<_> = document
                .ship
                .all_parts()
                .filter(|part| part.id > 3)
                .collect();
            assert_eq!(pasted.len(), 2);
            for (actual, ghost) in pasted.iter().zip(&preview.parts) {
                assert_eq!(
                    (
                        actual.x,
                        actual.y,
                        actual.angle,
                        actual.flip_x,
                        actual.flip_y
                    ),
                    (ghost.x, ghost.y, ghost.angle, ghost.flip_x, ghost.flip_y)
                );
            }
            assert!(document.ship.connections.contains(&before.connections[0]));
            let internal = Connection::Normal {
                parent: pasted[0].id,
                child: pasted[1].id,
                parent_attach: 2,
                child_attach: 1,
            };
            // 片段内部边始终恢复；吸附只决定是否新增对外连接并合入主分组。
            if snap_enabled {
                assert!(document.ship.disconnected.is_empty());
                assert_eq!(document.ship.connections.len(), 3);
                assert!(document.ship.connections.contains(&internal));
                assert!(document.ship.connections.contains(&Connection::Normal {
                    parent: 3,
                    child: pasted[1].id,
                    parent_attach: 1,
                    child_attach: 2,
                }));
            } else {
                assert_eq!(document.ship.connections, before.connections);
                assert_eq!(document.ship.disconnected.len(), 1);
                let group = &document.ship.disconnected[0];
                assert_eq!(group.parts.len(), 2);
                assert_eq!(group.parts[0].id, pasted[0].id);
                assert_eq!(group.parts[1].id, pasted[1].id);
                assert_eq!(group.connections, vec![internal]);
            }
            assert_eq!(
                document
                    .ship
                    .groups()
                    .map(|(_, _, edges)| edges.len())
                    .sum::<usize>(),
                2 + usize::from(snap_enabled),
                "原有边与粘贴内部边均保留，吸附开启时再新增一条对外边"
            );
            assert_eq!(document.history.undo_len(), 1);
            assert!(cursor.paste.is_none() && cursor.clipboard.is_some());
            assert_eq!(document.selected_keys().len(), 2);
            let after = document.ship.clone();
            assert!(document.undo());
            assert_eq!(document.ship, before);
            assert!(document.redo());
            assert_eq!(document.ship, after);
        }
    }
}

#[test]
fn selection_and_paste_free_snap_only_relax_whole_body_collision() {
    for free_mode in [false, true] {
        for snap_enabled in [false, true] {
            for paste in [false, true] {
                let mut document = document();
                document.free_mode = free_mode;
                document.snap_enabled = snap_enabled;
                let kind = document.catalog.get("pod").unwrap();
                document.ship.parts = vec![
                    kind.instantiate(1, (0.0, 0.0)),
                    kind.instantiate(2, (0.0, 2.0)),
                    kind.instantiate(3, (4.0, 0.0)),
                    document
                        .catalog
                        .get("engine")
                        .unwrap()
                        .instantiate(4, (3.0, 0.0)),
                ];
                document.ship.connections.clear();
                let before = document.ship.clone();
                if paste {
                    let mut cursor = EditorCursor {
                        valid: true,
                        world: (3.2, 0.0),
                        paste: Some(ShipFragment::capture(&document.ship, &[key(1)]).unwrap()),
                        ..default()
                    };
                    let preview = paste_preview(&document, &cursor).unwrap();
                    assert_eq!(preview.valid, free_mode);
                    assert_eq!(preview.snapped, free_mode && snap_enabled);
                    assert_eq!(commit_paste(&mut document, &mut cursor), free_mode);
                    if !free_mode {
                        assert_eq!(document.ship, before);
                        assert!(cursor.paste.is_some());
                        assert!(!document.history.can_undo());
                        continue;
                    }
                } else {
                    let (delta, command, valid) =
                        movement(&document, &[key(1), key(2)], (2.8, 0.0));
                    assert_eq!(valid, free_mode);
                    assert!(
                        (delta.0 - if free_mode && snap_enabled { 3.0 } else { 2.8 }).abs() < 1e-10
                    );
                    // 普通拖动保留既有“碰撞警告但允许落点”合同，不改成拒绝提交。
                    assert!(document.execute(command));
                }
                assert_eq!(
                    document.ship.connections.len(),
                    usize::from(free_mode && snap_enabled)
                );
                assert_eq!(document.history.undo_len(), 1);
                let after = document.ship.clone();
                assert!(document.undo());
                assert_eq!(document.ship, before);
                assert!(document.redo());
                assert_eq!(document.ship, after);
            }
        }
    }
}

#[test]
fn free_automatic_drag_connections_still_reject_occupancy_ambiguity_and_pair_overlap() {
    for scenario in ["source", "target", "ambiguous", "pair_overlap"] {
        let mut document = document();
        document.free_mode = true;
        document.snap_enabled = true;
        document.ship.parts[1].x = 5.0;
        document.ship.parts[2].x = 10.0;
        document.ship.connections.clear();
        match scenario {
            "source" => document.ship.connections.push(Connection::Normal {
                parent: 2,
                child: 3,
                parent_attach: 1,
                child_attach: 2,
            }),
            "target" => document.ship.connections.push(Connection::Normal {
                parent: 1,
                child: 3,
                parent_attach: 2,
                child_attach: 1,
            }),
            "ambiguous" => document.ship.parts.push(document.ship.parts[0].clone()),
            _ => {}
        }
        let before = document.ship.clone();
        let requested = (
            if scenario == "pair_overlap" {
                -4.8
            } else {
                -3.8
            },
            0.0,
        );
        let (delta, command, valid) = movement(&document, &[key(2)], requested);
        assert!(valid, "{scenario}");
        assert_eq!(delta, requested, "{scenario}");
        assert_eq!(document.ship, before);
        assert!(document.execute(command), "{scenario}");
        assert_eq!(document.ship.connections, before.connections, "{scenario}");
        assert_eq!(document.history.undo_len(), 1);
        assert!(document.undo());
        assert_eq!(document.ship, before);
    }
}

#[test]
fn finite_pose_rotation_permissions_and_mirrors_remain_strict_in_all_four_combinations() {
    for free_mode in [false, true] {
        for snap_enabled in [false, true] {
            let mut document = document();
            document.free_mode = free_mode;
            document.snap_enabled = snap_enabled;
            let before = document.ship.clone();
            let pose = SelectionPose {
                pivot: (0.5, 0.0),
                radians: 0.37,
                flip_x: true,
                flip_y: true,
            };
            let (_, command, valid) =
                movement_transformed(&document, &[key(1), key(2)], pose, (0.0, 3.0));
            assert!(valid);
            let expected = document
                .edit_command(command.clone())
                .preview_with_catalog(&document.ship, &document.catalog)
                .unwrap();
            assert!(document.execute(command));
            assert_eq!(document.ship, expected);
            assert_eq!(document.ship.connections, before.connections);
            assert!(document.undo());
            assert_eq!(document.ship, before);
            for (pivot, delta) in [
                ((f64::NAN, 0.0), (0.0, 0.0)),
                ((0.0, 0.0), (f64::INFINITY, 0.0)),
            ] {
                let (_, command, valid) = movement_pose(&document, &[key(1)], pivot, 0, delta);
                assert!(!valid);
                assert!(!document.execute(command));
                assert_eq!(document.ship, before);
                assert!(!document.history.can_undo());
                assert!(document.history.can_redo());
            }
            document.ship.parts[0] = document
                .catalog
                .get("tank")
                .unwrap()
                .instantiate(1, (0.0, 0.0));
            let before = document.ship.clone();
            let (_, command, valid) =
                movement_pose(&document, &[key(1)], (0.0, 0.0), 1, (0.0, 0.0));
            assert!(!valid);
            assert!(!document.execute(command));
            assert_eq!(document.ship, before);
            assert!(!document.history.can_undo());
        }
    }
}
