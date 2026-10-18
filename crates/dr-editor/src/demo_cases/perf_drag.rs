//! 1000 部件真实拖动：真实样本、真实画布鼠标、真实帧间隔统计。
//!
//! 帧间隔来自拖动步骤读取的 Time::delta_secs()，是被演示窗口真实渲染一帧的耗时；
//! 这里不测物理显示延迟，也不把演示节拍当成帧耗时。
use super::*;

const BULK: usize = 1000;
const STEPS: usize = 32;
const WAIT_STEPS: u8 = 60;
const TOLERANCE: f64 = 2e-3;
/// 自由模式连接点命中半径随缩放增大；取景不超过该缩放，避免按下被当成选点。
const MAX_PRESS_SCALE: f32 = 3.0;

/// 生产画布在自由模式下会把左键按下交给手动连接点选择，命中半径随缩放变化。
/// 在副本上用同一函数探测，确保性能样例的按下边沿真的开始拖动。
pub(super) fn press_is_plain_click(
    document: &EditorDocument,
    point: (f64, f64),
    scale: f32,
) -> bool {
    let mut probe = EditorDocument {
        free_mode: document.free_mode,
        snap_enabled: document.snap_enabled,
        revision: document.revision,
        ship: document.ship.clone(),
        catalog: document.catalog.clone(),
        selected: None,
        selection: BTreeSet::new(),
        dirty: false,
        history: EditorHistory::default(),
        saved_ship: document.ship.clone(),
        status: String::new(),
        sfx: Vec::new(),
    };
    let mut cursor = EditorCursor::default();
    !free_mode::click(
        &mut probe,
        &mut cursor,
        point,
        8.0 * f64::from(scale) / 60.0,
    )
}

#[derive(Default)]
pub(super) struct Probe {
    stage: u8,
    retries: u8,
    keys: Vec<PartKey>,
    anchor: Option<PartKey>,
    original: Option<Ship>,
    after: Option<Ship>,
    connections: usize,
    moved: usize,
    samples: Vec<f64>,
}

impl Probe {
    fn retry(&mut self, what: &str) {
        self.retries += 1;
        assert!(
            self.retries <= WAIT_STEPS,
            "1000 部件拖动回放等待超时：{what}（阶段 {}）",
            self.stage
        );
    }

    fn next(&mut self) {
        self.stage += 1;
        self.retries = 0;
    }

    /// 沿闭合轨迹移动；幅度跨过至少两个网格步长，量化后仍能产生真实位移。
    fn offset(index: usize) -> (f64, f64) {
        let t = index as f64;
        ((t * 0.35).sin() * 0.8, (t * 0.22).cos() * 0.5 - 0.5)
    }
}

/// 平均、中位、95 分位与最大帧耗时（毫秒）；样本按升序排序。
pub(super) fn frame_stats(samples: &mut [f64]) -> (f64, f64, f64, f64) {
    assert!(!samples.is_empty(), "帧耗时统计需要至少一个真实样本");
    samples.sort_by(f64::total_cmp);
    let count = samples.len();
    let average = samples.iter().sum::<f64>() / count as f64;
    let median = samples[count / 2];
    let p95 = samples[(count * 95 / 100).min(count - 1)];
    (average, median, p95, samples[count - 1])
}

fn set_pointer(state: &mut Run, ctx: &mut Context, world: (f64, f64)) {
    let Ok(mut window) = ctx.windows.single_mut() else {
        return;
    };
    let (camera, projection) = ctx.cameras.single().unwrap();
    let Projection::Orthographic(projection) = projection else {
        panic!("性能回放需要正交相机");
    };
    let point = layout::screen_point(
        world,
        camera.translation.truncate(),
        projection.scale,
        Vec2::new(window.width(), window.height()),
    );
    window.set_cursor_position(Some(point));
    state.pointer = Some(point);
}

/// 建立 1000 部件选区：真实样本、真实选中集合、真实相机取景。
pub(super) fn setup(state: &mut Run, ctx: &mut Context, scenario: Scenario) {
    if !matches!(scenario, Scenario::BulkDrag) {
        return;
    }
    state.perf = Probe::default();
    // 普通编辑模式：自由模式的端点点击会切换待连接状态，干扰整组拖动起点。
    ctx.document.free_mode = false;
    ctx.document.snap_enabled = false;
    ctx.options.follow_children = false;
    let total = ctx.document.ship.all_parts().count();
    assert!(
        total >= BULK,
        "性能样例需要至少 {BULK} 个部件，实际 {total}；请载入真实大样本"
    );
    let (low, high) = ctx.document.ship.all_parts().fold(
        (
            (f64::INFINITY, f64::INFINITY),
            (f64::NEG_INFINITY, f64::NEG_INFINITY),
        ),
        |(low, high), part| {
            (
                (low.0.min(part.x), low.1.min(part.y)),
                (high.0.max(part.x), high.1.max(part.y)),
            )
        },
    );
    let middle = ((low.0 + high.0) / 2., (low.1 + high.1) / 2.);
    let mut ordered: Vec<_> = ctx
        .document
        .ship
        .keyed_parts()
        .map(|(key, part)| ((part.x - middle.0).hypot(part.y - middle.1), key))
        .collect();
    ordered.sort_by(|a, b| a.0.total_cmp(&b.0));
    let keys: Vec<_> = ordered.into_iter().take(BULK).map(|(_, key)| key).collect();
    let anchor = *keys.first().expect("至少选中一个部件");
    ctx.document.selection = keys.iter().copied().collect();
    ctx.document.selected = Some(anchor);
    state.perf.keys = keys;
    state.perf.anchor = Some(anchor);
    state.perf.connections = ctx.document.ship.connections.len();
    state.perf.original = Some(ctx.document.ship.clone());
    state.metrics = serde_json::json!({
        "sample_parts": total,
        "selected_requested": BULK,
        "connections": ctx.document.ship.connections.len(),
    });
    // 取景只覆盖选区，截图能看清真实拖动的部件群。
    let size = ctx
        .windows
        .single()
        .map(|window| Vec2::new(window.width(), window.height()))
        .ok();
    if let Some(size) = size {
        let area = view::canvas(size);
        if let Some((center, fitted)) = view::fitted_in(&ctx.document, size, area, true, |_| None) {
            let press_point = state
                .perf
                .original
                .as_ref()
                .and_then(|ship| ship.part_at(anchor))
                .map(|part| (part.x, part.y))
                .expect("性能样例缺少锚点部件");
            let mut scale = fitted.min(MAX_PRESS_SCALE);
            // 连接点命中半径约为 8 * scale / 60；缩放过大时零件中心也会被当成连接点。
            while scale > 0.05 && !press_is_plain_click(&ctx.document, press_point, scale) {
                scale *= 0.7;
            }
            assert!(
                press_is_plain_click(&ctx.document, press_point, scale),
                "无法找到不会选中连接点的按下位置"
            );
            state.metrics["fit_scale"] = scale.into();
            state.metrics["free_mode_press_safe"] = true.into();
            if let Ok((mut camera, mut projection)) = ctx.cameras.single_mut()
                && let Projection::Orthographic(projection) = &mut *projection
            {
                camera.translation.x = center.x;
                camera.translation.y = center.y;
                projection.scale = scale;
            }
        }
    }
}

/// 窗口演示从仓库根运行，证据写到工作目录下的 target。
#[cfg(not(test))]
fn evidence_path(name: &str) -> std::path::PathBuf {
    std::path::Path::new("target").join(name)
}

/// 单元测试的工作目录是包目录；证据仍写到仓库根 target，与窗口演示共用同一位置，
/// 打包后的程序不受编译机路径影响。
#[cfg(test)]
fn evidence_path(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("target")
        .join(name)
}

fn expected(state: &Run, index: usize) -> (f64, f64) {
    let origin = state
        .perf
        .original
        .as_ref()
        .and_then(|ship| state.perf.anchor.and_then(|key| ship.part_at(key)))
        .map(|part| (part.x, part.y))
        .expect("性能样例缺少锚点部件");
    // 本章关闭吸附，拖动为连续位移（网格拖动由开启吸附的章节演示）。
    let offset = Probe::offset(index);
    (origin.0 + offset.0, origin.1 + offset.1)
}

fn near(a: (f64, f64), b: (f64, f64)) -> bool {
    (a.0 - b.0).hypot(a.1 - b.1) < TOLERANCE
}

fn sample(state: &mut Run, ctx: &Context) {
    if let Some(time) = ctx.time.as_deref() {
        state.perf.samples.push(time.delta_secs() as f64 * 1000.0);
    }
}

fn assert_dragging(state: &Run, ctx: &Context) {
    let anchor = state.perf.anchor.unwrap();
    assert_eq!(ctx.drag.id, Some(anchor), "拖动中锚点部件丢失");
    assert_eq!(
        ctx.drag.keys().len(),
        BULK,
        "拖动成员数应保持 1000 个选中部件"
    );
    unchanged(state, &ctx.document);
    assert_eq!(
        ctx.document.ship.connections.len(),
        state.perf.connections,
        "拖动过程中连接数发生变化"
    );
}

fn finish_metrics(state: &mut Run, ctx: &Context, delta: (f64, f64)) {
    let original = state.perf.original.as_ref().unwrap();
    let selected: BTreeSet<_> = state.perf.keys.iter().copied().collect();
    let mut moved = 0;
    for (key, before) in original.keyed_parts() {
        let now = ctx.document.ship.part_at(key).expect("提交后部件丢失");
        if selected.contains(&key) {
            moved += 1;
            assert!(
                near((now.x - before.x, now.y - before.y), delta),
                "选中部件没有按同一刚体位移移动：{key:?}"
            );
        } else {
            assert!(
                (now.x - before.x).abs() < 1e-9 && (now.y - before.y).abs() < 1e-9,
                "未选中的部件被拖动改写：{key:?}"
            );
        }
    }
    assert_eq!(moved, BULK);
    let (average, median, p95, max) = frame_stats(&mut state.perf.samples);
    state.metrics["selected"] = moved.into();
    state.metrics["part_count_after"] = ctx.document.ship.all_parts().count().into();
    let connections_after = ctx.document.ship.connections.len();
    state.metrics["connections_after"] = connections_after.into();
    state.metrics["connections_unchanged"] = (connections_after == state.perf.connections).into();
    state.metrics["external_connections_dropped"] = state
        .perf
        .connections
        .saturating_sub(connections_after)
        .into();
    state.metrics["drag_steps"] = STEPS.into();
    state.metrics["frame_samples"] = state.perf.samples.len().into();
    state.metrics["frame_average_ms"] = average.into();
    state.metrics["frame_median_ms"] = median.into();
    state.metrics["frame_p95_ms"] = p95.into();
    state.metrics["frame_max_ms"] = max.into();
    state.metrics["frame_time_source"] = "拖动步骤读取的真实相邻帧耗时 Time::delta_secs()".into();
    state.metrics["physical_display_latency_measured"] = false.into();
    state.metrics["grid_step"] = dr_core::geometry::EDITOR_GRID_STEP.into();
    state.metrics["rigid_delta"] = serde_json::json!([delta.0, delta.1]);
    state.metrics["all_selected_moved_together"] = true.into();
    state.metrics["unselected_unchanged"] = true.into();
    state.metrics["entity_count_unchanged"] = true.into();
    let path = evidence_path("performance-drag.xml");
    dr_core::save_ship(&path, &ctx.document.ship).expect("无法写性能样例 XML 证据");
    let reloaded = dr_core::load_ship(&path).expect("无法回读性能样例 XML 证据");
    assert_eq!(reloaded.all_parts().count(), original.all_parts().count());
    // 整组拖动会按既有语义断开外部连接；XML 往返核对的是提交后的真实连接。
    assert_eq!(reloaded.connections, ctx.document.ship.connections);
    state.metrics["xml_roundtrip"] = true.into();
}

pub(super) fn advance(state: &mut Run, ctx: &mut Context, scenario: Scenario) -> bool {
    if !matches!(scenario, Scenario::BulkDrag) {
        return false;
    }
    let anchor = state.perf.anchor.unwrap();
    loop {
        match state.perf.stage {
            0 => {
                let world = expected(state, 0);
                set_pointer(state, ctx, world);
                ctx.mouse.press(MouseButton::Left);
                state.perf.next();
                // 按下边沿由本帧稍后的画布输入处理，下一帧再核对拖动已建立。
                return false;
            }
            1 => {
                if ctx.drag.id != Some(anchor) {
                    state.perf.retry("等待真实按下建立 1000 部件拖动");
                    return false;
                }
                assert_dragging(state, ctx);
                state.perf.samples.clear();
                sample(state, ctx);
                state.perf.next();
            }
            2 => {
                if state.perf.moved > 0 {
                    let previous = state.perf.moved - 1;
                    assert_dragging(state, ctx);
                    // 拖动期间文档不变；跟随证据来自真实拖拽预览状态。
                    assert!(
                        near(ctx.drag.preview, expected(state, previous)),
                        "拖动预览没有跟随真实指针步骤 {previous}：{:?} != {:?}",
                        ctx.drag.preview,
                        expected(state, previous)
                    );
                }
                if state.perf.moved < STEPS {
                    set_pointer(state, ctx, expected(state, state.perf.moved));
                    state.perf.moved += 1;
                    sample(state, ctx);
                    return false;
                }
                ctx.mouse.release(MouseButton::Left);
                state.perf.next();
                return false;
            }
            3 => {
                if ctx.drag.id.is_some() {
                    state.perf.retry("等待松手提交 1000 部件拖动");
                    return false;
                }
                assert_eq!(
                    ctx.document.history.undo_len(),
                    1,
                    "一次拖动应只产生一条历史"
                );
                assert!(ctx.document.dirty);
                let original = state.perf.original.as_ref().unwrap();
                let before = original.part_at(anchor).unwrap();
                let now = ctx.document.ship.part_at(anchor).unwrap();
                let delta = (now.x - before.x, now.y - before.y);
                assert!(delta.0.hypot(delta.1) > 1e-6, "拖动没有产生实际位移");
                // 整组拖动按既有语义保留内部连接、断开与未选部件的外部连接；
                // 这里如实记录真实差值，撤销后再核对完整恢复。
                finish_metrics(state, ctx, delta);
                state.perf.after = Some(ctx.document.ship.clone());
                state.perf.next();
            }
            4 => {
                ctx.key(KeyCode::KeyZ, true, false);
                state.perf.next();
                return false;
            }
            5 => {
                assert_eq!(
                    state.perf.original.as_ref(),
                    Some(&ctx.document.ship),
                    "撤销没有恢复拖动前的完整文档"
                );
                state.metrics["undo_restored"] = true.into();
                ctx.key(KeyCode::KeyY, true, false);
                state.perf.next();
                // 重做也走真实快捷键，下一帧再断言结果。
                return false;
            }
            6 => {
                assert_eq!(
                    state.perf.after.as_ref(),
                    Some(&ctx.document.ship),
                    "重做没有恢复拖动结果"
                );
                state.metrics["redo_restored"] = true.into();
                state.perf.next();
                return true;
            }
            _ => return true,
        }
    }
}
