//! 窄窗与单侧抽屉布局：运行中真实改窗、真实入口开抽屉并核对紧凑布局。
//!
//! 录制窗口必须保持 1920x1080（render_demo_video.py 持续核对客户区），
//! 因此 presentation 下跳过物理改窗并在证据里写明，普通 detailed 运行执行完整断言。
use super::*;
use bevy_egui::egui;

const WAIT_STEPS: u8 = 60;

#[derive(Default)]
pub(super) struct Probe {
    stage: u8,
    retries: u8,
}

impl Probe {
    /// 每个阶段只重试有界次数；超时即失败，不用空等掩盖布局未生效。
    fn retry(&mut self, what: &str) {
        self.retries += 1;
        assert!(
            self.retries <= WAIT_STEPS,
            "窄窗回放等待超时：{what}（阶段 {}）",
            self.stage
        );
    }

    fn next(&mut self) {
        self.stage += 1;
        self.retries = 0;
    }
}

pub(super) fn setup(state: &mut Run, ctx: &mut Context, scenario: Scenario) {
    if !matches!(
        scenario,
        Scenario::NarrowWindow { .. } | Scenario::NarrowRestore
    ) {
        return;
    }
    state.layout = Probe::default();
    // 每个窄窗样例都从关闭抽屉、桌面相机开始，章节内互不影响。
    if let Some(ui) = ctx.ui.as_deref_mut() {
        ui.close_drawer();
    }
    if let Ok((mut camera, mut projection)) = ctx.cameras.single_mut() {
        camera.translation.x = 0.;
        camera.translation.y = 0.;
        if let Projection::Orthographic(projection) = &mut *projection {
            projection.scale = 1.;
        }
    }
}

fn rect_json(rect: egui::Rect) -> serde_json::Value {
    serde_json::json!([rect.min.x, rect.min.y, rect.max.x, rect.max.y])
}

fn size_json(width: f32, height: f32) -> serde_json::Value {
    serde_json::json!([width, height])
}

/// 画布矩形从 egui 点换算为窗口逻辑坐标，换算方式与生产 view::controls 一致。
pub(super) fn canvas_rect(window: &Window, ui: &panels::egui_panel::UiState) -> Option<egui::Rect> {
    let rect = ui.canvas_area?;
    let factor = ui.pixels_per_point / window.scale_factor();
    let converted = egui::Rect::from_min_max(rect.min * factor, rect.max * factor);
    (converted.is_positive()
        && converted.min.x >= 0.
        && converted.min.y >= 0.
        && converted.max.x <= window.width() + 0.5
        && converted.max.y <= window.height() + 0.5)
        .then_some(converted)
}

/// 世界坐标到窗口逻辑像素：y 轴向下，与相机平移和正交缩放一致。
pub(super) fn screen_point(world: (f64, f64), translation: Vec2, scale: f32, window: Vec2) -> Vec2 {
    Vec2::new(
        window.x / 2. + (world.0 as f32 * 60. - translation.x) / scale,
        window.y / 2. - (world.1 as f32 * 60. - translation.y) / scale,
    )
}

/// 部件中心与部件数一一对应，且全部位于画布矩形内。
pub(super) fn centers_inside(
    document: &EditorDocument,
    centers: &[Vec2],
    canvas: egui::Rect,
) -> bool {
    !centers.is_empty()
        && centers.len() == document.ship.all_parts().count()
        && centers
            .iter()
            .all(|center| canvas.contains(egui::pos2(center.x, center.y)))
}

fn part_centers(document: &EditorDocument, current: &(Vec2, f32), window: Vec2) -> Vec<Vec2> {
    document
        .ship
        .keyed_parts()
        .map(|(_, part)| screen_point((part.x, part.y), current.0, current.1, window))
        .collect()
}

fn camera_state(ctx: &Context) -> (Vec2, f32) {
    let (camera, projection) = ctx.cameras.single().unwrap();
    let Projection::Orthographic(projection) = projection else {
        panic!("窄窗回放需要正交相机");
    };
    (camera.translation.truncate(), projection.scale)
}

fn click(state: &mut Run, ctx: &mut Context, rect: egui::Rect) -> bool {
    let Ok(mut input) = ctx.inputs.single_mut() else {
        return false;
    };
    state.input.click_rect(rect, &mut input);
    true
}

fn resize(ctx: &mut Context, width: f32, height: f32) {
    if let Ok(mut window) = ctx.windows.single_mut() {
        window.resolution.set(width, height);
    }
}

fn window_size(ctx: &Context) -> Option<(f32, f32)> {
    ctx.windows
        .single()
        .ok()
        .map(|window| (window.width(), window.height()))
}

fn settled(ctx: &Context, width: f32, height: f32) -> bool {
    window_size(ctx)
        .is_some_and(|size| (size.0 - width).abs() < 0.5 && (size.1 - height).abs() < 0.5)
}

fn drawer_window_rect(
    window: &Window,
    ui: &panels::egui_panel::UiState,
    rect: egui::Rect,
) -> egui::Rect {
    let factor = ui.pixels_per_point / window.scale_factor();
    egui::Rect::from_min_max(rect.min * factor, rect.max * factor)
}

fn touch_target(rect: egui::Rect) -> bool {
    rect.width() >= 44. - 0.01 && rect.height() >= 44. - 0.01
}

fn walk_narrow(state: &mut Run, ctx: &mut Context, width: f32, height: f32) -> bool {
    loop {
        match state.layout.stage {
            0 => {
                let before = window_size(ctx).unwrap_or((0., 0.));
                state.metrics = serde_json::json!({
                    "requested_size": size_json(width, height),
                    "window_before": size_json(before.0, before.1),
                    "presentation": state.presentation,
                });
                if !state.presentation {
                    resize(ctx, width, height);
                }
                state.layout.next();
            }
            1 => {
                if !state.presentation && !settled(ctx, width, height) {
                    state.layout.retry("等待窗口逻辑尺寸生效");
                    return false;
                }
                let (actual_w, actual_h) = window_size(ctx).unwrap();
                state.metrics["window_after"] = size_json(actual_w, actual_h);
                state.metrics["resized"] = (!state.presentation).into();
                state.layout.next();
            }
            2 => {
                let Some(ui) = ctx.ui.as_deref() else {
                    state.layout.retry("面板布局尚未生成");
                    return false;
                };
                assert!(!ui.drawer_open(), "初始化后抽屉应关闭");
                let Some(toggle) = ui.part_toggle else {
                    state.layout.retry("底栏尚未生成部件目录入口");
                    return false;
                };
                let touch = touch_target(toggle);
                state.metrics["toggle_44"] = serde_json::json!([toggle.width(), toggle.height()]);
                state.metrics["toggle_touch_target"] = touch.into();
                if !click(state, ctx, toggle) {
                    state.layout.retry("无法注入真实底栏点击");
                    return false;
                }
                state.layout.next();
            }
            3 => {
                let Some(ui) = ctx.ui.as_deref() else {
                    state.layout.retry("面板布局尚未生成");
                    return false;
                };
                if !ui.drawer_open() || ui.palette_area.is_none() {
                    state.layout.retry("部件抽屉尚未真实打开");
                    return false;
                }
                let Some(window) = ctx.windows.single().ok() else {
                    state.layout.retry("无法读取窗口");
                    return false;
                };
                let logical = window.width();
                let compact = panels::compact(logical);
                let Some(canvas) = canvas_rect(window, ui) else {
                    state.layout.retry("画布矩形尚未就绪");
                    return false;
                };
                let drawer = drawer_window_rect(window, ui, ui.palette_area.unwrap());
                assert_eq!(ui.areas.len(), 2, "紧凑布局只应存在底栏与一个抽屉");
                assert!(ui.browser_area.is_none() && ui.tools_area.is_none());
                assert!(
                    drawer.max.x >= window.width() - 1.0,
                    "部件抽屉应停靠在单侧（右侧）"
                );
                let overlap = canvas.intersect(drawer);
                // 共享同一次 egui 点坐标换算，允许亚像素取整误差但拒绝真实遮挡。
                state.metrics["panel_overlaps_canvas"] =
                    (overlap.width() >= 0.5 && overlap.height() >= 0.5).into();
                assert!(
                    state.metrics["panel_overlaps_canvas"] == false,
                    "抽屉与画布实际重叠：{overlap:?}"
                );
                assert!(
                    canvas.width() > window.width() * 0.2,
                    "画布被抽屉挤到不足窗口宽度的 20%"
                );
                state.metrics["compact"] = compact.into();
                state.metrics["drawer"] = "parts".into();
                state.metrics["drawer_rect"] = rect_json(drawer);
                state.metrics["canvas_rect"] = rect_json(canvas);
                state.metrics["canvas_positive"] = true.into();
                state.metrics["drawer_count"] = ui.areas.len().into();
                if compact {
                    assert!(
                        state.metrics["toggle_touch_target"] == true,
                        "紧凑布局的底栏入口应保持 44px 触控尺寸"
                    );
                }
                let Some(fit) = ui.fit_button else {
                    state.layout.retry("底栏尚未生成适配按钮");
                    return false;
                };
                if !click(state, ctx, fit) {
                    state.layout.retry("无法注入真实适配点击");
                    return false;
                }
                state.layout.next();
            }
            4 => {
                let Some(ui) = ctx.ui.as_deref() else {
                    state.layout.retry("面板布局尚未生成");
                    return false;
                };
                let Some(window) = ctx.windows.single().ok() else {
                    state.layout.retry("无法读取窗口");
                    return false;
                };
                let Some(canvas) = canvas_rect(window, ui) else {
                    state.layout.retry("画布矩形尚未就绪");
                    return false;
                };
                let current = camera_state(ctx);
                let centers = part_centers(
                    &ctx.document,
                    &current,
                    Vec2::new(window.width(), window.height()),
                );
                if !centers_inside(&ctx.document, &centers, canvas) {
                    // 适配由真实按钮触发，等它在本帧之后生效。
                    state.layout.retry("适配后船体尚未完全进入画布");
                    return false;
                }
                state.metrics["zoom"] = current.1.into();
                state.metrics["hull_parts"] = centers.len().into();
                state.metrics["hull_inside_canvas"] = true.into();
                state.metrics["fit_applied"] = true.into();
                state.metrics["single_side_drawer"] = true.into();
                state.metrics["restored"] = false.into();
                state.layout.next();
            }
            _ => return true,
        }
    }
}

fn walk_restore(state: &mut Run, ctx: &mut Context) -> bool {
    let Some((width, height)) = state.window else {
        panic!("窄窗章节缺少起始窗口尺寸");
    };
    loop {
        match state.layout.stage {
            0 => {
                let before = window_size(ctx).unwrap_or((0., 0.));
                state.metrics = serde_json::json!({
                    "window_before": size_json(before.0, before.1),
                    "chapter_size": size_json(width, height),
                    "presentation": state.presentation,
                });
                if !state.presentation && !settled(ctx, width, height) {
                    resize(ctx, width, height);
                }
                state.layout.next();
            }
            1 => {
                if !state.presentation && !settled(ctx, width, height) {
                    state.layout.retry("等待窗口恢复章节起始尺寸");
                    return false;
                }
                let (actual_w, actual_h) = window_size(ctx).unwrap();
                state.metrics["window_after"] = size_json(actual_w, actual_h);
                state.layout.next();
            }
            2 => {
                let Some(ui) = ctx.ui.as_deref() else {
                    state.layout.retry("面板布局尚未生成");
                    return false;
                };
                let Some(window) = ctx.windows.single().ok() else {
                    state.layout.retry("无法读取窗口");
                    return false;
                };
                let Some(canvas) = canvas_rect(window, ui) else {
                    state.layout.retry("全宽画布尚未就绪");
                    return false;
                };
                // 窗口尺寸恢复后布局还要重排一帧；先等真实布局收敛再断言章末状态。
                if ui.drawer_open()
                    || ui.palette_area.is_some()
                    || canvas.width() < window.width() * 0.6
                {
                    state.layout.retry("等待恢复后的全宽布局");
                    return false;
                }
                assert!(!ui.drawer_open(), "章末不应保留打开的单侧抽屉");
                assert!(ui.palette_area.is_none());
                assert!(
                    canvas.width() >= window.width() * 0.6,
                    "恢复后画布应回到接近全宽"
                );
                let current = camera_state(ctx);
                let centers = part_centers(
                    &ctx.document,
                    &current,
                    Vec2::new(window.width(), window.height()),
                );
                assert!(
                    centers_inside(&ctx.document, &centers, canvas),
                    "恢复桌面布局后船体应完整位于画布"
                );
                state.metrics["drawer_closed"] = true.into();
                state.metrics["canvas_rect"] = rect_json(canvas);
                state.metrics["canvas_full_width"] = true.into();
                state.metrics["hull_parts"] = centers.len().into();
                state.metrics["hull_inside_canvas"] = true.into();
                state.metrics["restored"] = true.into();
                state.metrics["single_side_drawer"] = false.into();
                state.layout.next();
            }
            _ => return true,
        }
    }
}

pub(super) fn advance(state: &mut Run, ctx: &mut Context, scenario: Scenario) -> bool {
    match scenario {
        Scenario::NarrowWindow { width, height } => {
            walk_narrow(state, ctx, f32::from(width), f32::from(height))
        }
        Scenario::NarrowRestore => walk_restore(state, ctx),
        _ => unreachable!(),
    }
}
