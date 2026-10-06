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

/// 使用实际 mouse_editor 系统，测试相机固定为 1 世界像素/逻辑像素。
fn mouse_app() -> App {
    use bevy::camera::{ComputedCameraValues, RenderTargetInfo};
    let mut document = document();
    document.catalog = panels::tests::catalog();
    let mut app = App::new();
    app.insert_resource(document)
        .init_resource::<DragState>()
        .init_resource::<EditorCursor>()
        .init_resource::<view::ViewOptions>()
        .init_resource::<panels::UiPointer>()
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<ButtonInput<MouseButton>>()
        .add_systems(Update, mouse_editor);
    app.world_mut().spawn((
        Window {
            resolution: WindowResolution::new(1440, 900),
            focused: true,
            ..default()
        },
        bevy::window::PrimaryWindow,
    ));
    app.world_mut().spawn((
        Camera {
            computed: ComputedCameraValues {
                clip_from_view: Mat4::from_scale(Vec3::new(1.0 / 720.0, 1.0 / 450.0, 1.0)),
                target_info: Some(RenderTargetInfo {
                    physical_size: UVec2::new(1440, 900),
                    scale_factor: 1.0,
                }),
                ..default()
            },
            ..default()
        },
        GlobalTransform::IDENTITY,
    ));
    app
}

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
    pointer(&mut app, (0.1, 0.13));
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
    assert!((document.ship.parts[0].x - 0.1).abs() < 1e-5);
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
