use crate::demo;
use crate::document::EditorDocument;
use crate::document::new_ship;
use crate::properties::*;
use crate::window::SmokeTest;
use dr_core::EditorHistory;
use dr_core::PartKey;
use dr_core::Ship;
use dr_core::load_ship;
use dr_core::save_ship;
use std::time::Instant;

#[derive(Resource)]
pub struct Captured;

#[derive(Default)]
pub struct State {
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
    preview: serde_json::Value,
    hover_phase: u8,
    preview_before_hover: Option<Preview>,
    hover_rect: Option<bevy_egui::egui::Rect>,
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

fn visible_activations(hits: &crate::egui_ui::UiHits) -> Option<[(usize, usize); 2]> {
    let mut bounds: Option<[(usize, usize); 2]> = None;
    for (action, _) in &hits.actions {
        if let Action::SelectActivation(stage, index) = action {
            let key = (*stage, *index);
            bounds = Some(match bounds {
                Some([first, last]) => [first.min(key), last.max(key)],
                None => [key, key],
            });
        }
    }
    bounds
}

/// 寻址只发送真实正文滚轮，不选择缩略图；命中只取正式控件的裁剪后记录。
fn seek_activation(
    target: (usize, usize),
    terminal_stage: bool,
    hits: &crate::egui_ui::UiHits,
    input: &mut bevy_egui::EguiInput,
) -> Option<bevy_egui::egui::Rect> {
    use bevy_egui::egui;
    if hits.scrolling {
        return None;
    }
    let body = hits.body.filter(|body| body.is_positive())?;
    if let Some((_, rect)) = hits.actions.iter().find(|(action, rect)| {
        *action == Action::SelectActivation(target.0, target.1)
            && rect.is_positive()
            && body.contains_rect(*rect)
    }) {
        return Some(*rect);
    }
    let bounds = visible_activations(hits);
    let up = bounds.is_some_and(|[first, _]| target < first);
    // 末级远离当前视口时快滚；进入目标附近后保留重叠，避免跳过折行缩略图。
    let pages =
        if terminal_stage && bounds.is_some_and(|[_, last]| target.0 > last.0.saturating_add(2)) {
            4.0
        } else {
            0.75
        };
    let delta = body.height() * pages * if up { 1.0 } else { -1.0 };
    input.0.events.extend([
        egui::Event::PointerMoved(body.center()),
        egui::Event::MouseWheel {
            phase: egui::TouchPhase::Move,
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, delta),
            modifiers: egui::Modifiers::NONE,
        },
    ]);
    None
}

#[allow(clippy::too_many_arguments)]
pub fn run(
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
        "复杂分级交互自测超时：phase={}，hover={}，正文={:?}，目标级/动作={}/{}，可见动作={:?}，滚动={}，已选={:?}",
        state.phase,
        state.hover_phase,
        hits.body,
        state.stage,
        state.activation,
        visible_activations(&hits),
        hits.scrolling,
        hits.selected_activation,
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
            // 在末项上先只移动指针、不按下；核对本帧船体高亮，再移开恢复原焦点。
            match state.hover_phase {
                0 => {
                    if let Some(rect) = seek_activation(
                        (state.stage, state.activation),
                        state.stage + 1 == state.stages,
                        &hits,
                        &mut input,
                    ) {
                        state.preview_before_hover = Some(inspector.preview);
                        input
                            .0
                            .events
                            .push(bevy_egui::egui::Event::PointerMoved(rect.center()));
                        state.hover_phase = 1;
                    }
                    return;
                }
                1 => {
                    assert_eq!(
                        inspector.hovered,
                        Some(Preview::Activation(state.stage, state.activation))
                    );
                    assert_eq!(Some(inspector.preview), state.preview_before_hover);
                    assert_eq!(state.before.as_ref(), Some(&document.ship));
                    assert!(!document.history.can_undo());
                    let draft = inspector.draft.as_ref().unwrap();
                    let id = draft.staging.as_ref().unwrap().steps[state.stage].activations
                        [state.activation]
                        .id;
                    let key = PartKey::new(draft.key.group, id, 0);
                    assert_eq!(inspector.preview_keys(&document), vec![key]);
                    let rect = hits
                        .targets
                        .iter()
                        .find(|(target, _)| *target == key)
                        .expect("仅悬停也必须实际绘制对应部件高亮")
                        .1;
                    let canvas = hits.dock.unwrap().canvas;
                    assert!(canvas.contains_rect(rect));
                    state.hover_rect = Some(rect);
                    input
                        .0
                        .events
                        .push(bevy_egui::egui::Event::PointerMoved(canvas.center()));
                    state.hover_phase = 2;
                    return;
                }
                2 => {
                    assert_eq!(inspector.hovered, None, "移开指针后不能保留旧悬停");
                    assert_eq!(Some(inspector.preview), state.preview_before_hover);
                    assert_eq!(state.before.as_ref(), Some(&document.ship));
                    assert!(!document.history.can_undo());
                    state.hover_phase = 3;
                }
                _ => {}
            }
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
            let dock = hits.dock.expect("分级必须是停靠面板，不能用整屏 Modal");
            assert!(
                !dock.panel.intersect(dock.canvas).is_positive(),
                "面板不能覆盖船体区域"
            );
            let target =
                draft.staging.as_ref().unwrap().steps[state.stage].activations[state.activation].id;
            let key = PartKey::new(draft.key.group, target, 0);
            assert_eq!(inspector.preview_keys(&document), vec![key]);
            let (_, rect) = hits
                .targets
                .iter()
                .find(|(drawn, _)| *drawn == key)
                .expect("修改的分级目标必须实际在船体上绘制标记");
            assert!(
                dock.canvas.contains_rect(*rect),
                "目标部件标记必须完整处于船体区域"
            );
            let rect_value =
                |rect: bevy_egui::egui::Rect| [rect.min.x, rect.min.y, rect.max.x, rect.max.y];
            state.preview = serde_json::json!({
                "panel_rect": rect_value(dock.panel), "canvas_rect": rect_value(dock.canvas),
                "target_rect": rect_value(*rect), "target_group": key.group, "target_id": key.id,
                "panel_overlaps_canvas": false, "edited_target_visible": true,
                "hover_target_visible": true, "hover_restored": state.hover_phase == 3,
                "hover_target_rect": rect_value(state.hover_rect.unwrap()),
            });
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
            let report = serde_json::json!({
                "actual_ship": state.actual_ship, "parts": state.parts,
                "connections": state.connections, "stages": state.stages,
                "activations": state.activations, "edited_stage": state.stage,
                "edited_activation": state.activation, "open_ms": state.open_ms,
                "edit_ms": state.edit_ms, "apply_ms": state.apply_ms,
                "undo_ms": state.undo_ms, "redo_ms": state.redo_ms,
                "save_ms": save_ms, "load_ms": load_ms, "preview": state.preview,
            });
            let report = serde_json::to_string_pretty(&report).unwrap();
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

#[cfg(test)]
mod tests;
