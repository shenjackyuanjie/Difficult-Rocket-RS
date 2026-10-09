//! 真实窗口中的 2D 姿态与自由连接演示；复用实际控件、鼠标与快捷键路径。
use super::*;
use bevy_egui::EguiInput;

#[derive(Default)]
pub(crate) struct State {
    phase: u8,
    driver: egui_ui::UiTestInput,
    capturing: bool,
    assisted: Option<Ship>,
    far: Option<Ship>,
    undo: usize,
    overlap: f64,
    manual_distance: f64,
    pointer: Option<Vec2>,
    group_fixture: Option<Ship>,
    group_after: Option<Ship>,
    delay: u8,
}

#[derive(Resource, Default)]
pub(crate) struct Captured(u16);

fn pointer(state: &mut State, window: &mut Window, point: (f64, f64)) {
    window.focused = true;
    let position = Vec2::new(
        window.width() / 2.0 + point.0 as f32 * 60.0,
        window.height() / 2.0 - point.1 as f32 * 60.0,
    );
    window.set_cursor_position(Some(position));
    state.pointer = Some(position);
}

fn endpoint(document: &EditorDocument, connected: &Ship, id: i64) -> (f64, f64) {
    let Connection::Normal {
        parent,
        child,
        parent_attach,
        child_attach,
    } = connected.connections[0]
    else {
        panic!("本演示预期普通连接");
    };
    let index = if parent == id {
        parent_attach
    } else {
        assert_eq!(child, id);
        child_attach
    } as usize
        - 1;
    let part = document.ship.part(id).unwrap();
    let kind = document.catalog.get(&part.part_type).unwrap();
    let point = dr_core::part_world_attach(part, &kind.attach_points[index]);
    (point.x, point.y)
}

fn angle(document: &EditorDocument, id: i64, degrees: f64) {
    assert!(
        (document.ship.part(id).unwrap().angle.to_degrees() - degrees).abs() < 1e-6,
        "部件 {id} 角度不为 {degrees}°"
    );
}

fn assist_position(degrees: f64) -> (f64, f64) {
    let (sin, cos) = degrees.to_radians().sin_cos();
    (-2.0 + sin, -cos)
}

fn line_control(
    state: &mut State,
    controls: &connection_lines::Controls,
    input: &mut EguiInput,
    control: connection_lines::Control,
) -> bool {
    let Some((_, rect)) = controls.0.iter().find(|(action, _)| *action == control) else {
        return false;
    };
    state.driver.click_rect(*rect, input);
    true
}

fn tree_control(
    state: &mut State,
    topology: &topology_ui::ConnectionEditor,
    input: &mut EguiInput,
    action: topology_ui::Action,
) -> bool {
    let Some((_, rect)) = topology
        .hits
        .iter()
        .find(|(candidate, _)| *candidate == action)
    else {
        return false;
    };
    state.driver.click_rect(*rect, input);
    true
}

fn assert_group(document: &EditorDocument, before: &Ship) {
    assert_eq!(
        document.selected_keys(),
        vec![
            PartKey::new(0, 1, 0),
            PartKey::new(0, 2, 0),
            PartKey::new(0, 3, 0)
        ]
    );
    assert_eq!(document.ship.connections, before.connections);
    assert_eq!(document.ship.part(4), before.part(4));
    for (a, b) in [(1, 2), (2, 3), (1, 3)] {
        let distance = |ship: &Ship| {
            let a = ship.part(a).unwrap();
            let b = ship.part(b).unwrap();
            (a.x - b.x).hypot(a.y - b.y)
        };
        assert!((distance(&document.ship) - distance(before)).abs() < 1e-6);
    }
}

fn capture(
    commands: &mut Commands,
    state: &mut State,
    captured: &Captured,
    index: u8,
    path: &'static str,
) -> bool {
    if captured.0 & (1 << index) != 0 {
        state.capturing = false;
        return true;
    }
    if !state.capturing {
        use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk};
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(path))
            .observe(
                move |_: On<ScreenshotCaptured>, mut captured: ResMut<Captured>| {
                    captured.0 |= 1 << index
                },
            );
        state.capturing = true;
    }
    false
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(crate) fn run(
    mode: Res<SmokeTest>,
    mut state: Local<State>,
    mut document: ResMut<EditorDocument>,
    cursor: Res<EditorCursor>,
    drag: Res<DragState>,
    ui: Res<panels::egui_panel::UiState>,
    mut inputs: Query<&mut EguiInput>,
    mut windows: Query<&mut Window, With<bevy::window::PrimaryWindow>>,
    visuals: Query<(&render::PartVisual, &Transform, &Sprite)>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut commands: Commands,
    captured: Res<Captured>,
    mut exit: MessageWriter<AppExit>,
    mut showcase: Option<ResMut<demo::Showcase>>,
    (lines, line_controls, configs, topology, options): (
        Res<connection_lines::Settings>,
        Res<connection_lines::Controls>,
        Res<bevy::gizmos::config::GizmoConfigStore>,
        Res<topology_ui::ConnectionEditor>,
        Res<view::ViewOptions>,
    ),
) {
    if !mode.transforms || mode.started.elapsed().as_secs() < 3 {
        return;
    }
    assert!(
        demo::within_timeout(showcase.as_deref(), mode.started, 180),
        "旋转镜像与自由模式窗口自测超时，阶段 {}",
        state.phase
    );
    let (Ok(mut window), Ok(mut input)) = (windows.single_mut(), inputs.single_mut()) else {
        return;
    };
    if state.driver.tick(&mut input) {
        return;
    }
    if state.delay > 0 {
        state.delay -= 1;
        return;
    }
    // 与已有选区专项一致，持续保留内部测试光标；不把它记作系统键鼠验收。
    window.focused = true;
    if let Some(position) = state.pointer {
        window.set_cursor_position(Some(position));
    }
    keys.reset_all();
    let describe = demo::describe;
    match state.phase {
        0 => {
            let fuselage = document.catalog.get("fuselage-1").unwrap();
            let cone = document.catalog.get("nosecone-1").unwrap();
            document.ship = Ship {
                parts: vec![
                    fuselage.instantiate(1, (-2.0, 1.0)),
                    fuselage.instantiate(2, (1.5, -2.0)),
                    cone.instantiate(3, (4.2, 1.0)),
                ],
                ..default()
            };
            document.saved_ship = document.ship.clone();
            document.history = EditorHistory::default();
            document.free_mode = false;
            document.clear_selection();
            document.refresh();
            describe(
                &mut showcase,
                "2D KSP 风格精调：R 90°，Q/E ±15°，Shift+Q/E ±1°；连接点/边与贴图共用姿态",
            );
        }
        1 => {
            pointer(&mut state, &mut window, (1.5, -2.0));
            mouse.press(MouseButton::Left);
        }
        2 => {
            mouse.release(MouseButton::Left);
        }
        3 => {
            assert_eq!(document.selected, Some(PartKey::new(0, 2, 0)));
            keys.press(KeyCode::KeyE);
        }
        4 => {
            angle(&document, 2, 15.0);
            keys.press(KeyCode::ShiftLeft);
            keys.press(KeyCode::KeyE);
        }
        5 => {
            angle(&document, 2, 16.0);
            let Some(rect) = ui.rotate_right else {
                return;
            };
            state.driver.click_rect(rect, &mut input);
            describe(
                &mut showcase,
                "实际点击侧栏 +15° / −15°；精调不会把已有非直角角度覆盖成 90°",
            );
        }
        6 => {
            angle(&document, 2, 31.0);
            let Some(rect) = ui.rotate_left else {
                return;
            };
            state.driver.click_rect(rect, &mut input);
        }
        7 => {
            angle(&document, 2, 16.0);
            pointer(&mut state, &mut window, (1.5, -2.0));
            mouse.press(MouseButton::Left);
        }
        8 => {
            let (x, y) = assist_position(16.0);
            pointer(&mut state, &mut window, (x, y - 0.04));
            describe(
                &mut showcase,
                "16° 与 0° 的部件仍可吸附：连接点足够近，真实实体重叠严格小于 5%",
            );
        }
        9 => {
            assert!(
                drag.id.is_some() && !drag.blocked,
                "16° 吸附预览被拒绝：{}",
                document.status
            );
            let source = drag.pose(PartKey::new(0, 2, 0), document.ship.part(2).unwrap());
            let target = document.ship.part(1).unwrap();
            let kind = document.catalog.get("fuselage-1").unwrap();
            state.overlap = dr_core::geometry::overlap_ratio(&source, kind, target, kind);
            assert!(state.overlap > 0.0 && state.overlap < 0.05);
            assert!(
                drag.command
                    .as_ref()
                    .unwrap()
                    .preview_with_catalog(&document.ship, &document.catalog)
                    .unwrap()
                    .connections
                    .len()
                    == 1
            );
            if !capture(
                &mut commands,
                &mut state,
                &captured,
                0,
                "target/editor-angled-connection.png",
            ) {
                return;
            }
        }
        10 => {
            mouse.release(MouseButton::Left);
        }
        11 => {
            assert_eq!(document.ship.connections.len(), 1);
            info!("任意角度吸附建立的连接：{:?}", document.ship.connections);
            angle(&document, 2, 16.0);
            let (a, b) = dr_core::connections::positions(
                &document.ship,
                &document.catalog,
                &document.ship.connections[0],
            )
            .unwrap();
            assert!(a.distance(b) < 1e-6);
            state.assisted = Some(document.ship.clone());
            let source = document.ship.part(2).unwrap();
            pointer(&mut state, &mut window, (source.x, source.y));
            mouse.press(MouseButton::Left);
        }
        12 => {
            keys.press(KeyCode::KeyE);
            let (x, y) = assist_position(31.0);
            pointer(&mut state, &mut window, (x, y));
            describe(
                &mut showcase,
                "重叠超过 5% 时辅助模式拒绝新连接；拖拽落点仍保留，可一次撤销回原连接",
            );
        }
        13 => {
            let source = drag.pose(PartKey::new(0, 2, 0), document.ship.part(2).unwrap());
            let kind = document.catalog.get("fuselage-1").unwrap();
            assert!(
                dr_core::geometry::overlap_ratio(
                    &source,
                    kind,
                    document.ship.part(1).unwrap(),
                    kind
                ) >= 0.05
            );
            assert!(drag.blocked, "超限姿态没有显示碰撞警告");
            mouse.release(MouseButton::Left);
        }
        14 => {
            assert!(document.ship.connections.is_empty());
            keys.press(KeyCode::ControlLeft);
            keys.press(KeyCode::KeyZ);
        }
        15 => {
            assert_eq!(state.assisted.as_ref(), Some(&document.ship));
            state.undo = document.history.undo_len();
            let Some(rect) = ui.free_mode else {
                return;
            };
            state.driver.click_rect(rect, &mut input);
            describe(
                &mut showcase,
                "实际开启自由模式：不吸附、不依靠距离/碰撞/占用决定连接，移动旋转镜像保留已有连接",
            );
        }
        16 => {
            assert!(document.free_mode);
            assert_eq!(
                document.history.undo_len(),
                state.undo,
                "模式切换不应新增历史"
            );
            let source = document.ship.part(2).unwrap();
            pointer(&mut state, &mut window, (source.x, source.y));
            mouse.press(MouseButton::Left);
        }
        17 => {
            pointer(&mut state, &mut window, (1.1, -2.5));
            keys.press(KeyCode::KeyE);
        }
        18 => {
            keys.press(KeyCode::KeyX);
            describe(
                &mut showcase,
                "拖拽期间组合 15° 旋转、X 镜像、1° 精调、Y 镜像和 R：预览不改文档，松手一次提交",
            );
        }
        19 => {
            keys.press(KeyCode::ShiftLeft);
            keys.press(KeyCode::KeyQ);
        }
        20 => {
            keys.press(KeyCode::KeyY);
        }
        21 => {
            keys.press(KeyCode::KeyR);
        }
        22 => {
            let part = drag.pose(PartKey::new(0, 2, 0), document.ship.part(2).unwrap());
            assert!((part.angle.to_degrees() - 122.0).abs() < 1e-6);
            assert!(part.flip_x && part.flip_y);
            assert_eq!(state.assisted.as_ref(), Some(&document.ship));
            assert!(!drag.blocked);
            let (_, transform, sprite) = visuals
                .iter()
                .find(|(visual, _, _)| visual.id == 2)
                .unwrap();
            assert!((transform.translation.x - part.x as f32 * 60.0).abs() < 1e-3);
            assert!(
                (transform
                    .rotation
                    .dot(Quat::from_rotation_z(part.angle as f32))
                    .abs()
                    - 1.0)
                    .abs()
                    < 1e-5
            );
            assert!(sprite.flip_x && sprite.flip_y);
            if !capture(
                &mut commands,
                &mut state,
                &captured,
                1,
                "target/editor-combined-transform.png",
            ) {
                return;
            }
        }
        23 => {
            mouse.release(MouseButton::Left);
        }
        24 => {
            angle(&document, 2, 122.0);
            assert_eq!(
                document.ship.connections,
                state.assisted.as_ref().unwrap().connections
            );
            assert_eq!(document.history.undo_len(), state.undo + 1);
            state.far = Some(document.ship.clone());
            keys.press(KeyCode::ControlLeft);
            keys.press(KeyCode::KeyZ);
        }
        25 => {
            assert_eq!(state.assisted.as_ref(), Some(&document.ship));
            keys.press(KeyCode::ControlLeft);
            keys.press(KeyCode::KeyY);
        }
        26 => {
            assert_eq!(state.far.as_ref(), Some(&document.ship));
            let position = endpoint(&document, state.far.as_ref().unwrap(), 2);
            pointer(&mut state, &mut window, position);
            mouse.press(MouseButton::Left);
            describe(
                &mut showcase,
                "自由模式双点手动断开：先点击源连接点，再点击远处目标连接点；无需先移动贴合",
            );
        }
        27 => {
            assert!(cursor.manual_connection.is_some());
            info!("自由模式首点：{:?}", cursor.manual_connection);
            mouse.release(MouseButton::Left);
        }
        28 => {
            let position = endpoint(&document, state.far.as_ref().unwrap(), 1);
            pointer(&mut state, &mut window, position);
            mouse.press(MouseButton::Left);
        }
        29 => {
            assert!(
                document.ship.connections.is_empty(),
                "双点断连后仍有连接：{:?}，待连端点 {:?}，状态 {}",
                document.ship.connections,
                cursor.manual_connection,
                document.status
            );
            mouse.release(MouseButton::Left);
        }
        30 => {
            let position = endpoint(&document, state.far.as_ref().unwrap(), 2);
            pointer(&mut state, &mut window, position);
            mouse.press(MouseButton::Left);
            describe(
                &mut showcase,
                "再点两个远距离连接点直接连上；金色为已选端点，紫色为可点击的旋转后连接点/边",
            );
        }
        31 => {
            mouse.release(MouseButton::Left);
        }
        32 => {
            let position = endpoint(&document, state.far.as_ref().unwrap(), 1);
            pointer(&mut state, &mut window, position);
            mouse.press(MouseButton::Left);
        }
        33 => {
            mouse.release(MouseButton::Left);
        }
        34 => {
            assert_eq!(document.ship.connections.len(), 1);
            assert_eq!(state.far.as_ref().unwrap().parts, document.ship.parts);
            assert!(
                document.ship.connections[0]
                    .equivalent(&state.far.as_ref().unwrap().connections[0])
            );
            state.far = Some(document.ship.clone());
            let (a, b) = dr_core::connections::positions(
                &document.ship,
                &document.catalog,
                &document.ship.connections[0],
            )
            .unwrap();
            state.manual_distance = a.distance(b);
            assert!(state.manual_distance > dr_core::connections::CONNECTION_DISTANCE);
            if !capture(
                &mut commands,
                &mut state,
                &captured,
                2,
                "target/editor-free-connections.png",
            ) {
                return;
            }
        }
        35 => {
            let index = document
                .catalog
                .visible()
                .position(|kind| kind.id == "fuselage-1")
                .unwrap();
            let Some((_, rect)) = ui
                .hits
                .iter()
                .find(|(action, _)| *action == panels::PanelButton::Part(index))
            else {
                return;
            };
            state.driver.click_rect(*rect, &mut input);
            describe(
                &mut showcase,
                "自由模式新部件预览：精细旋转与双轴镜像，允许完全重叠放置，但不自动连接",
            );
        }
        36 => {
            assert!(cursor.placing);
            pointer(&mut state, &mut window, (-2.0, 1.0));
            keys.press(KeyCode::KeyE);
        }
        37 => {
            keys.press(KeyCode::ShiftLeft);
            keys.press(KeyCode::KeyE);
        }
        38 => {
            keys.press(KeyCode::KeyX);
        }
        39 => {
            keys.press(KeyCode::KeyY);
        }
        40 => {
            let (part, connection, allowed) = placement::preview(&document, &cursor).unwrap();
            assert!(allowed && connection.is_none());
            assert!((part.angle.to_degrees() - 16.0).abs() < 1e-6 && part.flip_x && part.flip_y);
            assert!((part.x + 2.0).abs() < 1e-6 && (part.y - 1.0).abs() < 1e-6);
            if !capture(
                &mut commands,
                &mut state,
                &captured,
                3,
                "target/editor-free-placement.png",
            ) {
                return;
            }
        }
        41 => {
            mouse.press(MouseButton::Left);
        }
        42 => {
            assert_eq!(document.ship.parts.len(), 4);
            assert_eq!(document.ship.connections.len(), 1);
            mouse.release(MouseButton::Left);
            keys.press(KeyCode::ControlLeft);
            keys.press(KeyCode::KeyZ);
        }
        43 => {
            assert_eq!(state.far.as_ref(), Some(&document.ship));
            keys.press(KeyCode::Escape);
        }
        44 => {
            assert!(!cursor.placing);
            pointer(&mut state, &mut window, (-2.0, 1.0));
            mouse.press(MouseButton::Left);
        }
        45 => {
            mouse.release(MouseButton::Left);
        }
        46 => {
            pointer(&mut state, &mut window, (1.1, -2.5));
            keys.press(KeyCode::ShiftLeft);
            mouse.press(MouseButton::Left);
            describe(
                &mut showcase,
                "多选整体精调与镜像：围绕选区中心变换，内部连接保持，未选部件不移动",
            );
        }
        47 => {
            mouse.release(MouseButton::Left);
        }
        48 => {
            assert_eq!(document.selected_keys().len(), 2);
            keys.press(KeyCode::KeyE);
        }
        49 => {
            keys.press(KeyCode::ShiftLeft);
            keys.press(KeyCode::KeyQ);
        }
        50 => {
            keys.press(KeyCode::KeyX);
        }
        51 => {
            keys.press(KeyCode::KeyY);
        }
        52 => {
            keys.press(KeyCode::KeyR);
        }
        53 => {
            assert_eq!(document.ship.connections.len(), 1);
            assert_eq!(document.ship.part(3), state.far.as_ref().unwrap().part(3));
            assert_eq!(document.selected_keys().len(), 2);
            if !capture(
                &mut commands,
                &mut state,
                &captured,
                4,
                "target/editor-transforms-smoke.png",
            ) {
                return;
            }
        }
        54 => {
            let kind = document.catalog.get("fuselage-1").unwrap();
            let cone = document.catalog.get("nosecone-1").unwrap();
            document.ship = Ship {
                parts: vec![
                    kind.instantiate(1, (-2.0, 0.0)),
                    kind.instantiate(2, (0.0, -2.0)),
                    kind.instantiate(3, (1.6, -2.0)),
                    cone.instantiate(4, (4.7, 2.6)),
                ],
                connections: vec![
                    Connection::Normal {
                        parent: 1,
                        child: 2,
                        parent_attach: 2,
                        child_attach: 1,
                    },
                    Connection::Normal {
                        parent: 2,
                        child: 3,
                        parent_attach: 4,
                        child_attach: 3,
                    },
                ],
                ..default()
            };
            document.saved_ship = document.ship.clone();
            document.history = default();
            document.clear_selection();
            document.refresh();
            state.group_fixture = Some(document.ship.clone());
            describe(
                &mut showcase,
                "连接线可调：显示开关、颜色/透明度、像素粗细、实线/虚线/点线、呼吸/流动、方向箭头",
            );
        }
        55 => {
            let Some(rect) = ui.connection_lines else {
                return;
            };
            state.driver.click_rect(rect, &mut input);
        }
        56 => {
            assert!(lines.open);
            if !line_control(
                &mut state,
                &line_controls,
                &mut input,
                connection_lines::Control::Visible,
            ) {
                return;
            }
        }
        57 => {
            assert!(!lines.enabled);
            assert!(!configs.config::<connection_lines::LineGizmos>().0.enabled);
            if !capture(
                &mut commands,
                &mut state,
                &captured,
                5,
                "target/editor-lines-hidden.png",
            ) {
                return;
            }
        }
        58 => {
            if !line_control(
                &mut state,
                &line_controls,
                &mut input,
                connection_lines::Control::Visible,
            ) {
                return;
            }
        }
        59 => {
            assert!(lines.enabled);
            if !line_control(
                &mut state,
                &line_controls,
                &mut input,
                connection_lines::Control::Purple,
            ) {
                return;
            }
        }
        60 => {
            assert!((lines.rgba[0] - 0.75).abs() < 1e-5);
            if !line_control(
                &mut state,
                &line_controls,
                &mut input,
                connection_lines::Control::Thicker,
            ) {
                return;
            }
        }
        61 => {
            assert_eq!(lines.width, 3.0);
            if !line_control(
                &mut state,
                &line_controls,
                &mut input,
                connection_lines::Control::Thicker,
            ) {
                return;
            }
        }
        62 => {
            assert_eq!(lines.width, 4.0);
            if !line_control(
                &mut state,
                &line_controls,
                &mut input,
                connection_lines::Control::Dashed,
            ) {
                return;
            }
        }
        63 => {
            assert_eq!(lines.pattern, connection_lines::Pattern::Dashed);
            if !line_control(
                &mut state,
                &line_controls,
                &mut input,
                connection_lines::Control::Flow,
            ) {
                return;
            }
        }
        64 => {
            assert_eq!(lines.effect, connection_lines::Effect::Flow);
            if !line_control(
                &mut state,
                &line_controls,
                &mut input,
                connection_lines::Control::Arrows,
            ) {
                return;
            }
        }
        65 => {
            assert!(lines.arrows);
            let config = configs.config::<connection_lines::LineGizmos>().0;
            assert_eq!(config.line.width, 4.0);
            assert!(matches!(
                config.line.style,
                bevy::gizmos::config::GizmoLineStyle::Dashed { .. }
            ));
            assert_eq!(state.group_fixture.as_ref(), Some(&document.ship));
            assert_eq!(document.history.undo_len(), 0);
            if !capture(
                &mut commands,
                &mut state,
                &captured,
                6,
                "target/editor-lines-flow.png",
            ) {
                return;
            }
        }
        66 => {
            if !line_control(
                &mut state,
                &line_controls,
                &mut input,
                connection_lines::Control::Dotted,
            ) {
                return;
            }
        }
        67 => {
            assert_eq!(lines.pattern, connection_lines::Pattern::Dotted);
            if !line_control(
                &mut state,
                &line_controls,
                &mut input,
                connection_lines::Control::Pulse,
            ) {
                return;
            }
        }
        68 => {
            assert_eq!(lines.effect, connection_lines::Effect::Pulse);
            assert_eq!(state.group_fixture.as_ref(), Some(&document.ship));
            assert_eq!(document.history.undo_len(), 0);
            if !capture(
                &mut commands,
                &mut state,
                &captured,
                7,
                "target/editor-lines-pulse.png",
            ) {
                return;
            }
            keys.press(KeyCode::Escape);
        }
        69 => {
            assert!(!lines.open);
            pointer(&mut state, &mut window, (0.0, 3.0));
        }
        70 => {
            keys.press(KeyCode::F6);
            state.delay = 3;
            describe(
                &mut showcase,
                "连接树实际选择子树：父、子、孙作为同一选区，共用精细旋转和双轴镜像，内部连接保持",
            );
        }
        71 => {
            assert!(topology.open);
            if !tree_control(
                &mut state,
                &topology,
                &mut input,
                topology_ui::Action::Node(PartKey::new(0, 1, 0)),
            ) {
                return;
            }
        }
        72 => {
            assert_eq!(
                document.selected,
                Some(PartKey::new(0, 1, 0)),
                "连接树实际点击根节点未生效"
            );
            if !tree_control(
                &mut state,
                &topology,
                &mut input,
                topology_ui::Action::Subtree,
            ) {
                return;
            }
        }
        73 => {
            assert_group(&document, state.group_fixture.as_ref().unwrap());
            keys.press(KeyCode::Escape);
        }
        74 => {
            assert!(!topology.open);
            pointer(&mut state, &mut window, (0.0, 3.0));
            keys.press(KeyCode::KeyE);
        }
        75 => {
            keys.press(KeyCode::KeyX);
        }
        76 => {
            keys.press(KeyCode::KeyY);
        }
        77 => {
            keys.press(KeyCode::KeyR);
        }
        78 => {
            assert_group(&document, state.group_fixture.as_ref().unwrap());
            for id in 1..=3 {
                angle(&document, id, 105.0);
                assert!(
                    document.ship.part(id).unwrap().flip_x
                        && document.ship.part(id).unwrap().flip_y
                );
            }
            assert_eq!(document.history.undo_len(), 4);
            if !capture(
                &mut commands,
                &mut state,
                &captured,
                8,
                "target/editor-tree-transforms.png",
            ) {
                return;
            }
        }
        79..=82 => {
            keys.press(KeyCode::ControlLeft);
            keys.press(KeyCode::KeyZ);
        }
        83 => {
            assert_eq!(state.group_fixture.as_ref(), Some(&document.ship));
            keys.press(KeyCode::F6);
            state.delay = 3;
            describe(
                &mut showcase,
                "连接树/图实际选择连通分量：从中间子节点选整坨，包括父节点；旋转镜像使用相同组中心",
            );
        }
        84 => {
            if !tree_control(
                &mut state,
                &topology,
                &mut input,
                topology_ui::Action::Node(PartKey::new(0, 2, 0)),
            ) {
                return;
            }
        }
        85 => {
            if !tree_control(
                &mut state,
                &topology,
                &mut input,
                topology_ui::Action::Component,
            ) {
                return;
            }
        }
        86 => {
            assert_group(&document, state.group_fixture.as_ref().unwrap());
            keys.press(KeyCode::Escape);
        }
        87 => {
            pointer(&mut state, &mut window, (0.0, 3.0));
            keys.press(KeyCode::ShiftLeft);
            keys.press(KeyCode::KeyE);
        }
        88 => {
            keys.press(KeyCode::KeyX);
        }
        89 => {
            assert_group(&document, state.group_fixture.as_ref().unwrap());
            if !capture(
                &mut commands,
                &mut state,
                &captured,
                9,
                "target/editor-component-transforms.png",
            ) {
                return;
            }
            keys.press(KeyCode::ControlLeft);
            keys.press(KeyCode::KeyZ);
        }
        90 => {
            keys.press(KeyCode::ControlLeft);
            keys.press(KeyCode::KeyZ);
        }
        91 => {
            assert_eq!(state.group_fixture.as_ref(), Some(&document.ship));
            document.clear_selection();
            pointer(&mut state, &mut window, (-3.1, 1.2));
            mouse.press(MouseButton::Middle);
            describe(
                &mut showcase,
                "中键真实框选父、子、孙：选区整体做 1°/15° 精调与镜像，不能逐个绕各自中心转",
            );
        }
        92 => {
            pointer(&mut state, &mut window, (2.65, -3.1));
        }
        93 => {
            mouse.release(MouseButton::Middle);
        }
        94 => {
            assert_group(&document, state.group_fixture.as_ref().unwrap());
            keys.press(KeyCode::ShiftLeft);
            keys.press(KeyCode::KeyQ);
        }
        95 => {
            keys.press(KeyCode::KeyY);
        }
        96 => {
            keys.press(KeyCode::KeyQ);
        }
        97 => {
            assert_group(&document, state.group_fixture.as_ref().unwrap());
            if !capture(
                &mut commands,
                &mut state,
                &captured,
                10,
                "target/editor-box-transforms.png",
            ) {
                return;
            }
            keys.press(KeyCode::ControlLeft);
            keys.press(KeyCode::KeyZ);
        }
        98..=99 => {
            keys.press(KeyCode::ControlLeft);
            keys.press(KeyCode::KeyZ);
        }
        100 => {
            assert_eq!(state.group_fixture.as_ref(), Some(&document.ship));
            document.select_only(Some(PartKey::new(0, 1, 0)));
            let Some(rect) = ui.follow_children else {
                return;
            };
            state.driver.click_rect(rect, &mut input);
            describe(
                &mut showcase,
                "开启子节点跟随：只抓父节点，也会带上全部后代同步做非直角旋转与组合镜像，松手只提交一次",
            );
        }
        101 => {
            assert!(options.follow_children);
            pointer(&mut state, &mut window, (-2.0, 0.0));
            mouse.press(MouseButton::Left);
        }
        102 => {
            assert_eq!(drag.keys().len(), 3);
            pointer(&mut state, &mut window, (-1.5, -0.5));
            keys.press(KeyCode::KeyE);
        }
        103 => {
            keys.press(KeyCode::KeyX);
        }
        104 => {
            keys.press(KeyCode::ShiftLeft);
            keys.press(KeyCode::KeyQ);
        }
        105 => {
            keys.press(KeyCode::KeyY);
        }
        106 => {
            keys.press(KeyCode::KeyR);
        }
        107 => {
            assert_eq!(state.group_fixture.as_ref(), Some(&document.ship));
            assert!(!drag.blocked && drag.keys().len() == 3);
            for id in 1..=3 {
                let part = drag.pose(PartKey::new(0, id, 0), document.ship.part(id).unwrap());
                assert!(
                    (part.angle.to_degrees() - 106.0).abs() < 1e-6 && part.flip_x && part.flip_y
                );
                let (_, transform, sprite) = visuals
                    .iter()
                    .find(|(visual, _, _)| visual.id == id)
                    .unwrap();
                assert!(sprite.flip_x && sprite.flip_y);
                assert!((transform.translation.x - part.x as f32 * 60.0).abs() < 1e-3);
                assert!(
                    (transform
                        .rotation
                        .dot(Quat::from_rotation_z(part.angle as f32))
                        .abs()
                        - 1.0)
                        .abs()
                        < 1e-5
                );
            }
            if !capture(
                &mut commands,
                &mut state,
                &captured,
                11,
                "target/editor-descendant-transforms.png",
            ) {
                return;
            }
        }
        108 => {
            mouse.release(MouseButton::Left);
        }
        109 => {
            assert_eq!(
                document.ship.connections,
                state.group_fixture.as_ref().unwrap().connections
            );
            assert_eq!(
                document.ship.part(4),
                state.group_fixture.as_ref().unwrap().part(4)
            );
            assert_eq!(document.history.undo_len(), 1);
            for id in 1..=3 {
                angle(&document, id, 106.0);
            }
            state.group_after = Some(document.ship.clone());
            keys.press(KeyCode::ControlLeft);
            keys.press(KeyCode::KeyZ);
        }
        110 => {
            assert_eq!(state.group_fixture.as_ref(), Some(&document.ship));
            keys.press(KeyCode::ControlLeft);
            keys.press(KeyCode::KeyY);
        }
        111 => {
            assert_eq!(state.group_after.as_ref(), Some(&document.ship));
            save_ship("target/transforms-smoke.xml", &document.ship).unwrap();
            assert_eq!(
                load_ship("target/transforms-smoke.xml").unwrap(),
                document.ship
            );
            let report = serde_json::json!({"completed": true, "assisted_overlap_ratio": state.overlap, "manual_connection_distance": state.manual_distance, "rotation_steps_degrees": [90,15,1], "drag_preview_degrees": 122, "drag_mirrors": ["x","y"], "free_mode_real_checkbox": true, "precision_real_buttons": true, "free_drag_preserved_connection": true, "free_overlapping_placement_without_auto_connection": true, "manual_connect_and_disconnect": true, "multiselect_mirrors_and_rotation": true, "tree_subtree_transforms":true,"connected_component_transforms":true,"rectangle_group_transforms":true,"follow_all_descendants_transforms":true,"connection_line_real_controls":true,"connection_line_width_px":lines.width,"connection_line_patterns":["solid","dashed","dotted"],"connection_line_effects":["still","flow","pulse"],"connection_line_settings_preserve_history":true,"undo_redo": true, "xml_roundtrip": true, "input_mode": "内部真实 UI/画布输入注入，不是系统键鼠或输入法验收"});
            std::fs::write(
                "target/transforms-smoke.json",
                serde_json::to_vec_pretty(&report).unwrap(),
            )
            .unwrap();
            describe(
                &mut showcase,
                "任意角度、自由双点连接、树/连通/框选/后代整组变换与连线样式全部通过；撤销重做及 XML 往返一致",
            );
            info!(
                "旋转镜像与自由模式窗口自测通过：真实控件、16° 吸附、122° 双镜像拖拽、自由双点连/断、多选及 XML 往返"
            );
            exit.write(AppExit::Success);
        }
        _ => return,
    }
    state.phase += 1;
    if state.phase >= 69 {
        info!("整组变换专项进入阶段 {}", state.phase);
    }
}
