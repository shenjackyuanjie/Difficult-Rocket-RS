use crate::egui_ui;
use crate::panels;
use crate::selection_smoke::*;

fn rect(x: f32, y: f32, width: f32, height: f32) -> egui::Rect {
    egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(width, height))
}

#[test]
fn drawer_navigation_clicks_the_formal_toggle_then_waits_for_layout() {
    let toggle = rect(48.0, 848.0, 44.0, 44.0);
    let area = rect(1144.0, 0.0, 296.0, 844.0);
    let mut driver = egui_ui::UiTestInput::default();
    let mut input = EguiInput::default();
    assert!(!wait_for_drawer(
        None,
        Some(toggle),
        &mut driver,
        &mut input
    ));
    assert!(matches!(
        input.0.events.as_slice(),
        [
            egui::Event::PointerMoved(moved),
            egui::Event::PointerButton {
                pos: pressed,
                button: egui::PointerButton::Primary,
                pressed: true,
                ..
            },
            egui::Event::PointerButton {
                pos: released,
                button: egui::PointerButton::Primary,
                pressed: false,
                ..
            }
        ] if *moved == toggle.center()
            && *pressed == toggle.center()
            && *released == toggle.center()
    ));
    assert!(driver.tick(&mut input));
    assert!(driver.tick(&mut input));
    assert!(!driver.tick(&mut input));
    input.0.events.clear();
    assert!(wait_for_drawer(
        Some(area),
        Some(toggle),
        &mut driver,
        &mut input,
    ));
    assert!(
        input.0.events.is_empty(),
        "已实际打开时不能重复点击关闭抽屉"
    );
}

#[test]
fn drawer_navigation_needs_a_real_toggle_and_positive_layout() {
    let mut driver = egui_ui::UiTestInput::default();
    let mut input = EguiInput::default();
    assert!(!wait_for_drawer(None, None, &mut driver, &mut input));
    assert!(!wait_for_drawer(
        Some(rect(0.0, 0.0, 0.0, 0.0)),
        Some(rect(0.0, 0.0, 0.0, 0.0)),
        &mut driver,
        &mut input,
    ));
    assert!(input.0.events.is_empty());
}

#[test]
fn parts_toggle_is_a_cancel_target_but_the_parts_drawer_remains_a_delete_target() {
    let mut state = panels::egui_panel::UiState::default();
    let toolbar = rect(0.0, 844.0, 1440.0, 56.0);
    let toggle = rect(48.0, 848.0, 44.0, 44.0);
    let palette = rect(1144.0, 0.0, 296.0, 844.0);
    let canvas = rect(0.0, 0.0, 1144.0, 844.0);
    state.areas = vec![toolbar, palette];
    state.part_toggle = Some(toggle);
    state.palette_area = Some(palette);
    state.canvas_area = Some(canvas);
    assert_eq!(
        non_delete_ui_target(&state, state.part_toggle),
        Some(toggle)
    );
    assert_eq!(non_delete_ui_target(&state, state.palette_area), None);
    assert_eq!(non_delete_ui_target(&state, state.canvas_area), None);
    assert_eq!(non_delete_ui_target(&state, None), None);
    state.areas.clear();
    assert_eq!(non_delete_ui_target(&state, state.part_toggle), None);
    state.areas.push(toolbar);
    state.palette_area = Some(toolbar);
    assert_eq!(non_delete_ui_target(&state, state.part_toggle), None);
    state.palette_area = Some(palette);
    state.canvas_area = Some(toolbar);
    assert_eq!(non_delete_ui_target(&state, state.part_toggle), None);
}

#[test]
fn ships_cancel_target_does_not_require_simultaneous_parts_layout() {
    let mut state = panels::egui_panel::UiState::default();
    let browser = rect(0.0, 0.0, 248.0, 844.0);
    state.browser_area = Some(browser);
    state.canvas_area = Some(rect(248.0, 0.0, 1192.0, 844.0));
    state.areas = vec![browser];
    assert!(state.palette_area.is_none());
    assert_eq!(
        non_delete_ui_target(&state, state.browser_area),
        Some(browser)
    );
    state.palette_area = Some(browser);
    assert_eq!(non_delete_ui_target(&state, state.browser_area), None);
}
