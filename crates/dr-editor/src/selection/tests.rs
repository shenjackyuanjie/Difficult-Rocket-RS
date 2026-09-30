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
