//! 画布连接线的会话级显示设置；不改变连接关系、XML 或编辑历史。
use crate::panels;

use bevy::gizmos::config::{GizmoConfig, GizmoConfigGroup, GizmoLineJoint, GizmoLineStyle};
use bevy::prelude::*;
use bevy_egui::{EguiContexts, egui};

#[derive(Default, Reflect, GizmoConfigGroup)]
pub struct LineGizmos;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Pattern {
    #[default]
    Solid,
    Dashed,
    Dotted,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Effect {
    #[default]
    Still,
    Pulse,
    Flow,
}

#[derive(Resource, Clone, Debug, PartialEq)]
pub struct Settings {
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
pub enum Control {
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
pub struct Controls(pub Vec<(Control, egui::Rect)>);

pub fn configure(settings: &Settings, config: &mut GizmoConfig) {
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

pub fn configure_system(
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
pub fn along(points: &[Vec2], fraction: f32) -> Option<(Vec2, Vec2)> {
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

pub fn draw_path(
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

/// 窗口外框除正文之外的高度：标题栏、内边距与描边。
const CHROME_HEIGHT: f32 = 64.0;
/// 宽屏下沿用的窗口宽度、左上角位置，以及贴边留白。
const WIDE_WIDTH: f32 = 340.0;
const WIDE_X: f32 = 800.0;
const TOP_OFFSET: f32 = 100.0;
const AREA_MARGIN: f32 = 12.0;
/// 正文至少保留的高度，短横屏下仍能滚动查看。
const MIN_BODY_HEIGHT: f32 = 60.0;
/// 紧凑布局下的触控目标边长；控件更高时靠正文滚动保证可达。
const TOUCH_TARGET: f32 = 44.0;

/// 设置窗口的位置、宽度与正文高度上限。宽屏保持 340 宽与 (800, 100) 的既有外观，
/// 窄窗与短横屏按可用区域收缩，正文滚动代替溢出屏幕。
fn window_layout(area: egui::Rect) -> (egui::Pos2, f32, f32) {
    let width = (area.width() - 2.0 * AREA_MARGIN).clamp(1.0, WIDE_WIDTH);
    let min_x = area.left() + AREA_MARGIN;
    let max_x = (area.right() - width - AREA_MARGIN).max(min_x);
    // 宽屏沿用原有的 (800, 100)，只有放不下时才收进可用区域。
    let x = (area.left() + WIDE_X).clamp(min_x, max_x);
    let min_y = area.top() + AREA_MARGIN;
    let lowest_y = area.bottom() - AREA_MARGIN - CHROME_HEIGHT - MIN_BODY_HEIGHT;
    let y = (area.top() + TOP_OFFSET).min(lowest_y.max(min_y));
    let body = (area.bottom() - AREA_MARGIN - y - CHROME_HEIGHT).max(MIN_BODY_HEIGHT);
    (egui::pos2(x, y), width, body)
}

/// 紧凑布局下把正文控件提升到触控目标；只作用于本窗口的正文，不改全局样式。
fn touch_spacing(ui: &mut egui::Ui) {
    let spacing = ui.spacing_mut();
    spacing.interact_size = spacing.interact_size.max(egui::Vec2::splat(TOUCH_TARGET));
    spacing.item_spacing.y = spacing.item_spacing.y.max(8.0);
}

fn record(controls: &mut Controls, control: Control, response: &egui::Response) {
    controls.0.push((control, response.rect));
}

fn show(
    ctx: &egui::Context,
    settings: &mut Settings,
    controls: &mut Controls,
) -> Option<(egui::Rect, egui::Rect)> {
    controls.0.clear();
    if !settings.open {
        return None;
    }
    let mut open = settings.open;
    let content = ctx.content_rect();
    let (pos, width, body) = window_layout(content);
    let compact = panels::compact(content.width());
    let mut window = egui::Window::new("画布连接线设置")
        .id(egui::Id::new("connection_line_settings"))
        .default_pos(pos)
        .default_width(width)
        .default_height(body + CHROME_HEIGHT)
        .max_height(body + CHROME_HEIGHT)
        .open(&mut open);
    // 只有可用宽度不足时才限制窗口宽度，宽屏保留原有的自由调整。
    if width < WIDE_WIDTH {
        window = window.max_width(width);
    }
    let shown = window.show(ctx, |ui| {
        egui::ScrollArea::vertical()
            .id_salt("connection_line_settings_body")
            .max_height(body)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                if compact {
                    touch_spacing(ui);
                }
                ui.small("仅改变显示；不修改船体、连接或撤销历史。");
                let response = ui.checkbox(&mut settings.enabled, "显示连接线");
                record(controls, Control::Visible, &response);
                ui.horizontal_wrapped(|ui| {
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
                ui.horizontal_wrapped(|ui| {
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
                ui.horizontal_wrapped(|ui| {
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
                ui.horizontal_wrapped(|ui| {
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
            })
    });
    settings.open = open;
    shown.and_then(|shown| {
        shown
            .inner
            .map(|scroll| (shown.response.rect, scroll.inner_rect))
    })
}

pub fn ui(
    mut contexts: EguiContexts,
    mut settings: ResMut<Settings>,
    mut controls: ResMut<Controls>,
) {
    if let Ok(ctx) = contexts.ctx_mut() {
        let _ = show(ctx, &mut settings, &mut controls);
    }
}

#[cfg(test)]
mod tests;
