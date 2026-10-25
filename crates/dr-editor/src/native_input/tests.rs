
use super::*;

#[test]
fn palette_drag_uses_the_visible_part_widget_and_egui_scale() {
    let mut ui = panels::egui_panel::UiState::default();
    ui.pixels_per_point = 1.5;
    assert_eq!(palette_row(&ui), None);
    let rect = bevy_egui::egui::Rect::from_min_size(
        bevy_egui::egui::pos2(1060.0, 120.0),
        bevy_egui::egui::vec2(260.0, 64.0),
    );
    ui.hits.push((panels::PanelButton::Part(0), rect));
    assert_eq!(palette_row(&ui), Some(Vec2::new(1160.0, 152.0) * 1.5));
    assert!(!ui.drawer_open(), "坐标查询不能直接改动抽屉状态");
}
