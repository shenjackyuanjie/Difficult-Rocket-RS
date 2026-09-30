use super::*;

#[derive(Resource)]
pub(crate) struct Captured;

#[derive(Default)]
pub(crate) struct State {
    phase: u8,
    delay: u8,
    before: Option<Ship>,
    after: Option<Ship>,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn run(
    mode: Res<SmokeTest>,
    mut state: Local<State>,
    mut document: ResMut<EditorDocument>,
    inspector: Res<Inspector>,
    mut scrolling: Query<(&ComputedNode, &mut ScrollPosition), With<InspectorScroll>>,
    mut buttons: Query<(&Action, &mut Interaction)>,
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
    let mut press = |action: Action| {
        let (_, mut interaction) = buttons
            .iter_mut()
            .find(|(button, _)| **button == action)
            .unwrap_or_else(|| panic!("未找到按钮 {action:?}"));
        *interaction = Interaction::Pressed;
    };
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
            press(Action::Open);
        }
        1 => {
            state.delay += 1;
            if state.delay < 5 {
                return;
            }
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
            let (node, mut scroll) = scrolling.single_mut().unwrap();
            scroll.y = (node.content_size().y - node.size().y) * node.inverse_scale_factor();
            assert!(scroll.y > 10000.0);
        }
        2 => press(Action::Moved(63, 15)),
        3 => {
            let draft = inspector.draft.as_ref().unwrap();
            assert!(draft.staging.as_ref().unwrap().steps[63].activations[15].moved);
            assert_eq!(state.before.as_ref(), Some(&document.ship));
            assert!(scrolling.single().unwrap().1.y > 10000.0);
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
            press(Action::Apply);
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
