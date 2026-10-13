use super::*;
use crate::canvas_input::mouse_editor;
use crate::view::camera_controls;
use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::window::WindowResolution;

fn app() -> App {
    let mut app = crate::test_support::mouse_app();
    app.add_plugins(native_buttons::NativeButtonsPlugin)
        .init_resource::<GestureInput>()
        .add_message::<TouchInput>()
        .add_message::<WindowFocused>()
        .add_message::<MouseWheel>()
        .add_systems(
            Update,
            (input, panels::pointer_over_ui, camera_controls)
                .chain()
                .before(mouse_editor),
        );
    app
}

fn window(app: &mut App) -> Entity {
    app.world_mut()
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(app.world())
        .unwrap()
}

fn event(app: &mut App, id: u64, phase: TouchPhase, point: Vec2) {
    let window = window(app);
    app.world_mut().write_message(TouchInput {
        window,
        id,
        phase,
        position: point,
        force: None,
    });
}

fn touch(app: &mut App, id: u64, phase: TouchPhase, x: f32, y: f32) {
    event(app, id, phase, Vec2::new(x, y));
    app.update();
}

fn camera(app: &mut App) -> (Vec2, f32) {
    let (transform, projection) = app
        .world_mut()
        .query_filtered::<(&Transform, &Projection), With<Camera2d>>()
        .single(app.world())
        .unwrap();
    let Projection::Orthographic(projection) = projection else {
        panic!("需要正交相机");
    };
    (transform.translation.truncate(), projection.scale)
}

#[test]
fn single_touch_drag_preserves_grab_offset_and_release_position_in_one_undo_step() {
    let mut app = app();
    let before = app.world().resource::<EditorDocument>().ship.clone();
    touch(&mut app, 1, TouchPhase::Started, 732.0, 444.0);
    assert!(app.world().resource::<DragState>().id.is_some());
    touch(&mut app, 1, TouchPhase::Moved, 738.0, 436.2);
    assert_eq!(app.world().resource::<EditorDocument>().ship, before);
    touch(&mut app, 1, TouchPhase::Ended, 742.8, 431.4);
    let document = app.world().resource::<EditorDocument>();
    assert!((document.ship.parts[0].x - 0.18).abs() < 1e-4);
    assert!((document.ship.parts[0].y - 0.21).abs() < 1e-4);
    assert!(app.world().resource::<DragState>().id.is_none());
    app.update();
    let mut document = app.world_mut().resource_mut::<EditorDocument>();
    assert!(document.undo());
    assert_eq!(document.ship, before);
    assert!(!document.history.can_undo());
}

#[test]
fn touch_placement_commits_only_once_on_release_at_the_final_point() {
    let mut app = app();
    app.world_mut().resource_mut::<EditorCursor>().placing = true;
    touch(&mut app, 1, TouchPhase::Started, 900.0, 450.0);
    assert_eq!(app.world().resource::<EditorDocument>().ship.parts.len(), 1);
    touch(&mut app, 1, TouchPhase::Moved, 960.0, 450.0);
    assert_eq!(app.world().resource::<EditorDocument>().ship.parts.len(), 1);
    touch(&mut app, 1, TouchPhase::Ended, 990.0, 450.0);
    app.update();
    let mut document = app.world_mut().resource_mut::<EditorDocument>();
    assert_eq!(document.ship.parts.len(), 2);
    assert!((document.ship.parts[1].x - 4.5).abs() < 1e-4);
    assert!(document.undo());
    assert_eq!(document.ship.parts.len(), 1);
    assert!(!document.history.can_undo());
}

#[test]
fn a_tap_batched_in_one_frame_places_once() {
    let mut app = app();
    app.world_mut().resource_mut::<EditorCursor>().placing = true;
    event(&mut app, 1, TouchPhase::Started, Vec2::new(900.0, 450.0));
    event(&mut app, 1, TouchPhase::Ended, Vec2::new(960.0, 450.0));
    app.update();
    assert_eq!(app.world().resource::<EditorDocument>().ship.parts.len(), 2);
    app.update();
    assert_eq!(app.world().resource::<EditorDocument>().ship.parts.len(), 2);
}

#[test]
fn a_single_finger_pans_blank_canvas_for_both_mouse_box_selection_preferences() {
    for preference in [view::BoxSelectButton::Left, view::BoxSelectButton::Middle] {
        let mut app = app();
        app.world_mut()
            .resource_mut::<view::ViewOptions>()
            .box_select_button = preference;
        touch(&mut app, 1, TouchPhase::Started, 1000.0, 650.0);
        touch(&mut app, 1, TouchPhase::Moved, 1020.0, 680.0);
        assert_eq!(camera(&mut app), (Vec2::new(-20.0, 30.0), 1.0));
        touch(&mut app, 1, TouchPhase::Ended, 1025.0, 690.0);
        assert_eq!(camera(&mut app), (Vec2::new(-25.0, 40.0), 1.0));
        assert!(app.world().resource::<DragState>().rectangle.is_none());
        assert!(!app.world().resource::<EditorDocument>().history.can_undo());
    }
}

#[test]
fn second_finger_cancels_drag_then_zoom_and_pan_keep_the_midpoint_world_anchor() {
    let mut app = app();
    let before = app.world().resource::<EditorDocument>().ship.clone();
    touch(&mut app, 1, TouchPhase::Started, 720.0, 450.0);
    touch(&mut app, 1, TouchPhase::Moved, 726.0, 450.0);
    touch(&mut app, 2, TouchPhase::Started, 846.0, 450.0);
    assert!(app.world().resource::<DragState>().id.is_none());
    let anchor = Vec2::new(786.0 - 720.0, 0.0);
    event(&mut app, 1, TouchPhase::Moved, Vec2::new(606.0, 450.0));
    event(&mut app, 2, TouchPhase::Moved, Vec2::new(966.0, 450.0));
    app.update();
    let (position, scale) = camera(&mut app);
    assert!((scale - 1.0 / 3.0).abs() < 1e-5);
    assert!((position + anchor * scale - anchor).length() < 1e-4);
    event(&mut app, 1, TouchPhase::Moved, Vec2::new(646.0, 470.0));
    event(&mut app, 2, TouchPhase::Moved, Vec2::new(1006.0, 470.0));
    app.update();
    let (next, next_scale) = camera(&mut app);
    assert!((next - position - Vec2::new(-40.0, 20.0) * scale).length() < 1e-4);
    assert!((next_scale - scale).abs() < 1e-5);
    assert_eq!(app.world().resource::<EditorDocument>().ship, before);
    assert!(!app.world().resource::<EditorDocument>().history.can_undo());
}

#[test]
fn navigation_cannot_restart_single_finger_editing_until_all_fingers_are_up() {
    let mut app = app();
    touch(&mut app, 1, TouchPhase::Started, 720.0, 450.0);
    touch(&mut app, 2, TouchPhase::Started, 840.0, 450.0);
    touch(&mut app, 2, TouchPhase::Ended, 840.0, 450.0);
    let before = camera(&mut app);
    touch(&mut app, 1, TouchPhase::Moved, 780.0, 450.0);
    assert_eq!(camera(&mut app), before);
    assert!(app.world().resource::<DragState>().id.is_none());
    touch(&mut app, 1, TouchPhase::Ended, 780.0, 450.0);
    touch(&mut app, 3, TouchPhase::Started, 720.0, 450.0);
    assert!(app.world().resource::<DragState>().id.is_some());
}

#[test]
fn second_finger_cancels_pending_placement_without_a_document_transaction() {
    let mut app = app();
    app.world_mut().resource_mut::<EditorCursor>().placing = true;
    touch(&mut app, 1, TouchPhase::Started, 960.0, 450.0);
    touch(&mut app, 2, TouchPhase::Started, 1080.0, 450.0);
    assert!(!app.world().resource::<EditorCursor>().placing);
    touch(&mut app, 2, TouchPhase::Ended, 1080.0, 450.0);
    touch(&mut app, 1, TouchPhase::Ended, 960.0, 450.0);
    let document = app.world().resource::<EditorDocument>();
    assert_eq!(document.ship.parts.len(), 1);
    assert!(!document.history.can_undo());
}

#[test]
fn canceled_and_focus_lost_touches_drop_previews_without_committing() {
    for placing in [false, true] {
        for loss in [false, true] {
            let mut app = app();
            app.world_mut().resource_mut::<EditorCursor>().placing = placing;
            let x = if placing { 960.0 } else { 720.0 };
            touch(&mut app, 1, TouchPhase::Started, x, 450.0);
            touch(&mut app, 1, TouchPhase::Moved, x + 20.0, 470.0);
            if loss {
                // 失焦及恢复发生在同一帧，也应丢弃拖动，不只读最终 focused 值。
                let window = window(&mut app);
                app.world_mut().write_message(WindowFocused {
                    window,
                    focused: false,
                });
                app.world_mut().write_message(WindowFocused {
                    window,
                    focused: true,
                });
                app.update();
            } else {
                touch(&mut app, 1, TouchPhase::Canceled, x + 20.0, 470.0);
            }
            touch(&mut app, 1, TouchPhase::Moved, x + 40.0, 470.0);
            touch(&mut app, 1, TouchPhase::Ended, x + 40.0, 470.0);
            assert!(!app.world().resource::<EditorDocument>().history.can_undo());
            assert_eq!(app.world().resource::<EditorDocument>().ship.parts.len(), 1);
            assert!(app.world().resource::<DragState>().id.is_none());
            assert!(!app.world().resource::<EditorCursor>().placing);
            assert!(app.world().resource::<CameraDrag>().0.is_none());
        }
    }
}

#[test]
fn pure_mouse_focus_loss_cancels_drag_without_suppressing_the_recovered_shift_click() {
    let mut app = app();
    let window = window(&mut app);
    let before = app.world().resource::<EditorDocument>().ship.clone();
    app.world_mut()
        .get_mut::<Window>(window)
        .unwrap()
        .set_cursor_position(Some(Vec2::new(720.0, 450.0)));
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    app.update();
    assert!(app.world().resource::<DragState>().id.is_some());
    assert_eq!(
        app.world()
            .resource::<EditorDocument>()
            .selected_keys()
            .len(),
        1
    );
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .clear();
    {
        let mut window = app.world_mut().get_mut::<Window>(window).unwrap();
        window.set_cursor_position(Some(Vec2::new(750.0, 438.0)));
        window.focused = false;
    }
    app.update();
    let input = app.world().resource::<GestureInput>();
    assert!(!input.override_mouse && !input.mouse_tail && !input.blocked());
    assert!(app.world().resource::<DragState>().id.is_none());
    assert_eq!(app.world().resource::<EditorDocument>().ship, before);
    assert!(!app.world().resource::<EditorDocument>().history.can_undo());

    // 松开、恢复焦点和新的按下发生在同一帧，适配器无需观察额外的鼠标 up 帧。
    {
        let mut window = app.world_mut().get_mut::<Window>(window).unwrap();
        window.focused = true;
        window.set_cursor_position(Some(Vec2::new(720.0, 450.0)));
    }
    {
        let mut mouse = app.world_mut().resource_mut::<ButtonInput<MouseButton>>();
        mouse.reset_all();
        mouse.press(MouseButton::Left);
    }
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::ShiftLeft);
    app.update();
    assert!(!app.world().resource::<GestureInput>().override_mouse);
    assert!(
        app.world()
            .resource::<EditorDocument>()
            .selected_keys()
            .is_empty()
    );
    assert_eq!(app.world().resource::<EditorDocument>().ship, before);
    assert!(!app.world().resource::<EditorDocument>().history.can_undo());
}

#[test]
fn touch_focus_loss_keeps_compatibility_mouse_suppressed_until_a_release_frame() {
    let mut app = app();
    let window = window(&mut app);
    let before = app.world().resource::<EditorDocument>().ship.clone();
    app.world_mut()
        .get_mut::<Window>(window)
        .unwrap()
        .set_cursor_position(Some(Vec2::new(720.0, 450.0)));
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    touch(&mut app, 1, TouchPhase::Started, 720.0, 450.0);
    touch(&mut app, 1, TouchPhase::Moved, 750.0, 438.0);
    app.world_mut().get_mut::<Window>(window).unwrap().focused = false;
    app.update();
    let input = app.world().resource::<GestureInput>();
    assert!(input.override_mouse && input.mouse_tail && input.blocked());
    assert!(input.contacts.is_empty());
    assert!(app.world().resource::<DragState>().id.is_none());
    assert_eq!(app.world().resource::<EditorDocument>().ship, before);
    assert!(!app.world().resource::<EditorDocument>().history.can_undo());

    app.world_mut().get_mut::<Window>(window).unwrap().focused = true;
    {
        let mut mouse = app.world_mut().resource_mut::<ButtonInput<MouseButton>>();
        mouse.reset_all();
        mouse.press(MouseButton::Left);
    }
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::ShiftLeft);
    app.update();
    assert!(app.world().resource::<GestureInput>().blocked());
    assert_eq!(
        app.world()
            .resource::<EditorDocument>()
            .selected_keys()
            .len(),
        1
    );
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .release(MouseButton::Left);
    app.update();
    assert!(!app.world().resource::<GestureInput>().mouse_tail);
    {
        let mut mouse = app.world_mut().resource_mut::<ButtonInput<MouseButton>>();
        mouse.reset_all();
        mouse.press(MouseButton::Left);
    }
    app.update();
    assert!(!app.world().resource::<GestureInput>().override_mouse);
    assert!(
        app.world()
            .resource::<EditorDocument>()
            .selected_keys()
            .is_empty()
    );
    assert_eq!(app.world().resource::<EditorDocument>().ship, before);
    assert!(!app.world().resource::<EditorDocument>().history.can_undo());
}

#[test]
fn ui_touch_origin_stays_ui_after_scrolling_across_the_canvas_at_high_dpi() {
    let mut app = app();
    let window = window(&mut app);
    app.world_mut()
        .get_mut::<Window>(window)
        .unwrap()
        .resolution = WindowResolution::new(2880, 1800).with_scale_factor_override(2.0);
    let mut ui = panels::egui_panel::UiState::default();
    ui.pixels_per_point = 2.0;
    ui.areas.push(bevy_egui::egui::Rect::from_min_max(
        bevy_egui::egui::pos2(0.0, 0.0),
        bevy_egui::egui::pos2(100.0, 900.0),
    ));
    app.insert_resource(ui);
    let before = camera(&mut app);
    touch(&mut app, 1, TouchPhase::Started, 50.0, 250.0);
    touch(&mut app, 1, TouchPhase::Moved, 720.0, 450.0);
    touch(&mut app, 1, TouchPhase::Ended, 800.0, 500.0);
    assert_eq!(camera(&mut app), before);
    assert!(app.world().resource::<panels::UiPointer>().blocked);
    assert!(app.world().resource::<DragState>().id.is_none());
    assert!(!app.world().resource::<EditorDocument>().history.can_undo());
}

#[test]
fn dpi_scaling_does_not_double_scale_the_touch_canvas_position() {
    let mut app = app();
    let window = window(&mut app);
    app.world_mut()
        .get_mut::<Window>(window)
        .unwrap()
        .resolution = WindowResolution::new(2880, 1800).with_scale_factor_override(2.0);
    touch(&mut app, 1, TouchPhase::Started, 720.0, 450.0);
    touch(&mut app, 1, TouchPhase::Ended, 750.0, 438.0);
    let document = app.world().resource::<EditorDocument>();
    assert!((document.ship.parts[0].x - 0.5).abs() < 1e-4);
    assert!((document.ship.parts[0].y - 0.2).abs() < 1e-4);
}

#[test]
fn ui_second_finger_cancels_canvas_drag_without_starting_navigation() {
    let mut app = app();
    let mut ui = panels::egui_panel::UiState::default();
    ui.pixels_per_point = 1.0;
    ui.areas.push(bevy_egui::egui::Rect::from_min_max(
        bevy_egui::egui::pos2(0.0, 0.0),
        bevy_egui::egui::pos2(100.0, 900.0),
    ));
    app.insert_resource(ui);
    touch(&mut app, 1, TouchPhase::Started, 720.0, 450.0);
    touch(&mut app, 2, TouchPhase::Started, 50.0, 450.0);
    touch(&mut app, 1, TouchPhase::Moved, 800.0, 450.0);
    assert_eq!(camera(&mut app), (Vec2::ZERO, 1.0));
    assert!(app.world().resource::<DragState>().id.is_none());
    assert!(app.world().resource::<GestureInput>().blocked());
}

#[test]
fn closing_a_modal_during_a_touch_cannot_promote_that_touch_to_canvas() {
    let mut app = app();
    app.insert_resource(help::HelpState {
        open: true,
        suppress_frame: false,
    });
    touch(&mut app, 1, TouchPhase::Started, 720.0, 450.0);
    app.world_mut().resource_mut::<help::HelpState>().open = false;
    touch(&mut app, 1, TouchPhase::Moved, 780.0, 450.0);
    touch(&mut app, 1, TouchPhase::Ended, 780.0, 450.0);
    assert!(app.world().resource::<DragState>().id.is_none());
    assert_eq!(camera(&mut app), (Vec2::ZERO, 1.0));
    assert!(!app.world().resource::<EditorDocument>().history.can_undo());
}

#[test]
fn concurrent_mouse_edges_do_not_duplicate_touch_placement_or_mutate_mouse_state() {
    let mut app = app();
    let window = window(&mut app);
    app.world_mut()
        .get_mut::<Window>(window)
        .unwrap()
        .set_cursor_position(Some(Vec2::new(1080.0, 450.0)));
    app.world_mut().resource_mut::<EditorCursor>().placing = true;
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    touch(&mut app, 1, TouchPhase::Started, 960.0, 450.0);
    assert!(
        app.world()
            .resource::<ButtonInput<MouseButton>>()
            .pressed(MouseButton::Left)
    );
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .clear();
    touch(&mut app, 1, TouchPhase::Ended, 960.0, 450.0);
    app.update();
    assert_eq!(app.world().resource::<EditorDocument>().ship.parts.len(), 2);
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .release(MouseButton::Left);
    app.update();
    assert_eq!(app.world().resource::<EditorDocument>().ship.parts.len(), 2);
}

#[derive(Resource, Default)]
struct NativeActions(Vec<files::FileAction>);

fn collect_actions(mut reader: MessageReader<files::FileAction>, mut seen: ResMut<NativeActions>) {
    seen.0.extend(reader.read().cloned());
}

#[test]
fn native_toolbar_touch_uses_its_real_position_and_fires_once_on_release() {
    let mut app = app();
    app.add_message::<files::FileAction>()
        .init_resource::<NativeActions>()
        .add_systems(
            Update,
            (files::toolbar_actions, collect_actions)
                .chain()
                .after(input),
        );
    let button = app
        .world_mut()
        .spawn((
            native_buttons::press_triggered(),
            Node::default(),
            panels::EditorPanel,
            files::FileButton(files::FileAction::OpenDialog),
            BackgroundColor(Color::BLACK),
            ComputedNode {
                size: Vec2::new(100.0, 44.0),
                ..default()
            },
            UiGlobalTransform::from_translation(Vec2::new(100.0, 34.0)),
        ))
        .id();
    // 旧鼠标位置故意在画布；接入触摸不移动 Window 的鼠标光标。
    let window = window(&mut app);
    app.world_mut()
        .get_mut::<Window>(window)
        .unwrap()
        .set_cursor_position(Some(Vec2::new(900.0, 450.0)));
    // 在触摸接管前排入鼠标 Activate，验证同帧 Update 仲裁也能丢弃它。
    app.world_mut()
        .trigger(bevy::ui_widgets::Activate { entity: button });
    touch(&mut app, 1, TouchPhase::Started, 100.0, 34.0);
    assert!(app.world().resource::<NativeActions>().0.is_empty());
    assert_eq!(
        app.world().resource::<GestureInput>().native_pressed,
        Some(button)
    );
    assert_eq!(
        app.world().get::<BackgroundColor>(button).unwrap().0,
        native_buttons::PRESSED
    );
    touch(&mut app, 1, TouchPhase::Moved, 200.0, 34.0);
    assert_eq!(app.world().resource::<GestureInput>().native_pressed, None);
    touch(&mut app, 1, TouchPhase::Moved, 100.0, 34.0);
    assert_eq!(
        app.world().resource::<GestureInput>().native_pressed,
        Some(button)
    );
    touch(&mut app, 1, TouchPhase::Ended, 100.0, 34.0);
    assert_eq!(app.world().resource::<GestureInput>().native_pressed, None);
    assert!(matches!(
        app.world().resource::<NativeActions>().0.as_slice(),
        [files::FileAction::OpenDialog]
    ));
    app.update();
    assert_eq!(app.world().resource::<NativeActions>().0.len(), 1);
    assert_eq!(
        app.world().get::<Window>(window).unwrap().cursor_position(),
        Some(Vec2::new(900.0, 450.0))
    );
    // 所有手指抬起后，新的鼠标 Activate 正常路由，作为屏蔽机制的正向对照。
    app.world_mut()
        .trigger(bevy::ui_widgets::Activate { entity: button });
    app.update();
    assert_eq!(app.world().resource::<NativeActions>().0.len(), 2);
}

#[test]
fn canceled_native_toolbar_touch_never_fires() {
    let mut app = app();
    app.add_message::<files::FileAction>()
        .init_resource::<NativeActions>()
        .add_systems(
            Update,
            (files::toolbar_actions, collect_actions)
                .chain()
                .after(input),
        );
    let button = app
        .world_mut()
        .spawn((
            native_buttons::press_triggered(),
            Node::default(),
            panels::EditorPanel,
            files::FileButton(files::FileAction::New),
            BackgroundColor(Color::BLACK),
            ComputedNode {
                size: Vec2::new(100.0, 44.0),
                ..default()
            },
            UiGlobalTransform::from_translation(Vec2::new(100.0, 34.0)),
        ))
        .id();
    touch(&mut app, 1, TouchPhase::Started, 100.0, 34.0);
    assert_eq!(
        app.world().resource::<GestureInput>().native_pressed,
        Some(button)
    );
    touch(&mut app, 1, TouchPhase::Canceled, 100.0, 34.0);
    assert_eq!(app.world().resource::<GestureInput>().native_pressed, None);
    touch(&mut app, 1, TouchPhase::Ended, 100.0, 34.0);
    assert_eq!(app.world().resource::<GestureInput>().native_pressed, None);
    assert!(app.world().resource::<NativeActions>().0.is_empty());
}

#[test]
fn degenerate_pinch_keeps_camera_finite_and_does_not_accept_wheel_duplicates() {
    let mut app = app();
    touch(&mut app, 1, TouchPhase::Started, 1000.0, 650.0);
    touch(&mut app, 2, TouchPhase::Started, 1000.0, 650.0);
    let window = window(&mut app);
    app.world_mut().write_message(MouseWheel {
        window,
        unit: MouseScrollUnit::Line,
        x: 0.0,
        y: 10.0,
        phase: TouchPhase::Moved,
    });
    touch(&mut app, 2, TouchPhase::Moved, 1040.0, 650.0);
    let (position, scale) = camera(&mut app);
    assert!(position.is_finite() && scale.is_finite());
    assert_eq!(scale, 1.0);
}

#[test]
fn touch_connection_hit_radius_is_22_logical_pixels_while_mouse_stays_at_8() {
    let mut touched = app();
    touched
        .world_mut()
        .resource_mut::<EditorDocument>()
        .free_mode = true;
    // 连接点在世界 x=0.5 格（AttachPoint x=1 折半），屏幕 x=750；偏移 20px 仍在 pod 半宽 1 格内：触摸 22px 命中、鼠标 8px 不命中。
    touch(&mut touched, 1, TouchPhase::Started, 770.0, 450.0);
    assert!(
        touched
            .world()
            .resource::<EditorCursor>()
            .manual_connection
            .is_some()
    );
    assert!(
        !touched
            .world()
            .resource::<EditorDocument>()
            .history
            .can_undo()
    );
    touch(&mut touched, 1, TouchPhase::Ended, 770.0, 450.0);

    let mut mouse = app();
    mouse.world_mut().resource_mut::<EditorDocument>().free_mode = true;
    let window = window(&mut mouse);
    mouse
        .world_mut()
        .get_mut::<Window>(window)
        .unwrap()
        .set_cursor_position(Some(Vec2::new(770.0, 450.0)));
    mouse
        .world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    mouse.update();
    assert!(
        mouse
            .world()
            .resource::<EditorCursor>()
            .manual_connection
            .is_none()
    );
}

#[test]
fn a_third_finger_blocks_navigation_without_switching_the_two_finger_pair() {
    let mut app = app();
    touch(&mut app, 10, TouchPhase::Started, 1000.0, 650.0);
    touch(&mut app, 20, TouchPhase::Started, 1100.0, 650.0);
    let before = camera(&mut app);
    touch(&mut app, 1, TouchPhase::Started, 900.0, 600.0);
    touch(&mut app, 20, TouchPhase::Moved, 1200.0, 700.0);
    assert_eq!(camera(&mut app), before);
    for (id, x, y) in [(1, 900.0, 600.0), (20, 1200.0, 700.0), (10, 1000.0, 650.0)] {
        touch(&mut app, id, TouchPhase::Ended, x, y);
    }
    touch(&mut app, 30, TouchPhase::Started, 1000.0, 650.0);
    touch(&mut app, 30, TouchPhase::Moved, 1010.0, 650.0);
    assert_eq!(camera(&mut app).0, before.0 + Vec2::new(-10.0, 0.0));
}

#[test]
fn touch_paste_uses_the_existing_preview_and_commits_one_transaction_on_release() {
    let mut app = app();
    app.init_resource::<panels::Palette>()
        .init_resource::<panels::ShipBrowser>()
        .add_message::<panels::PanelButton>()
        .add_message::<files::FileAction>()
        .add_systems(Update, panels::panel_actions.before(input));
    app.world_mut()
        .write_message(panels::PanelButton::SelectAll);
    app.world_mut().write_message(panels::PanelButton::Copy);
    app.world_mut().write_message(panels::PanelButton::Paste);
    app.update();
    assert!(app.world().resource::<EditorCursor>().paste.is_some());
    assert!(!app.world().resource::<EditorDocument>().history.can_undo());
    touch(&mut app, 1, TouchPhase::Started, 900.0, 450.0);
    touch(&mut app, 1, TouchPhase::Moved, 960.0, 450.0);
    assert_eq!(app.world().resource::<EditorDocument>().ship.parts.len(), 1);
    touch(&mut app, 1, TouchPhase::Ended, 990.0, 450.0);
    app.update();
    assert!(app.world().resource::<EditorCursor>().paste.is_none());
    let mut document = app.world_mut().resource_mut::<EditorDocument>();
    assert_eq!(document.ship.all_parts().count(), 2);
    let pasted_x = document
        .ship
        .disconnected
        .last()
        .expect("粘贴的部件应进入独立组")
        .parts
        .last()
        .expect("独立组应有粘贴出的部件")
        .x;
    assert!((pasted_x - 4.5).abs() < 1e-4);
    assert!(document.undo());
    assert_eq!(document.ship.all_parts().count(), 1);
    assert!(!document.history.can_undo());
}

#[test]
fn opening_a_modal_in_the_same_frame_discards_the_pending_two_finger_navigation() {
    for key in [KeyCode::F1, KeyCode::F2, KeyCode::F6] {
        let mut app = app();
        app.init_resource::<properties::Inspector>()
            .init_resource::<topology_ui::ConnectionEditor>()
            .init_resource::<files::PendingFileAction>()
            .init_resource::<help::HelpState>()
            .add_systems(
                Update,
                (help::input, egui_ui::prepare_input)
                    .chain()
                    .after(panels::pointer_over_ui)
                    .before(camera_controls),
            );
        touch(&mut app, 1, TouchPhase::Started, 1000.0, 650.0);
        touch(&mut app, 2, TouchPhase::Started, 1100.0, 650.0);
        let before = camera(&mut app);
        app.world_mut().resource_mut::<EditorDocument>().selected = Some(PartKey::new(0, 1, 0));
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(key);
        touch(&mut app, 2, TouchPhase::Moved, 1200.0, 700.0);
        assert_eq!(camera(&mut app), before, "{key:?}");
        assert!(app.world().resource::<GestureInput>().navigation.is_none());
        assert!(app.world().resource::<panels::UiPointer>().blocked);
        let opened = match key {
            KeyCode::F1 => app.world().resource::<help::HelpState>().open,
            KeyCode::F2 => app.world().resource::<properties::Inspector>().is_open(),
            KeyCode::F6 => app.world().resource::<topology_ui::ConnectionEditor>().open,
            _ => unreachable!(),
        };
        assert!(opened, "{key:?} 应已打开模态");
    }
}
