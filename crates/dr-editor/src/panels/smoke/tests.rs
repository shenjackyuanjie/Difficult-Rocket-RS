use crate::panels::smoke::*;
use bevy_egui::egui;

#[test]
fn smoke_placement_keeps_world_offset_at_both_window_sizes() {
    for (width, height, expected) in [
        (1440, 900, Vec2::new(850., 600.)),
        (1920, 1080, Vec2::new(1090., 690.)),
    ] {
        let window = Window {
            resolution: bevy::window::WindowResolution::new(width, height)
                .with_scale_factor_override(1.),
            ..default()
        };
        assert_eq!(placement_point(&window), expected);
        assert!(crate::view::canvas(Vec2::new(window.width(), window.height())).contains(expected));
        assert_eq!(
            expected - Vec2::new(window.width(), window.height()) / 2.,
            Vec2::new(130., 150.)
        );
    }
}

#[test]
fn smoke_scroll_targets_actual_part_drawer_hits_instead_of_a_fixed_side() {
    for width in [320., 1440., 1920.] {
        for left in [0., width - 296.] {
            let mut ui = egui_panel::UiState::default();
            let area =
                egui::Rect::from_min_max(egui::pos2(left, 52.), egui::pos2(left + 296., 844.));
            ui.palette_area = Some(area);
            let unclipped =
                egui::Rect::from_min_max(egui::pos2(left + 8., 36.), egui::pos2(left + 280., 86.));
            let row = unclipped.intersect(area);
            ui.hits = vec![
                (PanelButton::ExpandCategories, area),
                (PanelButton::Part(0), egui::Rect::NOTHING),
                (
                    PanelButton::Part(1),
                    egui::Rect::from_min_size(egui::pos2(left + 304., 100.), egui::vec2(40., 40.)),
                ),
                (PanelButton::Part(2), row),
            ];
            assert_ne!(row, unclipped, "使用 ScrollArea 已裁剪的实际命中");
            assert_eq!(palette_scroll_point(&ui), Some(row.center()));
            assert!(area.contains(row.center()));
        }
    }
}

#[test]
fn smoke_palette_waits_for_an_actual_positive_part_hit() {
    let mut ui = egui_panel::UiState::default();
    ui.pixels_per_point = 1.;
    let window = Window::default();
    assert!(palette_scroll_point(&ui).is_none());
    assert!(palette_window_point(&ui, &window).is_none());
    let area = egui::Rect::from_min_size(egui::pos2(0., 52.), egui::vec2(296., 792.));
    ui.palette_area = Some(area);
    assert!(
        palette_window_point(&ui, &window).is_none(),
        "无命中时等待下一帧"
    );
    ui.hits = vec![
        (PanelButton::ExpandCategories, area),
        (PanelButton::Part(0), egui::Rect::NOTHING),
        (
            PanelButton::Part(1),
            egui::Rect::from_min_size(egui::pos2(8., 100.), egui::vec2(0., 40.)),
        ),
        (
            PanelButton::Part(2),
            egui::Rect::from_min_size(egui::pos2(304., 100.), egui::vec2(40., 40.)),
        ),
    ];
    assert!(palette_scroll_point(&ui).is_none());
    assert!(palette_window_point(&ui, &window).is_none());
}

#[test]
fn smoke_palette_window_point_converts_egui_and_window_dpi_once() {
    let mut ui = egui_panel::UiState::default();
    let area = egui::Rect::from_min_size(egui::pos2(0., 52.), egui::vec2(296., 792.));
    let row = egui::Rect::from_min_size(egui::pos2(8., 300.), egui::vec2(272., 52.));
    ui.palette_area = Some(area);
    ui.hits.push((PanelButton::Part(0), row));
    for window_scale in [1., 1.25, 2.] {
        let window = Window {
            resolution: bevy::window::WindowResolution::new(1440, 900)
                .with_scale_factor_override(window_scale),
            ..default()
        };
        for pixels_per_point in [1., 1.5, 2.] {
            ui.pixels_per_point = pixels_per_point;
            let (point, window_point) = palette_window_point(&ui, &window).unwrap();
            assert_eq!(point, row.center());
            let physical = Vec2::new(point.x, point.y) * pixels_per_point;
            assert!(window_point.abs_diff_eq(physical / window_scale, 0.001));
            assert!((window_point * window.scale_factor()).abs_diff_eq(physical, 0.001));
        }
    }
}

#[test]
fn smoke_palette_window_point_waits_for_valid_dpi() {
    let mut ui = egui_panel::UiState::default();
    let row = egui::Rect::from_min_size(egui::pos2(8., 300.), egui::vec2(272., 52.));
    ui.palette_area = Some(row);
    ui.hits.push((PanelButton::Part(0), row));
    for pixels_per_point in [0., -1., f32::NAN, f32::INFINITY] {
        ui.pixels_per_point = pixels_per_point;
        assert_eq!(palette_scroll_point(&ui), Some(row.center()));
        assert!(palette_window_point(&ui, &Window::default()).is_none());
    }
}
