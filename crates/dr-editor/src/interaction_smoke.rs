//! 有渲染窗口的逐帧回归：在 Transform 传播后检查当帧输入，而非下一帧读落点。
use super::*;

#[derive(Resource, Default)]
pub(crate) struct Probe {
    step: usize,
    expected: Option<(bool, Vec2)>,
    checked: usize,
    max_error: f32,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn input(
    mode: Res<SmokeTest>,
    mut probe: ResMut<Probe>,
    mut document: ResMut<EditorDocument>,
    mut windows: Query<&mut Window, With<bevy::window::PrimaryWindow>>,
    mut cameras: Query<(&mut Transform, &mut Projection), With<Camera2d>>,
    mut options: ResMut<view::ViewOptions>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut drag: ResMut<DragState>,
    mut camera_drag: ResMut<CameraDrag>,
) {
    if !mode.interaction || mode.started.elapsed().as_secs() < 3 {
        return;
    }
    assert!(
        mode.started.elapsed().as_secs() < 60,
        "逐帧拖拽窗口验收超时"
    );
    let mut window = windows.single_mut().unwrap();
    assert_eq!(window.desired_maximum_frame_latency.unwrap().get(), 1);
    window.focused = true;
    let (mut camera, mut projection) = cameras.single_mut().unwrap();
    let Projection::Orthographic(projection) = &mut *projection else {
        panic!("需要正交相机")
    };
    let center = Vec2::new(window.width(), window.height()) * 0.5;
    keys.reset_all();
    mouse.clear();
    if probe.step == 0 {
        document.ship = Ship {
            parts: vec![
                document
                    .catalog
                    .get("detacher-1")
                    .unwrap()
                    .instantiate(1, (0.0, 0.0)),
            ],
            ..default()
        };
        document.saved_ship = document.ship.clone();
        document.history = EditorHistory::default();
        document.clear_selection();
        document.refresh();
        camera.translation = Vec3::ZERO;
        projection.scale = 1.0;
        options.box_select_button = view::BoxSelectButton::Middle;
        mouse.reset_all();
    } else if probe.step <= 41 {
        // 非中心抓取，连续亚像素、反向运动；预览不得提前写入文档。
        let delta = if probe.step == 1 {
            Vec2::ZERO
        } else {
            let t = (probe.step - 1) as f32;
            Vec2::new((t * 0.7).sin() * 8.0, (t * 0.4).cos() * 5.0)
        };
        window.set_cursor_position(Some(center + Vec2::new(3.0, -4.0) + delta));
        if probe.step == 1 {
            mouse.press(MouseButton::Left);
        }
        probe.expected = Some((true, Vec2::new(delta.x, -delta.y)));
    } else if probe.step == 42 {
        keys.press(KeyCode::Escape);
        mouse.reset_all();
    } else if probe.step <= 174 {
        // 六组：两种平移按钮 × 三种缩放，每组20次运动，最后一帧释放。
        let step = probe.step - 43;
        let group = step / 22;
        let frame = step % 22;
        let scale = [0.25, 1.0, 2.5][group % 3];
        let button = if group < 3 {
            view::BoxSelectButton::Middle
        } else {
            view::BoxSelectButton::Left
        };
        options.box_select_button = button;
        let start = center + Vec2::new(140.0, 180.0);
        if frame == 0 {
            mouse.reset_all();
            drag.cancel();
            camera_drag.0 = None;
            camera.translation = Vec3::ZERO;
            projection.scale = scale;
            window.set_cursor_position(Some(start));
            mouse.press(button.pan());
        } else {
            let t = frame as f32;
            let delta = Vec2::new((t * 0.6).sin() * 12.0, (t * 0.5).sin() * 9.0);
            window.set_cursor_position(Some(start + delta));
            if frame == 21 {
                mouse.release(button.pan());
            }
            probe.expected = Some((false, Vec2::new(-delta.x, delta.y) * scale));
        }
    }
    probe.step += 1;
}

pub(crate) fn verify(
    mode: Res<SmokeTest>,
    mut probe: ResMut<Probe>,
    document: Res<EditorDocument>,
    cameras: Query<&GlobalTransform, With<Camera2d>>,
    visuals: Query<&GlobalTransform, With<render::PartVisual>>,
    channels: Option<Res<bevy::render::pipelined_rendering::RenderAppChannels>>,
    mut commands: Commands,
) {
    if !mode.interaction || probe.step == 0 {
        return;
    }
    assert!(channels.is_none(), "不应启用跨帧并行渲染");
    if let Some((part, expected)) = probe.expected.take() {
        let actual = if part {
            visuals.single().unwrap()
        } else {
            cameras.single().unwrap()
        };
        let error = actual.translation().truncate().distance(expected);
        assert!(
            error < 0.002,
            "当帧输入未到达渲染 Transform：期望{expected:?}，实际{:?}",
            actual.translation()
        );
        probe.max_error = probe.max_error.max(error);
        probe.checked += 1;
        assert_eq!(document.ship, document.saved_ship);
        assert!(!document.history.can_undo());
    }
    if probe.step == 176 {
        assert_eq!(probe.checked, 167);
        std::fs::write("target/editor-interaction-latency.json", format!(
            "{{\n  \"same_frame_transform_checks\": {},\n  \"maximum_world_pixel_error\": {},\n  \"pipelined_rendering\": false,\n  \"desired_maximum_frame_latency\": 1,\n  \"physical_display_latency_measured\": false\n}}\n",
            probe.checked, probe.max_error
        )).unwrap();
        use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk};
        commands.spawn(Screenshot::primary_window())
            .observe(save_to_disk("target/editor-interaction-smoke.png"))
            .observe(|_: On<ScreenshotCaptured>, mut exit: MessageWriter<AppExit>| {
                info!("同帧拖拽与平移验收通过：167帧，非中心抓取、连续亚像素/反向输入、两种按钮、三种缩放与释放末段；未测量物理显示延迟");
                exit.write(AppExit::Success);
            });
    }
}
