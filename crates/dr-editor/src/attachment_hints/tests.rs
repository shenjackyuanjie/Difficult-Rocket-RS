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
fn placing_near_a_dangling_history_edge_hints_and_commits_the_same_new_id() {
    let mut document = document();
    let kind = document.catalog.get("pod").unwrap();
    document.ship.parts = vec![kind.instantiate(1, (0.0, 0.0))];
    // 部件 2 已被删除，连接里只剩下历史悬空引用。
    document.ship.connections = vec![Connection::Normal {
        parent: 1,
        child: 2,
        parent_attach: 1,
        child_attach: 1,
    }];
    let before = document.ship.clone();
    let cursor = EditorCursor {
        world: (1.2, 0.0),
        placing: true,
        valid: true,
        ..default()
    };
    let (preview, connection, allowed) = placement::preview(&document, &cursor).unwrap();
    assert!(allowed);
    assert_eq!((preview.id, preview.x), (3, 1.0));
    let hints = nearby(&document, &preview, None);
    assert_eq!(hints.len(), 1);
    assert!(hints[0].snapped);
    assert_eq!(hints[0].contact.0, hints[0].contact.1);
    assert_eq!(document.ship, before);
    assert!(!document.history.can_undo());
    let (hint_parent, hinted) = match connection {
        Some(EditorCommand::ConnectParts {
            parent,
            child,
            kind:
                LinkKind::Normal {
                    parent_attach,
                    child_attach,
                },
        }) => (
            parent,
            Connection::Normal {
                parent: parent.id,
                child: child.id,
                parent_attach,
                child_attach,
            },
        ),
        other => panic!("新放置的提示应给出普通连接候选: {other:?}"),
    };
    assert_eq!(hint_parent, PartKey::new(0, 1, 0));
    assert!(hinted.touches(preview.id));

    assert!(placement::place(&mut document, &cursor));
    assert_eq!(document.ship.part(3).unwrap().x, 1.0);
    // 历史悬空引用仍是 2，新部件只被提示那一对连接引用。
    assert!(document.ship.part(2).is_none());
    assert_eq!(document.ship.connections.len(), 2);
    assert!(document.ship.connections.contains(&hinted));
    assert!(document.ship.connections.contains(&Connection::Normal {
        parent: 1,
        child: 2,
        parent_attach: 1,
        child_attach: 1,
    }));
    assert_eq!(
        document
            .ship
            .connections
            .iter()
            .filter(|connection| connection.touches(3))
            .count(),
        1
    );
    let after = document.ship.clone();
    assert!(document.undo());
    assert_eq!(document.ship, before);
    assert!(document.redo());
    assert_eq!(document.ship, after);
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

fn multi_attachment_document() -> EditorDocument {
    let mut document = document();
    document.catalog = dr_core::catalog_from_xml(
        r#"<PartTypes>
        <PartType id="source" type="strut" width="8" height="2"><AttachPoints>
            <AttachPoint location="BottomCenter"/>
            <AttachPoint location="BottomSide"/>
        </AttachPoints></PartType>
        <PartType id="target" type="strut" width="2" height="2"><AttachPoints>
            <AttachPoint location="TopCenter"/>
            <AttachPoint location="TopSide"/>
        </AttachPoints></PartType>
    </PartTypes>"#,
    )
    .unwrap();
    let target = document.catalog.get("target").unwrap();
    document.ship.parts = vec![target.instantiate(1, (0.0, 0.0))];
    document
}

#[test]
fn multiple_attachment_candidates_show_one_pair_and_match_committed_snap() {
    let mut document = multi_attachment_document();
    let mut source = document
        .catalog
        .get("source")
        .unwrap()
        .instantiate(2, (0.0, 1.1));
    let st = document.catalog.get("source").unwrap();
    let target = &document.ship.parts[0];
    let tt = document.catalog.get("target").unwrap();
    let candidates = dr_core::connections::candidates(&source, st, target, tt, 0.75);
    assert!(candidates.len() > 1);
    let hints = nearby(&document, &source, None);
    assert_eq!(hints.len(), 1);
    assert_eq!(
        hints[0].source,
        dr_core::connections::segment(&source, st, &st.attach_points[candidates[0].source_index])
    );
    assert_eq!(
        hints[0].target,
        dr_core::connections::segment(target, tt, &tt.attach_points[candidates[0].target_index])
    );
    let command = placement::snap(&document.ship, &document.catalog, &mut source, None).unwrap();
    assert!(document.execute(EditorCommand::Batch(vec![
        EditorCommand::Place(Box::new(source)),
        command
    ])));
    assert_eq!(document.ship.connections.len(), 1);
    assert!(document.undo());
    assert!(document.ship.connections.is_empty());
    assert!(document.redo());
    assert_eq!(document.ship.connections.len(), 1);
}

#[test]
fn reducing_attachment_hints_keeps_one_candidate_for_each_different_target() {
    let mut document = multi_attachment_document();
    let target = document.catalog.get("target").unwrap();
    document.ship.parts = vec![
        target.instantiate(1, (-1.0, 0.0)),
        target.instantiate(2, (1.0, 0.0)),
    ];
    let source = document
        .catalog
        .get("source")
        .unwrap()
        .instantiate(3, (0.0, 1.1));
    let hints = nearby(&document, &source, None);
    assert_eq!(hints.len(), 2);
    assert!(hints[0].contact.1.x < 0.0);
    assert!(hints[1].contact.1.x > 0.0);
    assert!(!document.history.can_undo());
}
