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
}

#[derive(Resource, Default)]
pub(crate) struct Captured(u8);

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

#[allow(clippy::too_many_arguments)]
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
) {
    if !mode.transforms || mode.started.elapsed().as_secs() < 3 {
        return;
    }
    assert!(
        mode.started.elapsed().as_secs() < 180,
        "旋转镜像与自由模式窗口自测超时，阶段 {}",
        state.phase
    );
    let (Ok(mut window), Ok(mut input)) = (windows.single_mut(), inputs.single_mut()) else {
        return;
    };
    if state.driver.tick(&mut input) {
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
            save_ship("target/transforms-smoke.xml", &document.ship).unwrap();
            assert_eq!(
                load_ship("target/transforms-smoke.xml").unwrap(),
                document.ship
            );
            let report = serde_json::json!({"completed": true, "assisted_overlap_ratio": state.overlap, "manual_connection_distance": state.manual_distance, "rotation_steps_degrees": [90,15,1], "drag_preview_degrees": 122, "drag_mirrors": ["x","y"], "free_mode_real_checkbox": true, "precision_real_buttons": true, "free_drag_preserved_connection": true, "free_overlapping_placement_without_auto_connection": true, "manual_connect_and_disconnect": true, "multiselect_mirrors_and_rotation": true, "undo_redo": true, "xml_roundtrip": true, "input_mode": "内部真实 UI/画布输入注入，不是系统键鼠或输入法验收"});
            std::fs::write(
                "target/transforms-smoke.json",
                serde_json::to_vec_pretty(&report).unwrap(),
            )
            .unwrap();
            describe(
                &mut showcase,
                "任意角度吸附、组合镜像、自由双点连接、多选变换、原子撤销重做及 XML 往返全部通过",
            );
            info!(
                "旋转镜像与自由模式窗口自测通过：真实控件、16° 吸附、122° 双镜像拖拽、自由双点连/断、多选及 XML 往返"
            );
            exit.write(AppExit::Success);
        }
        _ => return,
    }
    state.phase += 1;
}
