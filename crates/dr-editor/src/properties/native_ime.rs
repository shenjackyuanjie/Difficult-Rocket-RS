use super::*;

#[derive(Default)]
pub(crate) struct State {
    phase: u8,
    before: Option<Ship>,
    status: String,
    native_phase: u8,
    native_preedit: String,
    input: crate::egui_ui::UiTestInput,
}

/// Windows 主线程上的 IMM 负责预编辑与提交；此处不注入 Bevy IME 消息。
#[allow(clippy::too_many_arguments)]
pub(crate) fn run(
    mode: Res<SmokeTest>,
    mut state: Local<State>,
    mut document: ResMut<EditorDocument>,
    mut inspector: ResMut<Inspector>,
    hits: Res<crate::egui_ui::UiHits>,
    mut inputs: Query<
        (
            &mut bevy_egui::EguiInput,
            &bevy_egui::input::EguiContextImeState,
        ),
        With<bevy_egui::PrimaryEguiContext>,
    >,
    mut ime: MessageReader<bevy::window::Ime>,
    mut windows: Query<
        (&mut Window, &bevy::window::RawHandleWrapper),
        With<bevy::window::PrimaryWindow>,
    >,
    _main_thread: bevy::ecs::system::NonSendMarker,
    mut commands: Commands,
) {
    if !mode.native_ime || mode.started.elapsed().as_secs() < 3 {
        return;
    }
    assert!(
        mode.started.elapsed().as_secs() < 90,
        "Windows 原生 IME 自测超时"
    );
    for event in ime.read() {
        match event {
            bevy::window::Ime::Preedit { value, .. } => state.native_preedit = value.clone(),
            bevy::window::Ime::Commit { .. } | bevy::window::Ime::Disabled { .. } => {
                state.native_preedit.clear()
            }
            _ => {}
        }
    }
    let Ok((mut input, ime_state)) = inputs.single_mut() else {
        return;
    };
    let Ok((mut window, raw)) = windows.single_mut() else {
        return;
    };
    window.focused = true;
    if state.input.tick(&mut input) {
        return;
    }
    match state.phase {
        0 => {
            state.before = Some(document.ship.clone());
            act(&Action::Open, &mut inspector, &mut document);
        }
        1 => {
            if !state
                .input
                .click(Action::Focus(Field::Name), &hits, &mut input)
            {
                return;
            }
        }
        2 => {
            // bevy_egui 直接更新 Winit，不回写 Bevy Window 的旧 IME 字段。
            if !ime_state.is_ime_allowed {
                return;
            }
            let anchor = ime_state.ime_rect.expect("egui 未定位 TextEdit").min;
            let draft = inspector.draft.as_ref().expect("输入法取消意外关闭了草稿");
            assert_eq!(state.before.as_ref(), Some(&document.ship));
            let status = format!(
                "ready\nname={}\npreedit={}\nanchor={},{}\n",
                draft.name, state.native_preedit, anchor.x, anchor.y
            );
            if status != state.status {
                std::fs::write("target/native-ime-state.txt", &status).unwrap();
                state.status = status;
            }
            #[cfg(windows)]
            {
                let preedit = state.native_preedit.clone();
                drive_native(&mut state.native_phase, &preedit, &window, raw, anchor);
            }
            #[cfg(not(windows))]
            panic!("此自测仅支持 Windows");
            if draft.name != "原生火箭" {
                return;
            }
            assert!(state.native_preedit.is_empty());
            if !state.input.click(Action::Apply, &hits, &mut input) {
                return;
            }
        }
        3 => {
            assert!(inspector.draft.is_none(), "{}", inspector.error);
            let after = document.ship.clone();
            assert!(document.undo());
            assert_eq!(state.before.as_ref(), Some(&document.ship));
            assert!(document.redo());
            assert_eq!(after, document.ship);
            save_ship("target/native-ime-smoke.xml", &document.ship).unwrap();
            assert_eq!(
                load_ship("target/native-ime-smoke.xml").unwrap(),
                document.ship
            );
            std::fs::write("target/native-ime-state.txt", "complete\n").unwrap();
            use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk};
            commands.spawn(Screenshot::primary_window()).observe(save_to_disk("target/editor-native-ime.png"))
                .observe(|_: On<ScreenshotCaptured>, mut exit: MessageWriter<AppExit>| {
                    info!("Windows 原生 IME 自测通过：原生消息提交中文、草稿隔离、应用与撤销重做及 XML 往返");
                    exit.write(AppExit::Success);
                });
        }
        _ => return,
    }
    state.phase += 1;
}

#[cfg(windows)]
fn drive_native(
    phase: &mut u8,
    preedit: &str,
    window: &Window,
    raw: &bevy::window::RawHandleWrapper,
    anchor: bevy_egui::egui::Pos2,
) {
    use windows_sys::Win32::UI::Input::{Ime::*, KeyboardAndMouse::*};
    let raw_window_handle::RawWindowHandle::Win32(handle) = raw.get_window_handle() else {
        panic!("未取得 Win32 窗口")
    };
    let hwnd = handle.hwnd.get() as windows_sys::Win32::Foundation::HWND;
    // 只在此窗口的创建线程上使用它的上下文，不跨进程操作 IMM 句柄。
    unsafe {
        if *phase == 0 {
            // 只切换本测试窗口所属线程到已加载的简体中文布局，不安装或改变系统默认布局。
            let count = GetKeyboardLayoutList(0, std::ptr::null_mut());
            let mut layouts = vec![std::ptr::null_mut(); count.max(0) as usize];
            GetKeyboardLayoutList(count, layouts.as_mut_ptr());
            let layout = layouts
                .into_iter()
                .find(|layout| (*layout as usize & 0xffff) == 0x0804)
                .expect("原生 IME 自测需要已加载的简体中文输入法");
            assert!(
                !ActivateKeyboardLayout(layout, 0).is_null(),
                "无法切换测试线程输入法"
            );
        }
        let context = ImmGetContext(hwnd);
        assert!(!context.is_null(), "主线程未取得 IMM 上下文");
        match *phase {
            0 => {
                let mut candidate: CANDIDATEFORM = std::mem::zeroed();
                assert!(
                    ImmGetCandidateWindow(context, 0, &mut candidate) != 0,
                    "未配置候选窗"
                );
                let scale = window.scale_factor();
                assert!((candidate.ptCurrentPos.x as f32 - anchor.x * scale).abs() <= 2.0);
                assert!((candidate.ptCurrentPos.y as f32 - anchor.y * scale).abs() <= 2.0);
                assert!(ImmSetOpenStatus(context, 1) != 0, "无法开启 IME");
                let text: Vec<u16> = "原生火箭".encode_utf16().collect();
                assert!(
                    ImmSetCompositionStringW(
                        context,
                        SCS_SETSTR,
                        text.as_ptr().cast(),
                        (text.len() * 2) as u32,
                        std::ptr::null(),
                        0
                    ) != 0,
                    "输入法不支持 IMM 预编辑"
                );
                *phase = 1;
            }
            1 if preedit == "原生火箭" => {
                assert!(
                    ImmNotifyIME(context, NI_COMPOSITIONSTR, CPS_COMPLETE, 0) != 0,
                    "输入法不支持 IMM 提交"
                );
                *phase = 2;
            }
            _ => {}
        }
        ImmReleaseContext(hwnd, context);
    }
}
