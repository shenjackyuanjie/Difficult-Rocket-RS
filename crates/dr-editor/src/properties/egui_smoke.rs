//! 通过真实 egui 输入事件验收 TextEdit、IME 和模态属性事务。
//! 不直接修改草稿字段或调用应用 Action；只有打开面板使用既有入口。

use super::*;
use bevy_egui::{EguiInput, PrimaryEguiContext, egui};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

const NAME: &str = "分级 egui 火箭 & <测试>";

#[derive(Default)]
pub(crate) struct State {
    phase: u8,
    before: Option<Ship>,
    after: Option<Ship>,
    release: Option<egui::Pos2>,
    capture: Option<Arc<AtomicBool>>,
    initial_debug: bool,
    initial_visible: bool,
}

fn key(input: &mut EguiInput, key: egui::Key, modifiers: egui::Modifiers) {
    for pressed in [true, false] {
        input.0.events.push(egui::Event::Key {
            key,
            physical_key: Some(key),
            pressed,
            repeat: false,
            modifiers,
        });
    }
}

fn select_all(input: &mut EguiInput) {
    key(
        input,
        egui::Key::A,
        egui::Modifiers {
            ctrl: true,
            command: true,
            ..Default::default()
        },
    );
}

/// 按下与释放隔一帧，保证测试经过 egui 的完整点击/聚焦流程。
/// 只接受实际绘制且被剪裁后仍可见的命中矩形。
fn click(
    action: Action,
    hits: &crate::egui_ui::UiHits,
    input: &mut EguiInput,
    state: &mut State,
) -> bool {
    if hits.1 {
        return false;
    }
    let Some((_, rect)) = hits.0.iter().find(|(candidate, _)| *candidate == action) else {
        if hits.0.is_empty() {
            // 等待 Modal 首次测量结束，不点击尚不可交互的布局 pass。
            return false;
        }
        if matches!(action, Action::Moved(_, _)) {
            // 较小窗口中分级动作可能在滚动区域下方，通过真实滚轮露出它。
            let (_, rect) = hits
                .0
                .iter()
                .find(|(candidate, _)| {
                    matches!(candidate, Action::Focus(Field::Target) | Action::AddStep)
                })
                .expect("egui 自测找不到可滚动的属性正文");
            input.0.events.extend([
                egui::Event::PointerMoved(rect.center()),
                egui::Event::MouseWheel {
                    phase: egui::TouchPhase::Move,
                    unit: egui::MouseWheelUnit::Point,
                    delta: egui::vec2(0.0, -180.0),
                    modifiers: egui::Modifiers::NONE,
                },
            ]);
            return false;
        }
        panic!(
            "egui 自测找不到实际控件 {action:?}，当前命中表：{:?}",
            hits.0
        );
    };
    assert!(rect.is_positive(), "控件命中矩形不可见：{action:?}");
    let pos = rect.center();
    input.0.events.extend([
        egui::Event::PointerMoved(pos),
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        },
    ]);
    state.release = Some(pos);
    true
}

fn protected_canvas_keys(keys: &mut ButtonInput<KeyCode>) {
    for code in [
        KeyCode::Delete,
        KeyCode::KeyR,
        KeyCode::KeyX,
        KeyCode::KeyY,
        KeyCode::F3,
        KeyCode::F4,
    ] {
        keys.press(code);
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn run(
    mode: Res<SmokeTest>,
    mut state: Local<State>,
    mut inspector: ResMut<Inspector>,
    mut document: ResMut<EditorDocument>,
    hits: Res<crate::egui_ui::UiHits>,
    mut inputs: Query<&mut EguiInput, With<PrimaryEguiContext>>,
    mut windows: Query<&mut Window, With<bevy::window::PrimaryWindow>>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    options: Res<crate::view::ViewOptions>,
    mut commands: Commands,
) {
    if (!mode.egui && !mode.properties) || mode.started.elapsed().as_secs() < 3 {
        return;
    }
    assert!(
        mode.started.elapsed().as_secs() < 90,
        "egui 属性交互自测超时，阶段 {}",
        state.phase
    );
    let (Ok(mut input), Ok(mut window)) = (inputs.single_mut(), windows.single_mut()) else {
        return;
    };
    window.focused = true;
    input.0.focused = true;
    if let Some(pos) = state.release.take() {
        input.0.events.extend([
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            },
        ]);
        return;
    }
    match state.phase {
        0 => {
            keys.reset_all();
            document.ship = crate::new_ship(&document.catalog);
            let detacher = document
                .catalog
                .get("detacher-1")
                .expect("egui 自测需要原版 detacher-1 部件")
                .instantiate(2, (3.0, 0.0));
            document.ship.parts.push(detacher);
            let pod = document.ship.parts[0]
                .pod
                .as_mut()
                .expect("egui 自测需要默认 Pod");
            pod.name = "原始 egui 名称".into();
            pod.staging = Some(StagingState {
                current_stage: 0,
                steps: vec![StageStep {
                    activations: vec![Activation {
                        id: 2,
                        moved: false,
                    }],
                }],
            });
            document.history = EditorHistory::default();
            document.saved_ship = document.ship.clone();
            document.select_only(Some(PartKey::new(0, 1, 0)));
            document.refresh();
            state.before = Some(document.ship.clone());
            state.initial_debug = options.debug;
            state.initial_visible = options.ship_visible;
            act(&Action::Open, &mut inspector, &mut document);
        }
        1 => {
            assert!(inspector.is_open());
            assert_eq!(state.before.as_ref(), Some(&document.ship));
            if !click(Action::Focus(Field::Name), &hits, &mut input, &mut state) {
                return;
            }
        }
        2 => {
            assert_eq!(
                inspector.focus,
                Some(Field::Name),
                "实际点击没有聚焦名称 TextEdit"
            );
            select_all(&mut input);
            input.0.events.push(egui::Event::Text("临时中文值".into()));
            protected_canvas_keys(&mut keys);
        }
        3 => {
            assert_eq!(
                inspector.draft.as_ref().unwrap().name,
                "临时中文值",
                "Ctrl+A/Text 没有替换完整的原名称"
            );
            assert_eq!(
                state.before.as_ref(),
                Some(&document.ship),
                "输入穿透到画布编辑命令"
            );
            assert_eq!(options.debug, state.initial_debug, "F3 穿透模态文本输入");
            assert_eq!(
                options.ship_visible, state.initial_visible,
                "F4 穿透模态文本输入"
            );
            assert!(!document.history.can_undo());
            keys.reset_all();
            select_all(&mut input);
            input
                .0
                .events
                .push(egui::Event::Ime(egui::ImeEvent::Preedit {
                    text: "fenji".into(),
                    active_range_chars: Some(0..5),
                }));
        }
        4 => {
            assert!(inspector.is_open(), "预编辑意外关闭属性模态");
            assert_eq!(inspector.focus, Some(Field::Name));
            assert_eq!(
                state.before.as_ref(),
                Some(&document.ship),
                "预编辑泄漏到文档"
            );
            input
                .0
                .events
                .push(egui::Event::Ime(egui::ImeEvent::Commit(NAME.into())));
            protected_canvas_keys(&mut keys);
        }
        5 => {
            assert_eq!(
                inspector.draft.as_ref().unwrap().name,
                NAME,
                "IME 提交丢字或重复插入"
            );
            assert_eq!(state.before.as_ref(), Some(&document.ship));
            assert!(!document.dirty);
            assert!(!document.history.can_undo());
            assert_eq!(options.debug, state.initial_debug);
            assert_eq!(options.ship_visible, state.initial_visible);
            keys.reset_all();
            if !click(Action::Moved(0, 0), &hits, &mut input, &mut state) {
                return;
            }
        }
        6 => {
            let draft = inspector.draft.as_ref().unwrap();
            assert_eq!(draft.name, NAME);
            assert!(
                draft.staging.as_ref().unwrap().steps[0].activations[0].moved,
                "真实移动标记复选框未改变草稿"
            );
            assert_eq!(state.before.as_ref(), Some(&document.ship));
            assert!(!document.dirty);
            assert!(!document.history.can_undo());
            use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk};
            let captured = Arc::new(AtomicBool::new(false));
            state.capture = Some(captured.clone());
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk("target/editor-egui-draft.png"))
                .observe(move |_: On<ScreenshotCaptured>| {
                    captured.store(true, Ordering::Release);
                });
        }
        7 => {
            // 必须等草稿实际渲染并捕获后才点击应用，防止截图抓到已关闭的模态。
            if !state.capture.as_ref().unwrap().load(Ordering::Acquire) {
                return;
            }
            if !click(Action::Apply, &hits, &mut input, &mut state) {
                return;
            }
        }
        8 => {
            assert!(!inspector.is_open(), "应用未关闭模态：{}", inspector.error);
            assert!(document.dirty);
            assert!(document.history.can_undo());
            let pod = document.ship.parts[0].pod.as_ref().unwrap();
            assert_eq!(pod.name, NAME);
            assert!(pod.staging.as_ref().unwrap().steps[0].activations[0].moved);
            state.after = Some(document.ship.clone());
            save_ship("target/egui-smoke.xml", &document.ship).unwrap();
            assert_eq!(
                load_ship("target/egui-smoke.xml").unwrap(),
                document.ship,
                "中文、XML 转义字符或分级 moved 在保存往返中发生变化"
            );
            // 留出一帧，让上一 pass 的 egui 键盘焦点状态更新后再发送画布快捷键。
            keys.reset_all();
        }
        9 => {
            keys.press(KeyCode::ControlLeft);
            keys.press(KeyCode::KeyZ);
        }
        10 => {
            assert_eq!(
                state.before.as_ref(),
                Some(&document.ship),
                "应用不是一次原子撤销"
            );
            assert!(!document.dirty);
            assert!(
                !document.history.can_undo(),
                "属性输入或分级草稿写入了额外历史"
            );
            assert!(document.history.can_redo());
            keys.reset_all();
            keys.press(KeyCode::ControlLeft);
            keys.press(KeyCode::KeyY);
        }
        11 => {
            keys.reset_all();
            assert_eq!(
                state.after.as_ref(),
                Some(&document.ship),
                "重做未恢复完整属性事务"
            );
            assert!(document.dirty);
            assert!(!document.history.can_redo());
            use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk};
            commands.spawn(Screenshot::primary_window())
                .observe(save_to_disk("target/editor-egui-smoke.png"))
                .observe(|_: On<ScreenshotCaptured>, mut exit: MessageWriter<AppExit>| {
                    info!("egui 属性交互自测通过：实际点击 TextEdit、Ctrl+A/Text 中文替换、IME 预编辑提交、模态快捷键隔离、分级 moved、草稿截图后应用、一次撤销重做及 XML 往返");
                    exit.write(AppExit::Success);
                });
        }
        _ => return,
    }
    state.phase += 1;
}
