use super::*;

#[test]
fn failed_and_unproven_success_exit_are_not_swallowed() {
    let folder = tempfile::tempdir().unwrap();
    let args = vec![
        "editor".into(),
        "--demo-showcase".into(),
        folder.path().to_string_lossy().into_owned(),
    ];
    for exit in [
        AppExit::Success,
        AppExit::Error(std::num::NonZeroU8::new(1).unwrap()),
    ] {
        let mut showcase = Showcase::from_args(&args).unwrap().unwrap();
        showcase.initialize = false;
        showcase.artifact_since = SystemTime::now() + Duration::from_secs(3600);
        let mut world = World::new();
        world.insert_resource(showcase);
        let mut exits = Messages::<AppExit>::default();
        exits.write(exit.clone());
        world.insert_resource(exits);
        finish(&mut world);
        assert_eq!(world.resource::<Showcase>().index, 0);
        assert!(world.resource::<Showcase>().reports.is_empty());
        assert_eq!(
            world
                .resource::<Messages<AppExit>>()
                .iter_current_update_messages()
                .next(),
            Some(&exit)
        );
        assert!(!folder.path().join("demo-report.json").exists());
        assert!(
            std::fs::read_to_string(folder.path().join("demo-timeline.jsonl"))
                .unwrap()
                .is_empty()
        );
    }
}

#[test]
fn action_log_waits_for_actual_replay_and_does_not_record_moving_frames() {
    let (mut app, entity, folder) = animation_app(450, Vec2::new(100., 0.));
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
    let (mut app, entity, folder) = animation_app(40, Vec2::ZERO);
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
fn timeline_is_immediate_integer_jsonl_and_outro_blocks_next_initialization() {
    let (mut app, _entity, folder) = animation_app(450, Vec2::ZERO);
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
