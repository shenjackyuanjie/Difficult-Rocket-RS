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
            windows.single_mut().unwrap().resolution.set(960.0, 640.0);
            state.delay = 5;
        }
        25 => {
            assert!(
                panel
                    .hits
                    .iter()
                    .any(|(action, rect)| *action == Action::Connect && rect.max.y < 640.0)
            );
            press!(Action::Mode(Mode::Tree));
        }
        26 => {
            assert_eq!(Some(&document.ship), state.before.as_ref());
            assert!(!document.dirty);
            let path = "target/topology-smoke.xml";
            dr_core::save_ship(path, &document.ship).unwrap();
            assert_eq!(dr_core::load_ship(path).unwrap(), document.ship);
            std::fs::write("target/topology-smoke.json",r#"{"steps":27,"input":"egui_injected","ime_tested":false,"minimum_window":"960x640","atomic_undo":true}"#).unwrap();
            commands.spawn(Screenshot::primary_window()).observe(save_to_disk("target/editor-topology-smoke.png")).observe(|_:On<ScreenshotCaptured>,mut exit:MessageWriter<AppExit>| {info!("连接树/图窗口自测通过：真实控件选择、子树删除、换父、拒绝树环、图环、分量选择、精确断边、撤销重做及最小窗口/XML 往返");exit.write(AppExit::Success);});
        }
        _ => return,
    }
    state.phase += 1;
}
