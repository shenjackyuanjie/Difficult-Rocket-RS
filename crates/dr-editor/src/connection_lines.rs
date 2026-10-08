//! 画布连接线的会话级显示设置；不改变连接关系、XML 或编辑历史。
use super::*;
use bevy::gizmos::config::{GizmoConfig, GizmoConfigGroup, GizmoLineJoint, GizmoLineStyle};
use bevy_egui::{EguiContexts, egui};

#[derive(Default, Reflect, GizmoConfigGroup)]
pub(crate) struct LineGizmos;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Pattern {
    #[default]
    Solid,
    Dashed,
    Dotted,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Effect {
    #[default]
    Still,
    Pulse,
    Flow,
}

#[derive(Resource, Clone, Debug, PartialEq)]
pub(crate) struct Settings {
    pub open: bool,
    pub enabled: bool,
    pub rgba: [f32; 4],
    pub width: f32,
    pub pattern: Pattern,
    pub effect: Effect,
    pub arrows: bool,
    pub speed: f32,
    pub spacing: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            open: false,
            enabled: true,
            rgba: [0.25, 0.9, 0.55, 1.0],
            width: 2.0,
            pattern: Pattern::Solid,
            effect: Effect::Still,
            arrows: false,
            speed: 1.0,
            spacing: 3.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Control {
    Visible,
    Purple,
    Orange,
    Green,
    Thicker,
    Thinner,
    Solid,
    Dashed,
    Dotted,
    Still,
    Pulse,
    Flow,
    Arrows,
    Reset,
}

#[derive(Resource, Default)]
pub(crate) struct Controls(pub Vec<(Control, egui::Rect)>);

pub(crate) fn configure(settings: &Settings, config: &mut GizmoConfig) {
    config.enabled = settings.enabled;
    config.line.width = settings.width.clamp(1.0, 12.0);
    config.line.joints = GizmoLineJoint::Round(4);
    config.line.style = match settings.pattern {
        Pattern::Solid => GizmoLineStyle::Solid,
        Pattern::Dotted => GizmoLineStyle::Dotted,
        Pattern::Dashed => GizmoLineStyle::Dashed {
            gap_scale: settings.spacing.clamp(1.0, 12.0),
            line_scale: settings.spacing.clamp(1.0, 12.0) * 1.5,
        },
    };
}

pub(crate) fn configure_system(
    settings: Res<Settings>,
    mut store: ResMut<bevy::gizmos::config::GizmoConfigStore>,
) {
    configure(&settings, store.config_mut::<LineGizmos>().0);
}

fn color(settings: &Settings, seconds: f32) -> Color {
    let [r, g, b, a] = settings.rgba;
    let pulse = if settings.effect == Effect::Pulse {
        0.6 + 0.4 * (seconds * settings.speed * std::f32::consts::TAU).sin()
    } else {
        1.0
    };
    Color::srgba(r, g, b, a * pulse)
}

/// 按总弧长移动，而不是在不同长度的线段上突然跳速。
fn along(points: &[Vec2], fraction: f32) -> Option<(Vec2, Vec2)> {
    let length: f32 = points.windows(2).map(|p| p[0].distance(p[1])).sum();
    if length < 1e-4 {
        return None;
    }
    let mut distance = fraction.clamp(0.0, 1.0) * length;
    for pair in points.windows(2) {
        let edge = pair[1] - pair[0];
        let size = edge.length();
        if size < 1e-4 {
            continue;
        }
        if distance <= size {
            return Some((pair[0] + edge * (distance / size), edge / size));
        }
        distance -= size;
    }
    None
}

pub(crate) fn draw_path(
    gizmos: &mut Gizmos<LineGizmos>,
    settings: &Settings,
    points: &[Vec2],
    seconds: f32,
    scale: f32,
) {
    if !settings.enabled {
        return;
    }
    let color = color(settings, seconds);
    let points: Vec<_> = points
        .iter()
        .copied()
        .enumerate()
        .filter(|(i, p)| *i == 0 || p.distance_squared(points[*i - 1]) > 1e-6)
        .map(|(_, p)| p)
        .collect();
    if points.len() < 2 {
        return;
    }
    gizmos.linestrip_2d(points.iter().copied(), color);
    let arrow = |gizmos: &mut Gizmos<LineGizmos>, fraction: f32| {
        if let Some((tip, direction)) = along(&points, fraction) {
            let length = (6.0 + settings.width) * scale;
            let side = Vec2::new(-direction.y, direction.x) * length * 0.45;
            let base = tip - direction * length;
            gizmos.line_2d(base + side, tip, color);
            gizmos.line_2d(base - side, tip, color);
        }
    };
    if settings.arrows {
        arrow(gizmos, 0.8);
    }
    if settings.effect == Effect::Flow {
        let fraction = (seconds * settings.speed * 0.25).rem_euclid(1.0);
        arrow(gizmos, fraction);
    }
}

fn record(controls: &mut Controls, control: Control, response: &egui::Response) {
    controls.0.push((control, response.rect));
}

fn show(ctx: &egui::Context, settings: &mut Settings, controls: &mut Controls) {
    controls.0.clear();
    if !settings.open {
        return;
    }
    let mut open = settings.open;
    egui::Window::new("画布连接线设置")
        .id(egui::Id::new("connection_line_settings"))
        .default_pos(egui::pos2(800.0, 100.0))
        .default_width(340.0)
        .open(&mut open)
        .show(ctx, |ui| {
            ui.small("仅改变显示；不修改船体、连接或撤销历史。");
            let response = ui.checkbox(&mut settings.enabled, "显示连接线");
            record(controls, Control::Visible, &response);
            ui.horizontal(|ui| {
                ui.label("颜色 / 透明度");
                ui.color_edit_button_rgba_unmultiplied(&mut settings.rgba);
                for (control, name, rgba) in [
                    (Control::Green, "翡翠", [0.25, 0.9, 0.55, 1.0]),
                    (Control::Purple, "紫色", [0.75, 0.4, 1.0, 1.0]),
                    (Control::Orange, "橙色", [1.0, 0.65, 0.2, 1.0]),
                ] {
                    let response = ui.button(name);
                    record(controls, control, &response);
                    if response.clicked() {
                        settings.rgba = rgba;
                    }
                }
            });
            ui.horizontal(|ui| {
                ui.add(egui::Slider::new(&mut settings.width, 1.0..=12.0).text("粗细 px"));
                for (control, label, delta) in
                    [(Control::Thinner, "−", -1.0), (Control::Thicker, "+", 1.0)]
                {
                    let response = ui.button(label);
                    record(controls, control, &response);
                    if response.clicked() {
                        settings.width = (settings.width + delta).clamp(1.0, 12.0);
                    }
                }
            });
            ui.horizontal(|ui| {
                ui.label("线型");
                for (control, name, pattern) in [
                    (Control::Solid, "实线", Pattern::Solid),
                    (Control::Dashed, "虚线", Pattern::Dashed),
                    (Control::Dotted, "点线", Pattern::Dotted),
                ] {
                    let response = ui.selectable_value(&mut settings.pattern, pattern, name);
                    record(controls, control, &response);
                }
            });
            if settings.pattern == Pattern::Dashed {
                ui.add(egui::Slider::new(&mut settings.spacing, 1.0..=12.0).text("虚线间距"));
            }
            ui.horizontal(|ui| {
                ui.label("效果");
                for (control, name, effect) in [
                    (Control::Still, "静态", Effect::Still),
                    (Control::Pulse, "呼吸", Effect::Pulse),
                    (Control::Flow, "流动", Effect::Flow),
                ] {
                    let response = ui.selectable_value(&mut settings.effect, effect, name);
                    record(controls, control, &response);
                }
            });
            if settings.effect != Effect::Still {
                ui.add(egui::Slider::new(&mut settings.speed, 0.2..=4.0).text("效果速度"));
            }
            let response = ui.checkbox(&mut settings.arrows, "父 → 子方向箭头");
            record(controls, Control::Arrows, &response);
            let response = ui.button("恢复默认");
            record(controls, Control::Reset, &response);
            if response.clicked() {
                *settings = Settings {
                    open: true,
                    ..default()
                };
            }
            ui.small("Esc 关闭；隐藏连接线不会隐藏自由模式的端点提示。");
        });
    settings.open = open;
}

pub(crate) fn ui(
    mut contexts: EguiContexts,
    mut settings: ResMut<Settings>,
    mut controls: ResMut<Controls>,
) {
    if let Ok(ctx) = contexts.ctx_mut() {
        show(ctx, &mut settings, &mut controls);
    }
}

#[cfg(test)]
mod tests {
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
        let ctx = egui::Context::default();
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
            show(&ctx, settings, controls);
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
}
