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
    mut windows: Query<&mut Window, With<bevy::window::PrimaryWindow>>,
    cameras: Query<(&Transform, &Projection), With<Camera2d>>,
    visuals: Query<&Visibility, With<render::PartVisual>>,
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
                    kind.instantiate(2, (30.0, 20.0)),
                ],
                ..default()
            };
            document.ship.parts[1].angle = 0.37;
            document.saved_ship = document.ship.clone();
            document.clear_selection();
            document.refresh();
            state.before = Some(document.ship.clone());
            keys.press(KeyCode::KeyF);
        }
        1 => {
            let (center, scale) =
                view::fitted(&document, Vec2::new(window.width(), window.height()), false).unwrap();
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
            let (center, scale) = view::fitted(&document, Vec2::new(960.0, 640.0), false).unwrap();
            assert!(camera.translation.truncate().abs_diff_eq(center, 1e-4));
            assert!((projection.scale - scale).abs() < 1e-4);
            state.previous_scale = scale;
            document.select_only(Some(PartKey::new(0, 2, 0)));
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
            assert!(visuals.iter().all(|v| *v == Visibility::Hidden));
            assert_eq!(labels.iter().len(), 1);
            keys.reset_all();
            keys.press(KeyCode::F4);
        }
        5 => {
            assert!(visuals.iter().all(|v| *v == Visibility::Inherited));
            assert_eq!(state.before.as_ref(), Some(&document.ship));
            assert!(!document.dirty);
            assert!(!document.history.can_undo());
            keys.reset_all();
            use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk};
            commands.spawn(Screenshot::primary_window()).observe(save_to_disk("target/editor-view-smoke.png"))
                .observe(|_: On<ScreenshotCaptured>, mut exit: MessageWriter<AppExit>| {
                    info!("视图交互自测通过：整船与选区适配、960×640 窗口缩放、调试标签与船体显隐、文档及历史不变");
                    exit.write(AppExit::Success);
                });
        }
        _ => return,
    }
    state.phase += 1;
}
