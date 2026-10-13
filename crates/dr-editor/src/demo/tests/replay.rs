use super::*;
use bevy::input::mouse::MouseWheel;
use bevy::window::WindowResolution;

#[test]
fn demo_pointer_is_removed_before_native_window_sync() {
    let folder = tempfile::tempdir().unwrap();
    let args = vec![
        "editor".into(),
        "--demo-showcase".into(),
        folder.path().to_string_lossy().into_owned(),
    ];
    let mut showcase = Showcase::from_args(&args).unwrap().unwrap();
    showcase.pointer = Some(Vec2::new(100., 200.));
    let mut app = App::new();
    app.init_resource::<ButtonInput<MouseButton>>()
        .init_resource::<ButtonInput<KeyCode>>();
    app.insert_resource(showcase)
        .add_systems(Update, (restore_pointer, isolate_pointer).chain());
    let window = app.world_mut().spawn(Window::default()).id();
    app.update();
    assert_eq!(
        app.world()
            .get::<Window>(window)
            .unwrap()
            .physical_cursor_position(),
        None
    );
    assert_eq!(
        app.world().resource::<Showcase>().pointer,
        Some(Vec2::new(100., 200.))
    );
}

#[test]
fn ordinary_window_cursor_is_not_modified() {
    let mut app = App::new();
    app.init_resource::<ButtonInput<MouseButton>>()
        .init_resource::<ButtonInput<KeyCode>>()
        .add_systems(Update, (restore_pointer, isolate_pointer).chain());
    let mut window = Window::default();
    window.set_cursor_position(Some(Vec2::new(100., 200.)));
    let entity = app.world_mut().spawn(window).id();
    app.update();
    assert_eq!(
        app.world().get::<Window>(entity).unwrap().cursor_position(),
        Some(Vec2::new(100., 200.))
    );
}

#[test]
fn replay_keeps_its_own_held_inputs_and_discards_native_clicks_without_repeating_edges() {
    let mut snapshot = ReplayInput::default();
    snapshot.mouse.press(MouseButton::Left);
    snapshot.keys.press(KeyCode::KeyR);
    let mut mouse = ButtonInput::default();
    mouse.press(MouseButton::Right);
    let mut keys = ButtonInput::default();
    keys.press(KeyCode::Escape);
    snapshot.restore(&mut mouse, &mut keys);
    assert!(mouse.pressed(MouseButton::Left));
    assert!(!mouse.pressed(MouseButton::Right));
    assert!(!mouse.just_pressed(MouseButton::Left));
    assert!(keys.pressed(KeyCode::KeyR));
    assert!(!keys.pressed(KeyCode::Escape));
    assert!(!keys.just_pressed(KeyCode::KeyR));
    snapshot.mouse.release(MouseButton::Left);
    snapshot.keys.release(KeyCode::KeyR);
    snapshot.restore(&mut mouse, &mut keys);
    assert!(!mouse.pressed(MouseButton::Left));
    assert!(!keys.pressed(KeyCode::KeyR));
    assert!(!mouse.just_released(MouseButton::Left));
}

#[test]
fn configuration_click_then_home_resets_camera_only_after_returning_to_canvas() {
    use bevy_egui::egui;
    #[derive(Resource, Default)]
    struct ResetRequested(bool);
    fn reset_input(
        mut requested: ResMut<ResetRequested>,
        mut windows: Query<&mut Window>,
        mut mouse: ResMut<ButtonInput<MouseButton>>,
        mut keys: ResMut<ButtonInput<KeyCode>>,
    ) {
        if requested.0 {
            crate::selection_smoke::reset_camera_on_canvas(
                &mut windows.single_mut().unwrap(),
                &mut mouse,
                &mut keys,
            );
            requested.0 = false;
        }
    }
    for step in [40, 450] {
        let ui_point = Vec2::new(1204., 108.);
        let initial = Vec3::new(-120., 120., 0.);
        let (mut app, entity, _folder) = animation_app(step, ui_point);
        app.world_mut().resource_mut::<Showcase>().display_pointer = Some(ui_point);
        {
            let mut window = app.world_mut().get_mut::<Window>(entity).unwrap();
            window.resolution = WindowResolution::new(1440, 900);
            window.focused = true;
        }
        app.init_resource::<crate::CameraDrag>()
            .init_resource::<crate::DragState>()
            .init_resource::<crate::EditorCursor>()
            .init_resource::<crate::view::ViewOptions>()
            .init_resource::<crate::panels::UiPointer>()
            .init_resource::<ResetRequested>()
            .add_message::<MouseWheel>();
        let mut ui = crate::panels::egui_panel::UiState::default();
        ui.pixels_per_point = 1.;
        ui.areas.push(egui::Rect::from_min_max(
            egui::pos2(1144., 0.),
            egui::pos2(1440., 900.),
        ));
        app.insert_resource(ui);
        let camera = app
            .world_mut()
            .spawn((
                Camera2d,
                Transform::from_translation(initial),
                Projection::Orthographic(OrthographicProjection {
                    scale: 2.,
                    ..OrthographicProjection::default_2d()
                }),
            ))
            .id();
        app.edit_schedule(Update, |schedule| *schedule = Schedule::new(Update));
        app.add_systems(
            Update,
            (
                animate,
                crate::panels::pointer_over_ui,
                crate::view::camera_controls,
                pace,
            )
                .chain(),
        );
        let mut driver = crate::egui_ui::UiTestInput::default();
        driver.click_rect(
            egui::Rect::from_center_size(egui::pos2(ui_point.x, ui_point.y), egui::vec2(30., 18.)),
            &mut app
                .world_mut()
                .get_mut::<bevy_egui::EguiInput>(entity)
                .unwrap(),
        );
        // 原 phase42 将 Home 与配置点击混在同帧；真实 UI 隔离必须拦住 Home。
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Home);
        app.update();
        assert!(app.world().resource::<crate::panels::UiPointer>().blocked);
        assert_eq!(
            app.world().get::<Transform>(camera).unwrap().translation,
            initial
        );
        assert!(app.world().resource::<crate::CameraDrag>().0.is_none());
        app.world_mut()
            .get_mut::<bevy_egui::EguiInput>(entity)
            .unwrap()
            .0
            .events
            .clear();
        app.update();
        assert_eq!(
            app.world().get::<Transform>(camera).unwrap().translation,
            initial
        );
        app.world_mut()
            .get_mut::<bevy_egui::EguiInput>(entity)
            .unwrap()
            .0
            .events
            .clear();
        // 下一独立步骤按生产顺序恢复指针→注入画布 Home→动画→UI 命中→相机→节拍。
        app.edit_schedule(Update, |schedule| *schedule = Schedule::new(Update));
        app.add_systems(
            Update,
            (
                restore_pointer,
                reset_input,
                animate,
                crate::panels::pointer_over_ui,
                crate::view::camera_controls,
                pace,
            )
                .chain(),
        );
        app.world_mut().resource_mut::<ResetRequested>().0 = true;
        app.update();
        if step == 450 {
            assert_eq!(
                app.world().get::<Transform>(camera).unwrap().translation,
                initial,
                "正常节奏应抵达画布后才执行 Home"
            );
            app.world_mut()
                .resource_mut::<Showcase>()
                .motion
                .as_mut()
                .unwrap()
                .started -= Duration::from_secs(1);
            app.update();
        }
        assert!(!app.world().resource::<crate::panels::UiPointer>().blocked);
        assert!(
            app.world()
                .get::<Transform>(camera)
                .unwrap()
                .translation
                .truncate()
                .length()
                < 1e-4
        );
        let Projection::Orthographic(projection) = app.world().get::<Projection>(camera).unwrap()
        else {
            panic!("需要正交相机")
        };
        assert_eq!(projection.scale, 1.);
        assert!(app.world().resource::<crate::CameraDrag>().0.is_none());
        assert!(
            !app.world()
                .resource::<ButtonInput<MouseButton>>()
                .pressed(MouseButton::Middle)
        );
    }
}

#[test]
fn egui_short_click_packet_waits_for_arrival_and_keeps_explicit_target() {
    use bevy_egui::egui;
    for step in [40, 450] {
        let ctx = crate::egui_ui::test_context();
        let mut selected = false;
        let mut frame = |time, events| {
            let mut rect = egui::Rect::NOTHING;
            let mut clicked = false;
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1440., 900.),
                    )),
                    focused: true,
                    time: Some(time),
                    events,
                    ..Default::default()
                },
                |ui| {
                    let response = ui.selectable_value(&mut selected, true, "Satellite");
                    rect = response.rect;
                    clicked = response.clicked();
                },
            );
            output.textures_delta.clear();
            (rect, clicked)
        };
        frame(0., vec![]);
        let (rect, _) = frame(0.02, vec![]);
        let pos = rect.center();
        let target = Vec2::new(pos.x, pos.y);
        let (mut app, entity, folder) = animation_app(step, target);
        app.add_systems(Update, pace.after(animate));
        app.world_mut()
            .get_mut::<Window>(entity)
            .unwrap()
            .set_cursor_position(Some(target + Vec2::new(0., 200.)));
        let mut driver = crate::egui_ui::UiTestInput::default();
        driver.click_rect(
            rect,
            &mut app
                .world_mut()
                .get_mut::<bevy_egui::EguiInput>(entity)
                .unwrap(),
        );
        app.update();
        let initial_events = std::mem::take(
            &mut app
                .world_mut()
                .get_mut::<bevy_egui::EguiInput>(entity)
                .unwrap()
                .0
                .events,
        );
        if step == 450 {
            assert!(!frame(1.0, initial_events).1, "抵达前不能发布点击");
            assert_eq!(action_log(folder.path()).len(), 0);
            app.world_mut()
                .resource_mut::<Showcase>()
                .motion
                .as_mut()
                .unwrap()
                .started -= Duration::from_secs(1);
            app.update();
            let arrived_events = std::mem::take(
                &mut app
                    .world_mut()
                    .get_mut::<bevy_egui::EguiInput>(entity)
                    .unwrap()
                    .0
                    .events,
            );
            assert!(frame(2.0, arrived_events).1, "慢帧抵达后短点击仍应生效");
        } else {
            assert!(frame(2.0, initial_events).1, "快速模式直接交付完整短点击");
        }
        // main 在非 smoke 步骤的空帧仍执行 restore → animate → pace。
        // pace 必须保存实际 egui 抵达位置，而不是 smoke 原有的画布坐标。
        assert_eq!(
            app.world().get::<Window>(entity).unwrap().cursor_position(),
            Some(target)
        );
        app.add_systems(Update, restore_pointer.before(animate));
        app.update();
        let idle_events = std::mem::take(
            &mut app
                .world_mut()
                .get_mut::<bevy_egui::EguiInput>(entity)
                .unwrap()
                .0
                .events,
        );
        assert!(!frame(3.0, idle_events).1);
        assert_eq!(
            app.world().resource::<Showcase>().display_pointer,
            Some(target)
        );
        assert!(selected);
        assert_eq!(
            action_log(folder.path()).len(),
            1,
            "同帧按下/释放只记录一次动作"
        );
    }
}
