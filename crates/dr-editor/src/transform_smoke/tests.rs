
use super::*;

fn rect(x: f32, y: f32, width: f32, height: f32) -> egui::Rect {
    egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(width, height))
}

fn assert_click(input: &EguiInput, target: egui::Rect) {
    assert!(matches!(input.0.events.as_slice(), [
            egui::Event::PointerMoved(position),
            egui::Event::PointerButton { pos: down, pressed: true, .. },
            egui::Event::PointerButton { pos: up, pressed: false, .. },
        ] if *position == target.center() && down == position && up == position));
}

#[test]
fn closed_tools_are_opened_by_the_real_settings_button_first() {
    let mut state = State::default();
    let mut ui = panels::egui_panel::UiState::default();
    let toggle = rect(248.0, 590.0, 44.0, 44.0);
    ui.settings_toggle = Some(toggle);
    let mut input = EguiInput::default();
    // 即使上一帧残留配置矩形，也先使用当前可见的正式入口。
    assert!(!config_control(
        &mut state,
        &ui,
        &mut input,
        Some(rect(20.0, 20.0, 50.0, 44.0))
    ));
    assert_click(&input, toggle);
    assert!(!ui.drawer_open(), "驱动不能绕过真实输入直接打开抽屉");
    assert_eq!(state.phase, 0);
}

#[test]
fn closed_parts_use_the_real_toggle_and_keep_transform_placement_at_phase_35() {
    use panels::PanelButton;
    let toggle = rect(144.0, 590.0, 44.0, 44.0);
    for toggle_visible in [false, true] {
        let mut state = State {
            phase: 35,
            ..default()
        };
        let mut ui = panels::egui_panel::UiState::default();
        ui.part_toggle = toggle_visible.then_some(toggle);
        // 当前工具条命中与过期 Part 混合时，仍只能从真实入口打开抽屉。
        ui.hits
            .push((PanelButton::Undo, rect(192.0, 590.0, 44.0, 44.0)));
        ui.hits
            .push((PanelButton::Part(7), rect(1120.0, 120.0, 44.0, 44.0)));
        let mut input = EguiInput::default();
        let ready = part_control(&mut state, &ui, &mut input, 7);
        // 与 phase35 一致：只有实际 Part 点击完成才允许后续预览阶段推进。
        if ready {
            state.phase += 1;
        }
        assert!(!ready, "打开入口不等于已选择部件");
        assert_eq!(state.phase, 35, "等待抽屉命中时不能提前推进阶段");
        assert!(ui.palette_area.is_none());
        assert!(
            !ui.drawer_open(),
            "驱动只能发真实点击，不能直接修改抽屉状态"
        );
        if toggle_visible {
            assert_click(&input, toggle);
        } else {
            assert!(
                input.0.events.is_empty(),
                "入口尚未布局时等待，不回退到固定坐标"
            );
        }
    }
}

#[test]
fn part_navigation_expands_resets_and_searches_until_a_clipped_hit_is_ready() {
    use panels::PanelButton;

    let mut state = State {
        phase: 35,
        ..default()
    };
    let mut ui = panels::egui_panel::UiState::default();
    let area = rect(1100.0, 20.0, 280.0, 700.0);
    let expand = rect(1110.0, 70.0, 180.0, 30.0);
    ui.palette_area = Some(area);
    ui.palette_offset = 1800.0;
    ui.hits.push((PanelButton::ExpandCategories, expand));
    // 旧区域的 Part 即使仍为正面积，也不能绕过真实目录导航。
    ui.hits
        .push((PanelButton::Part(7), rect(1110.0, 900.0, 180.0, 60.0)));
    let mut input = EguiInput::default();

    assert!(!part_control(&mut state, &ui, &mut input, 7));
    assert_click(&input, expand);
    input.0.events.clear();
    assert!(!part_control(&mut state, &ui, &mut input, 7));
    assert!(matches!(input.0.events.as_slice(), [
            egui::Event::PointerMoved(position),
            egui::Event::MouseWheel { delta, .. },
        ] if *position == area.center() && delta.y == 10_000.0));

    input.0.events.clear();
    ui.scrolling = true;
    assert!(!part_control(&mut state, &ui, &mut input, 7));
    assert!(input.0.events.is_empty(), "回顶惯性结束前不能继续寻址");
    ui.scrolling = false;
    ui.palette_offset = 0.0;
    assert!(!part_control(&mut state, &ui, &mut input, 7));
    assert!(matches!(input.0.events.as_slice(), [
            egui::Event::PointerMoved(position),
            egui::Event::MouseWheel { delta, .. },
        ] if *position == area.center() && delta.y == -96.0));

    let target = rect(1110.0, 708.0, 180.0, 60.0);
    ui.hits.push((PanelButton::Part(7), target));
    input.0.events.clear();
    ui.scrolling = true;
    assert!(!part_control(&mut state, &ui, &mut input, 7));
    assert!(input.0.events.is_empty(), "实际命中出现后仍等待滚动结束");
    ui.scrolling = false;
    assert!(part_control(&mut state, &ui, &mut input, 7));
    assert_click(&input, target.intersect(area));
    assert_eq!(state.phase, 35, "只有调用方在真实Part点击后推进阶段");
    assert_eq!(ui.palette_offset, 0.0, "驱动只发事件，不直接写滚动位置");
    assert!(!ui.drawer_open(), "测试快照也不能由驱动直接改抽屉");
}

#[test]
fn moving_palette_offsets_reset_stall_detection_and_inertia_does_not_count() {
    let mut state = State {
        phase: 35,
        part_search: PartSearch {
            expanded: true,
            reset: true,
            previous_offset: Some(500.0),
            stalled_scrolls: 2,
        },
        ..default()
    };
    let mut ui = panels::egui_panel::UiState::default();
    ui.palette_area = Some(rect(1100.0, 20.0, 280.0, 700.0));
    ui.palette_offset = 596.0;
    let mut input = EguiInput::default();
    assert!(!part_control(&mut state, &ui, &mut input, 7));
    assert_eq!(state.part_search.stalled_scrolls, 0);
    assert_eq!(state.part_search.previous_offset, Some(596.0));
    input.0.events.clear();
    ui.scrolling = true;
    assert!(!part_control(&mut state, &ui, &mut input, 7));
    assert!(input.0.events.is_empty());
    assert_eq!(state.part_search.stalled_scrolls, 0);
}

#[test]
#[should_panic(expected = "部件目录导航停滞：阶段 35，目标 Part(7)")]
fn repeated_scrolls_at_the_boundary_report_navigation_failure() {
    let mut state = State {
        phase: 35,
        part_search: PartSearch {
            expanded: true,
            reset: true,
            ..default()
        },
        ..default()
    };
    let mut ui = panels::egui_panel::UiState::default();
    ui.palette_area = Some(rect(1100.0, 20.0, 280.0, 700.0));
    ui.palette_offset = 2500.0;
    let mut input = EguiInput::default();
    for _ in 0..4 {
        input.0.events.clear();
        assert!(!part_control(&mut state, &ui, &mut input, 7));
        assert_eq!(state.phase, 35);
    }
}

#[test]
fn missing_controls_scroll_the_actual_tools_area_and_restart_for_a_new_target() {
    let mut state = State {
        phase: 5,
        ..default()
    };
    let mut ui = panels::egui_panel::UiState::default();
    let area = rect(214.0, 22.0, 226.0, 252.0);
    ui.tools_area = Some(area);
    let mut input = EguiInput::default();
    for (phase, sign) in [(5, 1.0), (5, -1.0), (15, 1.0)] {
        state.phase = phase;
        input.0.events.clear();
        assert!(!config_control(&mut state, &ui, &mut input, None));
        assert!(matches!(input.0.events.as_slice(), [
                egui::Event::PointerMoved(position),
                egui::Event::MouseWheel { delta, unit: egui::MouseWheelUnit::Point, .. },
            ] if *position == area.center() && delta.y * sign > 0.0));
        assert_eq!(state.phase, phase, "滚动查找不能提前推进专项阶段");
    }
}

#[test]
fn visible_controls_are_clicked_only_after_scrolling_stops() {
    let mut state = State::default();
    let mut ui = panels::egui_panel::UiState::default();
    ui.tools_area = Some(rect(214.0, 22.0, 226.0, 252.0));
    ui.scrolling = true;
    let target = rect(220.0, 210.0, 120.0, 44.0);
    let mut input = EguiInput::default();
    assert!(!config_control(&mut state, &ui, &mut input, Some(target)));
    assert!(input.0.events.is_empty());
    ui.scrolling = false;
    assert!(config_control(&mut state, &ui, &mut input, Some(target)));
    assert_click(&input, target);
}
