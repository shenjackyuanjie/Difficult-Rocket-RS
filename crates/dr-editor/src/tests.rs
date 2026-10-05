use super::*;

pub(super) fn document() -> EditorDocument {
    let ship =
        dr_core::ship_from_xml(r#"<Ship><Parts><Part id="1" partType="pod"/></Parts></Ship>"#)
            .unwrap();
    EditorDocument {
        revision: 0,
        saved_ship: ship.clone(),
        ship,
        catalog: PartCatalog::default(),
        selected: None,
        selection: default(),
        dirty: false,
        history: EditorHistory::default(),
        status: String::new(),
    }
}

#[test]
fn undo_to_saved_state_clears_dirty_and_stale_selection() {
    let mut document = document();
    let mut part = document.ship.parts[0].clone();
    part.id = 2;
    assert!(document.execute(EditorCommand::Place(part.into())));
    document.selected = Some(PartKey::new(0, 2, 0));
    assert!(document.dirty);
    assert!(document.undo());
    assert!(!document.dirty);
    assert_eq!(document.selected, None);
    assert!(document.redo());
    assert!(document.dirty);
    document.saved_ship = document.ship.clone();
    document.refresh();
    assert!(!document.dirty);
    assert!(!document.redo());
    assert!(!document.dirty);
}

#[test]
fn failed_and_no_op_edits_do_not_mark_document_dirty() {
    let mut document = document();
    assert!(!document.execute(EditorCommand::Delete(9)));
    assert!(!document.dirty);
    assert!(!document.status.is_empty());
    assert!(document.execute(EditorCommand::Move {
        id: 1,
        from: (0.0, 0.0),
        to: (0.0, 0.0)
    }));
    assert!(!document.dirty);
    assert!(!document.history.can_undo());
}

#[test]
fn moving_disconnects_old_links_in_the_same_undo_step() {
    let mut document = document();
    let mut part = document.ship.parts[0].clone();
    part.id = 2;
    document.ship.parts.push(part);
    document.ship.connections.push(Connection::Normal {
        parent: 1,
        child: 2,
        parent_attach: 1,
        child_attach: 1,
    });
    let before = document.ship.clone();
    let command = move_with_snap(
        &document.ship,
        &document.catalog,
        PartKey::new(0, 2, 0),
        (5.0, 5.0),
    )
    .unwrap();
    assert!(document.execute(command));
    assert!(document.ship.connections.is_empty());
    assert_eq!(document.ship.part(2).unwrap().x, 5.0);
    assert!(document.undo());
    assert_eq!(document.ship, before);
}

#[test]
fn loading_errors_are_reported_instead_of_opening_an_empty_document() {
    assert!(load_document(Some("missing-ship.xml"), "missing-catalog.xml").is_err());
}

#[test]
fn snapped_connections_use_one_based_sr1_indices() {
    let mut document = document();
    let mut kind = dr_core::PartType {
        shapes: vec![],
        id: "pod".into(),
        name: "测试部件".into(),
        description: String::new(),
        sprite: String::new(),
        kind: PartKind::Strut,
        mass: 1.0,
        width: 2,
        height: 2,
        category: String::new(),
        hidden: false,
        ignore_editor_intersections: false,
        disable_editor_rotation: false,
        max_occurrences: None,
        friction: None,
        can_explode: None,
        cover_height: None,
        sandbox_only: None,
        drag: None,
        buoyancy: None,
        damage: None,
        rcs: None,
        solar: None,
        lander: None,
        tank: None,
        engine: None,
        attach_points: vec![],
    };
    for x in [-1.0, 1.0] {
        kind.attach_points.push(dr_core::AttachPoint {
            location: String::new(),
            flip_y: false,
            x,
            y: 0.0,
            dock: false,
            fuel_line: false,
            flip_x: false,
            group: None,
            order: None,
            break_angle: None,
            break_force: None,
        });
    }
    document.catalog = PartCatalog::new("测试", vec![kind]);
    let mut part = document.ship.parts[0].clone();
    part.id = 2;
    part.x = 5.0;
    document.ship.parts.push(part);
    let before = document.ship.clone();
    let command = move_with_snap(
        &document.ship,
        &document.catalog,
        PartKey::new(0, 2, 0),
        (1.2, 0.0),
    )
    .unwrap();
    assert!(document.execute(command));
    assert_eq!(document.ship.part(2).unwrap().x, 1.0);
    assert_eq!(
        document.ship.connections,
        vec![Connection::Normal {
            parent: 1,
            child: 2,
            parent_attach: 2,
            child_attach: 1,
        }]
    );
    let (parent, child) = dr_core::connections::positions(
        &document.ship,
        &document.catalog,
        &document.ship.connections[0],
    )
    .unwrap();
    assert_eq!(parent, Vec2d { x: 0.5, y: 0.0 });
    assert_eq!(parent, child);
    assert!(document.undo());
    assert_eq!(document.ship, before);
}
