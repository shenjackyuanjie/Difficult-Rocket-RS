use super::*;

#[derive(Resource)]
pub(crate) struct PreviewCaptured;

#[derive(Resource)]
pub(crate) struct DragRotationCaptured;

#[derive(Default)]
pub(crate) struct State {
    phase: u8,
    before: Option<Ship>,
    after: Option<Ship>,
    pointer: Option<Vec2>,
    camera_before: Option<Vec2>,
    small_motion: bool,
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
    rotation_captured: Option<Res<DragRotationCaptured>>,
    ui: (Res<panels::egui_panel::UiState>, Res<panels::UiPointer>),
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
            if !state.small_motion {
                point(0.1, 0.13);
                state.small_motion = true;
                state.pointer = pointer;
                return;
            }
            assert!((drag.delta().0 - 0.1).abs() < 1e-5);
            assert!((drag.delta().1 - 0.13).abs() < 1e-5);
            point(1.0, 1.0);
        }
        7 => {
            assert_eq!(state.before.as_ref(), Some(&document.ship));
            for (visual, transform) in &visuals {
                if visual.id < 3 {
                    assert!((transform.translation.x - 60.0).abs() < 1e-3);
                }
            }
            mouse.release(MouseButton::Left);
        }
        8 => {
            assert!(document.selected_keys().is_empty());
            assert!(drag.id.is_none() && drag.members.is_empty() && drag.command.is_none());
            assert!(
                (document.ship.parts[0].x - 1.0).abs() < 1e-5,
                "{}",
                document.status
            );
            assert!((document.ship.parts[1].x - 1.0).abs() < 1e-5);
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
            let first_key = document.ship.keyed_parts().next().map(|(key, _)| key);
            document.select_only(first_key);
            point(3.0, -3.0);
            mouse.press(MouseButton::Left);
        }
        34 => {
            assert!(document.selected_keys().is_empty(), "空白左键没有取消选择");
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
            use bevy::render::view::screenshot::{Screenshot, save_to_disk};
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk("target/editor-selection-smoke.png"));
        }
        // 保留上面的原始阶段；以下全部通过窗口坐标、鼠标按住/松手和按键驱动。
        42 => {
            keys.reset_all();
            mouse.reset_all();
            options.box_select_button = view::BoxSelectButton::Middle;
            options.follow_children = false;
            point(0.0, -2.0);
            keys.press(KeyCode::Home);
        }
        43 => {
            assert!(camera.single().unwrap().translation.truncate().length() < 1e-4);
            keys.reset_all();
            let kind = document.catalog.get("detacher-1").unwrap();
            assert!(!kind.disable_editor_rotation);
            document.ship = Ship {
                parts: vec![
                    kind.instantiate(1, (0.0, 0.0)),
                    kind.instantiate(2, (0.0, 0.5)),
                    kind.instantiate(3, (0.0, 1.0)),
                    kind.instantiate(4, (2.0, 0.0)),
                    kind.instantiate(5, (-3.0, 0.0)),
                ],
                // 4 是外部父节点，而不是 1 的后代；5 是独立分量。
                connections: vec![
                    normal_connection(1, 2),
                    normal_connection(2, 3),
                    normal_connection(4, 1),
                ],
                ..Default::default()
            };
            state.before = Some(document.ship.clone());
            reset_drag_fixture(&mut document, state.before.as_ref().unwrap());
            point(0.0, 0.0);
            mouse.press(MouseButton::Left);
        }
        44 => {
            assert_eq!(drag.keys().len(), 1);
            point(1.0, -1.0);
            keys.press(KeyCode::KeyR);
        }
        45 => {
            assert_eq!(drag.turns, 1);
            assert_preview_unchanged(&document, state.before.as_ref().unwrap());
            assert_drag_visuals(&document, &drag, &visuals);
            let root = drag.pose(drag.id.unwrap(), &document.ship.parts[0]);
            assert_part_pose(&root, 1.0, -1.0, 1);
            use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk};
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk("target/editor-drag-rotation.png"))
                .observe(|_: On<ScreenshotCaptured>, mut commands: Commands| {
                    commands.insert_resource(DragRotationCaptured);
                });
        }
        46 => {
            // R 一直按住也只能旋转一次，截图完成前继续维持实际预览。
            assert_eq!(drag.turns, 1, "按住 R 重复触发拖拽旋转");
            assert_preview_unchanged(&document, state.before.as_ref().unwrap());
            if rotation_captured.is_none() {
                return;
            }
            keys.reset_all();
            keys.press(KeyCode::Escape);
        }
        47 => {
            assert!(drag.id.is_none() && drag.members.is_empty() && drag.command.is_none());
            assert_eq!(drag.turns, 0);
            assert_preview_unchanged(&document, state.before.as_ref().unwrap());
            mouse.release(MouseButton::Left);
            keys.reset_all();
        }
        48 => {
            // 取消保留原来的父节点选择，再用 Shift 真正选择子节点。
            point(0.0, 0.5);
            keys.press(KeyCode::ShiftLeft);
            mouse.press(MouseButton::Left);
        }
        49 => {
            assert_eq!(document.selected_keys().len(), 2);
            mouse.release(MouseButton::Left);
            keys.reset_all();
        }
        50 => {
            point(0.0, 0.0);
            mouse.press(MouseButton::Left);
        }
        51 => {
            assert_eq!(drag.keys().len(), 2);
            point(2.0, 0.0);
            keys.press(KeyCode::KeyR);
        }
        52 => {
            assert_eq!(drag.turns, 1);
            assert!(drag.blocked, "回归落点必须与外部部件重叠，不能被吸附改写");
            assert_preview_unchanged(&document, state.before.as_ref().unwrap());
            assert_drag_visuals(&document, &drag, &visuals);
            assert_part_pose(
                &drag.pose(drag.id.unwrap(), &document.ship.parts[0]),
                2.0,
                0.0,
                1,
            );
            keys.reset_all();
            mouse.release(MouseButton::Left);
        }
        53 => {
            assert_part_pose(&document.ship.parts[0], 2.0, 0.0, 1);
            assert_part_pose(&document.ship.parts[1], 1.5, 0.0, 1);
            assert_eq!(
                &document.ship.parts[2..],
                &state.before.as_ref().unwrap().parts[2..]
            );
            assert_eq!(document.ship.connections, vec![normal_connection(1, 2)]);
            assert_committed_once(&document, &drag);
            state.after = Some(document.ship.clone());
            keys.press(KeyCode::ControlLeft);
            keys.press(KeyCode::KeyZ);
        }
        54 => {
            assert_undone_once(&document, state.before.as_ref().unwrap());
            keys.reset_all();
            keys.press(KeyCode::ControlLeft);
            keys.press(KeyCode::KeyY);
        }
        55 => {
            assert_redone_once(&document, state.after.as_ref().unwrap());
            keys.reset_all();
            reset_drag_fixture(&mut document, state.before.as_ref().unwrap());
            options.follow_children = true;
        }
        56 => {
            point(0.0, 0.0);
            mouse.press(MouseButton::Left);
        }
        57 => {
            assert_eq!(document.selected_keys().len(), 1);
            let ids: Vec<_> = drag.keys().iter().map(|key| key.id).collect();
            assert_eq!(
                ids,
                vec![1, 2, 3],
                "子节点跟随必须含孙节点，但不能包含外部父节点"
            );
            point(2.0, 0.0);
        }
        58 => {
            assert!(drag.blocked);
            assert_preview_unchanged(&document, state.before.as_ref().unwrap());
            assert_drag_visuals(&document, &drag, &visuals);
            mouse.release(MouseButton::Left);
        }
        59 => {
            for (index, y) in [0.0, 0.5, 1.0].into_iter().enumerate() {
                assert_part_pose(&document.ship.parts[index], 2.0, y, 0);
            }
            assert_eq!(
                &document.ship.parts[3..],
                &state.before.as_ref().unwrap().parts[3..]
            );
            assert_eq!(
                document.ship.connections,
                vec![normal_connection(1, 2), normal_connection(2, 3)]
            );
            assert_committed_once(&document, &drag);
            state.after = Some(document.ship.clone());
            keys.press(KeyCode::ControlLeft);
            keys.press(KeyCode::KeyZ);
        }
        60 => {
            assert_undone_once(&document, state.before.as_ref().unwrap());
            keys.reset_all();
            keys.press(KeyCode::ControlLeft);
            keys.press(KeyCode::KeyY);
        }
        61 => {
            assert_redone_once(&document, state.after.as_ref().unwrap());
            keys.reset_all();
            reset_drag_fixture(&mut document, state.before.as_ref().unwrap());
            options.follow_children = false;
        }
        62 => {
            point(0.0, 0.0);
            mouse.press(MouseButton::Left);
        }
        63 => {
            assert_eq!(drag.keys().len(), 1, "关闭子节点跟随只能拖父节点");
            point(2.0, 0.0);
        }
        64 => {
            assert!(drag.blocked);
            assert_preview_unchanged(&document, state.before.as_ref().unwrap());
            assert_drag_visuals(&document, &drag, &visuals);
            mouse.release(MouseButton::Left);
        }
        65 => {
            assert_part_pose(&document.ship.parts[0], 2.0, 0.0, 0);
            assert_eq!(
                &document.ship.parts[1..],
                &state.before.as_ref().unwrap().parts[1..]
            );
            assert_eq!(document.ship.connections, vec![normal_connection(2, 3)]);
            assert_committed_once(&document, &drag);
            state.after = Some(document.ship.clone());
            keys.press(KeyCode::ControlLeft);
            keys.press(KeyCode::KeyZ);
        }
        66 => {
            assert_undone_once(&document, state.before.as_ref().unwrap());
            keys.reset_all();
            keys.press(KeyCode::ControlLeft);
            keys.press(KeyCode::KeyY);
        }
        67 => {
            assert_redone_once(&document, state.after.as_ref().unwrap());
            keys.reset_all();
            reset_drag_fixture(&mut document, state.before.as_ref().unwrap());
        }
        68 => {
            point(0.0, 0.0);
            mouse.press(MouseButton::Left);
        }
        69 => {
            assert!(drag.id.is_some());
            // 左侧船体目录属于 UI，但不是右侧部件删除落区。
            let area =
                ui.0.areas
                    .iter()
                    .find(|rect| rect.center().x < window.width() / 2.0)
                    .expect("左侧目录必须存在");
            let center = area.center();
            assert!(!ui.0.palette_area.unwrap().contains(center));
            let scale = ui.0.pixels_per_point / window.scale_factor();
            pointer = Some(Vec2::new(center.x, center.y) * scale);
            window.set_cursor_position(pointer);
        }
        70 => {
            assert!(
                ui.1.blocked && !ui.1.palette_drop,
                "非部件列表 UI 不得成为删除落区"
            );
            assert_preview_unchanged(&document, state.before.as_ref().unwrap());
            mouse.release(MouseButton::Left);
        }
        71 => {
            assert!(drag.id.is_none());
            assert_preview_unchanged(&document, state.before.as_ref().unwrap());
            point(0.0, 0.0);
            mouse.press(MouseButton::Left);
        }
        72 => {
            assert_eq!(drag.keys().len(), 1);
            let center =
                ui.0.palette_area
                    .expect("右侧部件列表删除落区必须存在")
                    .center();
            let scale = ui.0.pixels_per_point / window.scale_factor();
            pointer = Some(Vec2::new(center.x, center.y) * scale);
            window.set_cursor_position(pointer);
        }
        73 => {
            assert!(ui.1.palette_drop, "实际鼠标没有进入右侧部件列表删除落区");
            assert_preview_unchanged(&document, state.before.as_ref().unwrap());
            mouse.release(MouseButton::Left);
        }
        74 => {
            assert_eq!(
                document.ship.parts,
                vec![state.before.as_ref().unwrap().parts[4].clone()],
                "落入部件列表必须删除整个无向分量，包括父、子、孙，但保留独立分量"
            );
            assert!(document.ship.connections.is_empty());
            assert_committed_once(&document, &drag);
            state.after = Some(document.ship.clone());
            point(0.0, -2.0);
            keys.press(KeyCode::ControlLeft);
            keys.press(KeyCode::KeyZ);
        }
        75 => {
            assert_undone_once(&document, state.before.as_ref().unwrap());
            keys.reset_all();
            keys.press(KeyCode::ControlLeft);
            keys.press(KeyCode::KeyY);
        }
        76 => {
            assert_redone_once(&document, state.after.as_ref().unwrap());
            keys.reset_all();
            use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured};
            // 等最终帧读回后再退出，确保之前的两个观察截图也已完成写入。
            commands.spawn(Screenshot::primary_window())
                .observe(|_: On<ScreenshotCaptured>, mut exit: MessageWriter<AppExit>| {
                    info!("多选交互自测通过：原有选择/框选/粘贴/失焦/XML 阶段，拖拽 R 旋转预览不改文档与 dirty、按住不重复、取消、重叠落点位置角度保留、跟随后代开关与内部保留外部断连、右侧部件列表删除整个无向分量及非列表 UI 不删除、一次撤销重做");
                    exit.write(AppExit::Success);
                });
        }
        _ => return,
    }
    state.pointer = pointer;
    state.phase += 1;
}

fn normal_connection(parent: i64, child: i64) -> Connection {
    Connection::Normal {
        parent,
        child,
        parent_attach: 1,
        child_attach: 2,
    }
}

fn reset_drag_fixture(document: &mut EditorDocument, ship: &Ship) {
    document.ship = ship.clone();
    document.saved_ship = ship.clone();
    document.history = EditorHistory::default();
    document.clear_selection();
    document.refresh();
    assert!(!document.dirty);
}

fn assert_preview_unchanged(document: &EditorDocument, before: &Ship) {
    assert_eq!(&document.ship, before, "拖拽预览或取消不得修改文档");
    assert!(!document.dirty, "拖拽预览或取消不得标脏");
    assert_eq!(document.history.undo_len(), 0);
    assert_eq!(document.history.redo_len(), 0);
}

fn assert_part_pose(part: &Part, x: f64, y: f64, turns: i32) {
    assert!(
        (part.x - x).abs() < 1e-5,
        "部件 {} 的 x：{} != {x}",
        part.id,
        part.x
    );
    assert!(
        (part.y - y).abs() < 1e-5,
        "部件 {} 的 y：{} != {y}",
        part.id,
        part.y
    );
    let angle = f64::from(turns) * std::f64::consts::FRAC_PI_2;
    assert!(
        (part.angle - angle).abs() < 1e-5,
        "部件 {} 的角度没有保留",
        part.id
    );
    assert_eq!(part.editor_angle, turns);
}

fn assert_drag_visuals(
    document: &EditorDocument,
    drag: &DragState,
    visuals: &Query<(&render::PartVisual, &Transform)>,
) {
    let mut checked = 0;
    for (key, part) in document
        .ship
        .keyed_parts()
        .filter(|(key, _)| drag.contains(*key))
    {
        let pose = drag.pose(key, part);
        let (_, transform) = visuals
            .iter()
            .find(|(visual, _)| visual.id == part.id)
            .expect("拖拽部件必须有实际渲染实体");
        assert!((transform.translation.x - pose.x as f32 * 60.0).abs() < 1e-3);
        assert!((transform.translation.y - pose.y as f32 * 60.0).abs() < 1e-3);
        let expected = Quat::from_rotation_z(pose.angle as f32);
        assert!(
            transform.rotation.abs_diff_eq(expected, 1e-5),
            "渲染角度没有跟随拖拽旋转"
        );
        checked += 1;
    }
    assert_eq!(checked, drag.keys().len());
}

fn assert_committed_once(document: &EditorDocument, drag: &DragState) {
    assert!(document.dirty);
    assert_eq!(
        document.history.undo_len(),
        1,
        "一次松手必须只有一个历史步骤"
    );
    assert_eq!(document.history.redo_len(), 0);
    assert!(document.selected_keys().is_empty());
    assert!(drag.id.is_none() && drag.members.is_empty() && drag.command.is_none());
    assert_eq!(drag.turns, 0);
}

fn assert_undone_once(document: &EditorDocument, before: &Ship) {
    assert_eq!(&document.ship, before, "一次撤销必须恢复部件姿态及所有连接");
    assert!(!document.dirty);
    assert_eq!(document.history.undo_len(), 0);
    assert_eq!(document.history.redo_len(), 1);
}

fn assert_redone_once(document: &EditorDocument, after: &Ship) {
    assert_eq!(&document.ship, after, "一次重做必须恢复整次拖拽或删除");
    assert!(document.dirty);
    assert_eq!(document.history.undo_len(), 1);
    assert_eq!(document.history.redo_len(), 0);
}
