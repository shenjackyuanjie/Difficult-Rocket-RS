use super::*;

#[test]
fn changed_configuration_explanation_has_readable_pause_but_fast_keeps_40ms() {
    for step in [450, 40] {
        let (mut app, _, _folder) = animation_app(step, Vec2::ZERO);
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
    let (mut app, entity, _folder) = animation_app(450, Vec2::ZERO);
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
    let (mut app, _entity, _folder) = animation_app(450, Vec2::ZERO);
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
