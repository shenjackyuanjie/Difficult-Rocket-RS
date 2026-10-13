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
            ctx: crate::egui_ui::test_context(),
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
    let ctx = crate::egui_ui::test_context();
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

/// 触控/窄屏窗口：窗口本身与首屏控件都留在可用区域内，底部控件靠滚动到达。
struct Responsive {
    ctx: egui::Context,
    state: ConnectionEditor,
    document: EditorDocument,
    screen: egui::Rect,
    window: egui::Rect,
    wide: Option<(egui::Rect, bool)>,
    right_hit: Option<(Action, egui::Rect)>,
    frame: u32,
}

impl Responsive {
    fn new(width: f32, height: f32) -> Self {
        let mut result = Self {
            ctx: crate::egui_ui::test_context(),
            state: ConnectionEditor {
                open: true,
                ..Default::default()
            },
            document: document(),
            screen: egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height)),
            window: egui::Rect::NOTHING,
            wide: None,
            right_hit: None,
            frame: 0,
        };
        for _ in 0..4 {
            result.step(vec![]);
        }
        result
    }

    fn step(&mut self, events: Vec<egui::Event>) {
        let Self {
            ctx,
            state,
            document,
            screen,
            window,
            wide,
            right_hit,
            frame,
        } = self;
        let time = f64::from(*frame);
        *frame += 1;
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(*screen),
                time: Some(time),
                focused: true,
                events,
                ..Default::default()
            },
            |ui| {
                if let Some(action) = show(ui.ctx(), &mut *state, &*document) {
                    act(action, &mut *state, &mut *document);
                }
                *window = ui
                    .ctx()
                    .memory(|memory| memory.area_rect(egui::Id::new("connection_editor")))
                    .unwrap_or(egui::Rect::NOTHING);
            },
        );
        *wide = widest_shape(&output.shapes);
        *right_hit = state
            .hits
            .iter()
            .max_by(|a, b| a.1.max.x.total_cmp(&b.1.max.x))
            .map(|(action, rect)| (action.clone(), *rect));
        output.textures_delta.clear();
    }

    fn hit(&self, action: Action) -> Option<egui::Rect> {
        self.state
            .hits
            .iter()
            .find(|(candidate, _)| *candidate == action)
            .map(|(_, rect)| *rect)
    }

    fn visible(&self, action: Action) -> bool {
        self.hit(action)
            .is_some_and(|rect| rect.is_positive() && self.screen.contains_rect(rect))
    }

    fn window_inside_screen(&self) -> bool {
        self.window.is_positive() && self.screen.expand(1.0).contains_rect(self.window)
    }

    /// 向下滚动到底：底部控件应被滚动带到可见区域。
    fn scroll_to_bottom(&mut self) {
        let pointer = self.window.center();
        self.step(vec![egui::Event::PointerMoved(pointer)]);
        for _ in 0..12 {
            self.step(vec![egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: egui::vec2(0.0, -240.0),
                phase: egui::TouchPhase::Move,
                modifiers: egui::Modifiers::NONE,
            }]);
        }
        self.step(vec![]);
        self.step(vec![]);
    }
}

/// 最宽的图形及其是否为文本；窗口阴影向右下偏移，需据此区分装饰与内容。
fn widest_shape(shapes: &[egui::epaint::ClippedShape]) -> Option<(egui::Rect, bool)> {
    shapes
        .iter()
        .map(|clipped| {
            (
                clipped.shape.visual_bounding_rect(),
                matches!(&clipped.shape, egui::epaint::Shape::Text(_)),
            )
        })
        .max_by(|a, b| a.0.max.x.total_cmp(&b.0.max.x))
}

#[test]
fn connection_editor_window_fits_touch_screens_and_keeps_controls_reachable() {
    for (width, height) in [
        (320.0, 640.0),
        (390.0, 844.0),
        (640.0, 360.0),
        (1280.0, 800.0),
    ] {
        let mut panel = Responsive::new(width, height);
        assert!(
            panel.window_inside_screen(),
            "{width}×{height} 的连接编辑窗口应留在可用区域内：{:?}，最宽图形（矩形, 是否文本）{:?}，最右命中 {:?}",
            panel.window,
            panel.wide,
            panel.right_hit
        );
        // 关闭按钮在窗口标题栏左上角；窗口在屏内即可触达。
        panel
            .document
            .execute(EditorCommand::DeleteSelection(vec![PartKey::new(0, 2, 0)]));
        panel.step(vec![]);
        for action in [
            Action::Mode(Mode::Tree),
            Action::Mode(Mode::Graph),
            Action::Undo,
        ] {
            assert!(
                panel.visible(action.clone()),
                "{width}×{height} 首屏控件应可触达：{action:?}"
            );
        }
        let edge = panel.state.graph.edges[0].reference.clone();
        panel.state.edge = Some(edge);
        panel.step(vec![]);
        panel.scroll_to_bottom();
        assert!(
            panel.visible(Action::Unlink),
            "{width}×{height} 滚动后应能点到断开按钮：{:?}",
            panel.hit(Action::Unlink)
        );
        assert!(
            panel.window_inside_screen(),
            "{width}×{height} 滚动不应把窗口撑出可用区域：{:?}",
            panel.window
        );
    }
}
