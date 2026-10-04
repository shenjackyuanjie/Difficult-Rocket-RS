//! 只观察真实系统键鼠；不改写 Window.focused、ButtonInput 或 egui RawInput。
//! 用户要求暂跳过输入法/系统文字专项，此回归不打开文本字段或驱动 IME。
use super::*;
use std::path::Path;

#[derive(Default)]
pub struct State {
    step: usize,
    issued: bool,
    original: Option<Ship>,
    moved: Option<Ship>,
    snapped: Option<Ship>,
    foreground_checks: usize,
    status: String,
    capturing: Option<usize>,
}

#[derive(Resource)]
pub struct PreviewCaptured(usize);

fn request(folder: &Path, step: usize, action: &str, points: &[Vec2]) {
    let coordinates: String = points
        .iter()
        .enumerate()
        .map(|(i, p)| format!(",\"x{i}\":{},\"y{i}\":{}", p.x, p.y))
        .collect();
    let temporary = folder.join(format!("step-{step}.tmp"));
    std::fs::write(
        &temporary,
        format!("{{\"step\":{step},\"action\":\"{action}\"{coordinates}}}\n"),
    )
    .unwrap();
    std::fs::rename(temporary, folder.join(format!("step-{step}.json"))).unwrap();
}

#[cfg(windows)]
fn foreground(raw: &bevy::window::RawHandleWrapper) -> bool {
    let raw_window_handle::RawWindowHandle::Win32(handle) = raw.get_window_handle() else {
        panic!("系统键鼠自测未取得 Win32 句柄")
    };
    unsafe {
        windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow()
            == handle.hwnd.get() as windows_sys::Win32::Foundation::HWND
    }
}

#[cfg(not(windows))]
fn foreground(_: &bevy::window::RawHandleWrapper) -> bool {
    panic!("系统键鼠自测仅支持 Windows")
}

#[allow(clippy::too_many_arguments)]
pub fn run(
    mode: Res<SmokeTest>,
    mut state: Local<State>,
    mut document: ResMut<EditorDocument>,
    cursor: Res<EditorCursor>,
    drag: Res<DragState>,
    ui: Res<panels::egui_panel::UiState>,
    ghosts: Query<(&Sprite, &Transform), With<placement::PlacementVisual>>,
    captured: Option<Res<PreviewCaptured>>,
    windows: Query<(&Window, &bevy::window::RawHandleWrapper), With<bevy::window::PrimaryWindow>>,
    cameras: Query<(&Transform, &Projection), With<Camera2d>>,
    _main_thread: bevy::ecs::system::NonSendMarker,
    mut commands: Commands,
) {
    let Some(folder) = &mode.native_input else {
        return;
    };
    if mode.started.elapsed().as_secs() < 3 || state.step > 21 {
        return;
    }
    assert!(
        mode.started.elapsed().as_secs() < 120,
        "系统键鼠自测超时：步骤 {}",
        state.step
    );
    let Ok((window, raw)) = windows.single() else {
        return;
    };
    let Ok((camera, Projection::Orthographic(projection))) = cameras.single() else {
        return;
    };
    let focused = foreground(raw);
    let status = format!(
        "step={}\nforeground={}\nbevy_focused={}\nparts={}\nconnections={}\nplacing={}\npalette_drag={}\nvalid={}\nworld={:?}\n",
        state.step,
        focused,
        window.focused,
        document.ship.parts.len(),
        document.ship.connections.len(),
        cursor.placing,
        cursor.palette_drag,
        cursor.valid,
        cursor.world
    );
    if status != state.status {
        std::fs::write(folder.join("state.txt"), &status).unwrap();
        state.status = status;
    }
    if state.step != 17 && state.step != 18 && (!focused || !window.focused) {
        return;
    }
    if focused {
        state.foreground_checks += 1;
    }
    let world = |x: f32, y: f32| {
        Vec2::new(
            window.width() / 2.0 + (x * 60.0 - camera.translation.x) / projection.scale,
            window.height() / 2.0 - (y * 60.0 - camera.translation.y) / projection.scale,
        ) * window.scale_factor()
    };
    if state.original.is_none() {
        let kind = document
            .catalog
            .types
            .iter()
            .find(|kind| kind.kind == PartKind::Pod)
            .expect("系统键鼠自测需要驾驶舱");
        document.ship = Ship {
            parts: vec![
                kind.instantiate(1, (0.0, 0.0)),
                kind.instantiate(2, (0.0, 2.0)),
                kind.instantiate(3, (4.0, 0.0)),
            ],
            ..default()
        };
        document.history = EditorHistory::default();
        document.saved_ship = document.ship.clone();
        document.clear_selection();
        document.refresh();
        state.original = Some(document.ship.clone());
    }
    let ready = if state.issued && folder.join(format!("ack-{}", state.step)).exists() {
        match state.step {
            0 => document.selected == Some(PartKey::new(0, 1, 0)) && drag.id.is_none(),
            1 => {
                document.selected_keys().len() == 2
                    && document.is_selected(PartKey::new(0, 1, 0))
                    && document.is_selected(PartKey::new(0, 2, 0))
                    && drag.id.is_none()
            }
            2 => {
                if document.ship.parts[0].x == 2.0
                    && document.ship.parts[1].x == 2.0
                    && drag.id.is_none()
                {
                    let mut expected = state.original.as_ref().unwrap().clone();
                    expected.parts[0].x = 2.0;
                    expected.parts[1].x = 2.0;
                    assert_eq!(document.ship, expected);
                    state.moved = Some(expected);
                    true
                } else {
                    false
                }
            }
            3 => Some(&document.ship) == state.original.as_ref(),
            4 | 7 | 10 | 19 => Some(&document.ship) == state.moved.as_ref(),
            5 | 8 | 12 | 14 | 16 => {
                if cursor.palette_drag
                    && cursor.placing
                    && cursor.valid
                    && let Ok((sprite, transform)) = ghosts.single()
                {
                    assert_eq!(sprite.color.alpha(), render::UNLINKED_ALPHA);
                    let (part, connection, allowed) =
                        placement::preview(&document, &cursor).unwrap();
                    assert!(allowed);
                    assert_eq!(
                        sprite.color,
                        if state.step == 8 {
                            assert!(connection.is_some());
                            Color::srgb(0.35, 1.0, 0.65).with_alpha(render::UNLINKED_ALPHA)
                        } else {
                            assert!(connection.is_none());
                            Color::srgb(1.0, 1.0, 1.0).with_alpha(render::UNLINKED_ALPHA)
                        }
                    );
                    let expected = if state.step == 8 {
                        (2.0, 1.0)
                    } else {
                        (5.0, 4.0)
                    };
                    assert_eq!((part.x, part.y), expected);
                    if transform
                        .translation
                        .truncate()
                        .distance(Vec2::new(part.x as f32 * 60.0, part.y as f32 * 60.0))
                        > 0.01
                    {
                        false
                    } else {
                        assert_eq!(
                            Some(&document.ship),
                            if state.step <= 8 {
                                state.moved.as_ref()
                            } else {
                                state.snapped.as_ref()
                            }
                        );
                        true
                    }
                } else {
                    false
                }
            }
            6 => {
                if !cursor.placing && !cursor.palette_drag && document.ship.parts.len() == 4 {
                    let kind = document
                        .catalog
                        .get("detacher-1")
                        .expect("自测需要原版分离器");
                    let mut expected = state.moved.as_ref().unwrap().clone();
                    expected.parts.push(kind.instantiate(4, (5.0, 4.0)));
                    assert_eq!(document.ship, expected);
                    true
                } else {
                    false
                }
            }
            9 => {
                if !cursor.placing && !cursor.palette_drag && document.ship.parts.len() == 4 {
                    let kind = document.catalog.get("detacher-1").unwrap();
                    let mut expected = state.moved.as_ref().unwrap().clone();
                    expected.parts.push(kind.instantiate(4, (2.0, 1.0)));
                    expected.connections.push(Connection::Normal {
                        parent: 1,
                        child: 4,
                        parent_attach: 1,
                        child_attach: 2,
                    });
                    assert_eq!(document.ship, expected);
                    state.snapped = Some(expected);
                    true
                } else {
                    false
                }
            }
            11 | 20 => Some(&document.ship) == state.snapped.as_ref(),
            13 | 15 => !cursor.placing && !cursor.palette_drag && ghosts.is_empty(),
            17 => {
                !focused
                    && !window.focused
                    && !cursor.placing
                    && !cursor.palette_drag
                    && ghosts.is_empty()
            }
            18 => focused && window.focused,
            21 => false,
            _ => unreachable!(),
        }
    } else {
        false
    };
    if state.issued && ready && folder.join(format!("ack-{}", state.step)).exists() {
        if matches!(state.step, 5 | 8) {
            use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk};
            let step = state.step;
            if state.capturing != Some(step) {
                commands
                    .spawn(Screenshot::primary_window())
                    .observe(save_to_disk(folder.join(if step == 5 {
                        "ghost-free.png"
                    } else {
                        "ghost-snap.png"
                    })))
                    .observe(move |_: On<ScreenshotCaptured>, mut commands: Commands| {
                        commands.insert_resource(PreviewCaptured(step));
                    });
                state.capturing = Some(step);
                return;
            }
            if !captured.as_ref().is_some_and(|captured| captured.0 == step) {
                return;
            }
        }
        if (12..=18).contains(&state.step) {
            assert_eq!(Some(&document.ship), state.snapped.as_ref());
        }
        state.step += 1;
        state.issued = false;
    }
    if state.issued {
        return;
    }
    let row = || {
        ui.hits
            .iter()
            .find(|(action, _)| *action == panels::PanelButton::Part(0))
            .map(|(_, rect)| Vec2::new(rect.left() + 100.0, rect.center().y) * ui.pixels_per_point)
    };
    let (action, points) = match state.step {
        0 => ("click", vec![world(0.0, 0.0)]),
        1 => ("shift_click", vec![world(0.0, 2.0)]),
        2 => ("drag", vec![world(0.0, 0.0), world(2.0, 0.0)]),
        3 | 7 | 10 | 19 => ("undo", vec![]),
        4 | 11 | 20 => ("redo", vec![]),
        5 | 8 | 12 | 14 | 16 => {
            let Some(row) = row() else {
                return;
            };
            (
                "drag_hold",
                vec![
                    row,
                    if state.step == 8 {
                        world(2.0, 1.0)
                    } else {
                        world(5.0, 4.0)
                    },
                ],
            )
        }
        6 | 9 => ("release", vec![]),
        13 => {
            let Some(row) = row() else {
                return;
            };
            ("sidebar_release", vec![row])
        }
        15 => ("escape", vec![]),
        17 => ("blur", vec![]),
        18 => ("refocus", vec![]),
        21 => {
            assert_eq!(Some(&document.ship), state.snapped.as_ref());
            save_ship(folder.join("ship.xml"), &document.ship).unwrap();
            assert_eq!(load_ship(folder.join("ship.xml")).unwrap(), document.ship);
            std::fs::write(folder.join("report.json"), format!("{{\"foreground_checks\":{},\"native_mouse_keyboard\":true,\"palette_drag\":true,\"ghost_alpha\":{},\"real_focus_loss\":true,\"ime_tested\":false,\"xml_roundtrip\":true}}\n", state.foreground_checks, render::UNLINKED_ALPHA)).unwrap();
            use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk};
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(folder.join("window.png")))
                .observe(
                    |_: On<ScreenshotCaptured>, mut exit: MessageWriter<AppExit>| {
                        exit.write(AppExit::Success);
                    },
                );
            info!(
                "系统前台键鼠自测通过：多选拖动、目录拖出虚影与吸附连接、一次撤销重做、侧栏/Esc/真实失焦取消与 XML 往返；输入法专项未运行"
            );
            state.step += 1;
            return;
        }
        _ => unreachable!(),
    };
    request(folder, state.step, action, &points);
    state.issued = true;
}
