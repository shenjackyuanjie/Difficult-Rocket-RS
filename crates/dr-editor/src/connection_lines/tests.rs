
use super::*;

#[test]
fn actual_configuration_system_only_updates_connection_group() {
    use bevy::gizmos::config::{DefaultGizmoConfigGroup, GizmoConfigStore};
    let mut store = GizmoConfigStore::default();
    let mut default_config = GizmoConfig::default();
    default_config.line.width = 9.0;
    store.insert(default_config, DefaultGizmoConfigGroup);
    store.insert(GizmoConfig::default(), LineGizmos);
    let mut app = App::new();
    app.insert_resource(store)
        .insert_resource(Settings {
            enabled: false,
            width: 6.0,
            pattern: Pattern::Dotted,
            ..default()
        })
        .add_systems(Update, configure_system);
    app.update();
    let store = app.world().resource::<GizmoConfigStore>();
    assert!(!store.config::<LineGizmos>().0.enabled);
    assert_eq!(store.config::<LineGizmos>().0.line.width, 6.0);
    assert!(store.config::<DefaultGizmoConfigGroup>().0.enabled);
    assert_eq!(store.config::<DefaultGizmoConfigGroup>().0.line.width, 9.0);
    assert_eq!(
        store.config::<DefaultGizmoConfigGroup>().0.line.style,
        GizmoLineStyle::Solid
    );
}

#[test]
fn line_group_settings_do_not_change_other_gizmo_groups() {
    let settings = Settings {
        enabled: false,
        width: 7.0,
        pattern: Pattern::Dashed,
        spacing: 5.0,
        ..default()
    };
    let mut config = GizmoConfig::default();
    configure(&settings, &mut config);
    assert!(!config.enabled);
    assert_eq!(config.line.width, 7.0);
    assert_eq!(
        config.line.style,
        GizmoLineStyle::Dashed {
            gap_scale: 5.0,
            line_scale: 7.5
        }
    );
    assert_eq!(GizmoConfig::default().line.width, 2.0);
    configure(
        &Settings {
            pattern: Pattern::Dotted,
            ..default()
        },
        &mut config,
    );
    assert_eq!(config.line.style, GizmoLineStyle::Dotted);
}

#[test]
fn animated_color_and_path_are_finite_continuous_and_respect_opacity() {
    let settings = Settings {
        rgba: [0.7, 0.2, 1.0, 0.5],
        effect: Effect::Pulse,
        ..default()
    };
    assert!((color(&settings, 0.25).alpha() - 0.5).abs() < 1e-6);
    assert!((color(&settings, 0.75).alpha() - 0.1).abs() < 1e-6);
    let points = [
        Vec2::ZERO,
        Vec2::ZERO,
        Vec2::new(10.0, 0.0),
        Vec2::new(10.0, 30.0),
    ];
    assert_eq!(along(&points, 0.5), Some((Vec2::new(10.0, 10.0), Vec2::Y)));
    assert_eq!(along(&points, 1.0), Some((Vec2::new(10.0, 30.0), Vec2::Y)));
    assert_eq!(along(&[Vec2::ZERO, Vec2::ZERO], 0.5), None);
}

#[test]
fn actual_controls_toggle_color_width_patterns_effects_and_reset() {
    let ctx = crate::egui_ui::test_context();
    let mut settings = Settings {
        open: true,
        ..default()
    };
    let mut controls = Controls::default();
    let mut time = 0.0;
    let mut frame = |events, settings: &mut Settings, controls: &mut Controls| {
        time += 0.02;
        ctx.begin_pass(egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1440.0, 900.0),
            )),
            focused: true,
            time: Some(time),
            events,
            ..default()
        });
        let _ = show(&ctx, settings, controls);
        ctx.end_pass().textures_delta.clear();
    };
    for _ in 0..3 {
        frame(vec![], &mut settings, &mut controls);
    }
    for control in [
        Control::Visible,
        Control::Purple,
        Control::Thicker,
        Control::Dashed,
        Control::Flow,
        Control::Arrows,
    ] {
        let point = controls
            .0
            .iter()
            .find(|(c, _)| *c == control)
            .unwrap()
            .1
            .center();
        for pressed in [true, false] {
            frame(
                vec![
                    egui::Event::PointerMoved(point),
                    egui::Event::PointerButton {
                        pos: point,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
                &mut settings,
                &mut controls,
            );
        }
        frame(vec![], &mut settings, &mut controls);
    }
    assert!(!settings.enabled && settings.arrows);
    for (actual, expected) in settings.rgba.into_iter().zip([0.75, 0.4, 1.0, 1.0]) {
        assert!((actual - expected).abs() < 1e-5);
    }
    assert_eq!(settings.width, 3.0);
    assert_eq!(settings.pattern, Pattern::Dashed);
    assert_eq!(settings.effect, Effect::Flow);
    let point = controls
        .0
        .iter()
        .find(|(c, _)| *c == Control::Reset)
        .unwrap()
        .1
        .center();
    for pressed in [true, false] {
        frame(
            vec![
                egui::Event::PointerMoved(point),
                egui::Event::PointerButton {
                    pos: point,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            &mut settings,
            &mut controls,
        );
    }
    assert_eq!(
        settings,
        Settings {
            open: true,
            ..default()
        }
    );
}

/// 固定尺寸的 egui 帧驱动：真实 frame 与真实命中矩形，不用尺寸公式代替交互。
struct Frames {
    ctx: egui::Context,
    area: egui::Rect,
    body: egui::Rect,
    seconds: f64,
}

impl Frames {
    fn new(width: f32, height: f32) -> Self {
        Self {
            ctx: crate::egui_ui::test_context(),
            area: egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height)),
            body: egui::Rect::NOTHING,
            seconds: 0.0,
        }
    }

    fn frame(
        &mut self,
        events: Vec<egui::Event>,
        settings: &mut Settings,
        controls: &mut Controls,
    ) -> egui::Rect {
        self.seconds += 0.02;
        self.ctx.begin_pass(egui::RawInput {
            screen_rect: Some(self.area),
            focused: true,
            time: Some(self.seconds),
            events,
            ..default()
        });
        let (window, body) = show(&self.ctx, settings, controls).expect("设置窗口应保持打开");
        self.body = body;
        self.ctx.end_pass().textures_delta.clear();
        window
    }

    /// 按下再松开真实控件中心，返回点击后的窗口矩形。
    fn click(
        &mut self,
        point: egui::Pos2,
        settings: &mut Settings,
        controls: &mut Controls,
    ) -> egui::Rect {
        let mut window = egui::Rect::NOTHING;
        for pressed in [true, false] {
            window = self.frame(
                vec![
                    egui::Event::PointerMoved(point),
                    egui::Event::PointerButton {
                        pos: point,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
                settings,
                controls,
            );
        }
        window
    }

    /// 指针停在指定位置向下滚轮，返回滚动后的窗口矩形。
    fn scroll_down(
        &mut self,
        at: egui::Pos2,
        settings: &mut Settings,
        controls: &mut Controls,
    ) -> egui::Rect {
        let mut window = egui::Rect::NOTHING;
        for _ in 0..24 {
            window = self.frame(
                vec![
                    egui::Event::PointerMoved(at),
                    egui::Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        delta: egui::vec2(0.0, -120.0),
                        modifiers: egui::Modifiers::NONE,
                        phase: egui::TouchPhase::Move,
                    },
                ],
                settings,
                controls,
            );
        }
        window
    }
}

fn control_rect(controls: &Controls, control: Control) -> egui::Rect {
    controls
        .0
        .iter()
        .find(|(candidate, _)| *candidate == control)
        .unwrap_or_else(|| panic!("设置窗口缺少控件 {control:?}"))
        .1
}

/// 紧凑布局（窄于 960 逻辑像素）下每个命中矩形都在窗口内且达到 44 像素触控目标。
fn assert_touch_targets(controls: &Controls, window: egui::Rect, label: &str) {
    for (control, rect) in &controls.0 {
        assert!(
            rect.min.x >= window.min.x - 0.5 && rect.max.x <= window.max.x + 0.5,
            "{label} 的 {control:?} 横向超出窗口：{rect:?} / {window:?}"
        );
        assert!(
            rect.height() >= 44.0 - 0.01,
            "{label} 的 {control:?} 高度不足触控目标：{rect:?}"
        );
    }
}

/// 真实的 egui 布局：运行三帧后返回窗口外框，供各尺寸断言使用。
fn open_window(width: f32, height: f32) -> (Frames, Settings, Controls, egui::Rect) {
    let mut frames = Frames::new(width, height);
    let mut settings = Settings {
        open: true,
        ..default()
    };
    let mut controls = Controls::default();
    let mut window = egui::Rect::NOTHING;
    for _ in 0..3 {
        window = frames.frame(vec![], &mut settings, &mut controls);
    }
    (frames, settings, controls, window)
}

/// 在窗口内真实点击“方向箭头”和“恢复默认”，验证它们滚动后仍然生效。
fn click_last_controls(
    frames: &mut Frames,
    settings: &mut Settings,
    controls: &mut Controls,
    label: &str,
) {
    let arrows = control_rect(controls, Control::Arrows);
    assert!(
        frames.body.contains_rect(arrows),
        "{label} 方向箭头不在滚动视口内：{arrows:?} / {:?}",
        frames.body
    );
    frames.click(arrows.center(), settings, controls);
    assert!(settings.arrows, "{label} 方向箭头点击未生效");
    let reset = control_rect(controls, Control::Reset);
    assert!(
        frames.body.contains_rect(reset),
        "{label} “恢复默认”不在滚动视口内：{reset:?} / {:?}",
        frames.body
    );
    frames.click(reset.center(), settings, controls);
    assert_eq!(
        *settings,
        Settings {
            open: true,
            ..default()
        },
        "{label} “恢复默认”点击未生效"
    );
}

/// 390/1440 逻辑宽下正文放得下：窗口不超出可用区域、命中矩形不横向溢出，
/// 末尾控件不用滚动即可直接点；紧凑尺寸的控件同时满足 44 像素触控目标。
#[test]
fn settings_window_keeps_every_control_visible_where_content_fits() {
    for (width, height) in [(390.0, 844.0), (1440.0, 900.0)] {
        let label = format!("{width}x{height}");
        let (mut frames, mut settings, mut controls, window) = open_window(width, height);
        assert!(
            frames.area.contains_rect(window),
            "{label} 设置窗口超出可用区域：{window:?}"
        );
        if panels::compact(width) {
            assert_touch_targets(&controls, window, &label);
        } else {
            for (control, rect) in &controls.0 {
                assert!(
                    rect.min.x >= window.min.x - 0.5 && rect.max.x <= window.max.x + 0.5,
                    "{label} 的 {control:?} 横向超出窗口：{rect:?} / {window:?}"
                );
            }
        }
        let reset = control_rect(&controls, Control::Reset);
        assert!(
            frames.body.contains_rect(reset),
            "{label} 正文应当放得下：{reset:?} / {:?}",
            frames.body
        );
        click_last_controls(&mut frames, &mut settings, &mut controls, &label);
    }
}

/// 短横屏（640x360）加触控尺寸下正文必定超出窗口：滚动前末尾控件在窗口之外，
/// 滚动到底后“方向箭头”和“恢复默认”都进入窗口并被真实点击生效。
#[test]
fn settings_window_scrolls_to_the_last_controls_in_short_landscape() {
    let (width, height) = (640.0, 360.0);
    let label = format!("{width}x{height}");
    assert!(panels::compact(width), "短横屏应使用紧凑触控布局");
    let (mut frames, mut settings, mut controls, mut window) = open_window(width, height);
    assert!(
        frames.area.contains_rect(window),
        "{label} 设置窗口超出可用区域：{window:?}"
    );
    assert_touch_targets(&controls, window, &label);
    let reset = control_rect(&controls, Control::Reset);
    assert!(
        !frames.body.contains_rect(reset),
        "{label} 触控尺寸下正文应超出窗口：{reset:?} / {window:?}"
    );
    frames.scroll_down(window.center(), &mut settings, &mut controls);
    window = frames.frame(vec![], &mut settings, &mut controls);
    let arrows = control_rect(&controls, Control::Arrows);
    let reset = control_rect(&controls, Control::Reset);
    assert!(
        frames.body.contains_rect(arrows) && frames.body.contains_rect(reset),
        "{label} 滚动后仍无法触达尾部控件：{arrows:?} {reset:?} / {window:?}"
    );
    frames.click(arrows.center(), &mut settings, &mut controls);
    assert!(settings.arrows, "{label} 滚动后的方向箭头点击未生效");
    let reset = control_rect(&controls, Control::Reset);
    assert!(
        frames.body.contains_rect(reset),
        "{label} 滚动后的“恢复默认”被移出窗口：{reset:?}"
    );
    frames.click(reset.center(), &mut settings, &mut controls);
    assert_eq!(
        settings,
        Settings {
            open: true,
            ..default()
        },
        "{label} 滚动后的“恢复默认”点击未生效"
    );
}

/// 320x640 窄竖屏的正文高度接近上限，按真实布局走两条路径：放得下直接点，
/// 放不下先真实滚动；两条路径都必须让末尾控件可点且“恢复默认”真实生效。
#[test]
fn settings_window_reaches_the_last_control_on_a_narrow_portrait_screen() {
    let (width, height) = (320.0, 640.0);
    let label = format!("{width}x{height}");
    assert!(panels::compact(width), "窄竖屏应使用紧凑触控布局");
    let (mut frames, mut settings, mut controls, window) = open_window(width, height);
    assert!(
        frames.area.contains_rect(window),
        "{label} 设置窗口超出可用区域：{window:?}"
    );
    assert_touch_targets(&controls, window, &label);
    if !frames
        .body
        .contains_rect(control_rect(&controls, Control::Reset))
    {
        frames.scroll_down(window.center(), &mut settings, &mut controls);
        frames.frame(vec![], &mut settings, &mut controls);
    }
    click_last_controls(&mut frames, &mut settings, &mut controls, &label);
}
