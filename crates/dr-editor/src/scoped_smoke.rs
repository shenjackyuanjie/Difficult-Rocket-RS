use super::*;

#[derive(Default)]
pub(crate) struct State {
    phase: u8,
    before: Option<Ship>,
    after: Option<Ship>,
    pointer: Option<Vec2>,
}

/// 真实窗口输入验证重复 ID 的选中、移动、删除、跨组吸附和撤销，不改源样本。
#[allow(clippy::too_many_arguments)]
pub(crate) fn run(
    mode: Res<SmokeTest>,
    mut state: Local<State>,
    mut document: ResMut<EditorDocument>,
    drag: Res<DragState>,
    visuals: Query<(&render::PartVisual, &Transform, &Sprite)>,
    mut windows: Query<&mut Window, With<bevy::window::PrimaryWindow>>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut commands: Commands,
) {
    if !mode.scoped || mode.started.elapsed().as_secs() < 3 {
        return;
    }
    assert!(
        mode.started.elapsed().as_secs() < 60,
        "重复 ID 交互自测超时"
    );
    let Ok(mut window) = windows.single_mut() else {
        return;
    };
    // 保持测试注入的坐标与焦点，避免桌面消息在释放帧取消预览。
    window.focused = true;
    let mut pointer = state.pointer;
    if let Some(position) = pointer {
        window.set_cursor_position(Some(position));
    }
    let key = PartKey::new(1, 1, 0);
    let mut point = |x: f32, y: f32| {
        window.focused = true;
        let position = Vec2::new(
            window.width() / 2.0 + x * 60.0,
            window.height() / 2.0 - y * 60.0,
        );
        window.set_cursor_position(Some(position));
        pointer = Some(position);
    };
    match state.phase {
        0 => {
            let kind = document
                .catalog
                .get("detacher-1")
                .expect("自测需要原版部件目录");
            document.ship = Ship {
                parts: vec![kind.instantiate(1, (0.0, 0.0))],
                disconnected: vec![
                    dr_core::ShipGroup {
                        parts: vec![kind.instantiate(1, (4.0, 0.0))],
                        connections: vec![],
                    },
                    dr_core::ShipGroup {
                        parts: vec![kind.instantiate(1, (-4.0, -2.0))],
                        connections: vec![],
                    },
                ],
                ..Default::default()
            };
            document.history = EditorHistory::default();
            document.saved_ship = document.ship.clone();
            document.selected = None;
            document.refresh();
            state.before = Some(document.ship.clone());
        }
        1 => {
            point(4.0, 0.0);
            mouse.press(MouseButton::Left);
        }
        2 => {
            assert_eq!(document.selected, Some(key));
            assert_eq!(drag.id, Some(key));
            assert_eq!(
                visuals
                    .iter()
                    .filter(|(_, _, sprite)| sprite.color.with_alpha(1.0) != Color::WHITE)
                    .count(),
                1
            );
            point(4.0, 2.0);
        }
        3 => {
            let (_, moving, _) = visuals
                .iter()
                .find(|(visual, _, _)| visual.group == 1)
                .unwrap();
            assert!((moving.translation.y - 120.0).abs() < 1e-4);
            let (_, main, _) = visuals
                .iter()
                .find(|(visual, _, _)| visual.group == 0)
                .unwrap();
            assert_eq!(main.translation.y, 0.0);
            assert_eq!(state.before.as_ref(), Some(&document.ship));
            mouse.release(MouseButton::Left);
        }
        4 => {
            assert_eq!(document.ship.part_at(key).unwrap().y, 2.0);
            assert_eq!(document.ship.parts, state.before.as_ref().unwrap().parts);
            keys.press(KeyCode::Delete);
        }
        5 => {
            assert_eq!(document.ship.all_parts().count(), 2);
            assert_eq!(
                document.ship.disconnected[0],
                state.before.as_ref().unwrap().disconnected[1]
            );
            assert!(document.selected.is_none());
            keys.reset_all();
            keys.press(KeyCode::ControlLeft);
            keys.press(KeyCode::KeyZ);
        }
        6 => {
            assert_eq!(document.ship.part_at(key).unwrap().y, 2.0);
            keys.reset_all();
        }
        7 => {
            keys.press(KeyCode::ControlLeft);
            keys.press(KeyCode::KeyZ);
        }
        8 => {
            assert_eq!(state.before.as_ref(), Some(&document.ship));
            keys.reset_all();
            point(4.0, 0.0);
            mouse.press(MouseButton::Left);
        }
        9 => {
            assert_eq!(document.selected, Some(key));
            point(0.0, 0.5);
        }
        10 => {
            mouse.release(MouseButton::Left);
        }
        11 => {
            assert_eq!(document.ship.parts.len(), 2, "{}", document.status);
            assert_eq!(document.ship.connections.len(), 1);
            assert_ne!(document.ship.parts[0].id, document.ship.parts[1].id);
            assert_eq!(
                document.ship.disconnected,
                vec![state.before.as_ref().unwrap().disconnected[1].clone()]
            );
            assert_eq!(document.ship.parts[1].y, 0.5);
            state.after = Some(document.ship.clone());
            keys.press(KeyCode::ControlLeft);
            keys.press(KeyCode::KeyZ);
        }
        12 => {
            assert_eq!(state.before.as_ref(), Some(&document.ship));
            keys.reset_all();
            keys.press(KeyCode::ControlLeft);
            keys.press(KeyCode::KeyY);
        }
        13 => {
            assert_eq!(state.after.as_ref(), Some(&document.ship));
            keys.reset_all();
            save_ship("target/scoped-smoke.xml", &document.ship).unwrap();
            assert_eq!(load_ship("target/scoped-smoke.xml").unwrap(), document.ship);
        }
        14 => {
            use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk};
            commands.spawn(Screenshot::primary_window())
                .observe(save_to_disk("target/editor-scoped-smoke.png"))
                .observe(|_: On<ScreenshotCaptured>, mut exit: MessageWriter<AppExit>| {
                    info!("重复 ID 交互自测通过：精确选择与预览、组内移动和删除、跨组合并重编号、撤销重做及 XML 保存往返");
                    exit.write(AppExit::Success);
                });
        }
        _ => return,
    }
    state.pointer = pointer;
    state.phase += 1;
}
