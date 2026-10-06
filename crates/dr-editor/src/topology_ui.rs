//! 连接树与有向连接图共用文档命令；节点排版不改变船体物理位置。
use super::*;
use bevy_egui::{EguiContexts, egui};
use dr_core::{ConnectionRef, LinkForest, Topology};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    #[default]
    Tree,
    Graph,
}
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Mode(Mode),
    Node(PartKey),
    Edge(ConnectionRef),
    Subtree,
    Component,
    UseParent,
    UseChild,
    Connect,
    Unlink,
    Delete,
    Undo,
    Redo,
    Layout,
}

#[derive(Resource)]
pub struct ConnectionEditor {
    pub open: bool,
    pub mode: Mode,
    pub graph: Topology,
    pub forest: LinkForest,
    pub parent: Option<PartKey>,
    pub child: Option<PartKey>,
    pub connector: Option<PartKey>,
    pub docking: bool,
    pub parent_attach: i32,
    pub child_attach: i32,
    pub edge: Option<ConnectionRef>,
    pub hits: Vec<(Action, egui::Rect)>,
    pub positions: HashMap<PartKey, egui::Vec2>,
    pub collapsed: BTreeSet<PartKey>,
    pub zoom: f32,
    revision: Option<u64>,
}
impl Default for ConnectionEditor {
    fn default() -> Self {
        Self {
            open: false,
            mode: Mode::Tree,
            graph: Topology::default(),
            forest: LinkForest::default(),
            parent: None,
            child: None,
            connector: None,
            docking: false,
            parent_attach: 1,
            child_attach: 1,
            edge: None,
            hits: vec![],
            positions: HashMap::new(),
            collapsed: BTreeSet::new(),
            zoom: 1.0,
            revision: None,
        }
    }
}
impl ConnectionEditor {
    pub fn refresh(&mut self, document: &EditorDocument) {
        if self.revision == Some(document.revision) {
            return;
        }
        let graph = Topology::from_ship(&document.ship);
        if self.revision.is_some() && self.graph.nodes != graph.nodes {
            // 结构变化/分组合并后 PartKey 的位置可能复用，不沿用旧端点和排版身份。
            self.parent = None;
            self.child = None;
            self.connector = None;
            self.edge = None;
            self.positions.clear();
            self.collapsed.clear();
        }
        self.graph = graph;
        self.forest = self.graph.forest();
        self.positions
            .retain(|key, _| document.ship.part_at(*key).is_some());
        self.collapsed
            .retain(|key| document.ship.part_at(*key).is_some());
        for key in [&mut self.parent, &mut self.child, &mut self.connector] {
            if key.is_some_and(|key| document.ship.part_at(key).is_none()) {
                *key = None;
            }
        }
        if self.edge.as_ref().is_some_and(|edge| {
            document
                .ship
                .group(edge.group)
                .and_then(|(_, edges)| edges.get(edge.index))
                != Some(&edge.expected)
        }) {
            self.edge = None;
        }
        self.layout(false);
        self.revision = Some(document.revision);
    }
    pub fn layout(&mut self, reset: bool) {
        if reset {
            self.positions.clear();
        }
        let mut rows = [0usize; 8];
        for root in &self.forest.roots {
            for (node, depth) in self.forest.subtree(*root) {
                let column = depth.min(7);
                let row = rows[column];
                rows[column] += 1;
                self.positions
                    .entry(self.graph.nodes[node])
                    .or_insert(egui::vec2(
                        90.0 + column as f32 * 190.0,
                        45.0 + row as f32 * 80.0,
                    ));
            }
        }
    }
}
fn label(document: &EditorDocument, key: PartKey) -> String {
    let name = document
        .ship
        .part_at(key)
        .and_then(|p| document.catalog.get(&p.part_type))
        .map(|p| p.name.as_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("部件");
    format!(
        "{} · {}#{} · {name}",
        if key.group == 0 {
            "主组".into()
        } else {
            format!("组 {}", key.group)
        },
        key.id,
        key.occurrence + 1
    )
}
fn control(
    ui: &mut egui::Ui,
    hits: &mut Vec<(Action, egui::Rect)>,
    action: Action,
    text: &str,
    enabled: bool,
) -> Option<Action> {
    let response = ui.add_enabled(enabled, egui::Button::new(text));
    let rect = response.rect.intersect(ui.clip_rect());
    if enabled && rect.is_positive() {
        hits.push((action.clone(), rect));
    }
    response.clicked().then_some(action)
}
fn choose(
    ui: &mut egui::Ui,
    id: &str,
    selected: &mut Option<PartKey>,
    graph: &Topology,
    document: &EditorDocument,
) {
    egui::ComboBox::from_id_salt(id)
        .selected_text(
            selected
                .map(|key| label(document, key))
                .unwrap_or_else(|| "请选择".into()),
        )
        .width(210.0)
        .show_ui(ui, |ui| {
            egui::ScrollArea::vertical().max_height(200.0).show_rows(
                ui,
                22.0,
                graph.nodes.len(),
                |ui, rows| {
                    for row in rows {
                        let key = graph.nodes[row];
                        ui.selectable_value(selected, Some(key), label(document, key));
                    }
                },
            );
        });
}
fn attach(
    ui: &mut egui::Ui,
    id: &str,
    key: Option<PartKey>,
    selected: &mut i32,
    document: &EditorDocument,
) {
    let kind = key
        .and_then(|key| document.ship.part_at(key))
        .and_then(|part| document.catalog.get(&part.part_type));
    egui::ComboBox::from_id_salt(id)
        .selected_text(format!("连接点 {selected}"))
        .width(112.0)
        .show_ui(ui, |ui| {
            if let Some(kind) = kind {
                for (i, point) in kind.attach_points.iter().enumerate() {
                    ui.selectable_value(
                        selected,
                        (i + 1) as i32,
                        format!("{} {}", i + 1, point.location),
                    );
                }
            }
        });
}

pub fn act(action: Action, state: &mut ConnectionEditor, document: &mut EditorDocument) {
    state.refresh(document);
    match action {
        Action::Mode(mode) => state.mode = mode,
        Action::Node(key) => document.select_only(Some(key)),
        Action::Edge(reference) => state.edge = Some(reference),
        Action::UseParent => state.parent = document.selected,
        Action::UseChild => state.child = document.selected,
        Action::Subtree | Action::Component => {
            if let Some(node) = document
                .selected
                .and_then(|key| state.graph.nodes.iter().position(|value| *value == key))
            {
                let nodes = if action == Action::Subtree {
                    state
                        .forest
                        .subtree(node)
                        .into_iter()
                        .map(|(n, _)| n)
                        .collect()
                } else {
                    state.graph.reachable(node, false)
                };
                document.selection = nodes.into_iter().map(|n| state.graph.nodes[n]).collect();
            }
        }
        Action::Connect => {
            if let (Some(parent), Some(child)) = (state.parent, state.child) {
                let kind = if state.docking {
                    let Some(connector) = state.connector else {
                        document.status = "请选择对接插头".into();
                        return;
                    };
                    LinkKind::Dock { connector }
                } else {
                    LinkKind::Normal {
                        parent_attach: state.parent_attach,
                        child_attach: state.child_attach,
                    }
                };
                let command = if state.mode == Mode::Tree {
                    EditorCommand::Reparent {
                        parent,
                        child,
                        kind,
                    }
                } else {
                    EditorCommand::ConnectParts {
                        parent,
                        child,
                        kind,
                    }
                };
                document.execute(command);
            }
        }
        Action::Unlink => {
            if let Some(reference) = state.edge.clone() {
                document.execute(EditorCommand::RemoveConnection(reference));
            }
        }
        Action::Delete => {
            if !document.selected_keys().is_empty() {
                document.execute(EditorCommand::DeleteSelection(document.selected_keys()));
            }
        }
        Action::Undo => {
            document.undo();
        }
        Action::Redo => {
            document.redo();
        }
        Action::Layout => state.layout(true),
    }
    state.refresh(document);
}

fn tree(
    ui: &mut egui::Ui,
    state: &mut ConnectionEditor,
    document: &EditorDocument,
    action: &mut Option<Action>,
) {
    let mut rows = vec![];
    let mut stack: Vec<_> = state
        .forest
        .roots
        .iter()
        .rev()
        .map(|root| (*root, 0usize))
        .collect();
    while let Some((node, depth)) = stack.pop() {
        rows.push((node, depth));
        if !state.collapsed.contains(&state.graph.nodes[node]) {
            stack.extend(
                state.forest.children[node]
                    .iter()
                    .rev()
                    .map(|next| (*next, depth + 1)),
            );
        }
    }
    egui::ScrollArea::vertical()
        .id_salt("link_tree")
        .max_height(270.0)
        .show_rows(ui, 24.0, rows.len(), |ui, range| {
            for i in range {
                let (node, depth) = rows[i];
                let key = state.graph.nodes[node];
                ui.horizontal(|ui| {
                    ui.add_space(depth.min(18) as f32 * 13.0);
                    if !state.forest.children[node].is_empty() {
                        if ui
                            .small_button(if state.collapsed.contains(&key) {
                                "▶"
                            } else {
                                "▼"
                            })
                            .clicked()
                            && !state.collapsed.remove(&key)
                        {
                            state.collapsed.insert(key);
                        }
                    } else {
                        ui.add_space(20.0);
                    }
                    let response = ui.add(
                        egui::Button::new(label(document, key))
                            .selected(document.is_selected(key))
                            .truncate(),
                    );
                    let rect = response.rect.intersect(ui.clip_rect());
                    if rect.is_positive() {
                        state.hits.push((Action::Node(key), rect));
                    }
                    if response.clicked() {
                        *action = Some(Action::Node(key));
                    }
                    if let Some(edge) = state.forest.parent_edge[node]
                        && ui.small_button("父边").clicked()
                    {
                        *action = Some(Action::Edge(state.graph.edges[edge].reference.clone()));
                    }
                });
            }
        });
    ui.label(format!(
        "生成森林：{} 个根；{} 条额外边（环/多父）仍完整保留，可在图视图编辑。",
        state.forest.roots.len(),
        state.forest.extra_edges.len()
    ));
}

/// 连线在节点边缘止步；环/反向边绕开节点，不能把箭头和环藏在卡片下面。
fn edge_path(
    from: egui::Pos2,
    to: egui::Pos2,
    half: egui::Vec2,
    detour: bool,
    index: usize,
) -> Vec<egui::Pos2> {
    if from.distance_sq(to) < 1.0 {
        let right = from + egui::vec2(half.x, 0.0);
        let bottom = to + egui::vec2(0.0, half.y);
        return vec![
            right,
            right + egui::vec2(30.0, 0.0),
            right + egui::vec2(30.0, half.y + 30.0),
            bottom + egui::vec2(0.0, 30.0),
            bottom,
        ];
    }
    if detour || to.x <= from.x {
        let a = from + egui::vec2(0.0, half.y);
        let b = to + egui::vec2(0.0, half.y);
        let y = a.y.max(b.y) + 26.0 + (index % 5) as f32 * 12.0;
        return vec![a, egui::pos2(a.x, y), egui::pos2(b.x, y), b];
    }
    let delta = to - from;
    let fraction = (half.x / delta.x.abs().max(1e-6)).min(half.y / delta.y.abs().max(1e-6));
    // 节点拖叠时仍给出有限线段；排版不会改变物理连接。
    let fraction = fraction.min(0.45);
    vec![from + delta * fraction, to - delta * fraction]
}

fn graph(
    ui: &mut egui::Ui,
    state: &mut ConnectionEditor,
    document: &EditorDocument,
    action: &mut Option<Action>,
) {
    ui.horizontal(|ui| {
        ui.add(egui::Slider::new(&mut state.zoom, 0.4..=1.5).text("图缩放"));
        if let Some(a) = control(ui, &mut state.hits, Action::Layout, "重新排版", true) {
            *action = Some(a);
        }
        ui.label("拖节点仅调整图布局，不移动船体");
    });
    egui::ScrollArea::both()
        .id_salt("link_graph")
        .max_height(270.0)
        .show(ui, |ui| {
            let extent = state
                .positions
                .values()
                .fold(egui::vec2(680.0, 200.0), |size, p| {
                    size.max(*p + egui::vec2(110.0, 60.0))
                })
                * state.zoom;
            let (canvas, response) = ui.allocate_exact_size(extent, egui::Sense::click());
            let painter = ui.painter_at(canvas.intersect(ui.clip_rect()));
            let position = |key: &PartKey| canvas.min + state.positions[key] * state.zoom;
            let pointer = response.interact_pointer_pos();
            for (index, edge) in state.graph.edges.iter().enumerate() {
                let from = position(&state.graph.nodes[edge.parent]);
                for (target, docking) in std::iter::once((edge.child, false)).chain(
                    edge.dock
                        .filter(|node| *node != edge.parent && *node != edge.child)
                        .map(|node| (node, true)),
                ) {
                    let to = position(&state.graph.nodes[target]);
                    let selected = state.edge.as_ref() == Some(&edge.reference);
                    let extra = state.forest.extra_edges.binary_search(&index).is_ok();
                    let color = if selected {
                        egui::Color32::YELLOW
                    } else if docking || edge.dock.is_some() {
                        egui::Color32::from_rgb(185, 140, 240)
                    } else if extra {
                        egui::Color32::from_rgb(235, 165, 80)
                    } else {
                        egui::Color32::from_rgb(105, 185, 225)
                    };
                    let path =
                        edge_path(from, to, egui::vec2(83.0, 19.0) * state.zoom, extra, index);
                    for pair in path.windows(2) {
                        painter.line_segment(
                            [pair[0], pair[1]],
                            egui::Stroke::new(if selected { 3.0 } else { 1.5 }, color),
                        );
                        let delta = pair[1] - pair[0];
                        if response.clicked()
                            && pointer.is_some_and(|p| {
                                let t = ((p - pair[0]).dot(delta) / delta.length_sq().max(1e-6))
                                    .clamp(0.0, 1.0);
                                p.distance(pair[0] + delta * t) < 7.0
                            })
                        {
                            *action = Some(Action::Edge(edge.reference.clone()));
                        }
                    }
                    let end = path[path.len() - 1];
                    let direction = (end - path[path.len() - 2]).normalized();
                    let side = egui::vec2(-direction.y, direction.x) * 4.0;
                    painter.line_segment(
                        [end - direction * 9.0 + side, end],
                        egui::Stroke::new(1.5, color),
                    );
                    painter.line_segment(
                        [end - direction * 9.0 - side, end],
                        egui::Stroke::new(1.5, color),
                    );
                    let segment = path
                        .windows(2)
                        .max_by(|a, b| a[0].distance_sq(a[1]).total_cmp(&b[0].distance_sq(b[1])))
                        .unwrap();
                    let middle = segment[0].lerp(segment[1], 0.5);
                    let hit = egui::Rect::from_center_size(middle, egui::vec2(8.0, 8.0))
                        .intersect(ui.clip_rect());
                    if hit.is_positive() {
                        state.hits.push((Action::Edge(edge.reference.clone()), hit));
                    }
                    if extra {
                        painter.text(
                            middle + egui::vec2(0.0, 9.0),
                            egui::Align2::CENTER_TOP,
                            format!(
                                "额外边 {}:{}",
                                edge.reference.group,
                                edge.reference.index + 1
                            ),
                            egui::FontId::proportional(11.0),
                            color,
                        );
                    }
                }
            }
            for key in &state.graph.nodes {
                let rect = egui::Rect::from_center_size(
                    canvas.min + state.positions[key] * state.zoom,
                    egui::vec2(166.0, 38.0) * state.zoom,
                );
                if !ui.clip_rect().intersects(rect) {
                    continue;
                }
                let response = ui.put(
                    rect,
                    egui::Button::new(
                        egui::RichText::new(label(document, *key)).size(12.0 * state.zoom),
                    )
                    .selected(document.is_selected(*key))
                    .sense(egui::Sense::click_and_drag())
                    .truncate(),
                );
                state
                    .hits
                    .push((Action::Node(*key), response.rect.intersect(ui.clip_rect())));
                if response.clicked() {
                    *action = Some(Action::Node(*key));
                }
                if response.dragged() {
                    let delta = ui.input(|input| input.pointer.delta()) / state.zoom;
                    if let Some(p) = state.positions.get_mut(key) {
                        *p = (*p + delta).max(egui::vec2(85.0, 25.0));
                    }
                }
            }
        });
}

pub fn show(
    ctx: &egui::Context,
    state: &mut ConnectionEditor,
    document: &EditorDocument,
) -> Option<Action> {
    state.hits.clear();
    if !state.open {
        return None;
    }
    state.refresh(document);
    let mut open = state.open;
    let mut action = None;
    egui::Window::new("连接编辑 · 树 / 图（F6）")
        .id(egui::Id::new("connection_editor"))
        .open(&mut open)
        .default_pos(egui::pos2(30.0, 60.0))
        .default_size(egui::vec2(870.0, 520.0))
        .min_width(650.0)
        .collapsible(false)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                for (mode, name) in [(Mode::Tree, "连接树"), (Mode::Graph, "连接图")] {
                    let response = ui.add(egui::Button::new(name).selected(state.mode == mode));
                    state.hits.push((Action::Mode(mode), response.rect));
                    if response.clicked() {
                        action = Some(Action::Mode(mode));
                    }
                }
                ui.label(format!(
                    "{} 节点 · {} 连接 · {} 待修复",
                    state.graph.nodes.len(),
                    state.graph.edges.len(),
                    state.graph.unresolved.len()
                ));
                if let Some(a) = control(
                    ui,
                    &mut state.hits,
                    Action::Undo,
                    "撤销",
                    document.history.can_undo(),
                ) {
                    action = Some(a);
                }
                if let Some(a) = control(
                    ui,
                    &mut state.hits,
                    Action::Redo,
                    "重做",
                    document.history.can_redo(),
                ) {
                    action = Some(a);
                }
            });
            ui.horizontal_wrapped(|ui| {
                let selected = document.selected.is_some();
                for (a, name, enabled) in [
                    (Action::Subtree, "选择子树", selected),
                    (Action::Component, "选择连通分量", selected),
                    (Action::UseParent, "选中设为父", selected),
                    (Action::UseChild, "选中设为子", selected),
                    (
                        Action::Delete,
                        "删除所选",
                        !document.selected_keys().is_empty(),
                    ),
                ] {
                    if let Some(a) = control(ui, &mut state.hits, a, name, enabled) {
                        action = Some(a);
                    }
                }
            });
            match state.mode {
                Mode::Tree => tree(ui, state, document, &mut action),
                Mode::Graph => graph(ui, state, document, &mut action),
            }
            ui.separator();
            ui.horizontal_wrapped(|ui| {
                ui.label("父");
                choose(ui, "link_parent", &mut state.parent, &state.graph, document);
                ui.label("子");
                choose(ui, "link_child", &mut state.child, &state.graph, document);
                ui.checkbox(&mut state.docking, "对接边");
            });
            ui.horizontal_wrapped(|ui| {
                if state.docking {
                    ui.label("插头（父/子之一）");
                    if state
                        .connector
                        .is_some_and(|key| Some(key) != state.parent && Some(key) != state.child)
                    {
                        state.connector = None;
                    }
                    egui::ComboBox::from_id_salt("link_dock")
                        .selected_text(
                            state
                                .connector
                                .map(|key| label(document, key))
                                .unwrap_or_else(|| "请选择插头端".into()),
                        )
                        .show_ui(ui, |ui| {
                            for key in [state.parent, state.child].into_iter().flatten() {
                                ui.selectable_value(
                                    &mut state.connector,
                                    Some(key),
                                    label(document, key),
                                );
                            }
                        });
                } else {
                    ui.label("父端");
                    attach(
                        ui,
                        "parent_attach",
                        state.parent,
                        &mut state.parent_attach,
                        document,
                    );
                    ui.label("子端");
                    attach(
                        ui,
                        "child_attach",
                        state.child,
                        &mut state.child_attach,
                        document,
                    );
                }
                let name = if state.mode == Mode::Tree {
                    "设置父节点"
                } else {
                    "添加连接"
                };
                if let Some(a) = control(
                    ui,
                    &mut state.hits,
                    Action::Connect,
                    name,
                    state.parent.is_some() && state.child.is_some(),
                ) {
                    action = Some(a);
                }
            });
            ui.label(
                "连接仍校验实际连接面和占用；不接触时请先在画布移动部件。树换父拒绝环与多父歧义。",
            );
            ui.horizontal_wrapped(|ui| {
                egui::ComboBox::from_id_salt("edge_picker")
                    .width(420.0)
                    .selected_text(
                        state
                            .edge
                            .as_ref()
                            .map(|r| format!("组 {} / 边 {}", r.group, r.index + 1))
                            .unwrap_or_else(|| "选择要断开的边（也可点击图中连线）".into()),
                    )
                    .show_ui(ui, |ui| {
                        let refs: Vec<_> = state
                            .graph
                            .edges
                            .iter()
                            .map(|e| &e.reference)
                            .chain(state.graph.unresolved.iter().map(|e| &e.reference))
                            .collect();
                        egui::ScrollArea::vertical().max_height(150.0).show_rows(
                            ui,
                            22.0,
                            refs.len(),
                            |ui, rows| {
                                for row in rows {
                                    let r = refs[row];
                                    let response = ui.selectable_label(
                                        state.edge.as_ref() == Some(r),
                                        format!(
                                            "组 {} / 边 {}：{:?}",
                                            r.group,
                                            r.index + 1,
                                            r.expected
                                        ),
                                    );
                                    if response.clicked() {
                                        action = Some(Action::Edge(r.clone()));
                                    }
                                }
                            },
                        );
                    });
                if let Some(a) = control(
                    ui,
                    &mut state.hits,
                    Action::Unlink,
                    "断开所选边",
                    state.edge.is_some(),
                ) {
                    action = Some(a);
                }
            });
            if !state.graph.unresolved.is_empty() {
                ui.collapsing("无法解析的连接（未猜测或删除）", |ui| {
                    for edge in &state.graph.unresolved {
                        ui.label(format!(
                            "组 {} / 边 {}：{}",
                            edge.reference.group,
                            edge.reference.index + 1,
                            edge.reason
                        ));
                    }
                });
            }
            if !document.status.is_empty() {
                ui.colored_label(egui::Color32::from_rgb(230, 120, 80), &document.status);
            }
        });
    state.open = open;
    action
}

pub fn draw(
    mut contexts: EguiContexts,
    mut state: ResMut<ConnectionEditor>,
    mut document: ResMut<EditorDocument>,
    inspector: Res<properties::Inspector>,
    pending: Option<Res<files::PendingFileAction>>,
) {
    if inspector.is_open() || pending.is_some_and(|state| state.is_blocked()) {
        return;
    }
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };
    if let Some(action) = show(ctx, &mut state, &document) {
        act(action, &mut state, &mut document);
    }
}

#[cfg(test)]
mod tests;

pub mod smoke;
