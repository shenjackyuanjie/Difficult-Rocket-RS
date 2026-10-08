//! 运行真实输入→视觉同步→Transform 传播，逐帧断言而非只检查松手落点。
use super::*;

fn app() -> App {
    let mut app = crate::tests::mouse_app();
    app.add_plugins((MinimalPlugins, AssetPlugin::default(), TransformPlugin))
        .init_asset::<Image>()
        .init_resource::<CameraDrag>()
        .add_message::<MouseWheel>()
        .add_systems(Update, camera_controls.before(mouse_editor))
        .add_systems(Update, render::sync.after(mouse_editor));
    app
}

fn point(app: &mut App, position: Vec2) {
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .clear();
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .clear();
    let mut query = app.world_mut().query::<&mut Window>();
    query
        .single_mut(app.world_mut())
        .unwrap()
        .set_cursor_position(Some(position));
}

fn camera(app: &mut App) -> (Transform, GlobalTransform, f32) {
    let mut query = app
        .world_mut()
        .query_filtered::<(&Transform, &GlobalTransform, &Projection), With<Camera2d>>();
    let (local, global, Projection::Orthographic(projection)) = query.single(app.world()).unwrap()
    else {
        panic!("需要正交相机")
    };
    (*local, *global, projection.scale)
}

fn close(a: Vec2, b: Vec2) {
    assert!(a.distance(b) < 0.002, "同帧坐标不一致：{a:?} != {b:?}");
}

#[test]
fn each_drag_input_reaches_visual_and_global_transform_in_the_same_update() {
    let mut app = app();
    let start = Vec2::new(723.0, 446.0); // 非中心抓取，不能丢掉偏移。
    point(&mut app, start);
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    app.update();
    let mut query = app.world_mut().query::<(Entity, &render::PartVisual)>();
    let entity = query.single(app.world()).unwrap().0;
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
        assert_eq!(
            app.world().resource::<EditorDocument>().ship.parts[0].x,
            0.0
        );
    }
}

#[test]
fn drag_mirrors_and_fine_rotation_reach_sprite_and_cancellation_restores_them() {
    let mut app = app();
    let start = Vec2::new(720.0, 450.0);
    point(&mut app, start);
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    app.update();
    let mut query = app.world_mut().query::<(Entity, &render::PartVisual)>();
    let entity = query.single(app.world()).unwrap().0;
    for code in [
        KeyCode::KeyE,
        KeyCode::KeyX,
        KeyCode::KeyE,
        KeyCode::KeyY,
        KeyCode::KeyR,
    ] {
        point(&mut app, start + Vec2::new(180.0, 120.0));
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(code);
        app.update();
        let document = app.world().resource::<EditorDocument>();
        let part = app
            .world()
            .resource::<DragState>()
            .pose(PartKey::new(0, 1, 0), &document.ship.parts[0]);
        let sprite = app.world().get::<Sprite>(entity).unwrap();
        assert_eq!((sprite.flip_x, sprite.flip_y), (part.flip_x, part.flip_y));
        let transform = app.world().get::<Transform>(entity).unwrap();
        assert!(
            (transform
                .rotation
                .dot(Quat::from_rotation_z(part.angle as f32))
                .abs()
                - 1.0)
                .abs()
                < 1e-5
        );
        assert!(!document.ship.parts[0].flip_x && !document.ship.parts[0].flip_y);
    }
    point(&mut app, start);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Escape);
    app.update();
    let sprite = app.world().get::<Sprite>(entity).unwrap();
    assert!(!sprite.flip_x && !sprite.flip_y);
    assert_eq!(
        app.world().get::<Transform>(entity).unwrap().rotation,
        Quat::IDENTITY
    );
}

#[test]
fn each_background_pan_tracks_pixels_at_every_zoom_and_captures_release_delta() {
    for button in [view::BoxSelectButton::Left, view::BoxSelectButton::Middle] {
        for scale in [0.25, 1.0, 2.5] {
            let mut app = app();
            app.world_mut()
                .resource_mut::<view::ViewOptions>()
                .box_select_button = button;
            let mut cameras = app
                .world_mut()
                .query_filtered::<(&mut Transform, &mut Projection), With<Camera2d>>();
            let (mut transform, mut projection) = cameras.single_mut(app.world_mut()).unwrap();
            transform.translation = Vec3::new(37.0, -19.0, 0.0);
            let Projection::Orthographic(projection) = &mut *projection else {
                unreachable!()
            };
            projection.scale = scale;
            let origin = Vec2::new(37.0, -19.0);
            let start = Vec2::new(850.0, 650.0);
            point(&mut app, start);
            app.world_mut()
                .resource_mut::<ButtonInput<MouseButton>>()
                .press(button.pan());
            app.update();
            let anchor = app.world().resource::<EditorCursor>().world;
            for (index, delta) in [
                Vec2::new(0.5, 0.25),
                Vec2::new(8.0, -4.0),
                Vec2::new(-3.0, 2.0),
                Vec2::new(12.0, 5.0),
            ]
            .into_iter()
            .enumerate()
            {
                point(&mut app, start + delta);
                if index == 3 {
                    app.world_mut()
                        .resource_mut::<ButtonInput<MouseButton>>()
                        .release(button.pan());
                }
                app.update();
                let (local, global, _) = camera(&mut app);
                let expected = origin + Vec2::new(-delta.x, delta.y) * scale;
                close(local.translation.truncate(), expected);
                close(global.translation().truncate(), expected);
                let cursor = app.world().resource::<EditorCursor>().world;
                assert!(
                    (cursor.0 - anchor.0).hypot(cursor.1 - anchor.1) < 1e-5,
                    "输入仍使用上一帧相机"
                );
            }
            assert!(app.world().resource::<CameraDrag>().0.is_none());
            assert!(!app.world().resource::<EditorDocument>().history.can_undo());
        }
    }
}

#[test]
fn cursor_uses_current_zoom_and_window_size_without_camera_cache_refresh() {
    let mut app = app();
    point(&mut app, Vec2::new(800.0, 500.0));
    app.update();
    let mut query = app
        .world_mut()
        .query_filtered::<(&mut Transform, &mut Projection), With<Camera2d>>();
    let (mut transform, mut projection) = query.single_mut(app.world_mut()).unwrap();
    transform.translation = Vec3::new(120.0, -60.0, 0.0);
    let Projection::Orthographic(projection) = &mut *projection else {
        unreachable!()
    };
    projection.scale = 2.0;
    let mut windows = app.world_mut().query::<&mut Window>();
    windows
        .single_mut(app.world_mut())
        .unwrap()
        .resolution
        .set(1000.0, 700.0);
    point(&mut app, Vec2::new(600.0, 400.0));
    app.update();
    let cursor = app.world().resource::<EditorCursor>().world;
    assert!((cursor.0 - 320.0 / 60.0).abs() < 1e-5);
    assert!((cursor.1 + 160.0 / 60.0).abs() < 1e-5);
}

#[test]
fn pan_stops_on_focus_loss_sidebar_and_escape_without_a_restart_jump() {
    for mode in 0..3 {
        let mut app = app();
        point(&mut app, Vec2::new(850.0, 650.0));
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        point(&mut app, Vec2::new(900.0, 700.0));
        match mode {
            0 => {
                let mut query = app.world_mut().query::<&mut Window>();
                query.single_mut(app.world_mut()).unwrap().focused = false;
            }
            1 => app.world_mut().resource_mut::<panels::UiPointer>().blocked = true,
            _ => app
                .world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .press(KeyCode::Escape),
        }
        app.update();
        close(camera(&mut app).0.translation.truncate(), Vec2::ZERO);
        assert!(app.world().resource::<CameraDrag>().0.is_none());
        let mut query = app.world_mut().query::<&mut Window>();
        query.single_mut(app.world_mut()).unwrap().focused = true;
        app.world_mut().resource_mut::<panels::UiPointer>().blocked = false;
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .reset_all();
        point(&mut app, Vec2::new(1000.0, 750.0));
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        close(camera(&mut app).0.translation.truncate(), Vec2::ZERO);
        point(&mut app, Vec2::new(1002.0, 747.0));
        app.update();
        close(
            camera(&mut app).0.translation.truncate(),
            Vec2::new(-2.0, -3.0),
        );
    }
}
