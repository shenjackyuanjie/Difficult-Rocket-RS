
use super::*;

#[test]
fn smoke_fit_reads_the_actual_canvas_and_converts_egui_dpi() {
    let mut ui = panels::egui_panel::UiState::default();
    ui.canvas_area = Some(bevy_egui::egui::Rect::from_min_max(
        bevy_egui::egui::pos2(100.0, 120.0),
        bevy_egui::egui::pos2(380.0, 800.0),
    ));
    for (egui_scale, window_scale) in [(1.0, 1.0), (2.0, 2.0)] {
        ui.pixels_per_point = egui_scale;
        let window = Window {
            resolution: bevy::window::WindowResolution::new(
                (390.0 * window_scale) as u32,
                (844.0 * window_scale) as u32,
            )
            .with_scale_factor_override(window_scale),
            ..default()
        };
        assert_eq!(
            actual_canvas(&ui, &window),
            Rect::from_corners(Vec2::new(116.0, 136.0), Vec2::new(364.0, 784.0))
        );
    }
    ui.pixels_per_point = 2.0;
    let window = Window {
        resolution: bevy::window::WindowResolution::new(780, 1688).with_scale_factor_override(1.0),
        ..default()
    };
    assert_eq!(
        actual_canvas(&ui, &window),
        Rect::from_corners(Vec2::new(216.0, 256.0), Vec2::new(744.0, 1584.0))
    );
}
