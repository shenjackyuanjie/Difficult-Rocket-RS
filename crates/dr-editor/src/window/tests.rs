use crate::window::SmokeTest;
use crate::window::*;

#[test]
fn smoke_window_size_parses_only_explicit_smoke_dimensions() {
    assert_eq!(smoke_window_size(&[]).unwrap(), None);
    for size in [
        UVec2::new(320, 640),
        UVec2::new(390, 844),
        UVec2::new(640, 360),
    ] {
        let args = vec![
            "--smoke-test".into(),
            "--smoke-window-size".into(),
            format!("{}x{}", size.x, size.y),
        ];
        assert_eq!(smoke_window_size(&args).unwrap(), Some(size));
    }
}

#[test]
fn smoke_window_size_reports_invalid_or_conflicting_arguments() {
    let cases = [
        vec!["--smoke-window-size", "390x844"],
        vec!["--smoke-test", "--smoke-window-size"],
        vec!["--smoke-test", "--smoke-window-size", "390"],
        vec!["--smoke-test", "--smoke-window-size", "390x844x2"],
        vec!["--smoke-test", "--smoke-window-size", "0x844"],
        vec!["--smoke-test", "--smoke-window-size", "390x319"],
        vec!["--smoke-test", "--smoke-window-size", "390x-1"],
        vec!["--smoke-test", "--smoke-window-size", "4294967296x844"],
        vec![
            "--smoke-test",
            "--smoke-window-size",
            "390x844",
            "--demo-presentation",
        ],
        vec![
            "--smoke-test",
            "--smoke-window-size",
            "390x844",
            "--smoke-window-size",
            "320x640",
        ],
    ];
    for args in cases {
        let args: Vec<String> = args.into_iter().map(str::to_owned).collect();
        assert!(smoke_window_size(&args).is_err(), "参数应被拒绝：{args:?}");
    }
}

#[test]
fn smoke_window_resolution_keeps_normal_and_presentation_defaults() {
    let normal = initial_window_resolution(false, None);
    assert_eq!(normal.physical_width(), 1440);
    assert_eq!(normal.physical_height(), 900);
    assert_eq!(normal.scale_factor_override(), None);
    let presentation = initial_window_resolution(true, None);
    assert_eq!(presentation.physical_width(), 1920);
    assert_eq!(presentation.physical_height(), 1080);
    assert_eq!(presentation.scale_factor_override(), Some(1.0));
    for size in [
        UVec2::new(320, 640),
        UVec2::new(390, 844),
        UVec2::new(640, 360),
    ] {
        let resolution = initial_window_resolution(false, Some(size));
        assert_eq!(
            (resolution.physical_width(), resolution.physical_height()),
            (size.x, size.y)
        );
        assert_eq!(resolution.scale_factor_override(), Some(1.0));
    }
}

#[test]
fn smoke_window_capture_requires_matching_logical_physical_dimensions_and_dpi() {
    let requested = UVec2::new(390, 844);
    let mut window = Window {
        resolution: initial_window_resolution(false, Some(requested)),
        ..default()
    };
    assert!(smoke_window_matches(&window, requested));
    window.resolution.set_physical_resolution(1440, 900);
    assert!(!smoke_window_matches(&window, requested));
    window.resolution = WindowResolution::new(390, 844).with_scale_factor_override(2.0);
    assert!(!smoke_window_matches(&window, requested));
}

#[test]
fn combined_staging_smoke_does_not_request_startup_exit_or_timeout() {
    let mut mode = SmokeTest::from_args(
        &["--smoke-test".into(), "--staging-smoke-test".into()],
        None,
    );
    // 普通启动自测已超过超时；组合自测必须仍交由分级事务自身验收。
    mode.started = std::time::Instant::now() - std::time::Duration::from_secs(31);
    let mut app = App::new();
    app.insert_resource(mode)
        .init_resource::<ButtonInput<KeyCode>>()
        .add_systems(Update, capture_screenshot);
    // 不创建窗口，避免无头测试向仓库写窗口证据；真实尺寸观测由串行窗口专项核对。
    app.update();
    app.update();
    assert_eq!(
        app.world_mut()
            .query::<&bevy::render::view::screenshot::Screenshot>()
            .iter(app.world())
            .count(),
        0
    );
}

#[test]
fn automated_window_updates_continuously_without_changing_normal_editor_policy() {
    use bevy::winit::{UpdateMode, WinitSettings};
    let (automated, present_mode) = window_runtime_policy(true);
    assert_eq!(automated.focused_mode, UpdateMode::Continuous);
    assert_eq!(automated.unfocused_mode, UpdateMode::Continuous);
    assert_eq!(present_mode, PresentMode::AutoNoVsync);
    let (normal, present_mode) = window_runtime_policy(false);
    let defaults = WinitSettings::default();
    assert_eq!(normal.focused_mode, defaults.focused_mode);
    assert_eq!(normal.unfocused_mode, defaults.unfocused_mode);
    assert_eq!(present_mode, PresentMode::AutoVsync);
}
