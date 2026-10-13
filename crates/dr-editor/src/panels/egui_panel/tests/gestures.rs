use super::*;

/// 沿用目录行现成的 `part_row` + ScrollArea 最小框架。
#[derive(Debug, Default)]
struct PaletteGesture {
    clicked: bool,
    drag_started: bool,
    contains_pointer: bool,
    offset: f32,
    first_row: Option<(egui::Id, egui::Rect)>,
}

/// 300×240 屏幕、200 高视口里的 12 行 64px 高目录行。
fn palette_scroll_frame(
    ctx: &egui::Context,
    events: Vec<egui::Event>,
    time: f64,
) -> PaletteGesture {
    let texture = egui::TextureId::User(321);
    let mut gesture = PaletteGesture::default();
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(300.0, 240.0),
            )),
            focused: true,
            time: Some(time),
            events,
            ..default()
        },
        |ui| {
            let scroll_id = ui.make_persistent_id(egui::IdSalt::new("touch_palette_rows"));
            let scroll = egui::ScrollArea::vertical()
                .id_salt("touch_palette_rows")
                .max_height(200.0)
                .auto_shrink([false, false]);
            let output = scroll.show(ui, |ui| {
                touch_scroll_area(ui, scroll_id);
                for index in 0..12 {
                    let response = part_row(
                        ui,
                        texture,
                        Vec2::new(30.0, 100.0),
                        format!("部件 {index}"),
                        false,
                        240.0,
                    );
                    if index == 0 {
                        gesture.first_row =
                            Some((response.id, response.rect.intersect(ui.clip_rect())));
                    }
                    gesture.clicked |= response.clicked();
                    gesture.drag_started |= response.drag_started();
                    gesture.contains_pointer |= response.contains_pointer();
                }
            });
            gesture.offset = output.state.offset.y;
        },
    );
    output.textures_delta.clear();
    gesture
}

/// bevy_egui 的 `write_touch_message` 先发 `Event::Touch`，再同步 PointerMoved / PointerButton。
fn touch_event(phase: egui::TouchPhase, pos: egui::Pos2) -> egui::Event {
    egui::Event::Touch {
        device_id: egui::TouchDeviceId(1),
        id: egui::TouchId(7),
        phase,
        pos,
        force: None,
    }
}

fn touch_press(pos: egui::Pos2) -> Vec<egui::Event> {
    vec![
        touch_event(egui::TouchPhase::Start, pos),
        egui::Event::PointerMoved(pos),
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        },
    ]
}

fn touch_release(pos: egui::Pos2) -> Vec<egui::Event> {
    vec![
        touch_event(egui::TouchPhase::End, pos),
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        },
        egui::Event::PointerGone,
    ]
}

#[test]
fn touch_scroll_swipe_on_palette_rows_never_clicks_or_starts_a_drag() {
    let ctx = crate::egui_ui::test_context();
    let mut time = 0.0;
    for _ in 0..3 {
        palette_scroll_frame(&ctx, vec![], time);
        time += 0.016;
    }
    let start = egui::pos2(150.0, 180.0);
    let mut gesture = PaletteGesture::default();
    let mut frames = vec![touch_press(start)];
    for index in 1..=5 {
        let pos = egui::pos2(150.0, 180.0 - index as f32 * 20.0);
        frames.push(vec![
            touch_event(egui::TouchPhase::Move, pos),
            egui::Event::PointerMoved(pos),
        ]);
    }
    frames.push(touch_release(egui::pos2(150.0, 80.0)));
    for events in frames {
        time += 0.016;
        let frame = palette_scroll_frame(&ctx, events, time);
        gesture.clicked |= frame.clicked;
        gesture.drag_started |= frame.drag_started;
        gesture.contains_pointer |= frame.contains_pointer;
        gesture.offset = frame.offset;
    }
    // 与 bevy_egui 同序的触摸指针必须让行收到指针，同时把滑动交给 ScrollArea。
    assert!(
        gesture.contains_pointer,
        "同步的 PointerMoved 必须让目录行收到指针"
    );
    assert!(!gesture.clicked, "触摸滚动不能产生目录行点击");
    assert!(!gesture.drag_started, "触摸滚动不能从目录行开始拖拽");
    assert!(
        gesture.offset > 20.0,
        "首轮触摸滑动必须实际滚动列表，偏移 {}",
        gesture.offset
    );
}

/// 复现真实回放的秒级 pass 间隔；固定控件身份和命中位置，不放宽长按阈值。
#[test]
fn palette_short_replay_click_selects_once_but_one_second_hold_is_not_a_click() {
    for (hold, should_click) in [(0.045, true), (1.0, false)] {
        let ctx = crate::egui_ui::test_context();
        let mut row = PaletteGesture::default();
        for time in [0.0, 0.016, 0.032] {
            row = palette_scroll_frame(&ctx, vec![], time);
        }
        let identity = row.first_row.expect("预热后必须存在可见的首个目录行");
        assert!(identity.1.is_positive());
        let pos = identity.1.center();
        let pointer_button = |pressed| {
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ]
        };
        let pressed = palette_scroll_frame(&ctx, pointer_button(true), 0.1);
        assert_eq!(
            pressed.first_row,
            Some(identity),
            "按下时控件身份和命中矩形必须稳定"
        );
        assert!(pressed.contains_pointer);
        assert!(!pressed.clicked && !pressed.drag_started);

        // 与失败日志同构：按下/释放分属两次 pass，两次之间没有额外指针移动。
        let released = palette_scroll_frame(&ctx, pointer_button(false), 0.1 + hold);
        assert_eq!(
            released.first_row,
            Some(identity),
            "释放时控件不能移动或更换 ID"
        );
        assert_eq!(
            released.clicked, should_click,
            "按住 {hold}s 的实际目录行点击判定"
        );
        if should_click {
            assert!(!released.drag_started, "短点击不能同时触发目录拖出");
        }
        assert!(released.offset.abs() < 0.5, "原位点击/长按不应滚动目录");

        let settled = palette_scroll_frame(&ctx, vec![], 0.116 + hold);
        assert!(!settled.clicked, "释放后的等待 pass 不能再次选择目录行");
    }
}

#[test]
fn palette_shared_short_click_packet_selects_once_after_a_slow_frame() {
    let ctx = crate::egui_ui::test_context();
    let mut row = PaletteGesture::default();
    for time in [0.0, 0.016, 0.032] {
        row = palette_scroll_frame(&ctx, vec![], time);
    }
    let identity = row.first_row.expect("预热后的目录行");
    let mut state = UiState::default();
    state.hits.push((PanelButton::Part(0), identity.1));
    let mut driver = crate::egui_ui::UiTestInput::default();
    let mut input = bevy_egui::EguiInput::default();
    assert!(driver.click_panel(PanelButton::Part(0), &state, &mut input));
    let clicked = palette_scroll_frame(&ctx, std::mem::take(&mut input.0.events), 1.0);
    assert!(clicked.clicked && !clicked.drag_started);
    assert_eq!(clicked.first_row, Some(identity));
    for time in [2.0, 3.0] {
        assert!(driver.tick(&mut input));
        assert!(input.0.events.is_empty());
        assert!(!palette_scroll_frame(&ctx, vec![], time).clicked);
    }
    assert!(!driver.tick(&mut input));
}

#[test]
fn palette_touch_tap_selects_once_and_mouse_drag_survives_touch_input() {
    for tap_first in [false, true] {
        let ctx = crate::egui_ui::test_context();
        let mut time = 0.0;
        for _ in 0..3 {
            palette_scroll_frame(&ctx, vec![], time);
            time += 0.016;
        }
        let start = egui::pos2(150.0, 30.0);
        if tap_first {
            let mut clicks = 0;
            for events in [touch_press(start), touch_release(start), vec![]] {
                time += 0.016;
                let frame = palette_scroll_frame(&ctx, events, time);
                clicks += usize::from(frame.clicked);
                assert!(!frame.drag_started, "轻点目录行不应开始部件拖拽");
                assert!(frame.offset.abs() < 0.5, "轻点不应滚动目录");
            }
            assert_eq!(clicks, 1, "触摸轻点必须恰好选择一次目录行");
        }
        time += 0.016;
        palette_scroll_frame(
            &ctx,
            vec![
                egui::Event::PointerMoved(start),
                egui::Event::PointerButton {
                    pos: start,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            time,
        );
        time += 0.016;
        let moved = palette_scroll_frame(
            &ctx,
            vec![egui::Event::PointerMoved(egui::pos2(190.0, 100.0))],
            time,
        );
        assert!(moved.drag_started, "触摸前后都应保留鼠标从目录拖出路径");
        assert!(!moved.clicked, "鼠标拖出不应同时产生目录点击");
        assert!(moved.offset.abs() < 0.5, "鼠标拖部件不能变为触摸滚动");
        time += 0.016;
        palette_scroll_frame(
            &ctx,
            vec![egui::Event::PointerButton {
                pos: egui::pos2(190.0, 100.0),
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }],
            time,
        );
    }
}
