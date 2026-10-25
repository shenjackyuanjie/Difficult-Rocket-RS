use super::*;
use crate::canvas_input::{keyboard_commands, mouse_editor};

fn document() -> EditorDocument {
    let mut document = crate::test_support::document();
    document.catalog = crate::test_support::catalog();
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
fn snap_connection_keeps_instance_keys_and_converts_normal_indices_to_one_based() {
    let source = PartKey::new(2, 7, 1);
    let target = PartKey::new(3, 7, 2);
    for (source_index, target_index) in [(0, 0), (2, 5)] {
        let candidate = dr_core::SnapCandidate {
            source_index,
            target_index,
            position: dr_core::Vec2d { x: 1.25, y: -3.5 },
            distance: 0.2,
            dock: false,
        };
        let EditorCommand::ConnectParts {
            parent,
            child,
            kind:
                LinkKind::Normal {
                    parent_attach,
                    child_attach,
                },
        } = connection_command(source, target, PartKind::DockConnector, &candidate)
        else {
            panic!("普通候选必须生成 Normal 连接");
        };
        assert_eq!((parent, child), (target, source));
        assert_eq!(parent_attach, target_index as i32 + 1);
        assert_eq!(child_attach, source_index as i32 + 1);
    }
}

#[test]
fn snap_connection_uses_the_connector_key_on_either_side_of_docking() {
    let source = PartKey::new(2, 7, 1);
    let target = PartKey::new(3, 7, 2);
    let candidate = dr_core::SnapCandidate {
        source_index: 2,
        target_index: 5,
        position: dr_core::Vec2d { x: 1.25, y: -3.5 },
        distance: 0.2,
        dock: true,
    };
    for (source_kind, expected) in [
        (PartKind::DockConnector, source),
        (PartKind::DockPort, target),
    ] {
        let EditorCommand::ConnectParts {
            parent,
            child,
            kind: LinkKind::Dock { connector },
        } = connection_command(source, target, source_kind, &candidate)
        else {
            panic!("对接候选必须生成 Dock 连接");
        };
        assert_eq!((parent, child), (target, source));
        assert_eq!(connector, expected);
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
            .init_resource::<CameraDrag>()
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
        .init_resource::<crate::properties::Inspector>()
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
fn palette_preview_quantizes_small_motion_and_release_clears_selection() {
    let mut document = document();
    let mut cursor = dragged((4.13, 3.17));
    let (part, connection, allowed) = preview(&document, &cursor).unwrap();
    // 常规模式落点按 0.5 量化：4.13 与 3.17 都不足半格，回落到 4.0 与 3.0。
    assert_eq!((part.x, part.y), (4.0, 3.0));
    assert!(allowed && connection.is_none());
    assert!(finish_palette_drag(&mut document, &mut cursor, true));
    assert!(document.selected_keys().is_empty());
    assert!(!cursor.placing && !cursor.palette_drag);
}

#[test]
fn placement_snap_and_free_geometry_are_independent_in_all_four_combinations() {
    for free_mode in [false, true] {
        for snap_enabled in [false, true] {
            let mut document = document();
            document.free_mode = free_mode;
            document.snap_enabled = snap_enabled;
            let before = document.ship.clone();
            let mut cursor = dragged((1.2, 0.0));
            let (ghost, connection, allowed) = preview(&document, &cursor).unwrap();
            assert!(allowed, "free={free_mode}, snap={snap_enabled}");
            assert_eq!(connection.is_some(), snap_enabled);
            assert_eq!(
                (ghost.x, ghost.y),
                (if snap_enabled { 1.0 } else { 1.2 }, 0.0),
                "free={free_mode} snap={snap_enabled} ghost=({}, {})",
                ghost.x,
                ghost.y
            );
            assert_eq!(document.ship, before);
            assert!(finish_palette_drag(&mut document, &mut cursor, true));
            assert_eq!(document.ship.part(ghost.id), Some(&ghost));
            assert_eq!(document.ship.connections.len(), usize::from(snap_enabled));
            assert_eq!(document.history.undo_len(), 1);
            let after = document.ship.clone();
            assert!(document.undo());
            assert_eq!(document.ship, before);
            assert!(!document.history.can_undo());
            assert!(document.redo());
            assert_eq!(document.ship, after);
            assert_eq!(
                (document.free_mode, document.snap_enabled),
                (free_mode, snap_enabled)
            );
        }
    }
}

#[test]
fn placement_collision_policy_depends_only_on_free_mode() {
    for free_mode in [false, true] {
        for snap_enabled in [false, true] {
            let mut document = document();
            document.free_mode = free_mode;
            document.snap_enabled = snap_enabled;
            let before = document.ship.clone();
            // 发动机没有连接点，精确重叠时吸附开关不改变碰撞结论。
            let cursor = EditorCursor {
                catalog_index: 1,
                ..dragged((0.0, 0.0))
            };
            let (ghost, connection, allowed) = preview(&document, &cursor).unwrap();
            assert_eq!(allowed, free_mode);
            assert!(connection.is_none());
            assert_eq!(place(&mut document, &cursor), free_mode);
            if free_mode {
                assert_eq!(document.ship.part(ghost.id), Some(&ghost));
                assert_eq!(document.history.undo_len(), 1);
                assert!(document.undo());
            } else {
                assert!(!document.history.can_undo());
            }
            assert_eq!(document.ship, before);
        }
    }
}

#[test]
fn placement_quantity_rotation_and_mirror_contracts_survive_both_switches() {
    for free_mode in [false, true] {
        for snap_enabled in [false, true] {
            let mut document = document();
            document.free_mode = free_mode;
            document.snap_enabled = snap_enabled;
            let cursor = EditorCursor {
                catalog_index: 2,
                rotation: 3,
                fine_rotation: 0.37,
                flip_x: true,
                flip_y: true,
                ..dragged((8.13, 3.17))
            };
            let (ghost, connection, allowed) = preview(&document, &cursor).unwrap();
            assert!(allowed && connection.is_none());
            assert_eq!((ghost.angle, ghost.editor_angle), (0.0, 0));
            assert!(ghost.flip_x && ghost.flip_y);
            assert!(place(&mut document, &cursor));
            assert_eq!(document.ship.part(ghost.id), Some(&ghost));
            assert!(document.undo());
            document.ship.parts.push(
                document
                    .catalog
                    .get("engine")
                    .unwrap()
                    .instantiate(2, (5.0, 0.0)),
            );
            let before = document.ship.clone();
            let cursor = EditorCursor {
                catalog_index: 1,
                ..dragged((10.0, 10.0))
            };
            assert!(!preview(&document, &cursor).unwrap().2);
            assert!(!place(&mut document, &cursor));
            assert_eq!(document.ship, before);
            assert!(!document.history.can_undo());
            assert!(document.history.can_redo());
        }
    }
}
