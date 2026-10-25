use crate::topology_ui::smoke::*;

#[test]
fn smoke_presentation_preserves_native_1080p_and_scale_factor() {
    let mut window = Window {
        resolution: bevy::window::WindowResolution::new(1920, 1080).with_scale_factor_override(1.),
        ..default()
    };
    resize_for_smoke(&mut window, true);
    assert_eq!(window.physical_size(), UVec2::new(1920, 1080));
    assert_eq!(window.scale_factor(), 1.);
    assert_eq!(
        Vec2::new(window.width(), window.height()),
        target_viewport(true)
    );
}

#[test]
fn smoke_ordinary_mode_still_resizes_to_minimum_window() {
    let mut window = Window {
        resolution: bevy::window::WindowResolution::new(1920, 1080).with_scale_factor_override(1.),
        ..default()
    };
    resize_for_smoke(&mut window, false);
    assert_eq!(window.physical_size(), UVec2::new(960, 640));
    assert_eq!(
        Vec2::new(window.width(), window.height()),
        target_viewport(false)
    );
}

#[test]
fn smoke_reports_only_claim_minimum_window_when_it_was_tested() {
    let report = smoke_report(true);
    assert_eq!(report["window"], "1920x1080");
    assert_eq!(report["minimum_window_tested"], false);
    assert!(report.get("minimum_window").is_none());
    let report = smoke_report(false);
    assert_eq!(report["window"], "960x640");
    assert_eq!(report["minimum_window"], "960x640");
    assert_eq!(report["minimum_window_tested"], true);
    for presentation in [true, false] {
        let report = smoke_report(presentation);
        assert_eq!(report["steps"], 27);
        assert_eq!(report["input"], "egui_injected");
        assert_eq!(report["ime_tested"], false);
        assert_eq!(report["atomic_undo"], true);
    }
}
