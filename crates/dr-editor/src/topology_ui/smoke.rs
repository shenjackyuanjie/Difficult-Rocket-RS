use super::*;
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk};
use bevy_egui::{EguiInput, PrimaryEguiContext};

#[derive(Default)]
pub struct State {
    phase: u8,
    driver: egui_ui::UiTestInput,
    before: Option<Ship>,
    delay: u8,
}
#[derive(Resource)]
pub struct Captured(pub u8);

fn click(
    action: Action,
    panel: &ConnectionEditor,
    input: &mut EguiInput,
    state: &mut State,
) -> bool {
    let Some((_, rect)) = panel
        .hits
        .iter()
        .find(|(candidate, _)| *candidate == action)
    else {
        return false;
    };
    state.driver.click_rect(*rect, input);
    true
}
fn capture(commands: &mut Commands, path: &'static str, phase: u8) {
    commands
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(path))
        .observe(move |_: On<ScreenshotCaptured>, mut commands: Commands| {
            commands.insert_resource(Captured(phase));
        });
}

fn target_viewport(presentation: bool) -> Vec2 {
    if presentation {
        Vec2::new(1920., 1080.)
    } else {
        Vec2::new(960., 640.)
    }
}

fn resize_for_smoke(window: &mut Window, presentation: bool) {
    if !presentation {
        let size = target_viewport(false);
        window.resolution.set(size.x, size.y);
    }
}

fn smoke_report(presentation: bool) -> serde_json::Value {
    let mut report = serde_json::json!({"steps":27, "input":"egui_injected", "ime_tested":false, "atomic_undo":true, "presentation":presentation, "minimum_window_tested":!presentation, "window":if presentation { "1920x1080" } else { "960x640" }});
    if !presentation {
        report["minimum_window"] = "960x640".into();
    }
    report
}

#[allow(clippy::too_many_arguments)]
pub fn run(
    mode: Res<SmokeTest>,
    mut state: Local<State>,
    mut panel: ResMut<ConnectionEditor>,
    mut document: ResMut<EditorDocument>,
    mut input: Query<&mut EguiInput, With<PrimaryEguiContext>>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut windows: Query<&mut Window, With<bevy::window::PrimaryWindow>>,
    captured: Option<Res<Captured>>,
    mut commands: Commands,
    showcase: Option<Res<crate::demo::Showcase>>,
) {
    if !mode.topology || mode.started.elapsed().as_secs() < 3 {
        return;
    }
    assert!(
        mode.started.elapsed().as_secs() < 80,
        "连接编辑窗口自测超时，阶段 {}",
        state.phase
    );
    let Ok(mut input) = input.single_mut() else {
        return;
    };
    let presentation = showcase.is_some_and(|showcase| showcase.presentation);
    keys.reset_all();
    if state.driver.tick(&mut input) {
        return;
    }
    if state.delay > 0 {
        state.delay -= 1;
        return;
    }
    macro_rules! press {
        ($action:expr) => {
            if !click($action, &panel, &mut input, &mut state) {
                return;
            }
        };
    }
    match state.phase {
        0 => {
            document.catalog=dr_core::catalog_from_xml(r#"<PartTypes><PartType id="p" name="连接节点" width="2" height="2"><AttachPoints>
            <AttachPoint location="LeftCenter"/><AttachPoint location="RightCenter"/><AttachPoint location="TopCenter"/><AttachPoint location="BottomCenter"/>
            </AttachPoints></PartType></PartTypes>"#).unwrap();
            let kind = document.catalog.get("p").unwrap();
            document.ship = Ship {
                name: "连接编辑验收".into(),
                parts: [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)]
                    .into_iter()
                    .enumerate()
                    .map(|(i, pos)| kind.instantiate(i as i64 + 1, pos))
                    .collect(),
                ..Ship::default()
            };
            document.ship.connections = vec![
                Connection::Normal {
                    parent: 1,
                    child: 2,
                    parent_attach: 2,
                    child_attach: 1,
                },
                Connection::Normal {
                    parent: 2,
                    child: 3,
                    parent_attach: 3,
                    child_attach: 4,
                },
                Connection::Normal {
                    parent: 3,
                    child: 4,
                    parent_attach: 1,
                    child_attach: 2,
                },
            ];
            document.saved_ship = document.ship.clone();
            document.history = EditorHistory::default();
            document.clear_selection();
            document.refresh();
            state.before = Some(document.ship.clone());
            keys.press(KeyCode::F6);
            state.delay = 3;
        }
        1 => {
            assert!(panel.open);
            press!(Action::Node(PartKey::new(0, 2, 0)));
        }
        2 => {
            assert_eq!(document.selected, Some(PartKey::new(0, 2, 0)));
            press!(Action::Subtree);
        }
        3 => {
            assert_eq!(document.selected_keys().len(), 3);
            capture(&mut commands, "target/editor-topology-tree.png", 1);
        }
        4 => {
            if captured.as_ref().is_none_or(|value| value.0 != 1) {
                return;
            }
            press!(Action::Delete);
        }
        5 => {
            assert_eq!(document.ship.parts.len(), 1);
            assert_eq!(document.history.undo_len(), 1);
            press!(Action::Undo);
        }
        6 => {
            assert_eq!(Some(&document.ship), state.before.as_ref());
            press!(Action::Node(PartKey::new(0, 1, 0)));
        }
        7 => {
            press!(Action::UseParent);
        }
        8 => {
            assert_eq!(panel.parent, Some(PartKey::new(0, 1, 0)));
            press!(Action::Node(PartKey::new(0, 4, 0)));
        }
        9 => {
            press!(Action::UseChild);
        }
        10 => {
            assert_eq!(panel.child, Some(PartKey::new(0, 4, 0)));
            panel.parent_attach = 3;
            panel.child_attach = 4;
        }
        11 => {
            press!(Action::Connect);
        }
        12 => {
            assert_eq!(document.ship.connections.len(), 3);
            assert!(document.ship.connections.iter().any(|edge| matches!(
                edge,
                Connection::Normal {
                    parent: 1,
                    child: 4,
                    ..
                }
            )));
            assert_eq!(document.history.undo_len(), 1);
            press!(Action::Undo);
        }
        13 => {
            assert_eq!(Some(&document.ship), state.before.as_ref());
            panel.parent = Some(PartKey::new(0, 4, 0));
            panel.child = Some(PartKey::new(0, 1, 0));
            panel.parent_attach = 4;
            panel.child_attach = 3;
        }
        14 => {
            press!(Action::Connect);
        }
        15 => {
            assert_eq!(Some(&document.ship), state.before.as_ref());
            assert!(document.status.contains("后代"));
            assert!(document.history.can_redo());
            press!(Action::Mode(Mode::Graph));
        }
        16 => {
            assert_eq!(panel.mode, Mode::Graph);
            press!(Action::Connect);
        }
        17 => {
            assert_eq!(document.ship.connections.len(), 4);
            assert_eq!(panel.forest.extra_edges.len(), 1);
            press!(Action::Node(PartKey::new(0, 1, 0)));
        }
        18 => {
            press!(Action::Component);
        }
        19 => {
            assert_eq!(document.selected_keys().len(), 4);
            capture(&mut commands, "target/editor-topology-graph.png", 2);
        }
        20 => {
            if captured.as_ref().is_none_or(|value| value.0 != 2) {
                return;
            }
            press!(Action::Edge(panel.graph.edges[3].reference.clone()));
        }
        21 => {
            press!(Action::Unlink);
        }
        22 => {
            assert_eq!(Some(&document.ship), state.before.as_ref());
            press!(Action::Undo);
        }
        23 => {
            assert_eq!(document.ship.connections.len(), 4);
            press!(Action::Redo);
        }
        24 => {
            assert_eq!(Some(&document.ship), state.before.as_ref());
            resize_for_smoke(&mut windows.single_mut().unwrap(), presentation);
            state.delay = 5;
        }
        25 => {
            let expected = target_viewport(presentation);
            let window = windows.single().unwrap();
            assert_eq!(window.width(), expected.x);
            assert_eq!(window.height(), expected.y);
            assert!(
                panel
                    .hits
                    .iter()
                    .any(|(action, rect)| *action == Action::Connect
                        && rect.is_positive()
                        && rect.min.y >= 0.
                        && rect.max.y < expected.y)
            );
            press!(Action::Mode(Mode::Tree));
        }
        26 => {
            assert_eq!(Some(&document.ship), state.before.as_ref());
            assert!(!document.dirty);
            let path = "target/topology-smoke.xml";
            dr_core::save_ship(path, &document.ship).unwrap();
            assert_eq!(dr_core::load_ship(path).unwrap(), document.ship);
            std::fs::write(
                "target/topology-smoke.json",
                serde_json::to_vec(&smoke_report(presentation)).unwrap(),
            )
            .unwrap();
            commands.spawn(Screenshot::primary_window()).observe(save_to_disk("target/editor-topology-smoke.png")).observe(move |_:On<ScreenshotCaptured>,mut exit:MessageWriter<AppExit>| {info!("连接树/图窗口自测通过：真实控件选择、子树删除、换父、拒绝树环、图环、分量选择、精确断边、撤销重做及{} / XML 往返", if presentation { "1920×1080 演示窗口" } else { "960×640 最小窗口" });exit.write(AppExit::Success);});
        }
        _ => return,
    }
    state.phase += 1;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smoke_presentation_preserves_native_1080p_and_scale_factor() {
        let mut window = Window {
            resolution: bevy::window::WindowResolution::new(1920, 1080)
                .with_scale_factor_override(1.),
            ..default()
        };
        resize_for_smoke(&mut window, true);
        assert_eq!(window.physical_size(), UVec2::new(1920, 1080));
        assert_eq!(window.scale_factor(), 1.);
        assert_eq!(
            Vec2::new(window.width(), window.height()),
            target_viewport(true)
        );
    }

    #[test]
    fn smoke_ordinary_mode_still_resizes_to_minimum_window() {
        let mut window = Window {
            resolution: bevy::window::WindowResolution::new(1920, 1080)
                .with_scale_factor_override(1.),
            ..default()
        };
        resize_for_smoke(&mut window, false);
        assert_eq!(window.physical_size(), UVec2::new(960, 640));
        assert_eq!(
            Vec2::new(window.width(), window.height()),
            target_viewport(false)
        );
    }

    #[test]
    fn smoke_reports_only_claim_minimum_window_when_it_was_tested() {
        let report = smoke_report(true);
        assert_eq!(report["window"], "1920x1080");
        assert_eq!(report["minimum_window_tested"], false);
        assert!(report.get("minimum_window").is_none());
        let report = smoke_report(false);
        assert_eq!(report["window"], "960x640");
        assert_eq!(report["minimum_window"], "960x640");
        assert_eq!(report["minimum_window_tested"], true);
        for presentation in [true, false] {
            let report = smoke_report(presentation);
            assert_eq!(report["steps"], 27);
            assert_eq!(report["input"], "egui_injected");
            assert_eq!(report["ime_tested"], false);
            assert_eq!(report["atomic_undo"], true);
        }
    }
}
