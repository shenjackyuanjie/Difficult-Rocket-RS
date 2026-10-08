use super::*;

fn document() -> EditorDocument {
    let mut document = crate::tests::document();
    document.catalog = dr_core::catalog_from_xml(
        r#"<PartTypes>
      <PartType id="p" width="2" height="2"><AttachPoints>
        <AttachPoint location="LeftCenter"/><AttachPoint location="RightCenter"/>
        <AttachPoint location="Top"/><AttachPoint location="Bottom"/>
      </AttachPoints></PartType>
    </PartTypes>"#,
    )
    .unwrap();
    let kind = document.catalog.get("p").unwrap();
    document.ship = Ship {
        parts: vec![
            kind.instantiate(1, (-3.0, 0.0)),
            kind.instantiate(2, (3.0, 0.0)),
        ],
        ..default()
    };
    document.saved_ship = document.ship.clone();
    document.free_mode = true;
    document
}

#[test]
fn two_far_points_connect_and_repeat_clicks_unlink_with_atomic_history_and_roundtrip() {
    let mut document = document();
    let original = document.ship.clone();
    let mut cursor = EditorCursor::default();
    assert!(click(&mut document, &mut cursor, (-2.5, 0.0), 0.1));
    assert_eq!(document.ship, original);
    assert_eq!(document.history.undo_len(), 0);
    assert!(click(&mut document, &mut cursor, (2.5, 0.0), 0.1));
    assert!(cursor.manual_connection.is_none());
    assert_eq!(document.ship.connections.len(), 1);
    assert_eq!(document.history.undo_len(), 1);
    let connected = document.ship.clone();
    assert_eq!(
        dr_core::ship_from_xml(&dr_core::ship_to_xml(&connected).unwrap()).unwrap(),
        connected
    );
    assert!(document.undo());
    assert_eq!(document.ship, original);
    assert!(document.redo());
    assert_eq!(document.ship, connected);
    // 反向选点也能精确断开同一条边，不新增反向重复边。
    click(&mut document, &mut cursor, (2.5, 0.0), 0.1);
    click(&mut document, &mut cursor, (-2.5, 0.0), 0.1);
    assert!(document.ship.connections.is_empty());
    assert_eq!(document.history.undo_len(), 2);
    document.undo();
    assert_eq!(document.ship, connected);
}

#[test]
fn free_drag_rotation_and_mirror_keep_external_connections_and_do_not_snap() {
    let mut document = document();
    let mut cursor = EditorCursor::default();
    click(&mut document, &mut cursor, (-2.5, 0.0), 0.1);
    click(&mut document, &mut cursor, (2.5, 0.0), 0.1);
    let connected = document.ship.clone();
    let key = PartKey::new(0, 1, 0);
    let (delta, command, clear) =
        selection::movement_pose(&document, &[key], (-3.0, 0.0), 1, (5.99, 0.01));
    assert_eq!(delta, (5.99, 0.01));
    assert!(clear);
    assert!(document.execute(command));
    assert_eq!(document.ship.connections, connected.connections);
    assert!((document.ship.parts[0].x - 2.99).abs() < 1e-10);
    assert!(document.execute(EditorCommand::TransformSelection {
        parts: vec![key],
        transform: SelectionTransform::FlipX { center: (3.0, 0.0) }
    }));
    assert_eq!(document.ship.connections, connected.connections);
    document.undo();
    document.undo();
    assert_eq!(document.ship, connected);
}

#[test]
fn overlapping_points_can_be_selected_without_automatic_connection_or_ambiguity() {
    let mut document = document();
    document.ship.parts[1].x = -3.0;
    let mut cursor = EditorCursor::default();
    // 完全重叠时先命中上层，第二次同位置命中另一个部件。
    click(&mut document, &mut cursor, (-2.5, 0.0), 0.1);
    assert_eq!(cursor.manual_connection.unwrap().key.id, 2);
    click(&mut document, &mut cursor, (-2.5, 0.0), 0.1);
    assert_eq!(document.ship.connections.len(), 1);
    assert_eq!(
        document.ship.connections[0],
        Connection::Normal {
            parent: 2,
            child: 1,
            parent_attach: 2,
            child_attach: 2
        }
    );
}

#[test]
fn rotation_mirror_and_zoom_aware_edge_hit_use_world_segments() {
    let mut document = document();
    let part = &mut document.ship.parts[0];
    part.angle = 0.37;
    part.flip_y = true;
    let key = PartKey::new(0, 1, 0);
    let kind = document.catalog.get("p").unwrap();
    let edge = dr_core::connections::segment(part, kind, &kind.attach_points[2]);
    let position = (
        edge.0.x * 0.9 + edge.1.x * 0.1,
        edge.0.y * 0.9 + edge.1.y * 0.1,
    );
    let cursor = EditorCursor::default();
    assert_eq!(hit(&document, &cursor, position, 0.001).unwrap().key, key);
    assert_eq!(hit(&document, &cursor, position, 0.001).unwrap().attach, 2);
    assert!(
        hit(
            &document,
            &cursor,
            (position.0 + 0.01, position.1 + 0.01),
            0.001
        )
        .is_none()
    );
    assert!(
        hit(
            &document,
            &cursor,
            (position.0 + 0.01, position.1 + 0.01),
            0.1
        )
        .is_some()
    );
}

#[test]
fn mode_switch_revision_change_and_same_point_cancel_without_mutating_document() {
    let mut document = document();
    let original = document.ship.clone();
    let mut cursor = EditorCursor::default();
    click(&mut document, &mut cursor, (-2.5, 0.0), 0.1);
    click(&mut document, &mut cursor, (-2.5, 0.0), 0.1);
    assert!(cursor.manual_connection.is_none());
    click(&mut document, &mut cursor, (-2.5, 0.0), 0.1);
    document.refresh();
    discard_stale(&document, &mut cursor);
    assert!(cursor.manual_connection.is_none());
    let mut drag = DragState::default();
    cursor.placing = true;
    drag.id = Some(PartKey::new(0, 1, 0));
    set_enabled(&mut document, &mut cursor, &mut drag, false);
    assert!(!document.free_mode && !cursor.placing && drag.id.is_none());
    assert!(!click(&mut document, &mut cursor, (-2.5, 0.0), 0.1));
    assert_eq!(document.ship, original);
    assert_eq!(document.history.undo_len(), 0);
}

#[test]
fn free_placement_and_paste_allow_overlap_but_never_auto_connect() {
    let mut document = document();
    let mut cursor = EditorCursor {
        placing: true,
        valid: true,
        world: (-3.0, 0.0),
        ..default()
    };
    let (preview, connection, allowed) = placement::preview(&document, &cursor).unwrap();
    assert!(allowed && connection.is_none());
    assert_eq!(preview.x, -3.0);
    assert!(placement::place(&mut document, &cursor));
    assert!(document.ship.connections.is_empty());
    document.undo();
    cursor.placing = false;
    cursor.paste = Some(ShipFragment::capture(&document.ship, &[PartKey::new(0, 1, 0)]).unwrap());
    cursor.world = (3.001, 0.002);
    assert!(selection::commit_paste(&mut document, &mut cursor));
    assert!(document.ship.connections.is_empty());
    let pasted = document.ship.all_parts().find(|part| part.id > 2).unwrap();
    assert!((pasted.x - 3.001).abs() < 1e-12 && (pasted.y - 0.002).abs() < 1e-12);
}

#[test]
fn free_preview_markers_follow_the_same_fine_rotation_and_mirrors_as_the_ghost() {
    let document = document();
    let mut cursor = EditorCursor {
        placing: true,
        valid: true,
        world: (3.7, -2.1),
        fine_rotation: 0.37,
        flip_x: true,
        flip_y: true,
        ..default()
    };
    let segments = preview_segments(&document, &cursor);
    let (part, _, _) = placement::preview(&document, &cursor).unwrap();
    let kind = document.catalog.get(&part.part_type).unwrap();
    assert_eq!(segments.len(), kind.attach_points.len());
    for (segment, attach) in segments.iter().zip(&kind.attach_points) {
        assert_eq!(*segment, dr_core::connections::segment(&part, kind, attach));
    }
    cursor.valid = false;
    assert!(preview_segments(&document, &cursor).is_empty());
}
