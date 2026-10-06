use super::*;

fn document() -> EditorDocument {
    let mut document = crate::tests::document();
    document.catalog = panels::tests::catalog();
    document
}

#[test]
fn nearby_hints_precede_snap_and_match_its_contact_without_editing() {
    let document = document();
    let before = document.ship.clone();
    let kind = document.catalog.get("pod").unwrap();
    let mut source = kind.instantiate(2, (1.5, 0.0));
    let hints = nearby(&document, &source, None);
    assert_eq!(hints.len(), 1);
    assert!(!hints[0].snapped);
    assert!(placement::snap(&document.ship, &document.catalog, &mut source, None).is_none());
    source.x = 1.2;
    assert!(placement::snap(&document.ship, &document.catalog, &mut source, None).is_some());
    let hints = nearby(&document, &source, None);
    assert_eq!(hints.len(), 1);
    assert!(hints[0].snapped);
    assert_eq!(hints[0].contact.0, hints[0].contact.1);
    assert_eq!(document.ship, before);
    assert!(!document.history.can_undo());
}

#[test]
fn occupied_ambiguous_and_colliding_candidates_are_not_advertised() {
    let mut document = document();
    let kind = document.catalog.get("pod").unwrap().clone();
    let source = kind.instantiate(3, (1.5, 0.0));
    document.ship.parts.push(kind.instantiate(2, (1.0, 0.0)));
    document.ship.connections.push(Connection::Normal {
        parent: 1,
        child: 2,
        parent_attach: 2,
        child_attach: 1,
    });
    // 已占用的 0.5 接触点不能提示，但另一个部件的空闲点仍可连接。
    assert!(
        nearby(&document, &source, None)
            .iter()
            .all(|hint| hint.contact.1.x != 0.5)
    );
    document.ship.connections.clear();
    document.ship.parts[1] = document
        .catalog
        .get("engine")
        .unwrap()
        .instantiate(2, (1.0, 0.0));
    assert!(nearby(&document, &source, None).is_empty());
    document.ship.parts[1] = document.ship.parts[0].clone();
    assert!(nearby(&document, &source, None).is_empty());
}

#[test]
fn moving_source_ignores_itself_and_old_links_but_not_foreign_instances() {
    let mut document = document();
    let kind = document.catalog.get("pod").unwrap().clone();
    document.ship.parts.push(kind.instantiate(2, (1.0, 0.0)));
    document.ship.connections.push(Connection::Normal {
        parent: 1,
        child: 2,
        parent_attach: 2,
        child_attach: 1,
    });
    let key = PartKey::new(0, 2, 0);
    let source = kind.instantiate(2, (1.5, 0.0));
    assert_eq!(nearby(&document, &source, Some(key)).len(), 1);
    document.ship.disconnected.push(dr_core::ShipGroup {
        parts: vec![kind.instantiate(2, (1.0, 0.0))],
        connections: vec![],
    });
    assert!(
        nearby(&document, &source, Some(key))
            .iter()
            .all(|hint| hint.contact.1.x != 0.5)
    );
}

#[test]
fn group_hint_uses_the_validated_whole_selection_snap() {
    let mut document = document();
    let kind = document.catalog.get("pod").unwrap();
    document.ship.parts = vec![
        kind.instantiate(1, (0.0, 0.0)),
        kind.instantiate(2, (0.0, 2.0)),
        kind.instantiate(3, (4.0, 0.0)),
    ];
    let keys = [PartKey::new(0, 1, 0), PartKey::new(0, 2, 0)];
    let (delta, command, allowed) = selection::movement(&document, &keys, (2.8, 0.0));
    assert!(allowed);
    let drag = DragState {
        id: Some(keys[0]),
        members: keys.into_iter().collect(),
        preview: delta,
        ..default()
    };
    let hints = group_hints(&document, &drag, &command);
    assert_eq!(hints.len(), 1);
    assert!(hints[0].snapped);
    assert!(hints[0].contact.0.distance(hints[0].contact.1) < 1e-6);
    assert!(!document.history.can_undo());
}

#[test]
fn docking_group_hint_marks_attachment_contact_not_part_centers() {
    let mut document = document();
    document.catalog = dr_core::catalog_from_xml(
        r#"<PartTypes>
        <PartType id="plug" type="dockconnector" width="2" height="2"><AttachPoints>
            <AttachPoint location="RightCenter" dock="true"/>
        </AttachPoints></PartType>
        <PartType id="port" type="dockport" width="2" height="2"><AttachPoints>
            <AttachPoint location="LeftCenter" dock="true"/>
        </AttachPoints></PartType>
    </PartTypes>"#,
    )
    .unwrap();
    document.ship.parts = vec![
        document
            .catalog
            .get("plug")
            .unwrap()
            .instantiate(1, (0.0, 0.0)),
        document
            .catalog
            .get("port")
            .unwrap()
            .instantiate(2, (1.0, 0.0)),
    ];
    let source = PartKey::new(0, 1, 0);
    let drag = DragState {
        id: Some(source),
        ..default()
    };
    let command = EditorCommand::ConnectParts {
        parent: PartKey::new(0, 2, 0),
        child: source,
        kind: LinkKind::Dock { connector: source },
    };
    let hints = group_hints(&document, &drag, &command);
    assert_eq!(hints.len(), 1);
    assert_eq!(
        hints[0].contact,
        (Vec2d { x: 0.5, y: 0.0 }, Vec2d { x: 0.5, y: 0.0 })
    );
}

#[test]
fn palette_quantity_limit_does_not_advertise_a_connection_that_cannot_be_placed() {
    let mut document = document();
    document
        .catalog
        .types
        .iter_mut()
        .find(|kind| kind.id == "pod")
        .unwrap()
        .max_occurrences = Some(1);
    let source = document
        .catalog
        .get("pod")
        .unwrap()
        .instantiate(2, (1.5, 0.0));
    assert!(nearby(&document, &source, None).is_empty());
}
