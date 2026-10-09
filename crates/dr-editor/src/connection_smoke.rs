use super::*;

#[derive(Default)]
pub(crate) struct State {
    phase: u8,
    beam: i64,
    after: Option<Ship>,
    capturing: bool,
}

#[derive(Resource)]
pub(crate) struct HintsCaptured;

/// 使用原版目录验证长梁边缘的两个独立连接，不依赖桌面鼠标焦点。
#[allow(clippy::too_many_arguments)]
pub(crate) fn run(
    mode: Res<SmokeTest>,
    mut state: Local<State>,
    document: Res<EditorDocument>,
    cursor: Res<EditorCursor>,
    captured: Option<Res<HintsCaptured>>,
    mut buttons: MessageWriter<panels::PanelButton>,
    mut windows: Query<&mut Window, With<bevy::window::PrimaryWindow>>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut commands: Commands,
    showcase: Option<Res<demo::Showcase>>,
) {
    if !mode.connections || mode.started.elapsed().as_secs() < 3 {
        return;
    }
    assert!(
        demo::within_timeout(showcase.as_deref(), mode.started, 60),
        "连接交互自测超时"
    );
    let Ok(mut window) = windows.single_mut() else {
        return;
    };
    let mut choose = |id: &str| {
        let index = document
            .catalog
            .visible()
            .position(|kind| kind.id == id)
            .expect("自测需要原版部件目录");
        buttons.write(panels::PanelButton::Part(index));
    };
    let mut click = |position: (f32, f32)| {
        window.focused = true;
        let position = Vec2::new(
            window.width() / 2.0 + position.0 * 60.0,
            window.height() / 2.0 - position.1 * 60.0,
        );
        window.set_cursor_position(Some(position));
        mouse.press(MouseButton::Left);
    };
    match state.phase {
        0 => {
            assert_eq!(
                document.ship.all_parts().count(),
                1,
                "请从默认新建船体运行连接自测，不传 --ship"
            );
            choose("strut-1");
        }
        1 => click((0.0, -0.5)),
        2 => {
            assert_eq!(document.ship.all_parts().count(), 2);
            let beam = document
                .ship
                .all_parts()
                .find(|part| part.part_type == "strut-1")
                .unwrap();
            state.beam = beam.id;
            assert!(
                document
                    .ship
                    .all_connections()
                    .any(|connection| connection.touches(beam.id))
            );
            mouse.release(MouseButton::Left);
            choose("detacher-1");
        }
        3 => click((2.0, -1.25)),
        4 => {
            assert_eq!(
                document.ship.all_parts().count(),
                3,
                "长梁右侧放置未生效：{}",
                document.status
            );
            let part = document
                .ship
                .all_parts()
                .find(|part| part.part_type == "detacher-1")
                .unwrap();
            assert!(
                (part.x - 2.0).abs() < 1e-6 && (part.y + 1.25).abs() < 1e-6,
                "侧边吸附把部件拉到了梁中心"
            );
            mouse.release(MouseButton::Left);
        }
        5 => click((-2.0, -1.25)),
        6 => {
            assert_eq!(
                document.ship.all_parts().count(),
                4,
                "长梁左侧放置未生效：{}",
                document.status
            );
            let attached: Vec<_> = document.ship.all_connections().filter(|connection|
                matches!(connection, Connection::Normal { parent, parent_attach: 2, .. } if *parent == state.beam)).collect();
            assert_eq!(attached.len(), 2, "同一条边不同位置没有分别建立连接");
            for connection in attached {
                dr_core::connections::positions(&document.ship, &document.catalog, connection)
                    .map(|(a, b)| assert!(a.distance(b) < 1e-6))
                    .unwrap();
            }
            state.after = Some(document.ship.clone());
            mouse.release(MouseButton::Left);
            keys.press(KeyCode::ControlLeft);
            keys.press(KeyCode::KeyZ);
        }
        7 => {
            assert_eq!(document.ship.all_parts().count(), 3);
            assert_eq!(document.ship.all_connections().count(), 2);
            keys.reset_all();
            keys.press(KeyCode::ControlLeft);
            keys.press(KeyCode::KeyY);
        }
        8 => {
            assert_eq!(state.after.as_ref(), Some(&document.ship));
            keys.reset_all();
            keys.press(KeyCode::Escape);
            save_ship("target/connections-smoke.xml", &document.ship).unwrap();
            assert_eq!(
                load_ship("target/connections-smoke.xml").unwrap(),
                document.ship
            );
        }
        9 => {
            keys.reset_all();
            choose("detacher-1");
            window.focused = true;
            let position = Vec2::new(window.width() / 2.0 + 150.0, window.height() / 2.0 - 15.0);
            window.set_cursor_position(Some(position));
        }
        10 => {
            window.focused = true;
            let position = Vec2::new(window.width() / 2.0 + 150.0, window.height() / 2.0 - 15.0);
            window.set_cursor_position(Some(position));
            assert!(cursor.placing && cursor.valid);
            let (part, connection, allowed) = placement::preview(&document, &cursor).unwrap();
            assert!(
                connection.is_some() && allowed,
                "连接提示预览无合法吸附：world={:?}, part=({:?}, {:?}), connected={}, allowed={}",
                cursor.world,
                part.x,
                part.y,
                connection.is_some(),
                allowed
            );
            if !state.capturing {
                use bevy::render::view::screenshot::{
                    Screenshot, ScreenshotCaptured, save_to_disk,
                };
                commands
                    .spawn(Screenshot::primary_window())
                    .observe(save_to_disk("target/editor-connection-hints.png"))
                    .observe(|_: On<ScreenshotCaptured>, mut commands: Commands| {
                        commands.insert_resource(HintsCaptured);
                    });
                state.capturing = true;
            }
            assert!(part.x.is_finite());
            if captured.is_none() {
                return;
            }
            keys.press(KeyCode::Escape);
        }
        11 => {
            assert!(!cursor.placing);
            assert_eq!(state.after.as_ref(), Some(&document.ship));
            keys.reset_all();
            use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk};
            commands.spawn(Screenshot::primary_window())
                .observe(save_to_disk("target/editor-connections-smoke.png"))
                .observe(|_: On<ScreenshotCaptured>, mut exit: MessageWriter<AppExit>| {
                    info!("连接交互自测通过：原版长梁侧边吸附、同边多点连接、一次撤销重做及 XML 保存往返");
                    exit.write(AppExit::Success);
                });
        }
        _ => return,
    }
    state.phase += 1;
}
