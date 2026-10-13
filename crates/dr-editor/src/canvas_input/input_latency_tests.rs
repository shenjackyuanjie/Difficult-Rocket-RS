//! 无窗口/GPU：覆盖消息→InputPlugin→相机/编辑→视觉同步→Transform 传播。
use super::*;
use crate::render;
use crate::view::camera_controls;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::mouse::MouseButtonInput;
use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::input::touch::TouchPhase;
use bevy::input::{ButtonState, InputPlugin};

#[derive(Resource, Default, Debug)]
struct FrameChanges {
    document: bool,
    camera: bool,
    projection: bool,
}

fn observe(
    document: Res<EditorDocument>,
    cameras: Query<(Ref<Transform>, Ref<Projection>), With<Camera2d>>,
    mut changes: ResMut<FrameChanges>,
) {
    let (transform, projection) = cameras.single().unwrap();
    *changes = FrameChanges {
        document: document.is_changed(),
        camera: transform.is_changed(),
        projection: projection.is_changed(),
    };
}

fn app() -> App {
    let mut app = crate::test_support::mouse_app();
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin::default(),
        TransformPlugin,
        InputPlugin,
    ))
    .init_asset::<Image>()
    .init_resource::<panels::Palette>()
    .init_resource::<FrameChanges>()
    .add_systems(
        Update,
        (view::controls, camera_controls)
            .chain()
            .before(mouse_editor),
    )
    .add_systems(
        Update,
        (keyboard_commands, render::sync, observe)
            .chain()
            .after(mouse_editor),
    );
    app
}

fn window(app: &mut App) -> Entity {
    app.world_mut()
        .query_filtered::<Entity, With<bevy::window::PrimaryWindow>>()
        .single(app.world())
        .unwrap()
}

fn point(app: &mut App, position: Vec2) {
    let entity = window(app);
    app.world_mut()
        .get_mut::<Window>(entity)
        .unwrap()
        .set_cursor_position(Some(position));
}

fn button(app: &mut App, state: ButtonState) {
    let window = window(app);
    app.world_mut().write_message(MouseButtonInput {
        button: MouseButton::Left,
        state,
        window,
    });
}

fn key(app: &mut App, code: KeyCode, state: ButtonState) {
    let window = window(app);
    app.world_mut().write_message(KeyboardInput {
        key_code: code,
        logical_key: Key::Character(format!("{code:?}").into()),
        state,
        text: None,
        repeat: false,
        window,
    });
}

fn wheel(app: &mut App, y: f32) {
    let window = window(app);
    app.world_mut().write_message(MouseWheel {
        unit: MouseScrollUnit::Line,
        x: 0.0,
        y,
        window,
        phase: TouchPhase::Moved,
    });
}

fn visual(app: &mut App) -> Entity {
    app.world_mut()
        .query_filtered::<Entity, With<render::PartVisual>>()
        .single(app.world())
        .unwrap()
}

fn close(actual: Vec2, expected: Vec2) {
    assert!(
        actual.distance(expected) < 0.002,
        "同帧坐标不一致：{actual:?} != {expected:?}"
    );
}

fn cursor(app: &App) -> Vec2 {
    let world = app.world().resource::<EditorCursor>().world;
    Vec2::new(world.0 as f32 * 60.0, world.1 as f32 * 60.0)
}

#[test]
fn idle_and_pan_frames_keep_document_and_projection_on_the_incremental_path() {
    let mut app = app();
    let start = Vec2::new(850.0, 650.0);
    point(&mut app, start);
    app.update();
    let entity = visual(&mut app);
    app.update();
    for code in [KeyCode::ShiftLeft, KeyCode::F3, KeyCode::F4] {
        key(&mut app, code, ButtonState::Pressed);
        app.update();
        let changes = app.world().resource::<FrameChanges>();
        assert!(
            !changes.document && !changes.camera && !changes.projection,
            "无编辑快捷键不应触发全船/投影同步：{changes:?}"
        );
        key(&mut app, code, ButtonState::Released);
        app.update();
    }
    button(&mut app, ButtonState::Pressed);
    app.update();
    let anchor = cursor(&app);
    for (index, delta) in [
        Vec2::new(0.5, 0.25),
        Vec2::new(8.0, -4.0),
        Vec2::new(8.0, -4.0), // 按住但没有移动，不传播相机 Transform。
        Vec2::new(12.0, 5.0),
    ]
    .into_iter()
    .enumerate()
    {
        point(&mut app, start + delta);
        if index == 3 {
            button(&mut app, ButtonState::Released);
        }
        app.update();
        let changes = app.world().resource::<FrameChanges>();
        assert!(
            !changes.document && !changes.projection,
            "纯背景拖动只更新相机平移：{changes:?}"
        );
        assert_eq!(changes.camera, index != 2);
        let mut cameras = app
            .world_mut()
            .query_filtered::<(&Transform, &GlobalTransform), With<Camera2d>>();
        let (local, global) = cameras.single(app.world()).unwrap();
        let expected = Vec2::new(-delta.x, delta.y);
        close(local.translation.truncate(), expected);
        close(global.translation().truncate(), expected);
        close(cursor(&app), anchor);
        assert_eq!(visual(&mut app), entity);
    }
    assert!(app.world().resource::<CameraDrag>().0.is_none());
    assert!(!app.world().resource::<EditorDocument>().history.can_undo());
}

#[test]
fn mouse_messages_reach_drag_visual_and_global_transform_in_one_update() {
    let mut app = app();
    let start = Vec2::new(723.0, 446.0);
    point(&mut app, start);
    button(&mut app, ButtonState::Pressed);
    app.update();
    assert!(app.world().resource::<DragState>().id.is_some());
    let entity = visual(&mut app);
    for delta in [
        Vec2::new(0.5, 0.25),
        Vec2::new(3.0, -2.0),
        Vec2::new(-0.25, 0.5),
        Vec2::new(8.0, 5.0),
    ] {
        point(&mut app, start + delta);
        app.update();
        let expected = Vec2::new(delta.x, -delta.y);
        close(
            app.world()
                .get::<Transform>(entity)
                .unwrap()
                .translation
                .truncate(),
            expected,
        );
        close(
            app.world()
                .get::<GlobalTransform>(entity)
                .unwrap()
                .translation()
                .truncate(),
            expected,
        );
        let changes = app.world().resource::<FrameChanges>();
        assert!(!changes.document && !changes.camera && !changes.projection);
        assert_eq!(
            app.world().resource::<EditorDocument>().ship.parts[0].x,
            0.0
        );
        assert_eq!(visual(&mut app), entity);
    }
}

#[test]
fn resize_dpi_and_wheel_use_current_values_even_with_invalid_camera_cache() {
    let mut app = app();
    let window = window(&mut app);
    {
        let mut window = app.world_mut().get_mut::<Window>(window).unwrap();
        window.resolution.set_scale_factor_override(Some(2.0));
        window.resolution.set(1000.0, 700.0);
    }
    let mut cameras = app.world_mut().query_filtered::<(
        &mut Camera,
        &mut Transform,
        &mut GlobalTransform,
        &mut Projection,
    ), With<Camera2d>>();
    let (mut camera, mut local, mut global, mut projection) =
        cameras.single_mut(app.world_mut()).unwrap();
    camera.computed.clip_from_view = Mat4::ZERO;
    camera.computed.target_info.as_mut().unwrap().physical_size = UVec2::ONE;
    camera.computed.target_info.as_mut().unwrap().scale_factor = 40.0;
    local.translation = Vec3::new(120.0, -60.0, 0.0);
    *global = GlobalTransform::from_translation(Vec3::splat(10000.0));
    let Projection::Orthographic(projection) = &mut *projection else {
        unreachable!()
    };
    projection.scale = 2.0;
    point(&mut app, Vec2::new(600.0, 400.0));
    wheel(&mut app, 4.0); // 同帧从 2 缩放到 1，保持鼠标处世界点不变。
    app.update();
    close(cursor(&app), Vec2::new(320.0, -160.0));
    let mut cameras = app
        .world_mut()
        .query_filtered::<(&Camera, &Transform, &GlobalTransform, &Projection), With<Camera2d>>();
    let (camera, local, global, Projection::Orthographic(projection)) =
        cameras.single(app.world()).unwrap()
    else {
        unreachable!()
    };
    close(local.translation.truncate(), Vec2::new(220.0, -110.0));
    close(global.translation().truncate(), Vec2::new(220.0, -110.0));
    assert_eq!(projection.scale, 1.0);
    // 输入转换不需要刷新/污染渲染缓存，本测试也没有 CameraPlugin。
    assert_eq!(camera.computed.clip_from_view, Mat4::ZERO);
    assert_eq!(
        camera.computed.target_info.as_ref().unwrap().physical_size,
        UVec2::ONE
    );
}

#[test]
fn keyboard_edges_still_handle_selection_copy_paste_rotation_and_tab() {
    let mut app = app();
    point(&mut app, Vec2::new(850.0, 650.0));
    app.update();
    key(&mut app, KeyCode::ControlLeft, ButtonState::Pressed);
    key(&mut app, KeyCode::KeyA, ButtonState::Pressed);
    app.update();
    assert_eq!(
        app.world()
            .resource::<EditorDocument>()
            .selected_keys()
            .len(),
        1
    );
    assert!(app.world().resource::<FrameChanges>().document);
    app.update();
    assert!(!app.world().resource::<FrameChanges>().document);
    key(&mut app, KeyCode::KeyC, ButtonState::Pressed);
    app.update();
    assert!(app.world().resource::<EditorCursor>().clipboard.is_some());
    key(&mut app, KeyCode::KeyV, ButtonState::Pressed);
    app.update();
    assert!(app.world().resource::<EditorCursor>().paste.is_some());
    key(&mut app, KeyCode::ControlLeft, ButtonState::Released);
    key(&mut app, KeyCode::ShiftLeft, ButtonState::Pressed);
    key(&mut app, KeyCode::KeyE, ButtonState::Pressed);
    app.update();
    let angle = app
        .world()
        .resource::<EditorCursor>()
        .paste
        .as_ref()
        .unwrap()
        .parts()
        .next()
        .unwrap()
        .angle;
    assert!((angle - 1.0_f64.to_radians()).abs() < 1e-10);
    app.update();
    assert!(!app.world().resource::<FrameChanges>().document);
    key(&mut app, KeyCode::Tab, ButtonState::Pressed);
    app.update();
    let cursor = app.world().resource::<EditorCursor>();
    assert!(cursor.paste.is_none() && cursor.placing);
}
