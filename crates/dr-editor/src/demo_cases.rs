//! 详细模式专用边界回放。内部输入与精确事务明确区分，失败断言不会写通过证据。
use super::*;
use bevy::ecs::system::SystemParam;
use bevy_egui::EguiInput;
use dr_core::geometry::Vec2d;
use std::time::Instant;

mod geometry;
mod gestures;
pub(crate) use gestures::verify;
mod interactions;
mod manifest;
#[cfg(test)]
mod tests;
mod workflow;
pub(crate) use manifest::cases;
use manifest::{Cancel, Case, Scenario};

#[derive(Resource, Default)]
pub(crate) struct Captured(BTreeSet<String>);

#[derive(Resource, Default)]
pub(crate) struct Run {
    chapter: Option<&'static str>,
    case_index: usize,
    step: u16,
    input: egui_ui::UiTestInput,
    pointer: Option<Vec2>,
    before: Option<Ship>,
    catalog: Option<PartCatalog>,
    after: Option<Ship>,
    candidate: Option<Part>,
    metrics: serde_json::Value,
    gesture: gestures::Probe,
    verified: bool,
    capturing: bool,
    zero_path_checked: bool,
    read_until: Option<Instant>,
    results: Vec<serde_json::Value>,
}

impl Run {
    pub(crate) fn new(chapter: &'static str) -> Self {
        Self {
            chapter: (!cases(chapter).is_empty()).then_some(chapter),
            ..default()
        }
    }

    pub(crate) fn caption(&self) -> Option<String> {
        let list = cases(self.chapter?);
        let case = list.get(self.case_index)?;
        Some(format!(
            "样例 {}/{} · {}\n预期：{}\n路径：{}{}",
            self.case_index + 1,
            list.len(),
            case.title,
            case.expected,
            case.input_path,
            if self.verified {
                " · 断言通过"
            } else {
                ""
            }
        ))
    }
}

pub(crate) fn artifacts(chapter: &str) -> Vec<String> {
    cases(chapter)
        .iter()
        .map(|case| format!("case-{}.png", case.id))
        .collect()
}

#[derive(SystemParam)]
pub(crate) struct Context<'w, 's> {
    document: ResMut<'w, EditorDocument>,
    cursor: ResMut<'w, EditorCursor>,
    drag: ResMut<'w, DragState>,
    camera_drag: ResMut<'w, CameraDrag>,
    cameras: Query<'w, 's, (&'static mut Transform, &'static mut Projection), With<Camera2d>>,
    options: ResMut<'w, view::ViewOptions>,
    lines: ResMut<'w, connection_lines::Settings>,
    line_controls: Res<'w, connection_lines::Controls>,
    configs: Res<'w, bevy::gizmos::config::GizmoConfigStore>,
    topology: ResMut<'w, topology_ui::ConnectionEditor>,
    inspector: ResMut<'w, properties::Inspector>,
    property_hits: Res<'w, egui_ui::UiHits>,
    pending: Res<'w, files::PendingFileAction>,
    paths: ResMut<'w, EditorPaths>,
    windows: Query<'w, 's, &'static mut Window, With<bevy::window::PrimaryWindow>>,
    inputs: Query<'w, 's, &'static mut EguiInput, With<bevy_egui::PrimaryEguiContext>>,
    mouse: ResMut<'w, ButtonInput<MouseButton>>,
    keys: ResMut<'w, ButtonInput<KeyCode>>,
    file_actions: MessageWriter<'w, files::FileAction>,
}

impl Context<'_, '_> {
    fn point(&mut self, state: &mut Run, point: (f64, f64)) {
        let mut window = self.windows.single_mut().unwrap();
        let position = Vec2::new(
            window.width() / 2. + point.0 as f32 * 60.,
            window.height() / 2. - point.1 as f32 * 60.,
        );
        window.set_cursor_position(Some(position));
        state.pointer = Some(position);
    }

    fn click_point(&mut self, state: &mut Run, point: (f64, f64)) {
        self.point(state, point);
        self.mouse.press(MouseButton::Left);
    }

    fn endpoint(&self, id: i64, attach: usize) -> (f64, f64) {
        let part = self.document.ship.part(id).unwrap();
        let kind = self.document.catalog.get(&part.part_type).unwrap();
        let point = dr_core::part_world_attach(part, &kind.attach_points[attach]);
        (point.x, point.y)
    }

    fn key(&mut self, key: KeyCode, control: bool, shift: bool) {
        if control {
            self.keys.press(KeyCode::ControlLeft);
        }
        if shift {
            self.keys.press(KeyCode::ShiftLeft);
        }
        self.keys.press(key);
    }

    fn line_button(&mut self, state: &mut Run, action: connection_lines::Control) -> bool {
        let Some((_, rect)) = self
            .line_controls
            .0
            .iter()
            .find(|(candidate, _)| *candidate == action)
        else {
            return false;
        };
        state
            .input
            .click_rect(*rect, &mut self.inputs.single_mut().unwrap());
        true
    }

    fn topology_button(&mut self, state: &mut Run, action: topology_ui::Action) -> bool {
        let Some((_, rect)) = self
            .topology
            .hits
            .iter()
            .find(|(candidate, _)| *candidate == action)
        else {
            return false;
        };
        state
            .input
            .click_rect(*rect, &mut self.inputs.single_mut().unwrap());
        true
    }

    fn property_button(&mut self, state: &mut Run, action: properties::Action) -> bool {
        let Some((_, rect)) = self
            .property_hits
            .actions
            .iter()
            .find(|(candidate, _)| *candidate == action)
        else {
            return false;
        };
        state
            .input
            .click_rect(*rect, &mut self.inputs.single_mut().unwrap());
        true
    }
}

fn key(id: i64) -> PartKey {
    PartKey::new(0, id, 0)
}

fn fixture(document: &EditorDocument) -> Ship {
    let fuselage = document.catalog.get("fuselage-1").unwrap();
    let cone = document.catalog.get("nosecone-1").unwrap();
    Ship {
        parts: vec![
            fuselage.instantiate(1, (-2.5, 1.5)),
            fuselage.instantiate(2, (-0.3, -0.8)),
            fuselage.instantiate(3, (2.2, -0.8)),
            cone.instantiate(4, (3.6, 2.4)),
        ],
        connections: vec![normal(1, 2), normal(2, 3)],
        ..default()
    }
}

fn normal(parent: i64, child: i64) -> Connection {
    Connection::Normal {
        parent,
        child,
        parent_attach: 4,
        child_attach: 3,
    }
}

fn unchanged(state: &Run, document: &EditorDocument) {
    assert_eq!(
        Some(&document.ship),
        state.before.as_ref(),
        "拒绝/取消操作改写了文档"
    );
    assert_eq!(document.history.undo_len(), 0, "空操作或失败操作产生了历史");
    assert!(!document.dirty, "空操作或失败操作改变了脏标记");
}

fn close_enough(a: &Ship, b: &Ship) {
    assert_eq!(a.connections, b.connections);
    assert_eq!(a.all_parts().count(), b.all_parts().count());
    for (key, part) in a.keyed_parts() {
        let other = b.part_at(key).unwrap();
        assert!((part.x - other.x).hypot(part.y - other.y) < 1e-8);
        let angle = (part.angle - other.angle + std::f64::consts::PI)
            .rem_euclid(std::f64::consts::TAU)
            - std::f64::consts::PI;
        assert!(angle.abs() < 1e-8);
        assert_eq!((part.flip_x, part.flip_y), (other.flip_x, other.flip_y));
    }
}

fn setup(state: &mut Run, ctx: &mut Context, case: &Case) {
    if let Some(catalog) = &state.catalog {
        ctx.document.catalog = catalog.clone();
    } else {
        state.catalog = Some(ctx.document.catalog.clone());
    }
    *ctx.cursor = EditorCursor::default();
    *ctx.drag = DragState::default();
    *ctx.camera_drag = CameraDrag::default();
    let (mut camera, mut projection) = ctx.cameras.single_mut().unwrap();
    camera.translation.x = 0.;
    camera.translation.y = 0.;
    if let Projection::Orthographic(projection) = &mut *projection {
        projection.scale = 1.;
    }
    *ctx.options = view::ViewOptions::default();
    *ctx.lines = connection_lines::Settings::default();
    *ctx.topology = topology_ui::ConnectionEditor::default();
    *ctx.inspector = properties::Inspector::default();
    ctx.paths.ship = None;
    ctx.document.ship = fixture(&ctx.document);
    ctx.document.free_mode = true;
    ctx.document.clear_selection();
    ctx.mouse.reset_all();
    ctx.keys.reset_all();
    state.pointer = None;
    state.candidate = None;
    state.after = None;
    state.metrics = serde_json::json!({});
    state.gesture = gestures::Probe::default();
    geometry::setup(state, ctx, case.scenario);
    interactions::setup(ctx, case.scenario);
    workflow::setup(ctx, case.scenario);
    gestures::setup(ctx, case.scenario);
    ctx.document.saved_ship = ctx.document.ship.clone();
    ctx.document.history = EditorHistory::default();
    ctx.document.status.clear();
    ctx.document.refresh();
    state.before = Some(ctx.document.ship.clone());
    state.step = 1;
    state.verified = false;
    state.capturing = false;
    state.zero_path_checked = false;
    state.read_until = None;
    info!(
        "详细样例 {}：{}；预期 {}",
        case.id, case.title, case.expected
    );
}

pub(crate) fn run(
    mut state: ResMut<Run>,
    mut ctx: Context,
    mut commands: Commands,
    captured: Res<Captured>,
    mut showcase: Option<ResMut<demo::Showcase>>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(chapter) = state.chapter else {
        return;
    };
    let list = cases(chapter);
    if state.case_index == list.len() {
        let report = serde_json::json!({"completed":true, "chapter":chapter, "input_mode":"内部 UI/画布与精确事务；不是系统键鼠或输入法验收", "cases":state.results});
        let path = format!("target/{chapter}.json");
        std::fs::write(&path, serde_json::to_vec_pretty(&report).unwrap())
            .expect("无法写详细样例证据");
        state.chapter = None;
        exit.write(AppExit::Success);
        return;
    }
    let case = &list[state.case_index];
    if state.verified {
        if captured.0.contains(case.id) {
            let pause = showcase
                .as_deref()
                .map_or(std::time::Duration::ZERO, demo::Showcase::case_result_pause);
            let until = *state
                .read_until
                .get_or_insert_with(|| Instant::now() + pause);
            if Instant::now() < until {
                return;
            }
            let metrics = state.metrics.clone();
            state.results.push(serde_json::json!({"id":case.id, "title":case.title, "expected":case.expected, "input_path":case.input_path, "status":"passed", "screenshot":format!("case-{}.png", case.id), "metrics":metrics}));
            state.case_index += 1;
            state.step = 0;
            state.verified = false;
            state.capturing = false;
        } else if !state.capturing {
            use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk};
            let id = case.id;
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(format!("target/case-{id}.png")))
                .observe(
                    move |_: On<ScreenshotCaptured>, mut captured: ResMut<Captured>| {
                        captured.0.insert(id.to_owned());
                    },
                );
            state.capturing = true;
        }
        return;
    }
    if let Ok(mut window) = ctx.windows.single_mut() {
        window.focused = true;
        if let Some(pointer) = state.pointer {
            window.set_cursor_position(Some(pointer));
        }
    } else {
        return;
    }
    ctx.keys.reset_all();
    if state.input.tick(&mut ctx.inputs.single_mut().unwrap()) {
        return;
    }
    if state.step == 0 {
        setup(&mut state, &mut ctx, case);
        demo::describe(&mut showcase, case.expected);
        return;
    }
    let done = match case.scenario {
        Scenario::DragContinuous
        | Scenario::PanGesture
        | Scenario::HintSinglePair
        | Scenario::ManualDuplicatePair => gestures::advance(&mut state, &mut ctx, case.scenario),
        Scenario::Overlap { .. } | Scenario::Contained => {
            geometry::advance(&mut state, &mut ctx, case.scenario)
        }
        Scenario::SelfLink
        | Scenario::Cycle(_)
        | Scenario::CrossGroup
        | Scenario::EmptyHistory
        | Scenario::BatchRollback
        | Scenario::UndoBranch
        | Scenario::InvalidFuel(_)
        | Scenario::StaleDraft
        | Scenario::InvalidOpen
        | Scenario::SaveFailure
        | Scenario::UnsavedCancel => workflow::advance(&mut state, &mut ctx, case.scenario),
        _ => interactions::advance(&mut state, &mut ctx, case.scenario),
    };
    if done {
        gestures::report(&mut state);
        state.metrics["part_count"] = ctx.document.ship.all_parts().count().into();
        state.metrics["undo_len"] = ctx.document.history.undo_len().into();
        state.metrics["redo_len"] = ctx.document.history.redo_len().into();
        state.metrics["dirty"] = ctx.document.dirty.into();
        state.verified = true;
        info!("详细样例 {} 断言通过", case.id);
    }
}

/// 精确几何样例的候选轮廓不伪造实际鼠标预览；颜色表示真实事务的允许/拒绝。
pub(crate) fn draw(state: Res<Run>, document: Res<EditorDocument>, mut gizmos: Gizmos) {
    if state.chapter.is_none() {
        return;
    }
    let Some(part) = &state.candidate else {
        return;
    };
    let Some(kind) = document.catalog.get(&part.part_type) else {
        return;
    };
    let allowed = state.metrics["accepted"].as_bool().unwrap_or(false);
    let color = if allowed {
        Color::srgb(0.3, 1.0, 0.6)
    } else {
        Color::srgb(1.0, 0.25, 0.25)
    };
    let point = |p: Vec2d| Vec3::new(p.x as f32 * 60., p.y as f32 * 60., 8.);
    for shape in dr_core::geometry::world_shapes(part, kind) {
        match shape {
            dr_core::geometry::WorldShape::Polygon(vertices) => {
                for (a, b) in vertices
                    .iter()
                    .zip(vertices.iter().cycle().skip(1))
                    .take(vertices.len())
                {
                    gizmos.line(point(*a), point(*b), color);
                }
            }
            dr_core::geometry::WorldShape::Circle(center, radius) => {
                gizmos.circle(
                    Isometry3d::from_translation(point(center)),
                    radius as f32 * 60.,
                    color,
                );
            }
        }
    }
}

/// 与生产渲染一致，按父中心→父挂点→子挂点→子中心构造真实连接路径。
fn connection_path(document: &EditorDocument) -> [Vec2; 4] {
    let connection = &document.ship.connections[0];
    let Connection::Normal { parent, child, .. } = connection else {
        panic!("零长夹具需要普通连接");
    };
    let parent = document.ship.part(*parent).unwrap();
    let child = document.ship.part(*child).unwrap();
    let (a, b) =
        dr_core::connections::positions_between(&document.catalog, connection, parent, child)
            .unwrap();
    [
        Vec2::new(parent.x as f32, parent.y as f32),
        Vec2::new(a.x as f32, a.y as f32),
        Vec2::new(b.x as f32, b.y as f32),
        Vec2::new(child.x as f32, child.y as f32),
    ]
    .map(|point| point * 60.0)
}

pub(crate) fn draw_degenerate(
    mut state: ResMut<Run>,
    document: Res<EditorDocument>,
    lines: Res<connection_lines::Settings>,
    mut gizmos: Gizmos<connection_lines::LineGizmos>,
) {
    let Some(chapter) = state.chapter else {
        return;
    };
    if cases(chapter)
        .get(state.case_index)
        .is_some_and(|case| matches!(case.scenario, Scenario::ZeroLength))
        && state.step > 0
        && !state.zero_path_checked
    {
        let points = connection_path(&document);
        assert!(points.iter().all(|point| point.is_finite()));
        let length: f32 = points
            .windows(2)
            .map(|pair| pair[0].distance(pair[1]))
            .sum();
        assert_eq!(length, 0.0);
        let directions = [0.0, 0.5, 1.0]
            .into_iter()
            .filter_map(|fraction| connection_lines::along(&points, fraction))
            .count();
        assert_eq!(directions, 0);
        assert!(lines.enabled && lines.arrows && lines.effect == connection_lines::Effect::Flow);
        assert!(gizmos.config.enabled);
        let before = gizmos.list_positions.len() + gizmos.strip_positions.len();
        connection_lines::draw_path(&mut gizmos, &lines, &points, 0.5, 1.0);
        let queued = gizmos.list_positions.len() + gizmos.strip_positions.len() - before;
        assert_eq!(queued, 0, "退化路径不应提交连线或箭头顶点");
        state.metrics = serde_json::json!({
            "coincident_parts": points[0] == points[3],
            "path_point_count": points.len(),
            "path_length": length,
            "direction_sample_count": directions,
            "queued_vertex_count": queued,
            "degenerate_path_safely_skipped": queued == 0 && directions == 0,
            "flow_enabled": lines.effect == connection_lines::Effect::Flow,
            "arrows_enabled": lines.arrows,
        });
        state.zero_path_checked = true;
    }
}
