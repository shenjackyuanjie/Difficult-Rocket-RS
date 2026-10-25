
use super::*;
use bevy_egui::{EguiInput, egui};

fn rect(y: f32) -> egui::Rect {
    egui::Rect::from_min_size(egui::pos2(12.0, y), egui::vec2(44.0, 44.0))
}

#[test]
fn closed_browser_navigates_real_ships_toggle_despite_stale_scroll_cache() {
    let files = vec![PathBuf::from("samples/ship-0000.xml")];
    let toggle = rect(848.0);
    let mut ui = egui_panel::UiState::default();
    ui.ship_toggle = Some(toggle);
    ui.browser_rows = 24;
    ui.browser_offset = 43000.0;
    ui.browser_max_scroll = 43000.0;
    ui.hits = vec![(PanelButton::Open(files[0].clone()), rect(200.0))];
    let mut input = EguiInput::default();
    let mut driver = crate::egui_ui::UiTestInput::default();
    assert!(!wait_for_browser(&files, &ui, &mut input, &mut driver));
    assert_eq!(input.0.events.len(), 3);
    assert!(matches!(
        input.0.events.first(),
        Some(egui::Event::PointerMoved(pos)) if *pos == toggle.center()
    ));
    assert!(matches!(
        input.0.events.get(1),
        Some(egui::Event::PointerButton { pressed: true, .. })
    ));
    assert!(matches!(
        input.0.events.get(2),
        Some(egui::Event::PointerButton { pressed: false, .. })
    ));
    assert!(driver.tick(&mut input));
    assert!(driver.tick(&mut input));
    assert!(!driver.tick(&mut input));
    input.0.events.clear();
    ui.ship_toggle = None;
    assert!(!wait_for_browser(&files, &ui, &mut input, &mut driver));
    assert!(input.0.events.is_empty(), "入口缺失时不能用旧侧栏坐标代替");
}

#[test]
fn browser_readiness_requires_current_directory_visible_hits_and_stable_layout() {
    let files = vec![PathBuf::from("samples/ship-0000.xml")];
    let mut ui = egui_panel::UiState::default();
    ui.browser_area = Some(egui::Rect::from_min_size(
        egui::Pos2::ZERO,
        egui::vec2(248.0, 844.0),
    ));
    ui.hits = vec![(
        PanelButton::Open(PathBuf::from("old/ship.xml")),
        rect(200.0),
    )];
    let mut input = EguiInput::default();
    let mut driver = crate::egui_ui::UiTestInput::default();
    assert!(!wait_for_browser(&files, &ui, &mut input, &mut driver));
    ui.hits = vec![(PanelButton::Open(files[0].clone()), egui::Rect::NOTHING)];
    assert!(!wait_for_browser(&files, &ui, &mut input, &mut driver));
    ui.hits = vec![(PanelButton::Open(files[0].clone()), rect(200.0))];
    assert!(wait_for_browser(&files, &ui, &mut input, &mut driver));
    assert!(
        input.0.events.is_empty(),
        "真实布局就绪时不能点击文件或重复开关抽屉"
    );
    ui.scrolling = true;
    assert!(!wait_for_browser(&files, &ui, &mut input, &mut driver));
    assert!(input.0.events.is_empty());
}
