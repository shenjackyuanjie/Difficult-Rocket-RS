use super::*;

#[derive(Default)]
pub(crate) struct State {
    phase: u8,
    before: Option<Ship>,
    after: Option<Ship>,
}

/// 在真实窗口和 ECS 调度中验证目录→画布→撤销重做及 UI 输入隔离。
#[allow(clippy::too_many_arguments)]
pub(crate) fn run(
    mode: Res<SmokeTest>,
    mut state: Local<State>,
    document: Res<EditorDocument>,
    cursor: Res<EditorCursor>,
    mut buttons: Query<(&PanelButton, &mut Interaction)>,
    mut windows: Query<(Entity, &mut Window), With<bevy::window::PrimaryWindow>>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut wheels: MessageWriter<MouseWheel>,
    scrolling: Query<&ScrollPosition, With<PaletteScroll>>,
    cameras: Query<&Projection, With<Camera2d>>,
    previews: Query<(&Sprite, &Transform), With<placement::PlacementVisual>>,
    mut commands: Commands,
) {
    if !mode.panels || mode.started.elapsed().as_secs() < 3 {
        return;
    }
    assert!(mode.started.elapsed().as_secs() < 60, "面板交互自测超时");
    let Ok((window_id, mut window)) = windows.single_mut() else {
        return;
    };
    match state.phase {
        0 => {
            state.before = Some(document.ship.clone());
            let (_, mut interaction) = buttons
                .iter_mut()
                .find(|(button, _)| matches!(button, PanelButton::Part(0)))
                .expect("目录中缺少首个可见部件");
            *interaction = Interaction::Pressed;
        }
        1 => {
            assert!(cursor.placing, "目录选择未进入放置预览");
            assert_eq!(state.before.as_ref(), Some(&document.ship));
            window.focused = true;
            window.set_cursor_position(Some(Vec2::new(850.0, 600.0)));
        }
        2 => {
            assert_eq!(state.before.as_ref(), Some(&document.ship));
            assert!(previews.single().is_ok(), "画布中没有创建放置预览");
            keys.press(KeyCode::KeyR);
            keys.press(KeyCode::KeyX);
        }
        3 => {
            assert_eq!(
                state.before.as_ref(),
                Some(&document.ship),
                "预览变换改写了文档"
            );
            assert_eq!(cursor.rotation, 1);
            assert!(cursor.flip_x);
            let (sprite, transform) = previews.single().unwrap();
            assert!(sprite.flip_x);
            assert!(
                transform
                    .rotation
                    .abs_diff_eq(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2), 0.0001)
            );
            use bevy::render::view::screenshot::{Screenshot, save_to_disk};
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk("target/editor-placement-preview.png"));
        }
        4 => {
            keys.reset_all();
            let pod = document
                .ship
                .all_parts()
                .find(|part| part.pod.is_some())
                .expect("自测样本需要驾驶舱");
            window.focused = true;
            let position = Vec2::new(
                window.width() / 2.0 + pod.x as f32 * 60.0,
                window.height() / 2.0 - pod.y as f32 * 60.0,
            );
            window.set_cursor_position(Some(position));
            mouse.press(MouseButton::Left);
        }
        5 => {
            assert_eq!(
                state.before.as_ref(),
                Some(&document.ship),
                "重叠位置被错误放置"
            );
            assert!(!document.history.can_undo(), "被拒绝的放置产生了撤销记录");
            assert_eq!(
                previews.single().unwrap().0.color,
                Color::srgba(1.0, 0.2, 0.2, 0.65)
            );
            use bevy::render::view::screenshot::{Screenshot, save_to_disk};
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk("target/editor-collision-preview.png"));
        }
        6 => {
            mouse.release(MouseButton::Left);
            window.focused = true;
            window.set_cursor_position(Some(Vec2::new(850.0, 600.0)));
        }
        7 => {
            keys.reset_all();
            window.focused = true;
            window.set_cursor_position(Some(Vec2::new(850.0, 600.0)));
            mouse.press(MouseButton::Left);
        }
        8 => {
            assert_eq!(
                document.ship.all_parts().count(),
                state.before.as_ref().unwrap().all_parts().count() + 1,
                "合法放置失败：{}；光标 {:?}，有效 {}，预览 {}，焦点 {}",
                document.status,
                cursor.world,
                cursor.valid,
                cursor.placing,
                window.focused,
            );
            assert!(document.dirty);
            state.after = Some(document.ship.clone());
            mouse.release(MouseButton::Left);
            keys.press(KeyCode::ControlLeft);
            keys.press(KeyCode::KeyZ);
        }
        9 => {
            assert_eq!(
                state.before.as_ref(),
                Some(&document.ship),
                "一次撤销未还原整个放置操作"
            );
            keys.reset_all();
            keys.press(KeyCode::ControlLeft);
            keys.press(KeyCode::KeyY);
        }
        10 => {
            assert_eq!(state.after.as_ref(), Some(&document.ship), "重做未恢复部件");
            keys.reset_all();
            window.focused = true;
            window.set_cursor_position(Some(Vec2::new(120.0, 400.0)));
            mouse.press(MouseButton::Left);
        }
        11 => {
            assert_eq!(
                state.after.as_ref(),
                Some(&document.ship),
                "侧栏点击穿透到了画布"
            );
            mouse.release(MouseButton::Left);
            keys.press(KeyCode::Escape);
            window.focused = true;
            window.set_cursor_position(Some(Vec2::new(1300.0, 400.0)));
            wheels.write(MouseWheel {
                phase: bevy::input::touch::TouchPhase::Moved,
                unit: MouseScrollUnit::Line,
                x: 0.0,
                y: -3.0,
                window: window_id,
            });
        }
        12 => {
            assert!(!cursor.placing, "Esc 未取消预览");
            assert!(scrolling.single().unwrap().y > 0.0, "目录滚轮没有滚动列表");
            let Projection::Orthographic(projection) = cameras.single().unwrap() else {
                panic!("相机投影错误")
            };
            assert_eq!(projection.scale, 1.0, "目录滚轮意外缩放了画布");
            assert_eq!(state.after.as_ref(), Some(&document.ship));
            use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk};
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk("target/editor-panels-smoke.png"))
                .observe(
                    |_: On<ScreenshotCaptured>, mut exit: MessageWriter<AppExit>| {
                        info!("面板交互自测通过：选择、碰撞拒绝及红色预览、放置、撤销重做、取消及侧栏输入隔离");
                        exit.write(AppExit::Success);
                    },
                );
        }
        _ => return,
    }
    state.phase += 1;
}
