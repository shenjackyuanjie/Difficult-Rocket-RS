use super::*;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

#[derive(Default)]
pub(crate) struct State {
    phase: u8,
    before: Option<Ship>,
    after: Option<Ship>,
    new_id: i64,
    input: crate::egui_ui::UiTestInput,
    captured: Option<Arc<AtomicBool>>,
}

/// 用 Heronb 的真实发动机/油箱重号验证修复 UI，不自动修改源文件。
#[allow(clippy::too_many_arguments)]
pub(crate) fn run(
    mode: Res<SmokeTest>,
    mut state: Local<State>,
    inspector: Res<Inspector>,
    mut document: ResMut<EditorDocument>,
    hits: Res<crate::egui_ui::UiHits>,
    mut inputs: Query<&mut bevy_egui::EguiInput, With<bevy_egui::PrimaryEguiContext>>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut cameras: Query<(&mut Transform, &mut Projection), With<Camera2d>>,
    mut commands: Commands,
    showcase: Option<Res<demo::Showcase>>,
) {
    if !mode.repair || mode.started.elapsed().as_secs() < 3 {
        return;
    }
    assert!(
        demo::within_timeout(showcase.as_deref(), mode.started, 60),
        "重复编号修复自测超时"
    );
    let Ok(mut input) = inputs.single_mut() else {
        return;
    };
    if state.input.tick(&mut input) {
        return;
    }
    let id = 1_200_404;
    match state.phase {
        0 => {
            let key = PartKey::new(0, id, 1);
            assert_eq!(
                document
                    .ship
                    .part_at(key)
                    .expect("请使用 --ship ../Difficult-Rocket/assets/ships/Heronb.xml")
                    .part_type,
                "fueltank-0"
            );
            state.before = Some(document.ship.clone());
            document.selected = Some(key);
            keys.press(KeyCode::F2);
            if let Ok((mut camera, mut projection)) = cameras.single_mut() {
                camera.translation.x = -150.0;
                camera.translation.y = -2900.0;
                if let Projection::Orthographic(projection) = &mut *projection {
                    projection.scale = 2.0;
                }
            }
        }
        1 => {
            keys.reset_all();
            if !state.input.click(Action::OpenRepair, &hits, &mut input) {
                return;
            }
        }
        2 => {
            assert_eq!(inspector.repair.as_ref().unwrap().unassigned(), 4);
            if !state.input.click(Action::Apply, &hits, &mut input) {
                return;
            }
        }
        3 => {
            assert!(inspector.is_open());
            assert!(!inspector.error.is_empty());
            assert_eq!(state.before.as_ref(), Some(&document.ship));
            if !state
                .input
                .click(Action::RepairTarget(0), &hits, &mut input)
            {
                return;
            }
        }
        4 | 5 => {
            if !state
                .input
                .click(Action::RepairTarget(1), &hits, &mut input)
            {
                return;
            }
        }
        6 | 7 => {
            if !state
                .input
                .click(Action::RepairTarget(2), &hits, &mut input)
            {
                return;
            }
        }
        8 => {
            if !state
                .input
                .click(Action::RepairTarget(3), &hits, &mut input)
            {
                return;
            }
        }
        9 => {
            let repair = inspector.repair.as_ref().unwrap();
            assert_eq!(
                repair.unassigned(),
                0,
                "引用分配：{:?}",
                repair.references()
            );
            state.new_id = repair.new_ids()[1];
            assert_eq!(state.before.as_ref(), Some(&document.ship));
            use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk};
            let captured = Arc::new(AtomicBool::new(false));
            state.captured = Some(captured.clone());
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk("target/editor-repair-draft.png"))
                .observe(move |_: On<ScreenshotCaptured>| {
                    captured.store(true, Ordering::Release);
                });
        }
        10 => {
            if !state.captured.as_ref().unwrap().load(Ordering::Acquire) {
                return;
            }
            if !state.input.click(Action::Apply, &hits, &mut input) {
                return;
            }
        }
        11 => {
            assert!(!inspector.is_open(), "{}", inspector.error);
            let mut expected = state.before.as_ref().unwrap().clone();
            expected
                .parts
                .iter_mut()
                .filter(|part| part.id == id)
                .nth(1)
                .unwrap()
                .id = state.new_id;
            for connection in &mut expected.connections {
                if let Connection::Normal { parent, child, .. } = connection {
                    if *parent == 1_200_260 && *child == id {
                        *child = state.new_id;
                    }
                    if *parent == id && *child == 1_200_276 {
                        *parent = state.new_id;
                    }
                }
            }
            assert_eq!(document.ship, expected);
            state.after = Some(expected);
            save_ship("target/repair-smoke.xml", &document.ship).unwrap();
            assert_eq!(load_ship("target/repair-smoke.xml").unwrap(), document.ship);
            keys.press(KeyCode::ControlLeft);
            keys.press(KeyCode::KeyZ);
        }
        12 => {
            assert_eq!(state.before.as_ref(), Some(&document.ship));
            assert!(!document.dirty);
            keys.reset_all();
            keys.press(KeyCode::ControlLeft);
            keys.press(KeyCode::KeyY);
        }
        13 => {
            assert_eq!(state.after.as_ref(), Some(&document.ship));
            keys.reset_all();
            assert!(
                document.execute(EditorCommand::Delete(state.new_id)),
                "{}",
                document.status
            );
            assert_eq!(
                document.ship.parts.len() + 1,
                state.after.as_ref().unwrap().parts.len()
            );
            assert_eq!(
                document.ship.group_part(0, id).unwrap().part_type,
                "engine-0"
            );
            assert!(document.ship.connections.iter().any(|c| c.touches(id)));
            assert!(
                !document
                    .ship
                    .connections
                    .iter()
                    .any(|c| c.touches(state.new_id))
            );
            assert!(document.undo());
        }
        14 => {
            assert_eq!(state.after.as_ref(), Some(&document.ship));
            use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk};
            commands.spawn(Screenshot::primary_window()).observe(save_to_disk("target/editor-repair-smoke.png"))
                .observe(|_: On<ScreenshotCaptured>, mut exit: MessageWriter<AppExit>| {
                    info!("重复编号修复自测通过：真实 Heronb 实例、逐引用分配、未分配拒绝、一次撤销重做、独立删除及 XML 往返");
                    exit.write(AppExit::Success);
                });
        }
        _ => return,
    }
    state.phase += 1;
}
