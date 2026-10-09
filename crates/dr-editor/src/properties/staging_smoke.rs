use super::*;
use std::time::Instant;

#[derive(Resource)]
pub(crate) struct Captured;

#[derive(Default)]
pub(crate) struct State {
    phase: u8,
    input: crate::egui_ui::UiTestInput,
    before: Option<Ship>,
    after: Option<Ship>,
    key: Option<PartKey>,
    stage: usize,
    activation: usize,
    moved_before: bool,
    actual_ship: bool,
    parts: usize,
    connections: usize,
    stages: usize,
    activations: usize,
    timer: Option<Instant>,
    open_ms: f64,
    edit_ms: f64,
    apply_ms: f64,
    undo_ms: f64,
    redo_ms: f64,
}

fn staging_target(ship: &Ship) -> Option<(PartKey, usize, usize, bool, usize, usize)> {
    ship.keyed_parts().find_map(|(key, part)| {
        let staging = part.pod.as_ref()?.staging.as_ref()?;
        let stage = staging
            .steps
            .iter()
            .rposition(|step| !step.activations.is_empty())?;
        let activation = staging.steps[stage].activations.len() - 1;
        let moved = staging.steps[stage].activations[activation].moved;
        let activations = staging
            .steps
            .iter()
            .map(|step| step.activations.len())
            .sum();
        Some((
            key,
            stage,
            activation,
            moved,
            staging.steps.len(),
            activations,
        ))
    })
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
    showcase: Option<Res<demo::Showcase>>,
) {
    if !mode.staging || mode.started.elapsed().as_secs() < 3 {
        return;
    }
    assert!(
        demo::within_timeout(showcase.as_deref(), mode.started, 180),
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
            state.actual_ship = staging_target(&document.ship).is_some();
            if !state.actual_ship {
                let mut ship = new_ship(&document.catalog);
                let kind = document.catalog.get("detacher-1").unwrap();
                ship.parts
                    .extend((2..=17).map(|id| kind.instantiate(id, (id as f64 * 3.0, 0.0))));
                ship.parts[0].pod.as_mut().unwrap().staging = Some(StagingState {
                    current_stage: 32,
                    steps: (0..64)
                        .map(|_| StageStep {
                            activations: (2..=17)
                                .map(|id| Activation { id, moved: false })
                                .collect(),
                        })
                        .collect(),
                });
                document.ship = ship;
                document.saved_ship = document.ship.clone();
                document.refresh();
            }
            let (key, stage, activation, moved, stages, activations) =
                staging_target(&document.ship).expect("分级自测需要至少一个实际动作");
            state.key = Some(key);
            state.stage = stage;
            state.activation = activation;
            state.moved_before = moved;
            state.parts = document.ship.all_parts().count();
            state.connections = document.ship.all_connections().count();
            state.stages = stages;
            state.activations = activations;
            document.history = EditorHistory::default();
            document.select_only(Some(key));
            state.before = Some(document.ship.clone());
            state.timer = Some(Instant::now());
            act(&Action::Open, &mut inspector, &mut document);
        }
        1 => {
            let draft = inspector.draft.as_ref().expect("属性草稿未打开");
            assert_eq!(draft.key, state.key.unwrap());
            assert_eq!(draft.staging.as_ref().unwrap().steps.len(), state.stages);
            state.open_ms = state.timer.take().unwrap().elapsed().as_secs_f64() * 1000.0;
            state.timer = Some(Instant::now());
        }
        2 => {
            let action = Action::Moved(state.stage, state.activation);
            if !state.input.click(action, &hits, &mut input) {
                return;
            }
        }
        3 => {
            let draft = inspector.draft.as_ref().unwrap();
            assert_eq!(
                draft.staging.as_ref().unwrap().steps[state.stage].activations[state.activation]
                    .moved,
                !state.moved_before
            );
            assert_eq!(state.before.as_ref(), Some(&document.ship));
            state.edit_ms = state.timer.take().unwrap().elapsed().as_secs_f64() * 1000.0;
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
            state.timer = Some(Instant::now());
            if !state.input.click(Action::Apply, &hits, &mut input) {
                return;
            }
        }
        5 => {
            assert!(inspector.draft.is_none(), "{}", inspector.error);
            state.apply_ms = state.timer.take().unwrap().elapsed().as_secs_f64() * 1000.0;
            let mut expected = state.before.as_ref().unwrap().clone();
            expected
                .part_at_mut(state.key.unwrap())
                .unwrap()
                .pod
                .as_mut()
                .unwrap()
                .staging
                .as_mut()
                .unwrap()
                .steps[state.stage]
                .activations[state.activation]
                .moved = !state.moved_before;
            assert_eq!(document.ship, expected);
            state.after = Some(expected);
            state.timer = Some(Instant::now());
            keys.press(KeyCode::ControlLeft);
            keys.press(KeyCode::KeyZ);
        }
        6 => {
            state.undo_ms = state.timer.take().unwrap().elapsed().as_secs_f64() * 1000.0;
            assert_eq!(state.before.as_ref(), Some(&document.ship));
            assert!(!document.history.can_undo());
            keys.reset_all();
            state.timer = Some(Instant::now());
            keys.press(KeyCode::ControlLeft);
            keys.press(KeyCode::KeyY);
        }
        7 => {
            state.redo_ms = state.timer.take().unwrap().elapsed().as_secs_f64() * 1000.0;
            assert_eq!(state.after.as_ref(), Some(&document.ship));
            keys.reset_all();
            let save_started = Instant::now();
            save_ship("target/staging-smoke.xml", &document.ship).unwrap();
            let save_ms = save_started.elapsed().as_secs_f64() * 1000.0;
            let load_started = Instant::now();
            assert_eq!(
                load_ship("target/staging-smoke.xml").unwrap(),
                document.ship
            );
            let load_ms = load_started.elapsed().as_secs_f64() * 1000.0;
            let report = format!(
                concat!(
                    "{{\"actual_ship\":{},\"parts\":{},\"connections\":{},",
                    "\"stages\":{},\"activations\":{},\"edited_stage\":{},",
                    "\"edited_activation\":{},\"open_ms\":{:.3},\"edit_ms\":{:.3},",
                    "\"apply_ms\":{:.3},\"undo_ms\":{:.3},\"redo_ms\":{:.3},",
                    "\"save_ms\":{:.3},\"load_ms\":{:.3}}}\n"
                ),
                state.actual_ship,
                state.parts,
                state.connections,
                state.stages,
                state.activations,
                state.stage,
                state.activation,
                state.open_ms,
                state.edit_ms,
                state.apply_ms,
                state.undo_ms,
                state.redo_ms,
                save_ms,
                load_ms,
            );
            std::fs::write("target/editor-staging-performance.json", &report).unwrap();
            info!("复杂分级 UI 自测通过：{}", report.trim());
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
