use super::*;

pub(crate) fn catalog() -> PartCatalog {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("目录.xml");
    std::fs::write(&path, r#"<PartTypes>
        <PartType id="pod" name="驾驶舱" type="pod" width="2" height="2"><AttachPoints><AttachPoint x="-1"/><AttachPoint x="1"/></AttachPoints></PartType>
        <PartType id="hidden" name="隐藏" hidden="true"/>
        <PartType id="engine" name="发动机" type="engine" width="2" height="2" category="推进" maxOccurrences="1"><Engine power="1"/></PartType>
        <PartType id="tank" name="燃料箱" type="tank" width="2" height="2" category="推进" disableEditorRotation="true"><Tank fuel="100"/></PartType>
    </PartTypes>"#).unwrap();
    load_catalog(path).unwrap()
}

#[test]
fn collisions_include_other_instances_with_the_same_id() {
    let mut document = crate::tests::document();
    document.catalog = catalog();
    let kind = document.catalog.get("pod").unwrap();
    document.ship.parts = vec![kind.instantiate(1, (0.0, 0.0))];
    document.ship.disconnected.push(dr_core::ShipGroup {
        parts: vec![kind.instantiate(1, (5.0, 0.0))],
        connections: vec![],
    });
    let key = PartKey::new(1, 1, 0);
    let mut moving = document.ship.part_at(key).unwrap().clone();
    moving.x = 0.0;
    assert!(placement::collides(
        &document.ship,
        &document.catalog,
        &moving,
        Some(key)
    ));
    let before = document.ship.clone();
    let command = move_with_snap(&document.ship, &document.catalog, key, (0.0, 0.0)).unwrap();
    assert!(document.execute(command));
    assert_eq!(document.ship.part_at(key).unwrap().x, 0.0);
    assert!(document.undo());
    assert_eq!(document.ship, before);
    moving.x = 5.0;
    assert!(!placement::collides(
        &document.ship,
        &document.catalog,
        &moving,
        Some(key)
    ));
}

#[test]
fn same_id_parts_in_different_groups_snap_and_merge_atomically() {
    let mut document = crate::tests::document();
    document.catalog = catalog();
    let kind = document.catalog.get("pod").unwrap();
    document.ship.parts = vec![kind.instantiate(1, (0.0, 0.0))];
    document.ship.disconnected.push(dr_core::ShipGroup {
        parts: vec![kind.instantiate(1, (5.0, 0.0))],
        connections: vec![],
    });
    let before = document.ship.clone();
    let key = PartKey::new(1, 1, 0);
    document.selected = Some(key);
    let command = move_with_snap(&document.ship, &document.catalog, key, (1.2, 0.0)).unwrap();
    assert!(document.execute(command), "{}", document.status);
    assert_eq!(document.ship.parts.len(), 2);
    assert!(document.ship.disconnected.is_empty());
    assert_eq!(document.ship.connections.len(), 1);
    assert_eq!(document.ship.parts[1].x, 1.0);
    assert_ne!(document.ship.parts[1].id, 1);
    assert!(document.selected.is_none());
    assert!(document.undo());
    assert_eq!(document.ship, before);
}

#[test]
fn category_and_keyboard_navigation_exclude_hidden_parts() {
    let mut document = crate::tests::document();
    document.catalog = catalog();
    let mut palette = Palette::default();
    assert_eq!(palette.indices(&document.catalog), vec![0, 1, 2]);
    palette.category = Some("推进".into());
    assert_eq!(palette.indices(&document.catalog), vec![1, 2]);
    let mut cursor = EditorCursor {
        catalog_index: 1,
        ..default()
    };
    cycle_part(&document, &palette, &mut cursor, false);
    assert_eq!(cursor.catalog_index, 2);
    assert!(cursor.placing);
    cycle_part(&document, &palette, &mut cursor, false);
    assert_eq!(cursor.catalog_index, 1);
    cycle_part(&document, &palette, &mut cursor, true);
    assert_eq!(cursor.catalog_index, 2);
}

#[test]
fn folder_listing_filters_invalid_files_and_retains_previous_folder_on_error() {
    let directory = tempfile::tempdir().unwrap();
    save_ship(directory.path().join("B.xml"), &Ship::default()).unwrap();
    save_ship(directory.path().join("a.XML"), &Ship::default()).unwrap();
    std::fs::write(directory.path().join("bad.xml"), "<NotShip/>").unwrap();
    std::fs::write(directory.path().join("ignore.txt"), "<Ship/>").unwrap();
    std::fs::create_dir(directory.path().join("folder.xml")).unwrap();
    let mut browser = ShipBrowser::new(directory.path().to_path_buf());
    assert_eq!(browser.files.len(), 2);
    assert_eq!(browser.files[0].file_name().unwrap(), "a.XML");
    assert_eq!(browser.rejected, 1);
    let before = browser.files.clone();
    browser.set_folder(directory.path().join("missing"));
    assert_eq!(browser.folder, directory.path());
    assert_eq!(browser.files, before);
    assert!(browser.error.is_some());
}

#[test]
fn palette_click_selects_template_without_mutating_ship_or_history() {
    let mut document = crate::tests::document();
    document.catalog = catalog();
    let before = document.ship.clone();
    let mut app = App::new();
    app.insert_resource(document)
        .init_resource::<Palette>()
        .init_resource::<ShipBrowser>()
        .init_resource::<EditorCursor>()
        .init_resource::<DragState>()
        .add_message::<files::FileAction>()
        .add_message::<PanelButton>()
        .add_systems(Update, panel_actions);
    app.world_mut().write_message(PanelButton::Part(2));
    app.update();
    let cursor = app.world().resource::<EditorCursor>();
    assert_eq!(cursor.catalog_index, 2);
    assert!(cursor.placing);
    let document = app.world().resource::<EditorDocument>();
    assert_eq!(document.ship, before);
    assert!(!document.dirty);
    assert!(!document.history.can_undo());
}

#[test]
fn preview_and_placement_share_snap_and_undo_restores_whole_operation() {
    let mut document = crate::tests::document();
    document.catalog = catalog();
    let before = document.ship.clone();
    let cursor = EditorCursor {
        world: (1.2, 0.0),
        placing: true,
        valid: true,
        ..default()
    };
    let (preview, connection, allowed) = placement::preview(&document, &cursor).unwrap();
    assert!(allowed && connection.is_some());
    assert_eq!((preview.x, preview.y), (1.0, 0.0));
    assert_eq!(document.ship, before);
    assert!(placement::place(&mut document, &cursor));
    assert_eq!(document.ship.part(preview.id), Some(&preview));
    assert_eq!(document.ship.connections.len(), 1);
    assert!(document.ship.connections[0].touches(preview.id));
    assert!(document.ship.connections[0].touches(1));
    assert!(document.undo());
    assert_eq!(document.ship, before);
    assert!(!document.history.can_undo());
}

#[test]
fn templates_respect_fuel_rotation_flags_and_occurrence_limits() {
    let mut document = crate::tests::document();
    document.catalog = catalog();
    let mut cursor = EditorCursor {
        catalog_index: 2,
        world: (10.0, 10.0),
        rotation: 1,
        flip_x: true,
        ..default()
    };
    let (tank, _, _) = placement::preview(&document, &cursor).unwrap();
    assert_eq!(tank.fuel, Some(100.0));
    assert_eq!(tank.fuel_kind, Some(dr_core::FuelKind::Tank));
    assert_eq!(tank.angle, 0.0);
    assert!(tank.flip_x);
    cursor.catalog_index = 1;
    assert!(placement::place(&mut document, &cursor));
    let before = document.ship.clone();
    assert!(!placement::preview(&document, &cursor).unwrap().2);
    assert!(!placement::place(&mut document, &cursor));
    assert_eq!(document.ship, before);
    let pod = document
        .catalog
        .get("pod")
        .unwrap()
        .instantiate(99, (0.0, 0.0));
    assert!(pod.pod.unwrap().staging.is_some());
}

#[test]
fn new_ship_includes_hidden_pod_with_staging_at_ground_level() {
    let mut catalog = catalog();
    catalog.types[0].hidden = true;
    let ship = new_ship(&catalog);
    assert_eq!(ship.parts.len(), 1);
    let pod = &ship.parts[0];
    assert_eq!(pod.part_type, "pod");
    assert_eq!((pod.x, pod.y), (0.0, 0.5));
    assert!(!ship.touching_ground);
    assert_eq!(
        pod.pod
            .as_ref()
            .unwrap()
            .staging
            .as_ref()
            .unwrap()
            .current_stage,
        0
    );
    let restored = dr_core::ship_from_xml(&dr_core::ship_to_xml(&ship).unwrap()).unwrap();
    assert_eq!(restored, ship);
}

#[test]
fn collision_preview_rejects_placement_but_retains_dragged_overlap() {
    let mut document = crate::tests::document();
    document.catalog = catalog();
    document.execute(EditorCommand::SetActive(1, true));
    document.undo();
    let before = document.ship.clone();
    let cursor = EditorCursor {
        world: (0.0, 0.0),
        ..default()
    };
    assert!(!placement::preview(&document, &cursor).unwrap().2);
    assert!(!placement::place(&mut document, &cursor));
    assert_eq!(document.ship, before);
    assert!(document.history.can_redo());
    let source = document
        .catalog
        .get("pod")
        .unwrap()
        .instantiate(2, (5.0, 0.0));
    document.ship.parts.push(source);
    let (_, command, clear) = selection::movement(&document, &[PartKey::new(0, 2, 0)], (-5.0, 0.0));
    assert!(!clear);
    assert!(document.execute(command));
    assert_eq!(document.ship.part(2).unwrap().x, 0.0);
    assert!(document.undo());
    assert_eq!(document.ship.part(2).unwrap().x, 5.0);
    document.catalog.types[0].ignore_editor_intersections = true;
    assert!(
        move_with_snap(
            &document.ship,
            &document.catalog,
            PartKey::new(0, 2, 0),
            (0.0, 0.0)
        )
        .is_some()
    );
}

#[test]
fn rejected_rotation_keeps_pose_and_connections_and_valid_rotation_undoes() {
    let mut document = crate::tests::document();
    document.catalog = catalog();
    document.catalog.types[0].width = 4;
    let source = document
        .catalog
        .get("pod")
        .unwrap()
        .instantiate(2, (0.0, 1.25));
    document.ship.parts.push(source);
    document.ship.connections.push(Connection::Normal {
        parent: 1,
        child: 2,
        parent_attach: 1,
        child_attach: 2,
    });
    let before = document.ship.clone();
    placement::transform(
        &mut document,
        PartKey::new(0, 1, 0),
        EditorCommand::Rotate(1),
    );
    assert_eq!(document.ship, before);
    assert!(!document.history.can_undo());
    document.ship.part_mut(2).unwrap().y = 5.0;
    let before = document.ship.clone();
    placement::transform(
        &mut document,
        PartKey::new(0, 1, 0),
        EditorCommand::Rotate(1),
    );
    assert_eq!(document.ship.part(1).unwrap().editor_angle, 1);
    assert!(document.ship.connections.is_empty());
    assert!(document.undo());
    assert_eq!(document.ship, before);
}

#[test]
fn snap_skips_occupied_nearest_anchor_and_commits_the_alternative() {
    let mut document = crate::tests::document();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("PartList.xml");
    std::fs::write(
        &path,
        r#"<PartTypes>
      <PartType id="target" width="2" height="2" ignoreEditorIntersections="true"><AttachPoints>
        <AttachPoint x="1" y="0"/><AttachPoint x="1" y="0.4"/>
      </AttachPoints></PartType>
      <PartType id="source" width="2" height="2" ignoreEditorIntersections="true"><AttachPoints>
        <AttachPoint x="-1" y="0"/>
      </AttachPoints></PartType></PartTypes>"#,
    )
    .unwrap();
    document.catalog = load_catalog(path).unwrap();
    let source_kind = document.catalog.get("source").unwrap();
    let source = source_kind.instantiate(3, (1.1, 0.0));
    document.ship.parts = vec![
        document
            .catalog
            .get("target")
            .unwrap()
            .instantiate(1, (0.0, 0.0)),
        source_kind.instantiate(2, (1.0, 0.0)),
    ];
    document.ship.connections = vec![Connection::Normal {
        parent: 1,
        child: 2,
        parent_attach: 1,
        child_attach: 1,
    }];
    let before = document.ship.clone();
    let mut preview = source.clone();
    let connection =
        placement::snap(&document.ship, &document.catalog, &mut preview, None).unwrap();
    assert!(matches!(
        connection,
        EditorCommand::ConnectParts {
            parent: PartKey {
                group: 0,
                id: 1,
                occurrence: 0
            },
            child: PartKey {
                group: 0,
                id: 3,
                occurrence: 0
            },
            kind: LinkKind::Normal {
                parent_attach: 2,
                child_attach: 1
            }
        }
    ));
    assert!((preview.x - 1.0).abs() < 1e-6);
    assert!((preview.y - 0.2).abs() < 1e-6);
    assert!(document.execute(EditorCommand::Batch(vec![
        EditorCommand::Place(preview.into()),
        connection
    ])));
    assert_eq!(document.ship.connections.len(), 2);
    assert!(document.undo());
    assert_eq!(document.ship, before);
}

#[test]
fn dragging_a_catalog_row_starts_only_a_ghost_and_regular_click_resets_drag_mode() {
    let mut app = App::new();
    let mut document = crate::tests::document();
    document.catalog = catalog();
    let before = document.ship.clone();
    app.insert_resource(document)
        .init_resource::<Palette>()
        .init_resource::<ShipBrowser>()
        .init_resource::<EditorCursor>()
        .init_resource::<DragState>()
        .add_message::<files::FileAction>()
        .add_message::<PanelButton>()
        .add_systems(Update, panel_actions);
    app.world_mut().write_message(PanelButton::DragPart(0));
    app.update();
    let cursor = app.world().resource::<EditorCursor>();
    assert!(cursor.palette_drag && cursor.placing && !cursor.valid);
    assert_eq!(cursor.catalog_index, 0);
    let document = app.world().resource::<EditorDocument>();
    assert_eq!(document.ship, before);
    assert!(!document.history.can_undo());
    app.world_mut().write_message(PanelButton::Part(2));
    app.update();
    let cursor = app.world().resource::<EditorCursor>();
    assert!(cursor.placing && !cursor.palette_drag);
    assert_eq!(cursor.catalog_index, 2);
}
