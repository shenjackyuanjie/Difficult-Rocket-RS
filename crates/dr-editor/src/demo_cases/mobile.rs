//! 触摸抬指提交与鼠标按下提交：同一放置动作的两种真实提交时机。
//!
//! 触摸走 mobile_input 的单指会话（按下只预览、抬指提交），
//! 鼠标走画布的按下边沿；两者都必须只提交一次并留下一条历史。
use super::*;
use bevy::input::touch::TouchPhase;

const WAIT_STEPS: u8 = 60;
/// 画布上的空白落点（世界单位），远离夹具部件。
const PLACE_AT: (f64, f64) = (0., 5.);
const TOUCH_ID: u64 = 7;

#[derive(Default)]
pub(super) struct Probe {
    stage: u8,
    retries: u8,
    before: Option<Ship>,
    before_count: usize,
    connections: usize,
    point: Vec2,
}

impl Probe {
    fn retry(&mut self, what: &str) {
        self.retries += 1;
        assert!(
            self.retries <= WAIT_STEPS,
            "输入提交回放等待超时：{what}（阶段 {}）",
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
        Scenario::TouchPreviewCommit | Scenario::MousePressCommit
    ) {
        return;
    }
    state.mobile = Probe::default();
    ctx.document.snap_enabled = false;
    ctx.document.free_mode = true;
    let Some(index) = ctx
        .document
        .catalog
        .visible()
        .position(|kind| kind.id == "fuselage-1")
    else {
        panic!("触摸样例需要真实的机身部件");
    };
    ctx.cursor.catalog_index = index;
    ctx.cursor.placing = true;
    ctx.cursor.rotation = 0;
    let Ok(mut window) = ctx.windows.single_mut() else {
        panic!("输入提交样例需要主窗口");
    };
    let point = layout::screen_point(
        PLACE_AT,
        Vec2::ZERO,
        1.,
        Vec2::new(window.width(), window.height()),
    );
    // 画布输入读取真实窗口指针；测试驱动也要得到同一个落点。
    window.set_cursor_position(Some(point));
    state.mobile.before = Some(ctx.document.ship.clone());
    state.mobile.before_count = ctx.document.ship.all_parts().count();
    state.mobile.connections = ctx.document.ship.connections.len();
    state.mobile.point = point;
    state.pointer = Some(point);
}

fn write_touch(state: &Run, ctx: &mut Context, phase: TouchPhase) {
    let Ok(entity) = ctx.primary.single() else {
        return;
    };
    let Some(touch) = ctx.touch.as_deref_mut() else {
        return;
    };
    touch.write(bevy::input::touch::TouchInput {
        phase,
        position: state.mobile.point,
        window: entity,
        force: None,
        id: TOUCH_ID,
    });
}

fn unchanged_input(state: &Run, ctx: &Context) {
    assert_eq!(
        Some(&ctx.document.ship),
        state.mobile.before.as_ref(),
        "预览阶段不应改写文档"
    );
    assert_eq!(ctx.document.history.undo_len(), 0, "预览阶段不应产生历史");
    assert!(!ctx.document.dirty, "预览阶段不应改变脏标记");
}

fn after_commit(state: &Run, ctx: &Context) {
    assert_eq!(
        ctx.document.ship.all_parts().count(),
        state.mobile.before_count + 1,
        "提交次数不是恰好一次"
    );
    assert_eq!(ctx.document.history.undo_len(), 1, "提交应只产生一条历史");
    assert_eq!(
        ctx.document.ship.connections.len(),
        state.mobile.connections,
        "放置不应改写既有连接"
    );
    assert!(ctx.document.dirty);
}

/// 提交后静置若干帧仍不多提交：清空输入后位置与历史都不再变化。
fn settled(state: &mut Run, ctx: &Context) {
    if state.mobile.retries < 2 {
        state.mobile.retries += 1;
        after_commit(state, ctx);
        return;
    }
    state.mobile.retries = 0;
    state.mobile.next();
}

fn touch_case(state: &mut Run, ctx: &mut Context) -> bool {
    loop {
        match state.mobile.stage {
            0 => {
                write_touch(state, ctx, TouchPhase::Started);
                unchanged_input(state, ctx);
                state.mobile.next();
                // 触摸消息由本帧稍后的 mobile_input::input 消费，下一帧再看真实状态。
                return false;
            }
            1 => {
                let previewing = ctx
                    .gesture
                    .as_deref()
                    .is_some_and(|gesture| gesture.override_mouse);
                if !previewing || !ctx.cursor.valid {
                    state.mobile.retry("等待触摸按下建立真实预览");
                    return false;
                }
                unchanged_input(state, ctx);
                let Some((part, _, allowed)) = placement::preview(&ctx.document, &ctx.cursor)
                else {
                    state.mobile.retry("触摸预览没有生成候选部件");
                    return false;
                };
                assert!(allowed, "触摸预览候选应可放置");
                state.metrics["preview_part"] = part.part_type.clone().into();
                state.metrics["preview_committed"] = false.into();
                state.metrics["press_commits"] = false.into();
                write_touch(state, ctx, TouchPhase::Ended);
                state.mobile.next();
                return false;
            }
            2 => {
                if ctx.document.ship.all_parts().count() != state.mobile.before_count + 1 {
                    state.mobile.retry("等待触摸抬指提交");
                    return false;
                }
                after_commit(state, ctx);
                state.metrics["release_commits"] = 1.into();
                state.metrics["committed_on"] = "touch-release".into();
                state.mobile.next();
            }
            3 => settled(state, ctx),
            4 => {
                state.metrics["duplicate_commits"] = 0.into();
                ctx.key(KeyCode::KeyZ, true, false);
                state.mobile.next();
                // 快捷键由本帧稍后的画布键盘系统处理，下一帧再断言撤销结果。
                return false;
            }
            5 => {
                assert_eq!(
                    state.mobile.before.as_ref(),
                    Some(&ctx.document.ship),
                    "撤销没有恢复触摸提交前的文档"
                );
                ctx.key(KeyCode::KeyY, true, false);
                state.mobile.next();
                return false;
            }
            6 => {
                after_commit(state, ctx);
                state.metrics["undo_redo"] = true.into();
                state.metrics["history_entries"] = 1.into();
                state.mobile.next();
                return true;
            }
            _ => return true,
        }
    }
}

fn mouse_case(state: &mut Run, ctx: &mut Context) -> bool {
    loop {
        match state.mobile.stage {
            0 => {
                ctx.mouse.press(MouseButton::Left);
                state.mobile.next();
                return false;
            }
            1 => {
                if ctx.document.ship.all_parts().count() != state.mobile.before_count + 1 {
                    state.mobile.retry("等待鼠标按下提交");
                    return false;
                }
                after_commit(state, ctx);
                assert!(
                    ctx.mouse.pressed(MouseButton::Left),
                    "鼠标提交应发生在按下边沿，此时左键仍保持按下"
                );
                state.metrics["committed_on"] = "mouse-press".into();
                state.metrics["release_pending"] = true.into();
                state.mobile.next();
            }
            2 => {
                ctx.mouse.release(MouseButton::Left);
                state.mobile.next();
                return false;
            }
            3 => settled(state, ctx),
            4 => {
                state.metrics["duplicate_commits"] = 0.into();
                ctx.key(KeyCode::KeyZ, true, false);
                state.mobile.next();
                return false;
            }
            5 => {
                assert_eq!(
                    state.mobile.before.as_ref(),
                    Some(&ctx.document.ship),
                    "撤销没有恢复鼠标提交前的文档"
                );
                ctx.key(KeyCode::KeyY, true, false);
                state.mobile.next();
                return false;
            }
            6 => {
                after_commit(state, ctx);
                state.metrics["undo_redo"] = true.into();
                state.metrics["history_entries"] = 1.into();
                state.mobile.next();
                return true;
            }
            _ => return true,
        }
    }
}

pub(super) fn advance(state: &mut Run, ctx: &mut Context, scenario: Scenario) -> bool {
    match scenario {
        Scenario::TouchPreviewCommit => touch_case(state, ctx),
        Scenario::MousePressCommit => mouse_case(state, ctx),
        _ => unreachable!(),
    }
}
