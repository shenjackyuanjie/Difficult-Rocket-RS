use super::*;

fn key_event(key: bevy_egui::egui::Key, pressed: bool, repeat: bool) -> bevy_egui::egui::Event {
    use bevy_egui::egui;
    egui::Event::Key {
        key,
        physical_key: Some(key),
        pressed,
        repeat,
        modifiers: egui::Modifiers {
            ctrl: true,
            command: true,
            ..Default::default()
        },
    }
}

fn inject_undo(app: &mut App, entity: Entity, keyboard: bool, egui: bool) {
    if keyboard {
        let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        keys.press(KeyCode::ControlLeft);
        keys.press(KeyCode::KeyZ);
    }
    if egui {
        app.world_mut()
            .get_mut::<bevy_egui::EguiInput>(entity)
            .unwrap()
            .0
            .events
            .extend([
                key_event(bevy_egui::egui::Key::Z, true, false),
                key_event(bevy_egui::egui::Key::Z, false, false),
            ]);
    }
}

#[test]
fn key_hints_wait_for_motion_replay_for_each_input_source() {
    for (keyboard, egui) in [(true, false), (false, true), (true, true)] {
        let (mut app, entity, _folder) = animation_app(450, Vec2::new(100., 0.));
        inject_undo(&mut app, entity, keyboard, egui);
        app.update();
        assert!(
            app.world()
                .resource::<Showcase>()
                .key_hints
                .latest
                .is_none()
        );
        assert!(
            !app.world()
                .resource::<ButtonInput<KeyCode>>()
                .just_pressed(KeyCode::KeyZ)
        );
        assert!(
            !app.world()
                .get::<bevy_egui::EguiInput>(entity)
                .unwrap()
                .0
                .events
                .iter()
                .any(|event| matches!(event, bevy_egui::egui::Event::Key { .. }))
        );
        {
            let mut showcase = app.world_mut().resource_mut::<Showcase>();
            let motion = showcase.motion.as_mut().unwrap();
            // 无 sleep：明确保持中途帧未到达，然后强制到达。
            motion.started = Instant::now();
            motion.duration = Duration::from_secs(60);
        }
        app.update();
        assert!(
            app.world()
                .resource::<Showcase>()
                .key_hints
                .latest
                .is_none()
        );
        {
            let mut showcase = app.world_mut().resource_mut::<Showcase>();
            let motion = showcase.motion.as_mut().unwrap();
            motion.started = Instant::now() - motion.duration;
        }
        let replay_start = Instant::now();
        app.update();
        let showcase = app.world().resource::<Showcase>();
        let hint = showcase.key_hints.latest.as_ref().unwrap();
        assert_eq!(hint.text, "Ctrl+Z");
        assert!(hint.applied >= replay_start);
        assert!(showcase.motion.is_none());
        let applied = hint.applied;
        assert_eq!(
            app.world()
                .resource::<ButtonInput<KeyCode>>()
                .just_pressed(KeyCode::KeyZ),
            keyboard
        );
        let input = app.world().get::<bevy_egui::EguiInput>(entity).unwrap();
        assert_eq!(
            input
                .0
                .events
                .iter()
                .filter(|event| matches!(event, bevy_egui::egui::Event::Key { .. }))
                .count(),
            if egui { 2 } else { 0 }
        );
        // 模拟 egui pass 消费事件，保留 ButtonInput 边沿检验不逐帧刷新。
        app.world_mut()
            .get_mut::<bevy_egui::EguiInput>(entity)
            .unwrap()
            .0
            .events
            .clear();
        app.update();
        assert_eq!(
            app.world()
                .resource::<Showcase>()
                .key_hints
                .latest
                .as_ref()
                .unwrap()
                .applied,
            applied
        );
    }
}

#[test]
fn key_hints_capture_direct_inputs_in_fast_and_stationary_modes() {
    for (step, target) in [(1, Vec2::new(100., 0.)), (450, Vec2::ZERO)] {
        for (keyboard, egui) in [(true, false), (false, true), (true, true)] {
            let (mut app, entity, _folder) = animation_app(step, target);
            inject_undo(&mut app, entity, keyboard, egui);
            app.update();
            let showcase = app.world().resource::<Showcase>();
            assert!(showcase.motion.is_none());
            assert_eq!(showcase.key_hints.latest.as_ref().unwrap().text, "Ctrl+Z");
            assert!(showcase.click_release.is_none());
        }
    }
}

#[test]
fn key_hint_opacity_holds_then_fades_linearly_and_expires() {
    let now = Instant::now();
    let hint = KeyHint {
        text: "F2".into(),
        applied: now,
    };
    assert_eq!(hint.opacity(now), 1.);
    assert_eq!(hint.opacity(now + KEY_HINT_HOLD), 1.);
    assert!((hint.opacity(now + KEY_HINT_HOLD + KEY_HINT_FADE / 4) - 0.75).abs() < 0.001);
    assert!((hint.opacity(now + KEY_HINT_HOLD + KEY_HINT_FADE / 2) - 0.5).abs() < 0.001);
    assert_eq!(hint.opacity(now + KEY_HINT_HOLD + KEY_HINT_FADE), 0.);
    assert_eq!(hint.opacity(now + Duration::from_secs(10)), 0.);
}

#[test]
fn key_hints_ignore_modifiers_repeats_and_held_edges_but_accept_repress() {
    use bevy_egui::egui;
    let now = Instant::now();
    let mut hints = KeyHints::default();
    let mut keys = ButtonInput::default();
    for key in [
        KeyCode::ControlRight,
        KeyCode::ShiftRight,
        KeyCode::AltRight,
        KeyCode::SuperRight,
    ] {
        keys.press(key);
    }
    hints.collect(
        &keys,
        &[key_event(egui::Key::ControlLeft, true, false)],
        now,
    );
    assert!(hints.latest.is_none());
    keys.press(KeyCode::KeyZ);
    hints.collect(&keys, &[key_event(egui::Key::Z, true, true)], now);
    assert!(
        hints.latest.is_none(),
        "repeat 不得经由 ButtonInput fallback 重新出现"
    );
    hints.collect(&keys, &[], now);
    assert!(hints.latest.is_none());
    keys.clear();
    hints.collect(&keys, &[], now);
    keys.release(KeyCode::KeyZ);
    keys.press(KeyCode::KeyZ);
    hints.collect(&keys, &[], now);
    assert_eq!(hints.latest.as_ref().unwrap().text, "Ctrl+Alt+Shift+Cmd+Z");
    let later = now + Duration::from_millis(500);
    hints.collect(&keys, &[], later);
    assert_eq!(hints.latest.as_ref().unwrap().applied, now);
    keys.clear();
    hints.collect(&keys, &[], later);
    keys.release(KeyCode::KeyZ);
    keys.press(KeyCode::KeyZ);
    hints.collect(&keys, &[], later);
    assert_eq!(hints.latest.as_ref().unwrap().applied, later);
}

#[test]
fn key_hints_use_logical_names_and_deduplicate_physical_keys() {
    use bevy_egui::egui;
    let mut hints = KeyHints::default();
    let mut keys = ButtonInput::default();
    keys.press(KeyCode::KeyY);
    let mut event = key_event(egui::Key::Z, true, false);
    if let egui::Event::Key {
        physical_key,
        modifiers,
        ..
    } = &mut event
    {
        *physical_key = Some(egui::Key::Y);
        modifiers.shift = true;
    }
    hints.collect(&keys, &[event], Instant::now());
    assert_eq!(hints.latest.as_ref().unwrap().text, "Ctrl+Shift+Z");
    for (key, expected) in [
        (KeyCode::KeyR, "R"),
        (KeyCode::Digit1, "1"),
        (KeyCode::F2, "F2"),
        (KeyCode::Escape, "Escape"),
        (KeyCode::ArrowLeft, "Left"),
        (KeyCode::NumpadEnter, "Enter"),
    ] {
        assert_eq!(button_key_name(key), expected);
    }
}

#[test]
fn key_hint_painter_does_not_block_underlying_input_and_stops_after_expiry() {
    use bevy_egui::egui;
    let ctx = egui::Context::default();
    let now = Instant::now();
    let hint = KeyHint {
        text: "Ctrl+Z".into(),
        applied: now,
    };
    let rect = egui::Rect::from_center_size(egui::pos2(400., 72.), egui::vec2(120., 40.));
    let raw = || egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(800., 600.),
        )),
        ..Default::default()
    };
    let mut clicked = false;
    // 首帧布局，第二帧把点击送到提示覆盖的按钮。
    for click in [false, true] {
        let mut input = raw();
        if click {
            input.events.push(egui::Event::PointerMoved(rect.center()));
            for pressed in [true, false] {
                input.events.push(egui::Event::PointerButton {
                    pos: rect.center(),
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                });
            }
            input.events.push(key_event(egui::Key::Z, true, false));
        }
        let mut output = ctx.run_ui(input, |ui| {
            clicked |= ui.put(rect, egui::Button::new("下层按钮")).clicked();
            let ctx = ui.ctx();
            let before = ctx.input(|input| input.events.clone());
            let focus = ctx.memory(|memory| memory.focused());
            paint_key_hint(ctx, &hint, now, false);
            assert_eq!(ctx.input(|input| input.events.clone()), before);
            assert_eq!(ctx.memory(|memory| memory.focused()), focus);
        });
        // 无头测试不上传 GPU 字体图集，确认丢弃纹理增量。
        output.textures_delta.clear();
        assert!(output.shapes.iter().any(|shape| matches!(&shape.shape, egui::epaint::Shape::Text(text) if text.galley.text() == "Ctrl+Z")));
    }
    assert!(clicked, "提示所在位置的下层按钮必须仍收到点击");
    ctx.begin_pass(raw());
    paint_key_hint(&ctx, &hint, now + KEY_HINT_HOLD + KEY_HINT_FADE, false);
    let mut output = ctx.end_pass();
    output.textures_delta.clear();
    assert!(output.shapes.is_empty());
}

#[test]
fn key_hint_below_help_avoids_minimum_window_sheet() {
    use bevy_egui::egui;
    let ctx = egui::Context::default();
    let now = Instant::now();
    let hint = KeyHint {
        text: "F4 · R".into(),
        applied: now,
    };
    ctx.begin_pass(egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(960., 640.),
        )),
        ..Default::default()
    });
    paint_key_hint(&ctx, &hint, now, true);
    let mut output = ctx.end_pass();
    output.textures_delta.clear();
    let rect = output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::epaint::Shape::Rect(shape) => Some(shape.rect),
            _ => None,
        })
        .unwrap();
    assert!(rect.top() >= 580. && rect.bottom() <= 628., "{rect:?}");
}
