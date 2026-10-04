use super::*;

#[derive(Resource)]
pub(crate) struct Captured;

#[derive(Default)]
pub(crate) struct State {
    phase: u8,
    input: crate::egui_ui::UiTestInput,
    before: Option<Ship>,
    after: Option<Ship>,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn run(
    mode: Res<SmokeTest>,
    mut state: Local<State>,
    mut document: ResMut<EditorDocument>,
    mut inspector: ResMut<Inspector>,
    hits: Res<crate::egui_ui::UiHits>,
    mut inputs: Query<&mut bevy_egui::EguiInput, With<bevy_egui::PrimaryEguiContext>>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    captured: Option<Res<Captured>>,
    mut commands: Commands,
) {
    if !mode.staging || mode.started.elapsed().as_secs() < 3 {
        return;
    }
    assert!(
        mode.started.elapsed().as_secs() < 90,
        "复杂分级交互自测超时"
    );
    let Ok(mut input) = inputs.single_mut() else {
        return;
    };
    if state.input.tick(&mut input) {
        return;
    }
    match state.phase {
        0 => {
            let mut ship = new_ship(&document.catalog);
            let kind = document.catalog.get("detacher-1").unwrap();
            ship.parts
                .extend((2..=17).map(|id| kind.instantiate(id, (id as f64 * 3.0, 0.0))));
            ship.parts[0].pod.as_mut().unwrap().staging = Some(StagingState {
                current_stage: 32,
                steps: (0..64)
                    .map(|_| StageStep {
                        activations: (2..=17).map(|id| Activation { id, moved: false }).collect(),
                    })
                    .collect(),
            });
            document.ship = ship;
            document.saved_ship = document.ship.clone();
            document.history = EditorHistory::default();
            document.clear_selection();
            document.refresh();
            state.before = Some(document.ship.clone());
            act(&Action::Open, &mut inspector, &mut document);
        }
        1 => {
            assert_eq!(
                inspector
                    .draft
                    .as_ref()
                    .unwrap()
                    .staging
                    .as_ref()
                    .unwrap()
                    .steps
                    .len(),
                64
            );
        }
        2 => {
            if !state.input.click(Action::Moved(63, 15), &hits, &mut input) {
                return;
            }
        }
        3 => {
            let draft = inspector.draft.as_ref().unwrap();
            assert!(draft.staging.as_ref().unwrap().steps[63].activations[15].moved);
            assert_eq!(state.before.as_ref(), Some(&document.ship));
            use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk};
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk("target/editor-staging-draft.png"))
                .observe(|_: On<ScreenshotCaptured>, mut commands: Commands| {
                    commands.insert_resource(Captured);
                });
        }
        4 => {
            if captured.is_none() {
                return;
            }
            if !state.input.click(Action::Apply, &hits, &mut input) {
                return;
            }
        }
        5 => {
            assert!(inspector.draft.is_none(), "{}", inspector.error);
            let mut expected = state.before.as_ref().unwrap().clone();
            expected.parts[0]
                .pod
                .as_mut()
                .unwrap()
                .staging
                .as_mut()
                .unwrap()
                .steps[63]
                .activations[15]
                .moved = true;
            assert_eq!(document.ship, expected);
            state.after = Some(expected);
            keys.press(KeyCode::ControlLeft);
            keys.press(KeyCode::KeyZ);
        }
        6 => {
            assert_eq!(state.before.as_ref(), Some(&document.ship));
            assert!(!document.history.can_undo());
            keys.reset_all();
            keys.press(KeyCode::ControlLeft);
            keys.press(KeyCode::KeyY);
        }
        7 => {
            assert_eq!(state.after.as_ref(), Some(&document.ship));
            keys.reset_all();
            save_ship("target/staging-smoke.xml", &document.ship).unwrap();
            assert_eq!(
                load_ship("target/staging-smoke.xml").unwrap(),
                document.ship
            );
            info!(
                "复杂分级自测通过：64 级 1024 个动作、末项滚动编辑、草稿隔离、原子应用与撤销重做及 XML 往返"
            );
            use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk};
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk("target/editor-staging-smoke.png"))
                .observe(
                    |_: On<ScreenshotCaptured>, mut exit: MessageWriter<AppExit>| {
                        exit.write(AppExit::Success);
                    },
                );
        }
        _ => return,
    }
    state.phase += 1;
}
