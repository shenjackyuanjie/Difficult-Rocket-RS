use super::*;

fn document() -> EditorDocument {
    let mut document = crate::tests::document();
    document.catalog = panels::tests::catalog();
    document
}

fn dragged(world: (f64, f64)) -> EditorCursor {
    EditorCursor {
        world,
        placing: true,
        palette_drag: true,
        valid: true,
        ..default()
    }
}

#[test]
fn palette_drag_ghost_uses_the_same_alpha_as_unlinked_parts_for_all_preview_colors() {
    for allowed in [false, true] {
        for connected in [false, true] {
            assert_eq!(
                preview_color(allowed, connected, true).alpha(),
                render::UNLINKED_ALPHA
            );
        }
    }
    assert_eq!(preview_color(true, false, false).alpha(), 0.55);
}

#[test]
fn palette_release_places_and_connects_once_and_undoes_as_a_single_transaction() {
    let mut document = document();
    let before = document.ship.clone();
    let mut cursor = dragged((1.2, 0.0));
    let (ghost, connection, allowed) = preview(&document, &cursor).unwrap();
    assert!(allowed && connection.is_some());
    assert_eq!(document.ship, before);
    assert!(finish_palette_drag(&mut document, &mut cursor, true));
    assert!(!cursor.placing && !cursor.palette_drag);
    assert_eq!(document.ship.part(ghost.id), Some(&ghost));
    assert_eq!(document.ship.connections.len(), 1);
    let after = document.ship.clone();
    assert!(!finish_palette_drag(&mut document, &mut cursor, true));
    assert_eq!(document.ship, after);
    assert!(document.undo());
    assert_eq!(document.ship, before);
    assert!(!document.history.can_undo());
    assert!(document.redo());
    assert_eq!(document.ship, after);
}

#[test]
fn sidebars_invalid_positions_and_collisions_cancel_without_editing_or_losing_redo() {
    let mut document = document();
    assert!(document.execute(EditorCommand::SetActive(1, true)));
    assert!(document.undo());
    let before = document.ship.clone();
    for (canvas, valid, world) in [
        (false, true, (10.0, 10.0)),
        (true, false, (10.0, 10.0)),
        (true, true, (0.0, 0.0)),
    ] {
        let mut cursor = dragged(world);
        cursor.valid = valid;
        assert!(!finish_palette_drag(&mut document, &mut cursor, canvas));
        assert!(!cursor.placing && !cursor.palette_drag);
        assert_eq!(document.ship, before);
        assert!(document.history.can_redo());
        assert!(!document.history.can_undo());
    }
}

#[test]
fn palette_drag_limit_rejection_keeps_document_and_history_unchanged() {
    let mut document = document();
    let engine = document
        .catalog
        .get("engine")
        .unwrap()
        .instantiate(2, (5.0, 0.0));
    document.ship.parts.push(engine);
    let before = document.ship.clone();
    let mut cursor = dragged((10.0, 10.0));
    cursor.catalog_index = 1;
    assert!(!finish_palette_drag(&mut document, &mut cursor, true));
    assert_eq!(document.ship, before);
    assert!(!document.history.can_undo());
    assert!(!cursor.placing && !cursor.palette_drag);
}

#[test]
fn palette_drag_cancels_on_escape_real_focus_loss_or_sidebar_release() {
    for reason in ["escape", "focus", "sidebar"] {
        let mut app = App::new();
        let before = document().ship;
        app.insert_resource(document())
            .insert_resource(dragged((1.2, 0.0)))
            .init_resource::<DragState>()
            .init_resource::<view::ViewOptions>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<panels::UiPointer>()
            .add_systems(Update, mouse_editor);
        let mut window = Window::default();
        window.set_cursor_position(Some(Vec2::new(400.0, 300.0)));
        window.focused = reason != "focus";
        app.world_mut().spawn((window, bevy::window::PrimaryWindow));
        match reason {
            "escape" => app
                .world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .press(KeyCode::Escape),
            "focus" => app
                .world_mut()
                .resource_mut::<ButtonInput<MouseButton>>()
                .press(MouseButton::Left),
            "sidebar" => app.world_mut().resource_mut::<panels::UiPointer>().blocked = true,
            _ => unreachable!(),
        }
        app.update();
        let cursor = app.world().resource::<EditorCursor>();
        assert!(!cursor.placing && !cursor.palette_drag, "{reason}");
        let document = app.world().resource::<EditorDocument>();
        assert_eq!(document.ship, before);
        assert!(!document.history.can_undo());
    }
}

#[test]
fn file_actions_cancel_palette_drag_without_modifying_the_document() {
    let mut app = App::new();
    app.insert_resource(dragged((1.2, 0.0)))
        .add_message::<files::FileAction>()
        .add_systems(Update, cancel_for_file_action);
    app.world_mut().write_message(files::FileAction::New);
    app.update();
    let cursor = app.world().resource::<EditorCursor>();
    assert!(!cursor.placing && !cursor.palette_drag && !cursor.valid);
}

#[test]
fn keyboard_place_during_palette_drag_clears_the_ghost_before_mouse_release() {
    let mut app = App::new();
    app.insert_resource(document())
        .insert_resource(dragged((1.2, 0.0)))
        .init_resource::<DragState>()
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<panels::Palette>()
        .init_resource::<panels::UiPointer>()
        .add_systems(Update, keyboard_commands);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyP);
    app.update();
    let cursor = app.world().resource::<EditorCursor>();
    assert!(!cursor.placing && !cursor.palette_drag);
    let document = app.world().resource::<EditorDocument>();
    assert_eq!(document.ship.parts.len(), 2);
    assert_eq!(document.ship.connections.len(), 1);
}

#[test]
fn palette_preview_follows_small_motion_without_grid_quantization_and_release_clears_selection() {
    let mut document = document();
    let mut cursor = dragged((4.13, 3.17));
    let (part, connection, allowed) = preview(&document, &cursor).unwrap();
    assert_eq!((part.x, part.y), cursor.world);
    assert!(allowed && connection.is_none());
    assert!(finish_palette_drag(&mut document, &mut cursor, true));
    assert!(document.selected_keys().is_empty());
    assert!(!cursor.placing && !cursor.palette_drag);
}
