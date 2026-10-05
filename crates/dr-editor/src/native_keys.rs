//! 系统快捷键验收：外部驱动发送 Windows 输入，本系统仅设置初始样本并观察结果。
use super::*;
use bevy::input::{ButtonState, keyboard::KeyboardInput};

#[derive(Debug, Clone, Copy)]
enum Check {
    Selected(usize),
    Base,
    Rotated,
    Flipped(bool, bool),
    Clipboard,
    Paste(bool, bool),
    Empty,
    Placing(usize, i32, bool, bool),
    Rejected,
    Free,
    Placed,
    NotPlacing,
    Inspector(bool),
    InspectorGuard,
    Topology(bool),
    TopologyGuard,
    FitAll,
    FitSelected,
    Debug,
    Visible(bool),
    Home,
    Saved,
    New,
}
const STEPS: &[(&str, Check)] = &[
    ("click", Check::Selected(1)),
    ("key:none:R", Check::Rotated),
    ("key:ctrl:Z", Check::Base),
    ("key:ctrl:Y", Check::Rotated),
    ("key:ctrl:Z", Check::Base),
    ("click", Check::Selected(1)),
    ("key:none:X", Check::Flipped(true, false)),
    ("key:none:Y", Check::Flipped(true, true)),
    ("key:ctrl:Z", Check::Flipped(true, false)),
    ("key:ctrl:Z", Check::Base),
    ("key:ctrl:A", Check::Selected(3)),
    ("key:ctrl:C", Check::Clipboard),
    ("key:ctrl:V", Check::Paste(true, false)),
    ("key:none:R", Check::Paste(true, true)),
    ("key:none:ESC", Check::Paste(false, false)),
    ("key:ctrl:A", Check::Selected(3)),
    ("key:ctrl:X", Check::Empty),
    ("key:ctrl:Z", Check::Base),
    ("key:ctrl:A", Check::Selected(3)),
    ("key:none:DELETE", Check::Empty),
    ("key:ctrl:Z", Check::Base),
    ("key:none:TAB", Check::Placing(1, 0, false, false)),
    ("key:shift:TAB", Check::Placing(0, 0, false, false)),
    ("key:none:R", Check::Placing(0, 1, false, false)),
    ("key:none:X", Check::Placing(0, 1, true, false)),
    ("key:none:Y", Check::Placing(0, 1, true, true)),
    ("key:none:P", Check::Rejected),
    ("move", Check::Free),
    ("key:none:P", Check::Placed),
    ("key:ctrl:Z", Check::Base),
    ("key:none:ESC", Check::NotPlacing),
    ("click", Check::Selected(1)),
    ("key:none:F2", Check::Inspector(true)),
    ("key:none:DELETE", Check::InspectorGuard),
    ("key:none:F3", Check::InspectorGuard),
    ("key:none:F6", Check::InspectorGuard),
    ("key:none:ESC", Check::Inspector(false)),
    ("key:none:F6", Check::Topology(true)),
    ("tree_click", Check::Selected(1)),
    ("key:none:R", Check::TopologyGuard),
    ("key:none:X", Check::TopologyGuard),
    ("key:none:Y", Check::TopologyGuard),
    ("key:none:DELETE", Check::TopologyGuard),
    ("key:none:F6", Check::Topology(false)),
    ("key:none:F", Check::FitAll),
    ("key:shift:F", Check::FitSelected),
    ("key:none:F3", Check::Debug),
    ("key:none:F4", Check::Visible(false)),
    ("key:none:F4", Check::Visible(true)),
    ("key:none:HOME", Check::Home),
    ("key:none:R", Check::Rotated),
    ("key:ctrl:S", Check::Saved),
    ("key:ctrl:N", Check::New),
];
fn primary(action: &str) -> Option<KeyCode> {
    let key = action.strip_prefix("key:")?.split(':').nth(1)?;
    Some(match key {
        "A" => KeyCode::KeyA,
        "C" => KeyCode::KeyC,
        "V" => KeyCode::KeyV,
        "X" => KeyCode::KeyX,
        "Y" => KeyCode::KeyY,
        "Z" => KeyCode::KeyZ,
        "R" => KeyCode::KeyR,
        "P" => KeyCode::KeyP,
        "S" => KeyCode::KeyS,
        "N" => KeyCode::KeyN,
        "F" => KeyCode::KeyF,
        "TAB" => KeyCode::Tab,
        "DELETE" => KeyCode::Delete,
        "ESC" => KeyCode::Escape,
        "F2" => KeyCode::F2,
        "F3" => KeyCode::F3,
        "F4" => KeyCode::F4,
        "F6" => KeyCode::F6,
        "HOME" => KeyCode::Home,
        _ => panic!("未知测试按键 {key}"),
    })
}
#[derive(Default)]
pub struct State {
    step: usize,
    issued: bool,
    before: Option<Ship>,
    foreground_checks: usize,
    observed_keys: usize,
    saw_primary: bool,
    settled: u8,
    full_scale: f32,
    status: String,
}

#[allow(clippy::too_many_arguments)]
pub fn run(
    mode: Res<SmokeTest>,
    mut state: Local<State>,
    mut document: ResMut<EditorDocument>,
    mut paths: ResMut<EditorPaths>,
    mut cursor: ResMut<EditorCursor>,
    drag: Res<DragState>,
    inspector: Res<properties::Inspector>,
    topology: Res<topology_ui::ConnectionEditor>,
    options: Res<view::ViewOptions>,
    ui: Res<panels::egui_panel::UiState>,
    windows: Query<(&Window, &bevy::window::RawHandleWrapper), With<bevy::window::PrimaryWindow>>,
    cameras: Query<(&Transform, &Projection), With<Camera2d>>,
    mut events: MessageReader<KeyboardInput>,
    mut commands: Commands,
    _main_thread: bevy::ecs::system::NonSendMarker,
) {
    let Some(folder) = &mode.native_keys else {
        return;
    };
    if mode.started.elapsed().as_secs() < 3 || state.step > STEPS.len() {
        return;
    }
    assert!(
        mode.started.elapsed().as_secs() < 150,
        "系统快捷键自测超时：步骤 {} {:?}",
        state.step,
        STEPS.get(state.step)
    );
    let Ok((window, raw)) = windows.single() else {
        return;
    };
    let Ok((camera, Projection::Orthographic(projection))) = cameras.single() else {
        return;
    };
    let expected_key = STEPS
        .get(state.step)
        .and_then(|(action, _)| primary(action));
    for event in events.read() {
        if state.issued
            && event.state == ButtonState::Pressed
            && Some(event.key_code) == expected_key
        {
            state.saw_primary = true;
            state.observed_keys += 1;
        }
    }
    if !native_input::foreground(raw) || !window.focused {
        return;
    }
    state.foreground_checks += 1;
    if state.before.is_none() {
        let mut fixture=dr_core::catalog_from_xml(r#"<PartTypes><PartType id="p" name="按键节点" type="pod" sprite="Pod.png" width="2" height="2"/><PartType id="q" name="备用节点" sprite="DetacherVertical.png" width="2" height="2"/></PartTypes>"#).unwrap();
        fixture.name = document.catalog.name.clone();
        fixture.types.extend(document.catalog.types.clone());
        document.catalog = fixture;
        let kind = document.catalog.get("p").unwrap();
        document.ship = Ship {
            name: "快捷键验收".into(),
            parts: vec![
                kind.instantiate(1, (0.0, 0.0)),
                kind.instantiate(2, (4.0, 0.0)),
                kind.instantiate(3, (0.0, 4.0)),
            ],
            ..Ship::default()
        };
        document.saved_ship = document.ship.clone();
        document.history = EditorHistory::default();
        document.clear_selection();
        document.refresh();
        paths.ship = Some(
            folder
                .join("keyboard-ship.xml")
                .to_string_lossy()
                .into_owned(),
        );
        *cursor = EditorCursor::default();
        state.before = Some(document.ship.clone());
    }
    let first = document.ship.parts.first();
    let base = Some(&document.ship) == state.before.as_ref();
    let status = format!(
        "step={} action={:?} primary_seen={} parts={} selected={} placing={} valid={} paste={} inspector={} topology={} angle={:?} flips={:?} camera={:?}/{} dirty={} status={}\n",
        state.step,
        STEPS.get(state.step),
        state.saw_primary,
        document.ship.parts.len(),
        document.selected_keys().len(),
        cursor.placing,
        cursor.valid,
        cursor.paste.is_some(),
        inspector.is_open(),
        topology.open,
        first.map(|p| p.angle),
        first.map(|p| (p.flip_x, p.flip_y)),
        camera.translation,
        projection.scale,
        document.dirty,
        document.status
    );
    if status != state.status {
        std::fs::write(folder.join("state.txt"), &status).unwrap();
        state.status = status;
    }
    if state.step == STEPS.len() {
        assert_eq!(document.ship.parts.len(), 1);
        assert!(!document.dirty);
        std::fs::write(folder.join("report.json"),format!("{{\"steps\":{},\"foreground_checks\":{},\"observed_key_presses\":{},\"native_keyboard\":true,\"intentional_focus_test\":false,\"ime_tested\":false}}",STEPS.len(),state.foreground_checks,state.observed_keys)).unwrap();
        use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk};
        commands.spawn(Screenshot::primary_window()).observe(save_to_disk(folder.join("window.png"))).observe(|_:On<ScreenshotCaptured>,mut exit:MessageWriter<AppExit>| {info!("系统快捷键验收通过：变换、剪贴板、历史、放置、F2/F6 隔离、视图、保存与新建；主动失焦和输入法未测试");exit.write(AppExit::Success);});
        state.step += 1;
        return;
    }
    let (action, check) = STEPS[state.step];
    if state.issued && folder.join(format!("ack-{}", state.step)).exists() {
        let ready = match check {
            Check::Selected(count) => {
                base && document.selected_keys().len() == count && drag.id.is_none()
            }
            Check::Base => base,
            Check::Rotated => {
                first.is_some_and(|p| (p.angle - std::f64::consts::FRAC_PI_2).abs() < 1e-8)
                    && document.ship.parts.len() == 3
            }
            Check::Flipped(x, y) => {
                first.is_some_and(|p| p.flip_x == x && p.flip_y == y && p.angle.abs() < 1e-8)
            }
            Check::Clipboard => {
                base && cursor
                    .clipboard
                    .as_ref()
                    .is_some_and(|f| f.parts().count() == 3)
            }
            Check::Paste(exists, rotated) => {
                base && cursor.paste.is_some() == exists
                    && (!rotated
                        || cursor
                            .paste
                            .as_ref()
                            .is_some_and(|f| f.parts().all(|p| p.angle.abs() > 0.5)))
            }
            Check::Empty => document.ship.parts.is_empty() && document.history.undo_len() == 1,
            Check::Placing(index, rotation, x, y) => {
                base && cursor.placing
                    && cursor.catalog_index == index
                    && cursor.rotation == rotation
                    && cursor.flip_x == x
                    && cursor.flip_y == y
            }
            Check::Rejected => {
                base && cursor.placing && !cursor.valid && document.history.can_redo()
            }
            Check::Free => {
                base && cursor.valid
                    && (cursor.world.0 - 4.0).abs() < 1e-3
                    && (cursor.world.1 - 2.0).abs() < 1e-3
            }
            Check::Placed => {
                document.ship.parts.len() == 4
                    && document.ship.parts.last().is_some_and(|p| {
                        (p.x - 4.0).abs() < 1e-3
                            && (p.y - 2.0).abs() < 1e-3
                            && p.flip_x
                            && p.flip_y
                            && (p.angle - std::f64::consts::FRAC_PI_2).abs() < 1e-8
                    })
            }
            Check::NotPlacing => base && !cursor.placing && cursor.paste.is_none(),
            Check::Inspector(open) => base && inspector.is_open() == open,
            Check::InspectorGuard => {
                base && inspector.is_open() && !topology.open && !options.debug
            }
            Check::Topology(open) => base && topology.open == open,
            Check::TopologyGuard => {
                base && topology.open && document.selected == Some(PartKey::new(0, 1, 0))
            }
            Check::FitAll => base && camera.translation.truncate().length() > 1.0,
            Check::FitSelected => base && projection.scale < state.full_scale - 0.01,
            Check::Debug => base && options.debug,
            Check::Visible(visible) => base && options.ship_visible == visible,
            Check::Home => {
                base && camera.translation.truncate().length() < 1e-5
                    && (projection.scale - 1.0).abs() < 1e-5
            }
            Check::Saved => {
                !document.dirty
                    && dr_core::load_ship(folder.join("keyboard-ship.xml"))
                        .is_ok_and(|ship| ship == document.ship)
            }
            Check::New => {
                document.ship.parts.len() == 1
                    && paths.ship.is_none()
                    && document.ship.name.is_empty()
                    && !document.history.can_undo()
                    && !document.dirty
            }
        };
        if ready && state.saw_primary {
            state.settled += 1;
        } else {
            state.settled = 0;
        }
        if state.settled >= 3 {
            if matches!(check, Check::FitAll) {
                state.full_scale = projection.scale;
            }
            state.step += 1;
            state.issued = false;
            state.saw_primary = false;
            state.settled = 0;
        }
        return;
    }
    if state.issued {
        return;
    }
    let world = |x: f32, y: f32| {
        Vec2::new(
            window.width() / 2.0 + (x * 60.0 - camera.translation.x) / projection.scale,
            window.height() / 2.0 - (y * 60.0 - camera.translation.y) / projection.scale,
        ) * window.scale_factor()
    };
    let (action, points) =
        match action {
            "click" => ("click", vec![world(0.0, 0.0)]),
            "move" => ("move", vec![world(4.0, 2.0)]),
            "tree_click" => {
                let Some((_, rect)) = topology.hits.iter().find(|(action, _)| {
                    *action == topology_ui::Action::Node(PartKey::new(0, 1, 0))
                }) else {
                    return;
                };
                (
                    "click",
                    vec![Vec2::new(rect.center().x, rect.center().y) * ui.pixels_per_point],
                )
            }
            _ => (action, vec![]),
        };
    native_input::request(folder, state.step, action, &points);
    state.saw_primary = expected_key.is_none();
    state.issued = true;
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shortcut_scenario_requires_actual_key_events_and_never_tests_focus_or_text_input() {
        assert!(STEPS.len() > 40);
        for (action, _) in STEPS {
            if action.starts_with("key:") {
                assert!(primary(action).is_some());
            } else {
                assert!(matches!(*action, "click" | "move" | "tree_click"));
            }
        }
        assert!(matches!(STEPS.last(), Some(("key:ctrl:N", Check::New))));
    }
}
