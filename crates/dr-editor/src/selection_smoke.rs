use super::*;

#[derive(Resource)]
pub(crate) struct PreviewCaptured;

#[derive(Default)]
pub(crate) struct State {
    phase: u8,
    before: Option<Ship>,
    after: Option<Ship>,
    pointer: Option<Vec2>,
    camera_before: Option<Vec2>,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn run(
    mode: Res<SmokeTest>,
    mut state: Local<State>,
    mut document: ResMut<EditorDocument>,
    cursor: Res<EditorCursor>,
    drag: Res<DragState>,
    mut windows: Query<&mut Window, With<bevy::window::PrimaryWindow>>,
    camera: Query<&Transform, With<Camera2d>>,
    mut options: ResMut<view::ViewOptions>,
    visuals: Query<(&render::PartVisual, &Transform)>,
    previews: Query<(&Sprite, &Transform), With<selection::PasteVisual>>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut commands: Commands,
    captured: Option<Res<PreviewCaptured>>,
) {
    if !mode.selection || mode.started.elapsed().as_secs() < 3 {
        return;
    }
    assert!(mode.started.elapsed().as_secs() < 90, "多选交互自测超时");
    let Ok(mut window) = windows.single_mut() else {
        return;
    };
    // 系统桌面焦点/鼠标消息不应覆盖注入的输入；第 30 阶段单独测试失焦。
    if state.phase != 31 {
        window.focused = true;
    }
    let mut pointer = state.pointer;
    if let Some(point) = pointer {
        window.set_cursor_position(Some(point));
    }
    let mut point = |x: f32, y: f32| {
        window.focused = true;
        let p = Vec2::new(
            window.width() / 2.0 + x * 60.0,
            window.height() / 2.0 - y * 60.0,
        );
        window.set_cursor_position(Some(p));
        pointer = Some(p);
    };
    match state.phase {
        0 => {
            let kind = document
                .catalog
                .get("detacher-1")
                .expect("自测需要原版目录");
            document.ship = Ship {
                parts: vec![
                    kind.instantiate(1, (0.0, 0.0)),
                    kind.instantiate(2, (0.0, 0.5)),
                    kind.instantiate(3, (4.0, 0.0)),
                ],
                connections: vec![Connection::Normal {
                    parent: 1,
                    child: 2,
                    parent_attach: 1,
                    child_attach: 2,
                }],
                ..Default::default()
            };
            document.history = EditorHistory::default();
            document.saved_ship = document.ship.clone();
            document.clear_selection();
            document.refresh();
            state.before = Some(document.ship.clone());
        }
        1 => {
            point(0.0, 0.0);
            mouse.press(MouseButton::Left);
        }
        2 => {
            assert_eq!(document.selected_keys().len(), 1);
            mouse.release(MouseButton::Left);
        }
        3 => {
            keys.press(KeyCode::ShiftLeft);
            point(0.0, 0.5);
            mouse.press(MouseButton::Left);
        }
        4 => {
            assert_eq!(document.selected_keys().len(), 2);
            mouse.release(MouseButton::Left);
            keys.reset_all();
        }
        5 => {
            point(0.0, 0.0);
            mouse.press(MouseButton::Left);
        }
        6 => {
            assert_eq!(drag.members.len(), 2);
            point(1.0, 1.0);
        }
        7 => {
            assert_eq!(state.before.as_ref(), Some(&document.ship));
            for (visual, transform) in &visuals {
                if visual.id < 3 {
                    assert_eq!(transform.translation.x, 60.0);
                }
            }
            mouse.release(MouseButton::Left);
        }
        8 => {
            assert_eq!(document.ship.parts[0].x, 1.0, "{}", document.status);
            assert_eq!(document.ship.parts[1].x, 1.0);
            assert_eq!(document.ship.connections.len(), 1);
            keys.press(KeyCode::ControlLeft);
            keys.press(KeyCode::KeyZ);
        }
        9 => {
            assert_eq!(state.before.as_ref(), Some(&document.ship));
            keys.reset_all();
            point(-1.5, -0.5);
            mouse.press(MouseButton::Middle);
        }
        10 => {
            assert!(drag.rectangle.is_some());
            point(1.5, 1.0);
        }
        11 => {
            mouse.release(MouseButton::Middle);
        }
        12 => {
            assert_eq!(document.selected_keys().len(), 2);
            keys.press(KeyCode::ControlLeft);
            keys.press(KeyCode::KeyC);
            point(0.0, 0.0);
        }
        13 => {
            assert_eq!(cursor.clipboard.as_ref().unwrap().parts().count(), 2);
            keys.reset_all();
            keys.press(KeyCode::ControlLeft);
            keys.press(KeyCode::KeyV);
        }
        14 => {
            assert!(cursor.paste.is_some());
            assert_eq!(previews.iter().len(), 2);
            assert!(
                previews
                    .iter()
                    .all(|(sprite, _)| sprite.color == Color::srgba(1.0, 0.2, 0.2, 0.65))
            );
            assert_eq!(state.before.as_ref(), Some(&document.ship));
            keys.reset_all();
            point(-3.0, -2.0);
            keys.press(KeyCode::KeyR);
        }
        15 => {
            assert!(
                previews
                    .iter()
                    .all(|(_, transform)| transform.rotation != Quat::IDENTITY)
            );
            keys.reset_all();
            keys.press(KeyCode::Escape);
        }
        16 => {
            assert!(cursor.paste.is_none());
            assert!(previews.is_empty());
            assert_eq!(state.before.as_ref(), Some(&document.ship));
            keys.reset_all();
            keys.press(KeyCode::ControlLeft);
            keys.press(KeyCode::KeyV);
        }
        17 => {
            keys.reset_all();
            point(4.0, 1.0);
        }
        18 => {
            assert_eq!(previews.iter().len(), 2);
            assert!(
                previews
                    .iter()
                    .all(|(sprite, _)| sprite.color == Color::srgba(0.35, 1.0, 0.65, 0.75)),
                "世界位置 {:?}；预览 {:?}",
                cursor.world,
                previews
                    .iter()
                    .map(|(sprite, transform)| (sprite.color, transform.translation))
                    .collect::<Vec<_>>()
            );
            use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk};
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk("target/editor-selection-preview.png"))
                .observe(|_: On<ScreenshotCaptured>, mut commands: Commands| {
                    commands.insert_resource(PreviewCaptured);
                });
        }
        19 => {
            if captured.is_none() {
                return;
            }
            mouse.press(MouseButton::Left);
        }
        20 => {
            assert_eq!(document.ship.all_parts().count(), 5, "{}", document.status);
            assert_eq!(document.ship.all_connections().count(), 3);
            assert_eq!(document.selected_keys().len(), 2);
            assert!(cursor.paste.is_none());
            state.after = Some(document.ship.clone());
            mouse.release(MouseButton::Left);
            save_ship("target/selection-smoke.xml", &document.ship).unwrap();
            assert_eq!(
                load_ship("target/selection-smoke.xml").unwrap(),
                document.ship
            );
            keys.press(KeyCode::ControlLeft);
            keys.press(KeyCode::KeyZ);
        }
        21 => {
            assert_eq!(state.before.as_ref(), Some(&document.ship));
            keys.reset_all();
            keys.press(KeyCode::ControlLeft);
            keys.press(KeyCode::KeyY);
        }
        22 => {
            assert_eq!(state.after.as_ref(), Some(&document.ship));
            keys.reset_all();
            keys.press(KeyCode::ControlLeft);
            keys.press(KeyCode::KeyA);
        }
        23 => {
            assert_eq!(document.selected_keys().len(), 5);
            keys.reset_all();
            keys.press(KeyCode::Delete);
        }
        24 => {
            assert_eq!(document.ship.all_parts().count(), 0);
            keys.reset_all();
            keys.press(KeyCode::ControlLeft);
            keys.press(KeyCode::KeyZ);
        }
        25 => {
            assert_eq!(state.after.as_ref(), Some(&document.ship));
            keys.reset_all();
            keys.press(KeyCode::ControlLeft);
            keys.press(KeyCode::KeyA);
        }
        26 => {
            assert_eq!(document.selected_keys().len(), 5);
            keys.reset_all();
            point(0.0, 0.0);
            mouse.press(MouseButton::Left);
        }
        27 => {
            assert_eq!(drag.members.len(), 5);
            pointer = Some(Vec2::new(100.0, 400.0));
            window.set_cursor_position(pointer);
        }
        28 => {
            mouse.release(MouseButton::Left);
        }
        29 => {
            assert!(drag.id.is_none(), "侧栏上释放没有取消整体拖动");
            assert_eq!(state.after.as_ref(), Some(&document.ship));
            point(0.0, 0.0);
            mouse.press(MouseButton::Left);
        }
        30 => {
            assert_eq!(drag.members.len(), 5);
            window.focused = false;
        }
        31 => {
            assert!(drag.id.is_none(), "失焦没有取消整体拖动");
            assert_eq!(state.after.as_ref(), Some(&document.ship));
            mouse.reset_all();
            point(0.0, 0.0);
            keys.press(KeyCode::ShiftLeft);
            mouse.press(MouseButton::Left);
        }
        32 => {
            assert_eq!(document.selected_keys().len(), 4);
            assert!(drag.id.is_none());
            mouse.release(MouseButton::Left);
            keys.reset_all();
        }
        33 => {
            assert_eq!(options.box_select_button, view::BoxSelectButton::Middle);
            state.camera_before = Some(camera.single().unwrap().translation.truncate());
            point(3.0, -3.0);
            mouse.press(MouseButton::Left);
        }
        34 => {
            assert!(drag.rectangle.is_none());
            point(4.0, -2.0);
        }
        35 => {
            assert_ne!(
                state.camera_before,
                Some(camera.single().unwrap().translation.truncate()),
                "默认左键空白拖动没有移动视角"
            );
            assert!(drag.rectangle.is_none());
            mouse.release(MouseButton::Left);
            options.box_select_button = view::BoxSelectButton::Left;
        }
        36 => {
            point(3.0, -3.0);
            mouse.press(MouseButton::Left);
        }
        37 => {
            assert!(drag.rectangle.is_some(), "切为左键后没有开始框选");
            point(4.0, -2.0);
        }
        38 => {
            mouse.release(MouseButton::Left);
        }
        39 => {
            assert!(drag.rectangle.is_none());
            state.camera_before = Some(camera.single().unwrap().translation.truncate());
            point(3.0, -3.0);
            mouse.press(MouseButton::Middle);
        }
        40 => {
            assert!(drag.rectangle.is_none());
            point(4.0, -2.0);
        }
        41 => {
            assert_ne!(
                state.camera_before,
                Some(camera.single().unwrap().translation.truncate()),
                "切为左键框选后中键没有移动视角"
            );
            assert!(drag.rectangle.is_none());
            mouse.release(MouseButton::Middle);
            use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk};
            commands.spawn(Screenshot::primary_window()).observe(save_to_disk("target/editor-selection-smoke.png"))
                .observe(|_: On<ScreenshotCaptured>, mut exit: MessageWriter<AppExit>| {
                    info!("多选交互自测通过：Shift 多选、中键/左键框选切换、空白左键/中键视角拖动、整体拖动、复制预览与碰撞颜色、旋转取消、粘贴吸附、全选删除、原子撤销重做、侧栏释放与失焦取消及 XML 往返");
                    exit.write(AppExit::Success);
                });
        }
        _ => return,
    }
    state.pointer = pointer;
    state.phase += 1;
}
