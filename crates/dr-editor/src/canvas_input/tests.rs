use super::*;
use crate::test_support::mouse_app;
use crate::{egui_ui, files, help, properties, topology_ui};
use dr_core::{Connection, PartKey};

fn pointer(app: &mut App, logical: (f64, f64)) {
    let mut windows = app.world_mut().query::<&mut Window>();
    let mut window = windows.single_mut(app.world_mut()).unwrap();
    window.set_cursor_position(Some(Vec2::new(
        720.0 + logical.0 as f32 * 60.0,
        450.0 - logical.1 as f32 * 60.0,
    )));
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .clear();
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .clear();
}

#[test]
fn cancel_preview_drops_pending_edits_but_keeps_reusable_cursor_settings() {
    let mut document = crate::test_support::document();
    document.catalog = crate::test_support::catalog();
    document.free_mode = true;
    let key = PartKey::new(0, 1, 0);
    let fragment = dr_core::ShipFragment::capture(&document.ship, &[key]).unwrap();
    let mut cursor = EditorCursor {
        clipboard: Some(fragment.clone()),
        paste: Some(fragment),
        world: (3.0, 4.0),
        catalog_index: 2,
        placing: true,
        palette_drag: true,
        rotation: 3,
        fine_rotation: 0.25,
        flip_x: true,
        flip_y: true,
        valid: true,
        ..default()
    };
    assert!(free_mode::click(
        &mut document,
        &mut cursor,
        (0.5, 0.0),
        0.1
    ));
    assert!(cursor.manual_connection.is_some());
    let before = document.ship.clone();
    let selected = document.selected_keys();
    let mut drag = DragState {
        id: Some(key),
        members: [key].into_iter().collect(),
        blocked: true,
        command: Some(EditorCommand::Delete(key.id)),
        rectangle: Some((1.0, 2.0)),
        turns: 2,
        fine_rotation: 0.5,
        flip_x: true,
        flip_y: true,
        ..default()
    };
    let mut camera_drag = CameraDrag(Some(Vec2::new(10.0, 20.0)));
    // 重复取消仍应保留可复用设置，不产生文档事务。
    for _ in 0..2 {
        cancel_preview(&mut drag, &mut cursor, &mut camera_drag);
        assert!(drag.id.is_none() && drag.members.is_empty());
        assert!(drag.command.is_none() && drag.rectangle.is_none() && !drag.blocked);
        assert_eq!((drag.turns, drag.fine_rotation), (0, 0.0));
        assert!(!drag.flip_x && !drag.flip_y);
        assert!(cursor.manual_connection.is_none() && cursor.paste.is_none());
        assert!(!cursor.placing && !cursor.palette_drag);
        assert!(camera_drag.0.is_none());
        assert!(cursor.valid);
        assert_eq!(cursor.world, (3.0, 4.0));
        assert_eq!(cursor.catalog_index, 2);
        assert_eq!((cursor.rotation, cursor.fine_rotation), (3, 0.25));
        assert!(cursor.flip_x && cursor.flip_y);
        assert_eq!(
            cursor
                .clipboard
                .as_ref()
                .unwrap()
                .parts()
                .collect::<Vec<_>>(),
            before.parts.iter().collect::<Vec<_>>()
        );
        assert_eq!(document.ship, before);
        assert_eq!(document.selected_keys(), selected);
        assert!(!document.dirty && !document.history.can_undo());
    }
}

#[test]
fn actual_mouse_drag_follows_sub_grid_motion_and_release_cleans_highlight_and_history() {
    let mut app = mouse_app();
    let before = app.world().resource::<EditorDocument>().ship.clone();
    pointer(&mut app, (0.0, 0.0));
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    app.update();
    pointer(&mut app, (0.1, 0.13));
    app.update();
    let drag = app.world().resource::<DragState>();
    assert!((drag.preview.0 - 0.1).abs() < 1e-5);
    assert!((drag.preview.1 - 0.13).abs() < 1e-5);
    assert_eq!(app.world().resource::<EditorDocument>().ship, before);
    // 光标在松手帧继续移动；落点应包含这一帧末段位移。
    pointer(&mut app, (0.18, 0.21));
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .release(MouseButton::Left);
    app.update();
    let drag = app.world().resource::<DragState>();
    assert!(
        drag.id.is_none() && drag.members.is_empty() && drag.command.is_none() && !drag.blocked
    );
    let document = app.world().resource::<EditorDocument>();
    assert!(document.selected_keys().is_empty());
    assert!((document.ship.parts[0].x - 0.18).abs() < 1e-5);
    assert!((document.ship.parts[0].y - 0.21).abs() < 1e-5);
    assert!(document.history.can_undo());
    app.update();
    assert!(app.world_mut().resource_mut::<EditorDocument>().undo());
    let document = app.world().resource::<EditorDocument>();
    assert_eq!(document.ship, before);
    assert!(!document.history.can_undo());
}

#[test]
fn background_left_click_clears_selection_for_both_box_button_settings() {
    for button in [view::BoxSelectButton::Left, view::BoxSelectButton::Middle] {
        let mut app = mouse_app();
        app.world_mut()
            .resource_mut::<view::ViewOptions>()
            .box_select_button = button;
        app.world_mut()
            .resource_mut::<EditorDocument>()
            .select_only(Some(PartKey::new(0, 1, 0)));
        pointer(&mut app, (3.0, 3.0));
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        assert!(
            app.world()
                .resource::<EditorDocument>()
                .selected_keys()
                .is_empty()
        );
        assert_eq!(
            app.world().resource::<DragState>().rectangle.is_some(),
            button == view::BoxSelectButton::Left
        );
        assert!(!app.world().resource::<EditorDocument>().history.can_undo());
    }
}

#[test]
fn clicking_non_grid_part_keeps_its_pose_connections_and_selection() {
    let mut app = mouse_app();
    {
        let mut document = app.world_mut().resource_mut::<EditorDocument>();
        document.ship.parts[0].x = 0.13;
        document.ship.parts[0].y = 0.17;
    }
    let before = app.world().resource::<EditorDocument>().ship.clone();
    pointer(&mut app, (0.13, 0.17));
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    app.update();
    pointer(&mut app, (0.13, 0.17));
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .release(MouseButton::Left);
    app.update();
    let document = app.world().resource::<EditorDocument>();
    assert_eq!(document.ship, before);
    assert_eq!(document.selected, Some(PartKey::new(0, 1, 0)));
    assert!(!document.history.can_undo());
    assert!(app.world().resource::<DragState>().members.is_empty());
}

#[test]
fn missed_release_event_still_finishes_drag_and_sidebar_release_cancels() {
    for sidebar in [false, true] {
        let mut app = mouse_app();
        let before = app.world().resource::<EditorDocument>().ship.clone();
        pointer(&mut app, (0.0, 0.0));
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        pointer(&mut app, (0.1, 0.1));
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .reset_all();
        app.world_mut().resource_mut::<panels::UiPointer>().blocked = sidebar;
        app.update();
        assert!(app.world().resource::<DragState>().id.is_none());
        assert!(app.world().resource::<DragState>().members.is_empty());
        if sidebar {
            assert_eq!(app.world().resource::<EditorDocument>().ship, before);
        } else {
            assert_ne!(app.world().resource::<EditorDocument>().ship, before);
        }
    }
}

#[test]
fn held_drag_rotates_preview_blocks_clipboard_shortcuts_and_cancels_atomically() {
    let mut app = mouse_app();
    app.init_resource::<panels::Palette>()
        .init_resource::<properties::Inspector>()
        .add_systems(Update, keyboard_commands.after(mouse_editor));
    let before = app.world().resource::<EditorDocument>().ship.clone();
    pointer(&mut app, (0.0, 0.0));
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    app.update();
    pointer(&mut app, (2.1, 1.3));
    {
        let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        for code in [
            KeyCode::KeyR,
            KeyCode::KeyX,
            KeyCode::KeyY,
            KeyCode::KeyC,
            KeyCode::ControlLeft,
        ] {
            keys.press(code);
        }
    }
    app.update();
    let document = app.world().resource::<EditorDocument>();
    let drag = app.world().resource::<DragState>();
    assert_eq!(drag.turns, 1);
    let preview = drag.pose(PartKey::new(0, 1, 0), &document.ship.parts[0]);
    assert_eq!(preview.editor_angle, 1);
    assert!(!preview.flip_x && !preview.flip_y);
    assert_eq!(document.ship, before);
    assert!(!document.dirty && !document.history.can_undo());
    assert!(app.world().resource::<EditorCursor>().clipboard.is_none());
    pointer(&mut app, (2.1, 1.3));
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Escape);
    app.update();
    assert_eq!(app.world().resource::<EditorDocument>().ship, before);
    assert_eq!(app.world().resource::<DragState>().turns, 0);
}

#[test]
fn fine_rotation_and_mirrors_compose_during_drag_and_commit_in_one_undo() {
    for cancel in [false, true] {
        let mut app = mouse_app();
        let before = app.world().resource::<EditorDocument>().ship.clone();
        pointer(&mut app, (0.0, 0.0));
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        let mut expected_angle = 0.0_f64;
        let mut expected_flips = (false, false);
        for (code, fine) in [
            (KeyCode::KeyE, false),
            (KeyCode::KeyE, true),
            (KeyCode::KeyX, false),
            (KeyCode::KeyQ, true),
            (KeyCode::KeyY, false),
            (KeyCode::KeyR, false),
        ] {
            pointer(&mut app, (4.0, 3.0));
            let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            keys.reset_all();
            if fine {
                keys.press(KeyCode::ShiftLeft);
            }
            keys.press(code);
            app.update();
            match code {
                KeyCode::KeyE => {
                    expected_angle += if fine { 1.0_f64 } else { 15.0_f64 }.to_radians()
                }
                KeyCode::KeyQ => {
                    expected_angle -= if fine { 1.0_f64 } else { 15.0_f64 }.to_radians()
                }
                KeyCode::KeyR => expected_angle += std::f64::consts::FRAC_PI_2,
                KeyCode::KeyX => {
                    expected_angle = -expected_angle;
                    expected_flips.0 = !expected_flips.0;
                }
                KeyCode::KeyY => {
                    expected_angle = -expected_angle;
                    expected_flips.1 = !expected_flips.1;
                }
                _ => unreachable!(),
            }
            let document = app.world().resource::<EditorDocument>();
            let drag = app.world().resource::<DragState>();
            let part = drag.pose(PartKey::new(0, 1, 0), &document.ship.parts[0]);
            assert!((part.angle - expected_angle.rem_euclid(std::f64::consts::TAU)).abs() < 1e-10);
            assert_eq!((part.flip_x, part.flip_y), expected_flips);
            assert_eq!(document.ship, before);
            assert!(!document.history.can_undo());
        }
        pointer(&mut app, (4.0, 3.0));
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .reset_all();
        if cancel {
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .press(KeyCode::Escape);
        } else {
            app.world_mut()
                .resource_mut::<ButtonInput<MouseButton>>()
                .release(MouseButton::Left);
        }
        app.update();
        let mut document = app.world_mut().resource_mut::<EditorDocument>();
        if cancel {
            assert_eq!(document.ship, before);
            assert!(!document.history.can_undo());
        } else {
            assert_eq!(document.history.undo_len(), 1);
            assert!(
                (document.ship.parts[0].angle - expected_angle.rem_euclid(std::f64::consts::TAU))
                    .abs()
                    < 1e-10
            );
            document.undo();
            assert_eq!(document.ship, before);
        }
    }
}

#[test]
fn free_mouse_connection_consumes_click_and_cancels_on_escape_focus_loss_or_ui() {
    for cancel in ["escape", "focus", "modal"] {
        let mut app = mouse_app();
        app.world_mut().resource_mut::<EditorDocument>().free_mode = true;
        pointer(&mut app, (0.5, 0.0));
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        assert!(
            app.world()
                .resource::<EditorCursor>()
                .manual_connection
                .is_some()
        );
        assert!(app.world().resource::<DragState>().id.is_none());
        match cancel {
            "escape" => app
                .world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .press(KeyCode::Escape),
            "focus" => {
                let mut query = app.world_mut().query::<&mut Window>();
                query.single_mut(app.world_mut()).unwrap().focused = false;
            }
            "modal" => {
                app.init_resource::<properties::Inspector>()
                    .init_resource::<files::PendingFileAction>()
                    .init_resource::<help::HelpState>()
                    .init_resource::<topology_ui::ConnectionEditor>()
                    .add_systems(Update, egui_ui::prepare_input.before(mouse_editor));
                app.world_mut()
                    .resource_mut::<topology_ui::ConnectionEditor>()
                    .open = true;
            }
            _ => unreachable!(),
        }
        app.update();
        assert!(
            app.world()
                .resource::<EditorCursor>()
                .manual_connection
                .is_none()
        );
        assert!(!app.world().resource::<EditorDocument>().history.can_undo());
    }
}

#[test]
fn release_and_rotation_same_frame_commit_overlap_in_one_undo() {
    let mut app = mouse_app();
    {
        let mut document = app.world_mut().resource_mut::<EditorDocument>();
        let kind = document.catalog.get("pod").unwrap();
        document.ship.parts = vec![
            kind.instantiate(1, (0.0, 0.0)),
            kind.instantiate(2, (4.0, 0.0)),
        ];
        document.ship.connections = vec![Connection::Normal {
            parent: 1,
            child: 2,
            parent_attach: 2,
            child_attach: 1,
        }];
        document.saved_ship = document.ship.clone();
    }
    let before = app.world().resource::<EditorDocument>().ship.clone();
    pointer(&mut app, (0.0, 0.0));
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    app.update();
    pointer(&mut app, (4.0, 0.0));
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyR);
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .release(MouseButton::Left);
    app.update();
    let mut document = app.world_mut().resource_mut::<EditorDocument>();
    assert!((document.ship.parts[0].x - 4.0).abs() < 1e-5);
    assert_eq!(document.ship.parts[0].editor_angle, 1);
    assert!(document.ship.connections.is_empty());
    let after = document.ship.clone();
    assert!(document.undo());
    assert_eq!(document.ship, before);
    assert!(!document.history.can_undo());
    assert!(document.redo());
    assert_eq!(document.ship, after);
}

#[test]
fn follow_toggle_changes_drag_scope_preserves_descendant_links_and_one_history_step() {
    for follow in [false, true] {
        let mut app = mouse_app();
        app.world_mut()
            .resource_mut::<view::ViewOptions>()
            .follow_children = follow;
        {
            let mut document = app.world_mut().resource_mut::<EditorDocument>();
            let kind = document.catalog.get("pod").unwrap();
            document.ship.parts = vec![
                kind.instantiate(1, (0.0, 0.0)),
                kind.instantiate(2, (0.0, 2.0)),
                kind.instantiate(3, (0.0, 4.0)),
            ];
            document.ship.connections = vec![
                Connection::Normal {
                    parent: 1,
                    child: 2,
                    parent_attach: 1,
                    child_attach: 1,
                },
                Connection::Normal {
                    parent: 2,
                    child: 3,
                    parent_attach: 1,
                    child_attach: 1,
                },
            ];
            document.saved_ship = document.ship.clone();
        }
        let before = app.world().resource::<EditorDocument>().ship.clone();
        pointer(&mut app, (0.0, 0.0));
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        assert_eq!(
            app.world().resource::<DragState>().keys().len(),
            if follow { 3 } else { 1 }
        );
        // 范围在按下时固定，切换选项不改变正在拖拽的刚体。
        app.world_mut()
            .resource_mut::<view::ViewOptions>()
            .follow_children = !follow;
        pointer(&mut app, (2.0, 1.0));
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyR);
        app.update();
        assert_eq!(app.world().resource::<EditorDocument>().ship, before);
        pointer(&mut app, (2.0, 1.0));
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .release(MouseButton::Left);
        app.update();
        let mut document = app.world_mut().resource_mut::<EditorDocument>();
        assert_eq!(document.ship.parts[0].editor_angle, 1);
        if follow {
            assert!((document.ship.parts[1].x - 0.0).abs() < 1e-5);
            assert!((document.ship.parts[2].x + 2.0).abs() < 1e-5);
            assert_eq!(document.ship.parts[2].editor_angle, 1);
            assert_eq!(document.ship.connections, before.connections);
        } else {
            assert_eq!(document.ship.parts[1..], before.parts[1..]);
            assert_eq!(document.ship.connections, before.connections[1..]);
        }
        assert!(document.undo());
        assert_eq!(document.ship, before);
        assert!(!document.history.can_undo());
    }
}

#[test]
fn drag_four_turns_are_noop_and_disabled_rotation_is_rejected() {
    for disabled in [false, true] {
        let mut app = mouse_app();
        if disabled {
            let mut document = app.world_mut().resource_mut::<EditorDocument>();
            document.ship.parts[0] = document
                .catalog
                .get("tank")
                .unwrap()
                .instantiate(1, (0.0, 0.0));
            document.saved_ship = document.ship.clone();
        }
        let before = app.world().resource::<EditorDocument>().ship.clone();
        pointer(&mut app, (0.0, 0.0));
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        for _ in 0..4 {
            pointer(&mut app, (0.0, 0.0));
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .reset_all();
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .press(KeyCode::KeyR);
            app.update();
        }
        pointer(&mut app, (0.0, 0.0));
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .release(MouseButton::Left);
        app.update();
        let document = app.world().resource::<EditorDocument>();
        assert_eq!(document.ship, before);
        assert!(!document.history.can_undo());
    }
}

#[test]
fn palette_drop_deletes_connected_component_only_on_release_and_is_one_undo() {
    let mut app = mouse_app();
    app.init_resource::<panels::egui_panel::UiState>()
        .add_systems(Update, panels::pointer_over_ui.before(mouse_editor));
    {
        let mut state = app
            .world_mut()
            .resource_mut::<panels::egui_panel::UiState>();
        let right = bevy_egui::egui::Rect::from_min_max(
            bevy_egui::egui::pos2(1100.0, 70.0),
            bevy_egui::egui::pos2(1440.0, 900.0),
        );
        state.pixels_per_point = 1.0;
        state.areas = vec![right];
        state.palette_area = Some(right);
    }
    {
        let mut document = app.world_mut().resource_mut::<EditorDocument>();
        let kind = document.catalog.get("pod").unwrap();
        document.ship.parts = vec![
            kind.instantiate(1, (0.0, 0.0)),
            kind.instantiate(2, (0.0, 2.0)),
            kind.instantiate(3, (-4.0, 0.0)),
        ];
        document.ship.connections = vec![Connection::Normal {
            parent: 1,
            child: 2,
            parent_attach: 1,
            child_attach: 1,
        }];
        document.saved_ship = document.ship.clone();
    }
    let before = app.world().resource::<EditorDocument>().ship.clone();
    pointer(&mut app, (0.0, 2.0));
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    app.update();
    pointer(&mut app, (7.5, 0.0));
    app.update();
    assert!(app.world().resource::<panels::UiPointer>().palette_drop);
    assert_eq!(app.world().resource::<EditorDocument>().ship, before);
    pointer(&mut app, (7.5, 0.0));
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .release(MouseButton::Left);
    app.update();
    assert!(app.world().resource::<DragState>().id.is_none());
    let mut document = app.world_mut().resource_mut::<EditorDocument>();
    assert_eq!(document.ship.parts, before.parts[2..]);
    assert!(document.ship.connections.is_empty());
    let after = document.ship.clone();
    assert!(document.undo());
    assert_eq!(document.ship, before);
    assert!(!document.history.can_undo());
    assert!(document.redo());
    assert_eq!(document.ship, after);
}

#[test]
fn other_ui_release_and_escape_over_palette_never_delete() {
    for cancel in [false, true] {
        let mut app = mouse_app();
        let before = app.world().resource::<EditorDocument>().ship.clone();
        pointer(&mut app, (0.0, 0.0));
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        pointer(&mut app, (7.5, 0.0));
        {
            let mut ui = app.world_mut().resource_mut::<panels::UiPointer>();
            ui.blocked = true;
            ui.palette_drop = cancel;
        }
        if cancel {
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .press(KeyCode::Escape);
        }
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .release(MouseButton::Left);
        app.update();
        let document = app.world().resource::<EditorDocument>();
        assert_eq!(document.ship, before);
        assert!(!document.history.can_undo());
        assert!(app.world().resource::<DragState>().id.is_none());
    }
}

fn staging_document() -> EditorDocument {
    let mut document = crate::test_support::document();
    document.ship = dr_core::ship_from_xml(
        r#"<Ship><Parts><Part id="1" partType="pod"><Pod name="初始"><Staging currentStage="0"/></Pod></Part></Parts></Ship>"#,
    ).unwrap();
    document.saved_ship = document.ship.clone();
    document
}

fn draft_keyboard_app() -> App {
    let mut document = staging_document();
    assert!(document.execute(EditorCommand::SetMetadata {
        name: "文档历史".into(),
        description: "画布编辑".into(),
    }));
    let mut inspector = properties::Inspector::default();
    properties::open(&mut inspector, &document);
    assert!(inspector.docked());
    let mut app = App::new();
    app.insert_resource(document)
        .insert_resource(inspector)
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<EditorCursor>()
        .init_resource::<DragState>()
        .init_resource::<panels::Palette>()
        .init_resource::<panels::UiPointer>()
        .add_systems(
            Update,
            keyboard_commands.run_if(egui_ui::canvas_input_available),
        );
    app
}

fn draft_shortcut(app: &mut App, key: KeyCode) {
    // 每次重新产生边沿，模拟松开再按下同一快捷键。
    {
        let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        keys.reset_all();
        keys.press(KeyCode::ControlLeft);
        keys.press(key);
    }
    app.update();
}

fn draft_name(app: &App) -> &str {
    &app.world()
        .resource::<properties::Inspector>()
        .draft
        .as_ref()
        .unwrap()
        .name
}

#[test]
fn draft_shortcuts_consume_local_history_before_document_undo_and_redo() {
    let mut app = draft_keyboard_app();
    let before = app.world().resource::<EditorDocument>().ship.clone();
    let revision = app.world().resource::<EditorDocument>().revision;
    app.world_mut()
        .resource_mut::<properties::Inspector>()
        .draft
        .as_mut()
        .unwrap()
        .name = "草稿修改".into();
    // 两种历史入口都先清理未提交目录拖动和粘贴，而不把预览写入文档。
    let fragment = dr_core::ShipFragment::capture(&before, &[PartKey::new(0, 1, 0)]).unwrap();
    {
        let mut cursor = app.world_mut().resource_mut::<EditorCursor>();
        cursor.palette_drag = true;
        cursor.placing = true;
        cursor.paste = Some(fragment.clone());
    }
    draft_shortcut(&mut app, KeyCode::KeyZ);
    assert_eq!(draft_name(&app), "初始");
    assert_eq!(app.world().resource::<EditorDocument>().ship, before);
    assert_eq!(app.world().resource::<EditorDocument>().revision, revision);
    let cursor = app.world().resource::<EditorCursor>();
    assert!(!cursor.palette_drag && !cursor.placing && cursor.paste.is_none());

    draft_shortcut(&mut app, KeyCode::KeyZ);
    assert_eq!(draft_name(&app), "初始");
    let undone = app.world().resource::<EditorDocument>().ship.clone();
    let undone_revision = app.world().resource::<EditorDocument>().revision;
    assert_ne!(undone, before);
    assert_ne!(undone_revision, revision);
    {
        let mut cursor = app.world_mut().resource_mut::<EditorCursor>();
        cursor.palette_drag = true;
        cursor.placing = true;
        cursor.paste = Some(fragment);
    }
    draft_shortcut(&mut app, KeyCode::KeyY);
    assert_eq!(draft_name(&app), "草稿修改");
    assert_eq!(app.world().resource::<EditorDocument>().ship, undone);
    assert_eq!(
        app.world().resource::<EditorDocument>().revision,
        undone_revision
    );
    let cursor = app.world().resource::<EditorCursor>();
    assert!(!cursor.palette_drag && !cursor.placing && cursor.paste.is_none());

    draft_shortcut(&mut app, KeyCode::KeyY);
    assert_eq!(draft_name(&app), "草稿修改");
    assert_eq!(app.world().resource::<EditorDocument>().ship, before);
    assert_ne!(
        app.world().resource::<EditorDocument>().revision,
        undone_revision
    );
}

#[test]
fn draft_without_local_history_falls_back_to_document_undo() {
    let mut app = draft_keyboard_app();
    let before = app.world().resource::<EditorDocument>().ship.clone();
    assert!(!app.world().resource::<properties::Inspector>().can_undo());
    draft_shortcut(&mut app, KeyCode::KeyZ);
    assert_ne!(app.world().resource::<EditorDocument>().ship, before);
    assert_eq!(draft_name(&app), "初始");
    assert!(!app.world().resource::<EditorDocument>().history.can_undo());
}

#[test]
fn docked_staging_allows_canvas_and_fit_while_modal_properties_block_them() {
    use bevy::ecs::system::RunSystemOnce;
    let document = staging_document();
    let mut inspector = properties::Inspector::default();
    properties::open(&mut inspector, &document);
    let mut world = World::new();
    world.insert_resource(inspector);
    world.init_resource::<panels::egui_panel::UiState>();
    for fit in [false, true] {
        world
            .resource_mut::<panels::egui_panel::UiState>()
            .fit_requested = fit;
        assert!(
            world
                .run_system_once(egui_ui::canvas_input_available)
                .unwrap()
        );
        assert!(
            world
                .run_system_once(egui_ui::canvas_or_fit_available)
                .unwrap()
        );
        assert!(
            world
                .run_system_once(egui_ui::panel_actions_available)
                .unwrap()
        );
        assert!(!world.run_system_once(egui_ui::properties_modal).unwrap());
    }
    // 同样的 Pod 草稿进入修复流程时恢复模态，不借适配请求绕过隔离。
    let mut duplicate_ship = document.ship.clone();
    duplicate_ship.parts.push(duplicate_ship.parts[0].clone());
    world.resource_mut::<properties::Inspector>().repair =
        Some(dr_core::DuplicateRepair::new(&duplicate_ship, PartKey::new(0, 1, 0)).unwrap());
    assert!(
        !world
            .run_system_once(egui_ui::canvas_input_available)
            .unwrap()
    );
    assert!(
        !world
            .run_system_once(egui_ui::canvas_or_fit_available)
            .unwrap()
    );
    assert!(
        !world
            .run_system_once(egui_ui::panel_actions_available)
            .unwrap()
    );
    {
        let mut inspector = world.resource_mut::<properties::Inspector>();
        inspector.repair = None;
        inspector.draft.as_mut().unwrap().original.pod = None;
    }
    assert!(
        !world
            .run_system_once(egui_ui::canvas_input_available)
            .unwrap()
    );
    assert!(
        !world
            .run_system_once(egui_ui::canvas_or_fit_available)
            .unwrap()
    );
    assert!(world.run_system_once(egui_ui::properties_modal).unwrap());
}

#[test]
fn prepare_input_keeps_docked_staging_canvas_previews_and_pointer_available() {
    let document = staging_document();
    let fragment =
        dr_core::ShipFragment::capture(&document.ship, &[PartKey::new(0, 1, 0)]).unwrap();
    let mut inspector = properties::Inspector::default();
    properties::open(&mut inspector, &document);
    let mut app = App::new();
    app.insert_resource(document)
        .insert_resource(inspector)
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<panels::UiPointer>()
        .insert_resource(DragState {
            id: Some(PartKey::new(0, 1, 0)),
            rectangle: Some((2.0, 3.0)),
            ..default()
        })
        .insert_resource(EditorCursor {
            placing: true,
            palette_drag: true,
            paste: Some(fragment),
            ..default()
        })
        .insert_resource(CameraDrag(Some(Vec2::new(4.0, 5.0))))
        .add_systems(Update, egui_ui::prepare_input);
    app.update();
    let drag = app.world().resource::<DragState>();
    assert_eq!(drag.id, Some(PartKey::new(0, 1, 0)));
    assert_eq!(drag.rectangle, Some((2.0, 3.0)));
    let cursor = app.world().resource::<EditorCursor>();
    assert!(cursor.placing && cursor.palette_drag && cursor.paste.is_some());
    assert_eq!(
        app.world().resource::<CameraDrag>().0,
        Some(Vec2::new(4.0, 5.0))
    );
    assert!(!app.world().resource::<panels::UiPointer>().blocked);
}
