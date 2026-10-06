use super::*;

pub fn document() -> EditorDocument {
    let mut document = crate::tests::document();
    document.catalog=dr_core::catalog_from_xml(r#"<PartTypes><PartType id="p" name="测试部件" width="2" height="2"><AttachPoints>
    <AttachPoint location="LeftCenter"/><AttachPoint location="RightCenter"/><AttachPoint location="TopCenter"/><AttachPoint location="BottomCenter"/>
    </AttachPoints></PartType></PartTypes>"#).unwrap();
    let p = document.catalog.get("p").unwrap();
    document.ship = Ship {
        parts: [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)]
            .into_iter()
            .enumerate()
            .map(|(i, pos)| p.instantiate(i as i64 + 1, pos))
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
    document.refresh();
    document
}
struct Panel {
    ctx: egui::Context,
    state: ConnectionEditor,
    document: EditorDocument,
}
impl Panel {
    fn new() -> Self {
        let mut result = Self {
            ctx: egui::Context::default(),
            state: ConnectionEditor {
                open: true,
                ..Default::default()
            },
            document: document(),
        };
        for _ in 0..4 {
            result.frame(vec![]);
        }
        result
    }
    fn frame(&mut self, events: Vec<egui::Event>) {
        self.ctx.begin_pass(egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(960.0, 640.0),
            )),
            events,
            focused: true,
            ..Default::default()
        });
        if let Some(action) = show(&self.ctx, &mut self.state, &self.document) {
            act(action, &mut self.state, &mut self.document);
        }
        self.ctx.end_pass().textures_delta.clear();
    }
    fn click(&mut self, action: Action) {
        let rect = self
            .state
            .hits
            .iter()
            .find(|(a, _)| *a == action)
            .unwrap_or_else(|| panic!("不可见按钮：{action:?}"))
            .1;
        let pos = rect.center();
        for pressed in [true, false] {
            self.frame(vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ]);
        }
        self.frame(vec![]);
    }
}
#[test]
fn actual_tree_controls_select_subtree_delete_and_undo_atomically() {
    let mut panel = Panel::new();
    let before = panel.document.ship.clone();
    panel.click(Action::Node(PartKey::new(0, 2, 0)));
    panel.click(Action::Subtree);
    assert_eq!(panel.document.selected_keys().len(), 3);
    panel.click(Action::Delete);
    assert_eq!(panel.document.ship.parts.len(), 1);
    assert_eq!(panel.document.history.undo_len(), 1);
    panel.click(Action::Undo);
    assert_eq!(panel.document.ship, before);
    panel.click(Action::Redo);
    assert_eq!(panel.document.ship.parts.len(), 1);
}
#[test]
fn actual_graph_connect_and_unlink_keep_cycle_and_undo_history() {
    let mut panel = Panel::new();
    let before = panel.document.ship.clone();
    panel.click(Action::Mode(Mode::Graph));
    panel.state.parent = Some(PartKey::new(0, 4, 0));
    panel.state.child = Some(PartKey::new(0, 1, 0));
    panel.state.parent_attach = 4;
    panel.state.child_attach = 3;
    panel.frame(vec![]);
    panel.click(Action::Connect);
    assert_eq!(panel.document.ship.connections.len(), 4);
    assert_eq!(panel.state.forest.extra_edges.len(), 1);
    panel.click(Action::Edge(panel.state.graph.edges[3].reference.clone()));
    panel.frame(vec![]);
    panel.click(Action::Unlink);
    assert_eq!(panel.document.ship, before);
    panel.click(Action::Undo);
    assert_eq!(panel.document.ship.connections.len(), 4);
    panel.click(Action::Undo);
    assert_eq!(panel.document.ship, before);
}
#[test]
fn invalid_tree_cycle_preserves_document_and_dragged_graph_nodes_only_change_layout() {
    let mut panel = Panel::new();
    let before = panel.document.ship.clone();
    panel.state.parent = Some(PartKey::new(0, 4, 0));
    panel.state.child = Some(PartKey::new(0, 1, 0));
    panel.state.parent_attach = 4;
    panel.state.child_attach = 3;
    panel.frame(vec![]);
    panel.click(Action::Connect);
    assert_eq!(panel.document.ship, before);
    assert_eq!(panel.document.history.undo_len(), 0);
    assert!(panel.document.status.contains("后代"));
    panel.document.status.clear();
    panel.click(Action::Mode(Mode::Graph));
    let key = PartKey::new(0, 1, 0);
    let start = panel
        .state
        .hits
        .iter()
        .find(|(a, _)| *a == Action::Node(key))
        .unwrap()
        .1
        .center();
    let position = panel.state.positions[&key];
    panel.frame(vec![
        egui::Event::PointerMoved(start),
        egui::Event::PointerButton {
            pos: start,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        },
    ]);
    panel.frame(vec![egui::Event::PointerMoved(
        start + egui::vec2(20.0, 15.0),
    )]);
    panel.frame(vec![egui::Event::PointerButton {
        pos: start + egui::vec2(20.0, 15.0),
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    }]);
    assert_ne!(panel.state.positions[&key], position);
    assert_eq!(panel.document.ship, before);
    assert_eq!(panel.document.history.undo_len(), 0);
}
#[test]
fn topology_mode_blocks_canvas_and_cancels_existing_placement() {
    let mut app = App::new();
    app.insert_resource(document())
        .insert_resource(properties::Inspector::default())
        .insert_resource(ConnectionEditor {
            open: true,
            ..Default::default()
        })
        .insert_resource(ButtonInput::<KeyCode>::default())
        .insert_resource(panels::UiPointer::default())
        .insert_resource(DragState::default())
        .insert_resource(EditorCursor {
            placing: true,
            palette_drag: true,
            ..Default::default()
        })
        .insert_resource(CameraDrag::default())
        .add_systems(Update, egui_ui::prepare_input);
    app.update();
    assert!(app.world().resource::<panels::UiPointer>().blocked);
    assert!(!app.world().resource::<EditorCursor>().placing);
}

#[test]
fn graph_routes_cycles_outside_nodes_and_keeps_arrow_tips_visible() {
    let from = egui::pos2(100.0, 50.0);
    let to = egui::pos2(290.0, 50.0);
    let half = egui::vec2(83.0, 19.0);
    let forward = edge_path(from, to, half, false, 0);
    assert_eq!(
        forward,
        vec![egui::pos2(183.0, 50.0), egui::pos2(207.0, 50.0)]
    );
    let cycle = edge_path(to, from, half, true, 3);
    assert!(cycle[1].y > to.y + half.y);
    assert!(cycle[2].y > from.y + half.y);
    assert_eq!(cycle.last(), Some(&egui::pos2(100.0, 69.0)));
    assert!(
        edge_path(from, from, half, true, 4)
            .iter()
            .all(|p| p.x.is_finite() && p.y.is_finite())
    );
}

#[test]
fn structural_change_clears_cached_endpoint_identity() {
    let mut document = document();
    let mut state = ConnectionEditor::default();
    state.refresh(&document);
    state.parent = Some(PartKey::new(0, 1, 0));
    state.child = Some(PartKey::new(0, 2, 0));
    state.edge = Some(state.graph.edges[0].reference.clone());
    document.ship.parts.remove(0);
    document.refresh();
    state.refresh(&document);
    assert!(state.parent.is_none() && state.child.is_none() && state.edge.is_none());
    assert!(!state.positions.contains_key(&PartKey::new(0, 1, 0)));
    assert!(!state.graph.unresolved.is_empty());
}

#[test]
fn docking_connect_requires_connector_and_disabled_controls_have_no_hits() {
    let mut panel = Panel::new();
    assert!(!panel.state.hits.iter().any(|(a, _)| *a == Action::Undo));
    assert!(!panel.state.hits.iter().any(|(a, _)| *a == Action::Connect));
    panel.state.parent = Some(PartKey::new(0, 1, 0));
    panel.state.child = Some(PartKey::new(0, 2, 0));
    panel.state.docking = true;
    panel.frame(vec![]);
    assert!(!panel.state.hits.iter().any(|(a, _)| *a == Action::Connect));
    panel.state.connector = panel.state.parent;
    panel.frame(vec![]);
    assert!(panel.state.hits.iter().any(|(a, _)| *a == Action::Connect));
    panel.state.connector = Some(PartKey::new(0, 3, 0));
    panel.frame(vec![]);
    assert!(panel.state.connector.is_none());
    assert!(!panel.state.hits.iter().any(|(a, _)| *a == Action::Connect));
}

#[test]
fn disabled_unlink_shows_prerequisite_and_cannot_dispatch() {
    let ctx = egui::Context::default();
    let mut rect = egui::Rect::NOTHING;
    let mut saw_hint = false;
    for frame in 0..6 {
        let mut hits = vec![];
        let mut result = None;
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(600.0, 240.0),
                )),
                time: Some(f64::from(frame)),
                // 移入一次后推进静止帧；重复 Move 会重置 egui 的悬停等待。
                events: if frame == 2 {
                    vec![egui::Event::PointerMoved(rect.center())]
                } else {
                    vec![]
                },
                ..Default::default()
            },
            |ui| {
                result = control(ui, &mut hits, Action::Unlink, "− 断开", false);
                rect = ui.min_rect();
            },
        );
        output.textures_delta.clear();
        assert!(hits.is_empty());
        assert!(result.is_none());
        fn has_hint(shape: &egui::epaint::Shape) -> bool {
            match shape {
                egui::epaint::Shape::Text(text) => text.galley.job.text.contains("请先选边"),
                egui::epaint::Shape::Vec(shapes) => shapes.iter().any(has_hint),
                _ => false,
            }
        }
        saw_hint |= output.shapes.iter().any(|shape| has_hint(&shape.shape));
    }
    assert!(saw_hint, "无所选边时仍应解释断开按钮的可用条件");
}
