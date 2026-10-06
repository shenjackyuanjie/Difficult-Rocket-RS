use super::*;

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
        let (mut app, entity, _folder) = tests::animation_app(step, ui_point);
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
            .add_message::<crate::MouseWheel>();
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
                crate::camera_controls,
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
        app.world_mut()
            .resource_mut::<Showcase>()
            .click_release
            .as_mut()
            .unwrap()
            .0 = Instant::now();
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
                crate::camera_controls,
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

fn action_log(folder: &std::path::Path) -> Vec<serde_json::Value> {
    std::fs::read_to_string(folder.join("demo-actions.jsonl"))
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

#[test]
fn egui_click_release_keeps_explicit_target_when_smoke_window_pointer_is_stale() {
    use bevy_egui::egui;
    for step in [40, 450] {
        let ctx = egui::Context::default();
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
        let (mut app, entity, folder) = tests::animation_app(step, target);
        app.add_systems(Update, pace.after(animate));
        app.world_mut().resource_mut::<Showcase>().display_pointer = Some(target);
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
        let press_events = std::mem::take(
            &mut app
                .world_mut()
                .get_mut::<bevy_egui::EguiInput>(entity)
                .unwrap()
                .0
                .events,
        );
        assert!(!frame(0.1, press_events).1);
        // main 在非 smoke 步骤的空帧仍执行 restore → animate → pace。
        // pace 必须保存实际 egui 抵达位置，否则短按期间会先回到旧窗口位置。
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
        assert!(!frame(0.11, idle_events).1);
        assert_eq!(
            app.world().resource::<Showcase>().display_pointer,
            Some(target)
        );
        // restore 仅处理原生输入，模拟 main 中位于其后的 smoke release。
        app.edit_schedule(Update, |schedule| {
            *schedule = Schedule::new(Update);
        });
        app.add_systems(Update, (animate, pace).chain());
        // smoke 在下一步骤会恢复它保存的旧画布/滚轮坐标，而 egui release 仍指向控件。
        app.world_mut()
            .get_mut::<Window>(entity)
            .unwrap()
            .set_cursor_position(Some(target + Vec2::new(0., 200.)));
        assert!(
            driver.tick(
                &mut app
                    .world_mut()
                    .get_mut::<bevy_egui::EguiInput>(entity)
                    .unwrap()
            )
        );
        // 模拟短按已到自动释放时间，不能让先注入的旧坐标取消同帧 release。
        app.world_mut().resource_mut::<Showcase>().click_release = Some((Instant::now(), pos));
        app.update();
        assert!(
            app.world().resource::<Showcase>().motion.is_none(),
            "release 不应向旧窗口坐标移动"
        );
        assert_eq!(
            app.world().resource::<Showcase>().display_pointer,
            Some(target)
        );
        let release_events = std::mem::take(
            &mut app
                .world_mut()
                .get_mut::<bevy_egui::EguiInput>(entity)
                .unwrap()
                .0
                .events,
        );
        assert!(
            frame(0.14, release_events).1,
            "分类控件应收到有效的短按点击：step={step}"
        );
        assert!(selected);
        assert_eq!(
            action_log(folder.path()).len(),
            1,
            "自动/驱动释放不应额外写动作"
        );
    }
}
#[test]
fn action_log_waits_for_actual_replay_and_does_not_record_moving_frames() {
    let (mut app, entity, folder) = tests::animation_app(450, Vec2::new(100., 0.));
    assert!(folder.path().join("demo-actions.jsonl").exists());
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyR);
    app.update();
    assert!(action_log(folder.path()).is_empty());
    assert!(
        app.world()
            .resource::<Showcase>()
            .mouse_hints
            .latest
            .is_none()
    );
    {
        let mut showcase = app.world_mut().resource_mut::<Showcase>();
        let motion = showcase.motion.as_mut().unwrap();
        motion.started = Instant::now() - motion.duration / 2;
    }
    app.update();
    assert!(action_log(folder.path()).is_empty());
    {
        let mut showcase = app.world_mut().resource_mut::<Showcase>();
        let motion = showcase.motion.as_mut().unwrap();
        motion.started = Instant::now() - motion.duration;
    }
    let before = unix_ms(SystemTime::now());
    app.update();
    let after = unix_ms(SystemTime::now());
    let actions = action_log(folder.path());
    assert_eq!(actions.len(), 2);
    assert_eq!(actions[0]["kind"], "left");
    assert_eq!(actions[1]["kind"], "key");
    for action in actions {
        assert!((before..=after).contains(&action["unix_ms"].as_u64().unwrap()));
        assert_eq!(action["chapter_id"], "panels");
    }
    app.world_mut()
        .get_mut::<bevy_egui::EguiInput>(entity)
        .unwrap()
        .0
        .events
        .clear();
    app.update();
    assert_eq!(action_log(folder.path()).len(), 2, "持有边沿不应逐帧重复");
    assert!(
        std::fs::read_to_string(folder.path().join("demo-timeline.jsonl"))
            .unwrap()
            .is_empty()
    );
}

#[test]
fn mouse_actions_distinguish_buttons_and_deduplicate_egui_and_bevy_presses() {
    use bevy_egui::egui;
    let mut hints = MouseHints::default();
    let now = Instant::now();
    for (button, ui_button, expected) in [
        (MouseButton::Left, egui::PointerButton::Primary, "left"),
        (MouseButton::Middle, egui::PointerButton::Middle, "middle"),
        (MouseButton::Right, egui::PointerButton::Secondary, "right"),
    ] {
        let mut mouse = ButtonInput::default();
        mouse.press(button);
        let event = |pressed| egui::Event::PointerButton {
            pos: egui::Pos2::ZERO,
            button: ui_button,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        let actions = hints.collect(&mouse, &[event(true)], now);
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].0, expected);
        assert!(hints.collect(&mouse, &[], now).is_empty());
        mouse.release(button);
        mouse.clear();
        assert!(hints.collect(&mouse, &[event(false)], now).is_empty());
        mouse.press(button);
        assert_eq!(
            hints.collect(&mouse, &[], now).len(),
            1,
            "再次按下应正常记录"
        );
    }
    let mut unknown = ButtonInput::default();
    unknown.press(MouseButton::Other(8));
    assert!(
        hints
            .collect(
                &unknown,
                &[egui::Event::PointerMoved(egui::Pos2::ZERO)],
                now
            )
            .is_empty()
    );
}

#[test]
fn fast_action_log_records_key_scroll_and_right_once_not_release_or_motion() {
    use bevy_egui::egui;
    let (mut app, entity, folder) = tests::animation_app(40, Vec2::ZERO);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyR);
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Right);
    app.world_mut()
        .get_mut::<bevy_egui::EguiInput>(entity)
        .unwrap()
        .0
        .events
        .extend([
            egui::Event::Key {
                key: egui::Key::R,
                physical_key: Some(egui::Key::R),
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            },
            egui::Event::MouseWheel {
                phase: egui::TouchPhase::Move,
                unit: egui::MouseWheelUnit::Point,
                delta: egui::vec2(0., -120.),
                modifiers: egui::Modifiers::NONE,
            },
        ]);
    app.update();
    let actions = action_log(folder.path());
    assert_eq!(actions.len(), 3);
    assert_eq!(actions.iter().filter(|a| a["kind"] == "key").count(), 1);
    assert_eq!(actions.iter().filter(|a| a["kind"] == "scroll").count(), 1);
    assert_eq!(actions.iter().filter(|a| a["kind"] == "right").count(), 1);
    app.world_mut()
        .get_mut::<bevy_egui::EguiInput>(entity)
        .unwrap()
        .0
        .events
        .clear();
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .release(MouseButton::Right);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .release(KeyCode::KeyR);
    app.update();
    assert_eq!(action_log(folder.path()).len(), 3);
}

#[test]
fn changed_configuration_explanation_has_readable_pause_but_fast_keeps_40ms() {
    for step in [450, 40] {
        let (mut app, _, _folder) = tests::animation_app(step, Vec2::ZERO);
        {
            let mut showcase = app.world_mut().resource_mut::<Showcase>();
            showcase.description = Some("已有配置说明");
            showcase.description_changed = true;
        }
        app.update();
        let showcase = app.world().resource::<Showcase>();
        assert!(!showcase.description_changed);
        let remaining = showcase.next_tick.saturating_duration_since(Instant::now());
        if step == 40 {
            assert!(remaining <= Duration::from_millis(40));
        } else {
            assert!(remaining > Duration::from_millis(600));
        }
    }
}

#[test]
fn actions_have_distinct_readable_pauses_and_fast_remains_fast() {
    let step = Duration::from_millis(450);
    let mouse = ButtonInput::default();
    let keys = ButtonInput::default();
    assert_eq!(
        classify_action(false, &mouse, &mouse, &keys, &[]),
        Action::Technical
    );
    assert_eq!(
        classify_action(true, &mouse, &mouse, &keys, &[]),
        Action::Navigation
    );
    let mut pressed = ButtonInput::default();
    pressed.press(MouseButton::Left);
    assert_eq!(
        classify_action(false, &mouse, &pressed, &keys, &[]),
        Action::Click
    );
    pressed.clear();
    assert_eq!(
        classify_action(true, &pressed, &pressed, &keys, &[]),
        Action::Drag
    );
    let mut key = ButtonInput::default();
    key.press(KeyCode::KeyR);
    assert_eq!(
        classify_action(false, &mouse, &mouse, &key, &[]),
        Action::Key
    );
    for action in [Action::Click, Action::Key] {
        assert!(
            (Duration::from_millis(500)..=Duration::from_millis(900))
                .contains(&action_pause(step, action))
        );
    }
    assert!(action_pause(step, Action::Technical) < action_pause(step, Action::Navigation));
    assert!(
        action_pause(Duration::from_millis(300), Action::Click) < action_pause(step, Action::Click)
    );
    assert!(movement_duration(Duration::from_millis(300), 600.) < movement_duration(step, 600.));
    assert_eq!(input_hold(step), Duration::from_millis(45));
    assert_eq!(
        input_hold(Duration::from_millis(40)),
        Duration::from_millis(20)
    );
    assert_eq!(chapter_pause(step, true), Duration::from_millis(1200));
    assert_eq!(chapter_pause(step, false), Duration::from_millis(900));
    for action in [
        Action::Technical,
        Action::Navigation,
        Action::Click,
        Action::Key,
        Action::Drag,
    ] {
        assert_eq!(
            action_pause(Duration::from_millis(40), action),
            Duration::from_millis(40)
        );
    }
    assert_eq!(
        chapter_pause(Duration::from_millis(40), true),
        Duration::from_millis(40)
    );
    assert_eq!(
        chapter_pause(Duration::from_millis(40), false),
        Duration::from_millis(40)
    );
    assert_eq!(movement_duration(step, 10.), Duration::from_millis(200));
    assert!(movement_duration(step, 600.) > movement_duration(step, 300.));
    assert_eq!(movement_duration(step, 2000.), Duration::from_millis(800));
    assert_eq!(
        movement_duration(Duration::from_millis(40), 2000.),
        Duration::ZERO
    );
}

#[derive(Resource, Default)]
struct Advances(u32);

fn click_once(
    mut advances: ResMut<Advances>,
    mut windows: Query<&mut Window>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
) {
    advances.0 += 1;
    if advances.0 == 1 {
        windows
            .single_mut()
            .unwrap()
            .set_cursor_position(Some(Vec2::new(100., 0.)));
        mouse.press(MouseButton::Left);
    }
}

#[test]
fn real_schedule_waits_for_arrival_then_preserves_click_pause_against_pace() {
    let (mut app, entity, _folder) = tests::animation_app(450, Vec2::ZERO);
    // 替换仅含 animate 的测试调度，使用 main 的实际顺序。
    app.edit_schedule(Update, |schedule| {
        *schedule = Schedule::new(Update);
    });
    app.init_resource::<Advances>().add_systems(
        Update,
        (
            restore_pointer,
            click_once.run_if(advance_ready),
            animate,
            pace,
        )
            .chain(),
    );
    app.world_mut().resource_mut::<Showcase>().pointer = Some(Vec2::ZERO);
    app.update();
    assert_eq!(app.world().resource::<Advances>().0, 1);
    assert!(app.world().resource::<Showcase>().motion.is_some());
    assert!(
        !app.world()
            .resource::<ButtonInput<MouseButton>>()
            .pressed(MouseButton::Left)
    );
    {
        let mut showcase = app.world_mut().resource_mut::<Showcase>();
        let motion = showcase.motion.as_mut().unwrap();
        motion.started = Instant::now() - motion.duration / 2;
    }
    app.update();
    let x = app
        .world()
        .get::<Window>(entity)
        .unwrap()
        .cursor_position()
        .unwrap()
        .x;
    assert!((45. ..55.).contains(&x), "线性中间帧：{x}");
    assert_eq!(app.world().resource::<Advances>().0, 1);
    {
        let mut showcase = app.world_mut().resource_mut::<Showcase>();
        let motion = showcase.motion.as_mut().unwrap();
        motion.started = Instant::now() - motion.duration;
    }
    app.update();
    assert!(
        app.world()
            .resource::<ButtonInput<MouseButton>>()
            .just_pressed(MouseButton::Left)
    );
    let deadline = app.world().resource::<Showcase>().next_tick;
    assert!(deadline.saturating_duration_since(Instant::now()) > Duration::from_millis(600));
    app.update();
    assert_eq!(app.world().resource::<Advances>().0, 1);
    assert_eq!(app.world().resource::<Showcase>().next_tick, deadline);
}

#[test]
fn short_key_press_does_not_hold_for_the_reading_pause() {
    let (mut app, _entity, _folder) = tests::animation_app(450, Vec2::ZERO);
    {
        let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        keys.press(KeyCode::ControlLeft);
        keys.press(KeyCode::KeyZ);
    }
    app.update();
    let deadline = app.world().resource::<Showcase>().next_tick;
    assert!(
        app.world()
            .resource::<ButtonInput<KeyCode>>()
            .pressed(KeyCode::KeyZ)
    );
    app.world_mut()
        .resource_mut::<Showcase>()
        .key_release
        .as_mut()
        .unwrap()
        .0 = Instant::now();
    app.update();
    assert!(
        !app.world()
            .resource::<ButtonInput<KeyCode>>()
            .pressed(KeyCode::KeyZ)
    );
    assert!(
        app.world()
            .resource::<ButtonInput<KeyCode>>()
            .pressed(KeyCode::ControlLeft)
    );
    assert_eq!(app.world().resource::<Showcase>().next_tick, deadline);
}

#[test]
fn timeline_is_immediate_integer_jsonl_and_outro_blocks_next_initialization() {
    let (mut app, _entity, folder) = tests::animation_app(450, Vec2::ZERO);
    let path = folder.path().join("demo-timeline.jsonl");
    assert!(path.exists());
    let started = SystemTime::now();
    append_timeline(folder.path(), "chapter_started", &CHAPTERS[0], started);
    {
        let mut showcase = app.world_mut().resource_mut::<Showcase>();
        showcase.chapter_start_unix_ms = unix_ms(started);
        // 模拟已经通过证据验证的成果停留状态，不写入或污染 target 截图。
        showcase.finishing = Some(Instant::now() + Duration::from_secs(60));
    }
    app.init_resource::<Messages<AppExit>>();
    finish(app.world_mut());
    begin(app.world_mut());
    assert_eq!(app.world().resource::<Showcase>().index, 0);
    assert!(!app.world().resource::<Showcase>().initialize);
    assert_eq!(std::fs::read_to_string(&path).unwrap().lines().count(), 1);
    app.world_mut().resource_mut::<Showcase>().finishing = Some(Instant::now());
    finish(app.world_mut());
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.ends_with('\n'));
    let events: Vec<serde_json::Value> = text
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(events.len(), 2);
    for (event, expected) in events.iter().zip(["chapter_started", "chapter_finished"]) {
        assert_eq!(event["event"], expected);
        assert_eq!(event["id"], CHAPTERS[0].id);
        assert_eq!(event["title"], CHAPTERS[0].title);
        assert!(event["unix_ms"].is_u64());
    }
    let showcase = app.world().resource::<Showcase>();
    assert!(showcase.initialize);
    assert_eq!(showcase.index, 1);
    assert_eq!(showcase.reports[0]["start_unix_ms"], events[0]["unix_ms"]);
    assert_eq!(showcase.reports[0]["end_unix_ms"], events[1]["unix_ms"]);
    assert_eq!(showcase.reports[0]["status"], "passed");
    // 未到 intro 开始前，begin 不得先加载文档（无 EditorPaths 也不会 panic）。
    app.world_mut().resource_mut::<Showcase>().next_tick =
        Instant::now() + chapter_pause(Duration::from_millis(450), true);
    begin(app.world_mut());
    assert_eq!(std::fs::read_to_string(&path).unwrap().lines().count(), 2);
}

#[test]
fn presentation_argument_is_independent_of_fast_pacing() {
    let folder = tempfile::tempdir().unwrap();
    let args = vec![
        "editor".into(),
        "--demo-showcase".into(),
        folder.path().to_string_lossy().into_owned(),
        "--demo-presentation".into(),
        "--demo-step-ms".into(),
        "40".into(),
    ];
    let showcase = Showcase::from_args(&args).unwrap().unwrap();
    assert!(showcase.presentation);
    assert_eq!(showcase.step, Duration::from_millis(40));
}
