use super::*;

fn document() -> EditorDocument {
    let mut document = crate::tests::document();
    document.catalog = panels::tests::catalog();
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
