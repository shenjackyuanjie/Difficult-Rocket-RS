//! 内部输入回放：当帧视觉跟随与同部件对连接唯一性，不测量物理显示延迟。
use super::*;

#[derive(Clone, Copy)]
enum Target {
    Part,
    Camera(MouseButton),
}

#[derive(Default)]
pub(super) struct Probe {
    target: Option<Target>,
    anchor: Vec2,
    scale: f32,
    checks: usize,
    release_checks: usize,
    max_error: f32,
    hinted_connection: Option<Connection>,
}

const GRAB: (f64, f64) = (0.16, -0.12);
const DRAG_PATH: &[(f64, f64)] = &[
    (0., 0.),
    (0.004, 0.006),
    (0.8, 0.4),
    (2., 1.),
    (0.5, -0.7),
    (-1.5, -1.),
    (1., -0.5),
    (0.75, 0.25),
    (0.751, 0.254),
    (1.35, 0.7),
];
const PAN_ANCHOR: Vec2 = Vec2::new(170., 120.);
const PAN_PATH: &[Vec2] = &[
    Vec2::ZERO,
    Vec2::new(0.25, 0.4),
    Vec2::new(80., 30.),
    Vec2::new(-45., -60.),
    Vec2::new(100., 60.),
    Vec2::new(120., 70.),
];

pub(super) fn setup(ctx: &mut Context, scenario: Scenario) {
    match scenario {
        Scenario::DragContinuous | Scenario::PanGesture => {
            ctx.document.ship = Ship {
                parts: vec![
                    ctx.document
                        .catalog
                        .get("fuselage-1")
                        .unwrap()
                        .instantiate(1, (0., 0.)),
                ],
                ..default()
            };
            ctx.document.free_mode = false;
        }
        Scenario::HintSinglePair => {
            // 显式多连接面夹具，仅追加目录，保留原版部件的安装点与外观资源。
            let mut catalog = dr_core::catalog_from_xml(
                r#"<PartTypes>
                <PartType id="demo-hint-source" type="strut" width="8" height="2"><AttachPoints>
                    <AttachPoint location="BottomCenter"/><AttachPoint location="BottomSide"/>
                </AttachPoints></PartType>
                <PartType id="demo-hint-target" type="strut" width="2" height="2"><AttachPoints>
                    <AttachPoint location="TopCenter"/><AttachPoint location="TopSide"/>
                </AttachPoints></PartType>
                </PartTypes>"#,
            )
            .unwrap();
            let sprite = ctx
                .document
                .catalog
                .get("fuselage-1")
                .unwrap()
                .sprite
                .clone();
            for kind in &mut catalog.types {
                kind.sprite = sprite.clone();
                kind.name = "多连接面（临时演示夹具）".into();
            }
            ctx.document.ship = Ship {
                parts: vec![
                    catalog
                        .get("demo-hint-target")
                        .unwrap()
                        .instantiate(1, (0., 0.)),
                ],
                ..default()
            };
            ctx.document.catalog.types.extend(catalog.types);
            ctx.document.free_mode = false;
            ctx.cursor.catalog_index = ctx
                .document
                .catalog
                .visible()
                .position(|kind| kind.id == "demo-hint-source")
                .unwrap();
            ctx.cursor.placing = true;
        }
        Scenario::ManualDuplicatePair => {
            let mut catalog = dr_core::catalog_from_xml(
                r#"<PartTypes><PartType id="demo-manual-pair" width="4" height="4"><AttachPoints>
                    <AttachPoint location="TopCenter"/><AttachPoint location="BottomCenter"/>
                    <AttachPoint location="LeftSide"/><AttachPoint location="RightSide"/>
                </AttachPoints></PartType></PartTypes>"#,
            )
            .unwrap();
            let kind = &mut catalog.types[0];
            kind.sprite = ctx
                .document
                .catalog
                .get("fuselage-1")
                .unwrap()
                .sprite
                .clone();
            kind.name = "四连接点（临时演示夹具）".into();
            ctx.document.ship = Ship {
                parts: vec![
                    kind.instantiate(1, (-2., 0.)),
                    kind.instantiate(2, (2., 0.)),
                ],
                ..default()
            };
            ctx.document.catalog.types.extend(catalog.types);
            ctx.document.free_mode = true;
        }
        _ => {}
    }
}

/// 本样例保持默认吸附，回放期望落点走与生产相同的网格量化。
fn path_point(point: (f64, f64)) -> (f64, f64) {
    dr_core::geometry::quantize_point(point)
}

fn near(actual: (f64, f64), expected: (f64, f64)) {
    assert!(
        (actual.0 - expected.0).hypot(actual.1 - expected.1) < 1e-5,
        "回放落点不一致：{actual:?} != {expected:?}"
    );
}

fn drag(state: &mut Run, ctx: &mut Context) -> bool {
    let count = DRAG_PATH.len() as u16;
    if state.step <= count {
        let index = usize::from(state.step - 1);
        if index > 0 {
            assert_eq!(ctx.drag.id, Some(key(1)));
            near(ctx.drag.preview, path_point(DRAG_PATH[index - 1]));
            unchanged(state, &ctx.document);
        }
        let delta = DRAG_PATH[index];
        ctx.point(state, (GRAB.0 + delta.0, GRAB.1 + delta.1));
        if index == 0 {
            state.gesture.target = Some(Target::Part);
            state.gesture.anchor = state.pointer.unwrap();
            state.gesture.scale = 1.;
            ctx.mouse.press(MouseButton::Left);
        } else if index + 1 == DRAG_PATH.len() {
            // 最后一段位移与释放在同一帧注入，不能丢掉松手末段。
            ctx.mouse.release(MouseButton::Left);
        }
    } else {
        match state.step - count {
            1 => {
                assert!(ctx.drag.id.is_none());
                let part = ctx.document.ship.part(1).unwrap();
                near((part.x, part.y), path_point(*DRAG_PATH.last().unwrap()));
                assert_eq!(ctx.document.history.undo_len(), 1);
                assert!(ctx.document.dirty);
                assert!(state.gesture.checks >= DRAG_PATH.len());
                assert_eq!(state.gesture.release_checks, 1);
                state.after = Some(ctx.document.ship.clone());
                state.gesture.target = None;
                ctx.key(KeyCode::KeyZ, true, false);
            }
            2 => {
                unchanged(state, &ctx.document);
                ctx.key(KeyCode::KeyY, true, false);
            }
            3 => {
                assert_eq!(state.after.as_ref(), Some(&ctx.document.ship));
                assert_eq!(ctx.document.history.undo_len(), 1);
                state.metrics = serde_json::json!({"path_points":DRAG_PATH.len(),
                    "grab_offset": [GRAB.0, GRAB.1], "grid_step": dr_core::geometry::EDITOR_GRID_STEP,
                    "subpixel_path":true, "reverse_motion":true,
                    "release_last_delta":true, "atomic_history":true, "undo_redo":true});
                return true;
            }
            _ => unreachable!(),
        }
    }
    state.step += 1;
    false
}

fn pan(state: &mut Run, ctx: &mut Context) -> bool {
    let count = PAN_PATH.len() as u16;
    if state.step <= count * 2 {
        let group = (state.step - 1) / count;
        let index = usize::from((state.step - 1) % count);
        let scale = if group == 0 { 1. } else { 2. };
        if index == 0 {
            if group != 0 {
                assert!(ctx.camera_drag.0.is_none());
                let (camera, _) = ctx.cameras.single().unwrap();
                let delta = *PAN_PATH.last().unwrap();
                assert!(
                    camera
                        .translation
                        .truncate()
                        .distance(Vec2::new(-delta.x, delta.y))
                        < 0.002
                );
            }
            *ctx.camera_drag = CameraDrag::default();
            ctx.mouse.reset_all();
            ctx.options.box_select_button = if group == 0 {
                view::BoxSelectButton::Middle
            } else {
                view::BoxSelectButton::Left
            };
            let (mut camera, mut projection) = ctx.cameras.single_mut().unwrap();
            camera.translation.x = 0.;
            camera.translation.y = 0.;
            let Projection::Orthographic(projection) = &mut *projection else {
                unreachable!()
            };
            projection.scale = scale;
            state.gesture.target = Some(Target::Camera(ctx.options.box_select_button.pan()));
            state.gesture.scale = scale;
        }
        let offset = PAN_ANCHOR + PAN_PATH[index];
        ctx.point(
            state,
            (f64::from(offset.x / 60.), f64::from(-offset.y / 60.)),
        );
        if index == 0 {
            state.gesture.anchor = state.pointer.unwrap();
            ctx.mouse.press(ctx.options.box_select_button.pan());
        } else if index + 1 == PAN_PATH.len() {
            ctx.mouse.release(ctx.options.box_select_button.pan());
        }
    } else {
        assert!(ctx.camera_drag.0.is_none());
        let (camera, _) = ctx.cameras.single().unwrap();
        let delta = *PAN_PATH.last().unwrap() * 2.;
        assert!(
            camera
                .translation
                .truncate()
                .distance(Vec2::new(-delta.x, delta.y))
                < 0.002
        );
        unchanged(state, &ctx.document);
        assert!(state.gesture.checks >= PAN_PATH.len() * 2);
        assert_eq!(state.gesture.release_checks, 2);
        state.gesture.target = None;
        state.metrics = serde_json::json!({"button_modes":["左键平移/中键框选", "中键平移/左键框选"],
            "scales":[1,2], "path_points_per_gesture":PAN_PATH.len(),
            "release_last_delta":true, "document_unchanged":true});
        return true;
    }
    state.step += 1;
    false
}

fn single_hint(state: &mut Run, ctx: &Context) -> usize {
    unchanged(state, &ctx.document);
    let (source, connection, allowed) = placement::preview(&ctx.document, &ctx.cursor).unwrap();
    assert!(allowed && connection.is_some());
    let target = ctx.document.ship.part(1).unwrap();
    let st = ctx.document.catalog.get(&source.part_type).unwrap();
    let tt = ctx.document.catalog.get(&target.part_type).unwrap();
    let candidates = dr_core::connections::candidates(&source, st, target, tt, 0.75);
    assert!(candidates.len() > 1, "夹具没有覆盖同一部件对的多连接面候选");
    let hints = attachment_hints::nearby(&ctx.document, &source, None);
    assert_eq!(hints.len(), 1, "同一对部件展示了重复连接提示");
    assert!(hints[0].snapped);
    assert_eq!(
        hints[0].source,
        dr_core::connections::segment(&source, st, &st.attach_points[candidates[0].source_index])
    );
    assert_eq!(
        hints[0].target,
        dr_core::connections::segment(target, tt, &tt.attach_points[candidates[0].target_index])
    );
    state.gesture.hinted_connection = Some(Connection::Normal {
        parent: target.id,
        child: source.id,
        parent_attach: candidates[0].target_index as i32 + 1,
        child_attach: candidates[0].source_index as i32 + 1,
    });
    candidates.len()
}

fn hint(state: &mut Run, ctx: &mut Context) -> bool {
    match state.step {
        1 => ctx.point(state, (0., 1.1)),
        2 => {
            single_hint(state, ctx);
            ctx.key(KeyCode::KeyP, false, false);
        }
        3 => {
            ctx.cursor.cancel_placement();
            assert_eq!(ctx.document.ship.parts.len(), 2);
            assert_eq!(ctx.document.ship.connections.len(), 1);
            assert_eq!(
                ctx.document.ship.connections.first(),
                state.gesture.hinted_connection.as_ref(),
                "实际提交的连接点与唯一提示不同"
            );
            assert_eq!(ctx.document.history.undo_len(), 1);
            state.after = Some(ctx.document.ship.clone());
            ctx.key(KeyCode::KeyZ, true, false);
        }
        4 => {
            unchanged(state, &ctx.document);
            ctx.key(KeyCode::KeyY, true, false);
        }
        5 => {
            assert_eq!(state.after.as_ref(), Some(&ctx.document.ship));
            assert_eq!(ctx.document.ship.connections.len(), 1);
            ctx.key(KeyCode::KeyZ, true, false);
        }
        6 => {
            unchanged(state, &ctx.document);
            ctx.cursor.placing = true;
            ctx.point(state, (0., 1.1));
        }
        7 => {
            let count = single_hint(state, ctx);
            state.metrics = serde_json::json!({"geometric_candidates":count, "displayed_pairs":1,
                "committed_connections":1, "hinted_connection_matches_commit":true,
                "undo_redo":true, "preview_unchanged":true});
            return true;
        }
        _ => unreachable!(),
    }
    state.step += 1;
    false
}

fn click_end(state: &mut Run, ctx: &mut Context, id: i64, attach: usize) {
    let point = ctx.endpoint(id, attach);
    ctx.click_point(state, point);
}

fn duplicate_rejected(state: &Run, ctx: &Context) {
    assert_eq!(state.after.as_ref(), Some(&ctx.document.ship));
    assert_eq!(ctx.document.ship.connections.len(), 1);
    assert_eq!(ctx.document.history.undo_len(), 1);
    assert_eq!(ctx.document.history.redo_len(), 0);
    assert!(ctx.document.dirty);
    assert!(
        ctx.document
            .status
            .contains("两个部件之间只能建立一对连接点连接"),
        "重复连接未走核心拒绝规则：{}",
        ctx.document.status
    );
}

fn manual(state: &mut Run, ctx: &mut Context) -> bool {
    match state.step {
        1 => click_end(state, ctx, 1, 0),
        2 | 4 | 6 | 8 | 11 | 13 => ctx.mouse.release(MouseButton::Left),
        3 => {
            assert!(ctx.cursor.manual_connection.is_some());
            click_end(state, ctx, 2, 1);
        }
        5 => {
            assert!(ctx.cursor.manual_connection.is_none());
            assert_eq!(ctx.document.ship.connections.len(), 1);
            assert_eq!(ctx.document.history.undo_len(), 1);
            state.after = Some(ctx.document.ship.clone());
            click_end(state, ctx, 1, 1);
        }
        7 => click_end(state, ctx, 2, 0),
        9 => {
            duplicate_rejected(state, ctx);
            state.metrics = serde_json::json!({"different_attach_rejected":true,
                "error":ctx.document.status, "history_unchanged_on_rejection":true});
            ctx.key(KeyCode::Escape, false, false);
        }
        10 => {
            assert!(ctx.cursor.manual_connection.is_none());
            // 反向换另一对连接点；原连接点的精确反向点击是主动断开，不是重复建边。
            click_end(state, ctx, 2, 2);
        }
        12 => click_end(state, ctx, 1, 3),
        14 => {
            duplicate_rejected(state, ctx);
            state.metrics["reverse_with_other_attaches_rejected"] = true.into();
            ctx.key(KeyCode::Escape, false, false);
        }
        15 => {
            assert!(ctx.cursor.manual_connection.is_none());
            ctx.key(KeyCode::KeyZ, true, false);
        }
        16 => {
            unchanged(state, &ctx.document);
            ctx.key(KeyCode::KeyY, true, false);
        }
        17 => {
            assert_eq!(state.after.as_ref(), Some(&ctx.document.ship));
            assert_eq!(ctx.document.ship.connections.len(), 1);
            assert_eq!(ctx.document.history.undo_len(), 1);
            assert!(ctx.document.dirty);
            state.metrics["committed_connections"] = 1.into();
            state.metrics["undo_redo"] = true.into();
            return true;
        }
        _ => unreachable!(),
    }
    state.step += 1;
    false
}

pub(super) fn advance(state: &mut Run, ctx: &mut Context, scenario: Scenario) -> bool {
    match scenario {
        Scenario::DragContinuous => drag(state, ctx),
        Scenario::PanGesture => pan(state, ctx),
        Scenario::HintSinglePair => hint(state, ctx),
        Scenario::ManualDuplicatePair => manual(state, ctx),
        _ => unreachable!(),
    }
}

pub(super) fn report(state: &mut Run) {
    if state.gesture.checks != 0 {
        state.metrics["same_frame_checks"] = state.gesture.checks.into();
        state.metrics["release_frame_checks"] = state.gesture.release_checks.into();
        state.metrics["max_world_pixel_error"] = state.gesture.max_error.into();
        state.metrics["physical_display_latency_measured"] = false.into();
    }
}

/// 在当前 Transform 传播之后、演示隔离光标之前核对真实视觉实体。
/// 期望值来自本帧实际光标并按生产网格量化，而非计划中的目标或 drag.preview；
/// 因此也覆盖慢速插值帧与网格台阶。
#[allow(clippy::too_many_arguments)]
pub(crate) fn verify(
    mut state: ResMut<Run>,
    document: Res<EditorDocument>,
    drag: Res<DragState>,
    camera_drag: Res<CameraDrag>,
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    visuals: Query<(&render::PartVisual, &GlobalTransform)>,
    cameras: Query<&GlobalTransform, With<Camera2d>>,
) {
    let Some(target) = state.gesture.target else {
        return;
    };
    let (button, active) = match target {
        Target::Part => (MouseButton::Left, drag.id == Some(key(1))),
        Target::Camera(button) => (button, camera_drag.0.is_some()),
    };
    let released = mouse.just_released(button);
    if !active && !released {
        return;
    }
    let cursor = windows
        .single()
        .unwrap()
        .cursor_position()
        .expect("回放帧没有实际光标");
    let delta = (cursor - state.gesture.anchor) * state.gesture.scale;
    let actual = match target {
        Target::Part => {
            let (_, transform) = visuals
                .iter()
                .find(|(visual, _)| visual.id == 1 && visual.group == 0 && visual.occurrence == 0)
                .unwrap();
            transform.translation().truncate()
        }
        Target::Camera(_) => cameras.single().unwrap().translation().truncate(),
    };
    let expected = match target {
        Target::Part => {
            // 部件位移按网格量化：屏幕像素先换算成逻辑格，再落回世界像素。
            let (x, y) = path_point((f64::from(delta.x) / 60.0, f64::from(-delta.y) / 60.0));
            Vec2::new((x * 60.0) as f32, (y * 60.0) as f32)
        }
        Target::Camera(_) => Vec2::new(-delta.x, delta.y),
    };
    let error = actual.distance(expected);
    assert!(
        error < 0.002,
        "本帧视觉未跟随输入：{actual:?} != {expected:?}，误差 {error}"
    );
    match target {
        Target::Part if released => {
            assert_eq!(document.history.undo_len(), 1);
            assert!(document.dirty);
        }
        _ => unchanged(&state, &document),
    }
    state.gesture.checks += 1;
    state.gesture.release_checks += usize::from(released);
    state.gesture.max_error = state.gesture.max_error.max(error);
}
