use super::*;

#[derive(Default)]
pub(crate) struct State {
    phase: u8,
    before: Option<Ship>,
    delay: u8,
    previous_scale: f32,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn run(
    mode: Res<SmokeTest>,
    mut state: Local<State>,
    mut document: ResMut<EditorDocument>,
    options: Res<view::ViewOptions>,
    assets: Res<AssetServer>,
    images: Res<Assets<Image>>,
    mut windows: Query<&mut Window, With<bevy::window::PrimaryWindow>>,
    cameras: Query<(&Transform, &Projection), With<Camera2d>>,
    visuals: Query<(
        &Visibility,
        &render::PartVisual,
        &Sprite,
        &bevy::sprite::Anchor,
    )>,
    labels: Query<&Text, With<view::DebugLabel>>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut commands: Commands,
) {
    if !mode.view || mode.started.elapsed().as_secs() < 3 {
        return;
    }
    assert!(mode.started.elapsed().as_secs() < 60, "视图交互自测超时");
    let Ok(mut window) = windows.single_mut() else {
        return;
    };
    let Ok((camera, Projection::Orthographic(projection))) = cameras.single() else {
        return;
    };
    window.focused = true;
    match state.phase {
        0 => {
            let kind = document.catalog.get("detacher-1").unwrap();
            document.ship = Ship {
                parts: vec![
                    kind.instantiate(1, (-10.0, 0.0)),
                    document
                        .catalog
                        .get("lander-1")
                        .unwrap()
                        .instantiate(2, (30.0, 20.0)),
                    document
                        .catalog
                        .get("dock-1")
                        .unwrap()
                        .instantiate(3, (32.0, 20.0)),
                ],
                disconnected: vec![dr_core::ShipGroup {
                    parts: vec![
                        document
                            .catalog
                            .get("lander-1")
                            .unwrap()
                            .instantiate(4, (34.0, 20.0)),
                    ],
                    connections: vec![],
                }],
                ..default()
            };
            document.ship.parts[1].angle = 0.37;
            document.ship.parts[2].flip_x = true;
            document.ship.disconnected[0].parts[0].flip_x = true;
            document.ship.disconnected[0].parts[0].flip_y = true;
            document.saved_ship = document.ship.clone();
            document.clear_selection();
            document.refresh();
            state.before = Some(document.ship.clone());
            keys.press(KeyCode::KeyF);
        }
        1 => {
            // 新加入的贴图异步加载完再通过真实 F 输入适配，不能拿物理尺寸替代验收。
            if document.ship.all_parts().any(|part| {
                render::image_size(
                    &document.catalog.get(&part.part_type).unwrap().sprite,
                    &assets,
                    &images,
                )
                .is_none()
            }) {
                keys.reset_all();
                return;
            }
            if state.delay == 0 {
                state.delay = 1;
                keys.reset_all();
                keys.press(KeyCode::KeyF);
                return;
            }
            state.delay = 0;
            let (center, scale) = view::fitted(
                &document,
                Vec2::new(window.width(), window.height()),
                false,
                |texture| render::image_size(texture, &assets, &images),
            )
            .unwrap();
            assert!(camera.translation.truncate().abs_diff_eq(center, 1e-4));
            assert!((projection.scale - scale).abs() < 1e-4);
            keys.reset_all();
            window.resolution.set(960.0, 640.0);
        }
        2 => {
            state.delay += 1;
            if state.delay < 15 {
                return;
            }
            assert_eq!(window.width(), 960.0);
            assert_eq!(window.height(), 640.0);
            keys.press(KeyCode::KeyF);
        }
        3 => {
            let (center, scale) =
                view::fitted(&document, Vec2::new(960.0, 640.0), false, |texture| {
                    render::image_size(texture, &assets, &images)
                })
                .unwrap();
            assert!(camera.translation.truncate().abs_diff_eq(center, 1e-4));
            assert!((projection.scale - scale).abs() < 1e-4);
            state.previous_scale = scale;
            document.select_only(Some(PartKey::new(0, 2, 0)));
            document.selection.extend([
                PartKey::new(0, 2, 0),
                PartKey::new(0, 3, 0),
                PartKey::new(1, 4, 0),
            ]);
            keys.reset_all();
            keys.press(KeyCode::ShiftLeft);
            keys.press(KeyCode::KeyF);
            keys.press(KeyCode::F3);
            keys.press(KeyCode::F4);
        }
        4 => {
            assert!(projection.scale < state.previous_scale);
            assert!(options.debug);
            assert!(!options.ship_visible);
            assert!(visuals.iter().all(|(v, _, _, _)| *v == Visibility::Hidden));
            assert_eq!(labels.iter().len(), 1);
            keys.reset_all();
            keys.press(KeyCode::F4);
        }
        5 => {
            assert!(
                visuals
                    .iter()
                    .all(|(v, _, _, _)| *v == Visibility::Inherited)
            );
            for (_, visual, sprite, anchor) in &visuals {
                assert_eq!(sprite.custom_size, None, "原图不得按物理边界缩放");
                let part = document
                    .ship
                    .part_at(PartKey::new(visual.group, visual.id, visual.occurrence))
                    .unwrap();
                let size = images.get(&sprite.image).unwrap().size().as_vec2();
                if part.part_type == "lander-1" {
                    assert_eq!(size, Vec2::new(84.0, 207.0));
                } else if part.part_type == "dock-1" {
                    assert_eq!(size, Vec2::new(61.0, 29.0));
                }
                assert_eq!(
                    *anchor,
                    render::pixel_anchor(size, part.flip_x, part.flip_y)
                );
                assert!(
                    (sprite.color.alpha()
                        - if visual.group == 0 {
                            1.0
                        } else {
                            100.0 / 255.0
                        })
                    .abs()
                        < 1e-6
                );
                if document.is_selected(PartKey::new(visual.group, visual.id, visual.occurrence)) {
                    for corner in render::image_corners(part, size) {
                        let offset = (corner - camera.translation.truncate()) / projection.scale;
                        let screen = Vec2::new(window.width(), window.height()) * 0.5
                            + Vec2::new(offset.x, -offset.y);
                        assert!(
                            view::canvas(Vec2::new(window.width(), window.height()))
                                .contains(screen)
                        );
                    }
                }
            }
            assert_eq!(state.before.as_ref(), Some(&document.ship));
            assert!(!document.dirty);
            assert!(!document.history.can_undo());
            keys.reset_all();
            use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk};
            commands.spawn(Screenshot::primary_window()).observe(save_to_disk("target/editor-view-smoke.png"))
                .observe(|_: On<ScreenshotCaptured>, mut exit: MessageWriter<AppExit>| {
                    info!("视图交互自测通过：原尺寸着陆腿与对接器、奇数像素镜像锚点、断开组透明度、整船与选区适配、960×640 窗口缩放、调试显隐、文档及历史不变");
                    exit.write(AppExit::Success);
                });
        }
        _ => return,
    }
    state.phase += 1;
}
