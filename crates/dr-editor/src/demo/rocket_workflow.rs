//! 从默认 Command Pod 连续搭建、自动分级、真实分帧拖放；只向生产输入路径送事件。
use super::*;
use bevy::asset::{AssetId, LoadState};
use bevy::ecs::{query::QueryData, system::SystemParam};
use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk};
use bevy::sprite::Anchor;
use bevy::sprite_render::{MeshMaterial2d, SpriteMeshMaterial};
use bevy_egui::{EguiInput, PrimaryEguiContext, egui};
use dr_core::StagingState;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

const PARTS: &[&str] = &[
    "fueltank-0",
    "engine-0",
    "detacher-1",
    "fueltank-0",
    "engine-0",
    "detacher-1",
    "fueltank-0",
    "engine-0",
];
const AUTO: &[&[i64]] = &[&[9], &[7], &[6], &[4], &[3]];
const MANUAL: &[&[i64]] = &[&[9], &[7, 6], &[], &[4], &[3]];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Phase {
    #[default]
    Build,
    Generate,
    Manual,
    Finished,
}

#[derive(Resource, Default)]
pub(crate) struct State {
    phase: Phase,
    step: u8,
    part: usize,
    input: egui_ui::UiTestInput,
    before: Option<Ship>,
    assembled: Option<Ship>,
    automatic: Option<Ship>,
    manual: Option<StagingState>,
    revision: u64,
    plan: Option<Placement>,
    seeking: Option<usize>,
    captures: Option<Arc<AtomicBool>>,
    placements: Vec<serde_json::Value>,
    traces: Vec<serde_json::Value>,
    gesture: Option<Gesture>,
    frame: u64,
    attempts: u16,
    cases: Vec<serde_json::Value>,
    visuals: Option<serde_json::Value>,
}

#[derive(SystemParam)]
pub(crate) struct Context<'w, 's> {
    document: Res<'w, EditorDocument>,
    cursor: Res<'w, EditorCursor>,
    inspector: Res<'w, properties::Inspector>,
    ui: Res<'w, panels::egui_panel::UiState>,
    hits: Res<'w, egui_ui::UiHits>,
    assets: Res<'w, AssetServer>,
    images: Res<'w, Assets<Image>>,
    materials: Res<'w, Assets<SpriteMeshMaterial>>,
    visuals: Query<'w, 's, VisualComponents>,
    windows: Query<'w, 's, (Entity, &'static mut Window), With<bevy::window::PrimaryWindow>>,
    inputs: Query<'w, 's, &'static mut EguiInput, With<PrimaryEguiContext>>,
    cameras: Query<'w, 's, (&'static Transform, &'static Projection), With<Camera2d>>,
    mouse: ResMut<'w, ButtonInput<MouseButton>>,
    keys: ResMut<'w, ButtonInput<KeyCode>>,
    wheels: MessageWriter<'w, MouseWheel>,
}

#[derive(QueryData)]
struct VisualComponents {
    entity: Entity,
    visual: &'static render::PartVisual,
    sprite: &'static Sprite,
    local: &'static Transform,
    global: &'static GlobalTransform,
    visibility: &'static Visibility,
    view: Option<&'static ViewVisibility>,
    material: Option<&'static MeshMaterial2d<SpriteMeshMaterial>>,
    anchor: Option<&'static Anchor>,
}

#[derive(Clone, Debug)]
struct VisualObservation {
    entity: Entity,
    key: PartKey,
    expected_image: Option<AssetId<Image>>,
    image: AssetId<Image>,
    image_size: Option<UVec2>,
    load_failed: bool,
    material_image: Option<AssetId<Image>>,
    local: Vec3,
    global: Vec3,
    visible: bool,
    in_view: Option<bool>,
    record: serde_json::Value,
}

#[derive(Debug, PartialEq, Eq)]
enum VisualIssue {
    Pending(String),
    Invalid(String),
}

/// 只给异步图片或尚未应用的 Commands 留等待窗口；身份、坐标与可见性错配立即失败。
fn check_visuals(ship: &Ship, observations: &[VisualObservation]) -> Result<(), VisualIssue> {
    let expected: std::collections::BTreeMap<_, _> = ship
        .keyed_parts()
        .map(|(key, part)| (key, Vec2::new(part.x as f32 * 60.0, part.y as f32 * 60.0)))
        .collect();
    let mut keys = BTreeSet::new();
    let mut entities = BTreeSet::new();
    let mut pending = None;
    for observed in observations {
        let key = observed.key;
        if !keys.insert(key) || !entities.insert(observed.entity) {
            return Err(VisualIssue::Invalid(format!(
                "视觉 key 或 Entity 重复：{key:?}"
            )));
        }
        let Some(position) = expected.get(&key) else {
            return Err(VisualIssue::Invalid(format!(
                "文档中不存在的视觉实体：{key:?}"
            )));
        };
        if let Some(image) = observed.expected_image {
            if observed.image != image {
                return Err(VisualIssue::Invalid(format!(
                    "目录贴图与 Sprite.image 不符：{key:?}"
                )));
            }
        } else {
            pending = Some(format!("目录图片句柄尚未注册：{key:?}"));
        }
        if observed.load_failed {
            return Err(VisualIssue::Invalid(format!("图片加载失败：{key:?}")));
        }
        match observed.image_size {
            Some(size) if size.x == 0 || size.y == 0 => {
                return Err(VisualIssue::Invalid(format!("已加载图片尺寸为空：{key:?}")));
            }
            None => pending = Some(format!("图片尚未解码：{key:?}")),
            _ => {}
        }
        match observed.material_image {
            Some(image) if image != observed.image => {
                return Err(VisualIssue::Invalid(format!(
                    "派生 SpriteMeshMaterial.image 与 Sprite.image 不符：{key:?}"
                )));
            }
            None => pending = Some(format!("派生 SpriteMeshMaterial 尚未同步：{key:?}")),
            _ => {}
        }
        if !observed.local.is_finite()
            || !observed.global.is_finite()
            || observed.local.truncate().distance(*position) > 1e-3
            || observed.global.truncate().distance(*position) > 1e-3
        {
            return Err(VisualIssue::Invalid(format!(
                "视觉 local/global 坐标与 part*60 不符：{key:?}"
            )));
        }
        if !observed.visible || observed.in_view == Some(false) {
            return Err(VisualIssue::Invalid(format!(
                "F 适配后视觉实体被隐藏或不在镜头内：{key:?}"
            )));
        }
        if observed.in_view.is_none() {
            pending = Some(format!("ViewVisibility 尚未同步：{key:?}"));
        }
    }
    if keys.len() != expected.len() {
        pending = Some(format!(
            "视觉实体尚未齐备：{}/{}，缺少 {:?}",
            keys.len(),
            expected.len(),
            expected
                .keys()
                .filter(|key| !keys.contains(*key))
                .collect::<Vec<_>>()
        ));
    }
    pending.map_or(Ok(()), |reason| Err(VisualIssue::Pending(reason)))
}

impl Context<'_, '_> {
    fn observe_visuals(&self) -> Vec<VisualObservation> {
        let mut observations: Vec<_> = self.visuals.iter().map(|observed| {
            let key = PartKey::new(observed.visual.group, observed.visual.id, observed.visual.occurrence);
            let part = self.document.ship.part_at(key);
            let path = part.and_then(|part| self.document.catalog.get(&part.part_type))
                .map(|kind| render::part_sprite_path(&kind.sprite));
            let expected_image = path.as_ref().and_then(|path| self.assets.get_handle::<Image>(path.clone()))
                .map(|handle| handle.id());
            let image = observed.sprite.image.id();
            let image_size = self.images.get(image).map(Image::size);
            let load_state = self.assets.get_load_state(image);
            let material = observed.material.and_then(|handle| self.materials.get(&handle.0));
            let material_image = material.map(|material| material.image.id());
            let local = observed.local.translation;
            let global = observed.global.translation();
            let visible = *observed.visibility != Visibility::Hidden;
            let in_view = observed.view.map(|view| view.get());
            VisualObservation {
                entity: observed.entity, key, expected_image, image, image_size,
                load_failed: matches!(&load_state, Some(LoadState::Failed(_))),
                material_image, local, global, visible, in_view,
                record: serde_json::json!({
                    "id":key.id, "group":key.group, "occurrence":key.occurrence,
                    "entity":format!("{:?}", observed.entity),
                    "part_type":part.map(|part| &part.part_type), "expected_path":path,
                    "expected_image":expected_image.map(|id| format!("{id:?}")),
                    "image":format!("{image:?}"),
                    "image_path":self.assets.get_path(image).map(|path| path.to_string()),
                    "image_loaded":image_size.is_some(), "image_size":image_size.map(|size| size.to_array()),
                    "image_load_state":format!("{load_state:?}"),
                    "material":observed.material.map(|handle| format!("{:?}", handle.0.id())),
                    "material_image":material_image.map(|id| format!("{id:?}")),
                    "material_image_path":material_image.and_then(|id| self.assets.get_path(id)).map(|path| path.to_string()),
                    "material_anchor":material.map(|material| material.anchor.to_array()),
                    "material_custom_size":material.and_then(|material| material.custom_size).map(|size| size.to_array()),
                    "material_flip":material.map(|material| [material.flip_x, material.flip_y]),
                    "material_color":material.map(|material| format!("{:?}", material.color)),
                    "anchor":observed.anchor.map(|anchor| anchor.0.to_array()),
                    "sprite_custom_size":observed.sprite.custom_size.map(|size| size.to_array()),
                    "sprite_flip":[observed.sprite.flip_x, observed.sprite.flip_y],
                    "sprite_color":format!("{:?}", observed.sprite.color),
                    "local_translation":local.to_array(), "global_translation":global.to_array(),
                    "visibility":format!("{:?}", observed.visibility), "in_view":in_view,
                }),
            }
        }).collect();
        observations.sort_by_key(|observation| observation.key);
        observations
    }
}

#[derive(Clone, Copy, Debug)]
struct Placement {
    position: (f64, f64),
    parent: i64,
    child: i64,
    parent_attach: i32,
    child_attach: i32,
}

/// 落点来自实际目录连接点；不按部件图片高度或固定屏幕坐标猜位置。
fn placement(document: &EditorDocument, parent: i64, kind_id: &str) -> Placement {
    placement_from(
        &document.catalog,
        document.ship.part(parent).expect("搭建父部件不存在"),
        document.ship.next_part_id(),
        kind_id,
    )
}

fn placement_from(
    catalog: &PartCatalog,
    parent_part: &dr_core::Part,
    child_id: i64,
    kind_id: &str,
) -> Placement {
    let parent_kind = catalog.get(&parent_part.part_type).unwrap();
    let child_kind = catalog.get(kind_id).expect("原版目录缺少搭建部件");
    let child = child_kind.instantiate(child_id, (0.0, 0.0));
    let (parent_index, parent_point) = parent_kind
        .attach_points
        .iter()
        .enumerate()
        .filter(|(_, point)| !point.dock)
        .map(|(index, point)| (index, dr_core::part_world_attach(parent_part, point)))
        .min_by(|(_, a), (_, b)| a.y.total_cmp(&b.y))
        .expect("父部件需要底部连接点");
    let (child_index, child_point) = child_kind
        .attach_points
        .iter()
        .enumerate()
        .filter(|(_, point)| !point.dock)
        .map(|(index, point)| (index, dr_core::part_world_attach(&child, point)))
        .max_by(|(_, a), (_, b)| a.y.total_cmp(&b.y))
        .expect("新部件需要顶部连接点");
    Placement {
        position: (
            parent_point.x - child_point.x,
            parent_point.y - child_point.y,
        ),
        parent: parent_part.id,
        child: child.id,
        parent_attach: parent_index as i32 + 1,
        child_attach: child_index as i32 + 1,
    }
}

/// 只计算未来串联末端，给生产滚轮缩放留足画布；不创建文档部件或历史。
fn assembly_bottom(document: &EditorDocument) -> (f64, f64) {
    let mut parent = document.ship.part(1).unwrap().clone();
    for kind_id in PARTS {
        let plan = placement_from(&document.catalog, &parent, parent.id + 1, kind_id);
        parent = document
            .catalog
            .get(kind_id)
            .unwrap()
            .instantiate(plan.child, plan.position);
    }
    let bottom = document
        .catalog
        .get(&parent.part_type)
        .unwrap()
        .attach_points
        .iter()
        .filter(|point| !point.dock)
        .map(|point| dr_core::part_world_attach(&parent, point))
        .min_by(|a, b| a.y.total_cmp(&b.y))
        .unwrap();
    (bottom.x, bottom.y)
}

fn screen(window: &Window, camera: &Transform, projection: &Projection, point: (f64, f64)) -> Vec2 {
    let Projection::Orthographic(projection) = projection else {
        panic!("火箭工作流需要二维正交相机")
    };
    Vec2::new(
        window.width() * 0.5 + (point.0 as f32 * 60.0 - camera.translation.x) / projection.scale,
        window.height() * 0.5 - (point.1 as f32 * 60.0 - camera.translation.y) / projection.scale,
    )
}

fn staging(ship: &Ship) -> Option<&StagingState> {
    ship.part(1)?.pod.as_ref()?.staging.as_ref()
}

fn rows(staging: &StagingState) -> Vec<Vec<i64>> {
    staging
        .steps
        .iter()
        .map(|step| step.activations.iter().map(|a| a.id).collect())
        .collect()
}

fn inventory(staging: &StagingState) -> Vec<i64> {
    let mut ids: Vec<_> = staging
        .steps
        .iter()
        .flat_map(|step| &step.activations)
        .map(|a| a.id)
        .collect();
    ids.sort_unstable();
    ids
}

fn assert_staging(actual: &StagingState, expected: &[&[i64]], moved: bool) {
    let expected: Vec<Vec<i64>> = expected.iter().map(|row| row.to_vec()).collect();
    assert_eq!(rows(actual), expected, "真实 UI 产生的分级顺序错误");
    assert_eq!(actual.current_stage, 0);
    assert_eq!(
        inventory(actual),
        vec![3, 4, 6, 7, 9],
        "激活动作不能丢失或复制"
    );
    for activation in actual.steps.iter().flat_map(|step| &step.activations) {
        assert_eq!(activation.moved, moved && activation.id == 6);
    }
}

fn assert_rocket(document: &EditorDocument) -> f64 {
    assert_eq!(document.ship.parts.len(), 9);
    assert_eq!(document.ship.connections.len(), 8);
    assert!(document.ship.disconnected.is_empty());
    assert_eq!(
        document
            .ship
            .parts
            .iter()
            .filter(|p| document.catalog.get(&p.part_type).unwrap().kind == PartKind::Engine)
            .count(),
        3
    );
    assert_eq!(
        document
            .ship
            .parts
            .iter()
            .filter(|p| document.catalog.get(&p.part_type).unwrap().kind == PartKind::Detacher)
            .count(),
        2
    );
    let mut maximum: f64 = 0.0;
    for (index, edge) in document.ship.connections.iter().enumerate() {
        let Connection::Normal {
            parent,
            child,
            parent_attach,
            child_attach,
        } = edge
        else {
            panic!("串联火箭只应产生 Normal 接触连接")
        };
        assert_eq!((*parent, *child), (index as i64 + 1, index as i64 + 2));
        let endpoint = |id, attach: i32| {
            let part = document.ship.part(id).unwrap();
            let kind = document.catalog.get(&part.part_type).unwrap();
            dr_core::part_world_attach(part, &kind.attach_points[(attach - 1) as usize])
        };
        let error = endpoint(*parent, *parent_attach).distance(endpoint(*child, *child_attach));
        assert!(
            error < 1e-5,
            "真实吸附连接端点不一致：{edge:?}，误差 {error}"
        );
        maximum = maximum.max(error);
    }
    maximum
}

fn xml(ship: &Ship, name: &str) {
    let path = format!("target/{name}");
    save_ship(&path, ship).expect("无法保存工作流 XML 证据");
    assert_eq!(
        load_ship(&path).expect("工作流 XML 无法重载"),
        *ship,
        "工作流 XML 往返不一致"
    );
}

fn capture(commands: &mut Commands, state: &mut State, name: &'static str) {
    let done = Arc::new(AtomicBool::new(false));
    let signal = done.clone();
    commands
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(format!("target/{name}")))
        .observe(move |_: On<ScreenshotCaptured>| {
            signal.store(true, Ordering::Release);
        });
    state.captures = Some(done);
}

impl Context<'_, '_> {
    fn point(&mut self, point: (f64, f64)) {
        let (_, mut window) = self.windows.single_mut().unwrap();
        let (camera, projection) = self.cameras.single().unwrap();
        let point = screen(&window, camera, projection, point);
        assert!(point.is_finite());
        window.set_cursor_position(Some(point));
    }

    fn key(&mut self, key: KeyCode, control: bool) {
        if control {
            self.keys.press(KeyCode::ControlLeft);
        }
        self.keys.press(key);
    }

    fn click(&mut self, state: &mut State, action: properties::Action) -> bool {
        state
            .input
            .click(action, &self.hits, &mut self.inputs.single_mut().unwrap())
    }

    fn draft(&self) -> &StagingState {
        self.inspector
            .draft
            .as_ref()
            .expect("生产分级面板未打开")
            .staging
            .as_ref()
            .expect("分级草稿缺失")
    }

    fn unchanged(&self, ship: &Ship, history: usize) {
        assert_eq!(self.document.ship, *ship, "草稿或预览提前修改了文档");
        assert_eq!(self.document.history.undo_len(), history);
        assert_eq!(self.document.history.redo_len(), 0);
    }

    fn choose_part(&mut self, state: &mut State, index: usize) -> bool {
        let action = panels::PanelButton::Part(index);
        let mut input = self.inputs.single_mut().unwrap();
        if self
            .ui
            .hits
            .iter()
            .any(|(candidate, _)| *candidate == action)
        {
            state.seeking = None;
        } else if !self.ui.scrolling
            && let Some(area) = self.ui.palette_area
            && state.seeking != Some(index)
        {
            // 目录按类别排列而非 catalog 索引排列。换件先真实滚回顶部，
            // 后续交给共享驱动逐段向下查找，不推测固定行号或屏幕坐标。
            state.seeking = Some(index);
            wheel(&mut input, area.center(), 100_000.0);
            return false;
        }
        state.input.click_panel(action, &self.ui, &mut input)
    }
}

fn wheel(input: &mut EguiInput, pos: egui::Pos2, delta: f32) {
    input.0.events.extend([
        egui::Event::PointerMoved(pos),
        egui::Event::MouseWheel {
            phase: egui::TouchPhase::Move,
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, delta),
            modifiers: egui::Modifiers::NONE,
        },
    ]);
}

#[derive(Clone, Copy, Debug)]
enum Drag {
    Activation,
    CardDown,
    CardUp,
}

struct Gesture {
    kind: Drag,
    source: egui::Pos2,
    target: egui::Pos2,
    before: StagingState,
    events: Vec<serde_json::Value>,
}

fn pointer(input: &mut EguiInput, pos: egui::Pos2, pressed: Option<bool>) {
    input.0.events.push(egui::Event::PointerMoved(pos));
    if let Some(pressed) = pressed {
        input.0.events.push(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
    }
}

fn drag_frame(
    input: &mut EguiInput,
    source: egui::Pos2,
    target: egui::Pos2,
    step: u8,
) -> &'static str {
    let (event, point, pressed) = match step {
        0 => ("press", source, Some(true)),
        1 => ("held_threshold", source + egui::vec2(14.0, 0.0), None),
        2 => ("held_target", target, None),
        3 => ("release", target, Some(false)),
        _ => unreachable!(),
    };
    pointer(input, point, pressed);
    event
}

/// 拖放严格分为按下、越阈值保持、悬停落点、释放四个独立 Update/pass。
fn drag(context: &mut Context, state: &mut State, kind: Drag, step: u8) -> bool {
    use properties::Action::{SelectActivation, SelectStage};
    let draft = context.draft().clone();
    let mut input = context.inputs.single_mut().unwrap();
    if step == 0 {
        if context.hits.scrolling {
            return false;
        }
        let (source, target) = match kind {
            Drag::Activation => (SelectActivation(2, 0), SelectStage(1)),
            Drag::CardDown => (SelectStage(0), SelectStage(1)),
            Drag::CardUp => (SelectStage(1), SelectStage(0)),
        };
        let rect = |action: properties::Action| {
            context
                .hits
                .actions
                .iter()
                .find(|(candidate, rect)| *candidate == action && rect.is_positive())
                .map(|(_, rect)| *rect)
        };
        let (Some(source), Some(target)) = (rect(source), rect(target)) else {
            if let Some(body) = context.hits.body {
                wheel(&mut input, body.center(), 350.0);
            }
            return false;
        };
        let target = match kind {
            Drag::Activation => target.center(),
            Drag::CardDown => egui::pos2(target.center().x, target.bottom() - 2.0),
            Drag::CardUp => egui::pos2(target.center().x, target.top() + 2.0),
        };
        let event = drag_frame(&mut input, source.center(), target, 0);
        state.gesture = Some(Gesture {
            kind,
            source: source.center(),
            target,
            before: draft,
            events: vec![serde_json::json!({"event":event, "frame":state.frame})],
        });
    } else {
        let gesture = state.gesture.as_mut().unwrap();
        assert_eq!(draft, gesture.before, "释放前只能有拖动预览，不能修改草稿");
        let event = drag_frame(&mut input, gesture.source, gesture.target, step);
        gesture
            .events
            .push(serde_json::json!({"event":event, "frame":state.frame}));
        if step == 3 {
            let gesture = state.gesture.take().unwrap();
            state.traces.push(serde_json::json!({"kind":format!("{:?}", gesture.kind),
                "source":[gesture.source.x,gesture.source.y], "target":[gesture.target.x,gesture.target.y],
                "threshold_distance":14.0, "events":gesture.events}));
        }
    }
    true
}

/// 与其他章节共用 advance_ready/animate；没有直接执行 Place、SetStaging 或拖放 Action。
pub(crate) fn run(
    mut context: Context,
    state: Option<ResMut<State>>,
    mut showcase: Option<ResMut<Showcase>>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(show) = showcase.as_ref() else {
        return;
    };
    if show.chapter().id != "rocket-workflow" {
        return;
    }
    let mut state = state.expect("火箭工作流章节未初始化");
    if state.phase == Phase::Finished {
        return;
    }
    state.frame += 1;
    state.attempts += 1;
    assert!(
        state.attempts < 600,
        "火箭连续工作流未完成：{:?}/{}，命中 {:?}",
        state.phase,
        state.step,
        context.hits.actions
    );
    context.keys.reset_all();
    context.windows.single_mut().unwrap().1.focused = true;
    if state
        .captures
        .as_ref()
        .is_some_and(|done| !done.load(Ordering::Acquire))
    {
        return;
    }
    state.captures = None;
    if state.input.tick(&mut context.inputs.single_mut().unwrap()) {
        return;
    }
    let advanced = match state.phase {
        Phase::Build => build(&mut context, &mut state, &mut showcase, &mut commands),
        Phase::Generate => generate(&mut context, &mut state, &mut showcase, &mut commands),
        Phase::Manual => manual(&mut context, &mut state, &mut showcase, &mut commands),
        Phase::Finished => false,
    };
    if advanced {
        state.step += 1;
        state.attempts = 0;
    }
    if state.phase == Phase::Finished {
        let report = serde_json::json!({"chapter":"rocket-workflow", "completed":true,
            "input_mode":"真实目录命中 / 生产画布鼠标 / egui 分帧拖放", "cases":state.cases});
        std::fs::write(
            "target/rocket-workflow.json",
            serde_json::to_vec_pretty(&report).unwrap(),
        )
        .expect("无法写火箭工作流报告");
        exit.write(AppExit::Success);
    }
}

fn next_phase(state: &mut State, phase: Phase) -> bool {
    state.phase = phase;
    state.step = 0;
    state.attempts = 0;
    false
}

fn build(
    context: &mut Context,
    state: &mut State,
    showcase: &mut Option<ResMut<Showcase>>,
    commands: &mut Commands,
) -> bool {
    match state.step {
        0 => {
            describe(
                showcase,
                "第一阶段：从默认 Command Pod 开始，目录选件、预览、吸附逐件搭建三级火箭",
            );
            let document = &context.document;
            assert_eq!(document.ship.parts.len(), 1);
            assert_eq!(document.ship.parts[0].part_type, "pod-1");
            assert!(document.catalog.get("pod-1").unwrap().hidden);
            assert!(document.ship.connections.is_empty());
            assert!(staging(&document.ship).is_none_or(|s| s.steps.is_empty()));
            assert_eq!(document.history.undo_len(), 0);
            assert_eq!(document.history.redo_len(), 0);
            assert!(!document.dirty);
            state.before = Some(document.ship.clone());
            capture(commands, state, "editor-rocket-command-pod.png");
        }
        1 => {
            let bottom = assembly_bottom(&context.document);
            let (entity, mut window) = context.windows.single_mut().unwrap();
            let (camera, projection) = context.cameras.single().unwrap();
            let target = screen(&window, camera, projection, bottom);
            if target.y > window.height() - 80.0 {
                let center = Vec2::new(window.width() * 0.5, window.height() * 0.5);
                window.set_cursor_position(Some(center));
                context.wheels.write(MouseWheel {
                    phase: bevy::input::touch::TouchPhase::Moved,
                    unit: MouseScrollUnit::Line,
                    x: 0.0,
                    y: -2.0,
                    window: entity,
                });
                return false;
            }
        }
        2 => {
            let id = PARTS[state.part];
            let index = context
                .document
                .catalog
                .visible()
                .position(|kind| kind.id == id)
                .expect("待放部件不在可见目录");
            if !context.choose_part(state, index) {
                return false;
            }
            state.before = Some(context.document.ship.clone());
            state.revision = context.document.revision;
            let rect = context
                .ui
                .hits
                .iter()
                .find(|(action, _)| *action == panels::PanelButton::Part(index))
                .unwrap()
                .1;
            state.placements.push(
                serde_json::json!({"id":state.part + 2, "part_type":id, "catalog_index":index,
                "catalog_hit":[rect.min.x,rect.min.y,rect.max.x,rect.max.y]}),
            );
        }
        3 => {
            assert!(
                context.cursor.placing && !context.cursor.palette_drag,
                "真实目录点击未进入光标预览"
            );
            let plan = placement(&context.document, state.part as i64 + 1, PARTS[state.part]);
            context.point(plan.position);
            state.plan = Some(plan);
        }
        4 => {
            context.unchanged(state.before.as_ref().unwrap(), state.part);
            assert_eq!(
                context.document.revision, state.revision,
                "光标预览不能产生文档修订"
            );
            assert!(context.cursor.valid, "真实画布光标未形成有效预览");
            let (part, connection, allowed) =
                placement::preview(&context.document, &context.cursor).expect("生产预览不存在");
            let plan = state.plan.unwrap();
            assert!(allowed, "生产预览拒绝了串联落点");
            assert!((part.x - plan.position.0).hypot(part.y - plan.position.1) < 1e-5);
            assert!(
                matches!(connection, Some(EditorCommand::ConnectParts { parent, child, kind: LinkKind::Normal { parent_attach, child_attach } })
                if parent.id == plan.parent && child.id == plan.child && parent_attach == plan.parent_attach && child_attach == plan.child_attach)
            );
            if state.part == 0 {
                capture(commands, state, "editor-rocket-placement-preview.png");
            }
        }
        5 => {
            context.point(state.plan.unwrap().position);
            context.mouse.press(MouseButton::Left);
        }
        6 => {
            let plan = state.plan.unwrap();
            assert_eq!(context.document.ship.parts.len(), state.part + 2);
            assert_eq!(context.document.ship.connections.len(), state.part + 1);
            assert_eq!(
                context.document.history.undo_len(),
                state.part + 1,
                "放置与接触连接必须合为单步历史"
            );
            assert_eq!(
                context.document.ship.parts.last().unwrap().part_type,
                PARTS[state.part]
            );
            assert!(
                context
                    .document
                    .ship
                    .connections
                    .last()
                    .is_some_and(|edge| *edge
                        == Connection::Normal {
                            parent: plan.parent,
                            child: plan.child,
                            parent_attach: plan.parent_attach,
                            child_attach: plan.child_attach
                        })
            );
            let record = state.placements.last_mut().unwrap();
            record["preview_unchanged"] = true.into();
            record["normal_snap"] = true.into();
            record["position"] = serde_json::json!([plan.position.0, plan.position.1]);
            context.mouse.release(MouseButton::Left);
            state.part += 1;
            if state.part < PARTS.len() {
                state.step = 2;
                return false;
            }
        }
        7 => {
            context.key(KeyCode::Escape, false);
            if let Some(rect) = context
                .ui
                .part_toggle
                .filter(|_| context.ui.palette_area.is_some())
            {
                state
                    .input
                    .click_rect(rect, &mut context.inputs.single_mut().unwrap());
            }
        }
        8 => {
            assert!(!context.cursor.placing);
            context.key(KeyCode::KeyF, false);
        }
        9 => {
            // 上一步 F 已经走过生产 Update 和 PostUpdate；再要求相同观测跨一个稳定 tick。
            let observations = context.observe_visuals();
            let table = serde_json::json!(
                observations
                    .iter()
                    .map(|value| &value.record)
                    .collect::<Vec<_>>()
            );
            match check_visuals(&context.document.ship, &observations) {
                Err(VisualIssue::Invalid(reason)) => {
                    panic!("火箭视觉身份验收失败：{reason}\n{table:#}")
                }
                Err(VisualIssue::Pending(reason)) => {
                    state.visuals = None;
                    assert!(
                        state.attempts < 120,
                        "火箭视觉资源同步超时：{reason}\n{table:#}"
                    );
                    return false;
                }
                Ok(()) => {}
            }
            assert!(state.attempts < 120, "火箭视觉观测未稳定：\n{table:#}");
            if state.visuals.as_ref() != Some(&table) {
                state.visuals = Some(table);
                return false;
            }
            info!("装配截图前的真实视觉身份观测：{table:#}");
            let endpoint_error = assert_rocket(&context.document);
            assert!(staging(&context.document.ship).is_none_or(|s| s.steps.is_empty()));
            state.assembled = Some(context.document.ship.clone());
            xml(&context.document.ship, "rocket-assembled.xml");
            state.cases.push(serde_json::json!({"id":"rocket-assembly", "status":"passed", "metrics":{
                "start_parts":1, "start_connections":0, "start_staging_steps":0,
                "parts":9, "normal_connections":8, "engines":3, "detachers":2, "physical_stages":3,
                "placement_history_steps":context.document.history.undo_len(), "endpoint_max_error":endpoint_error,
                "placements":state.placements, "xml_roundtrip":true,
                "visual_entities":observations.len(), "visual_stable_frames":2, "visuals":table}}));
            capture(commands, state, "editor-rocket-assembled.png");
        }
        10 => {
            let pod = context.document.ship.part(1).unwrap();
            context.point((pod.x, pod.y));
            context.mouse.press(MouseButton::Left);
        }
        11 => {
            context.mouse.release(MouseButton::Left);
        }
        12 => {
            assert_eq!(
                context.document.selected,
                Some(PartKey::new(0, 1, 0)),
                "必须真实画布选中 Pod 后打开分级"
            );
            context.key(KeyCode::F2, false);
            return next_phase(state, Phase::Generate);
        }
        _ => unreachable!(),
    }
    true
}

fn generate(
    context: &mut Context,
    state: &mut State,
    showcase: &mut Option<ResMut<Showcase>>,
    commands: &mut Commands,
) -> bool {
    match state.step {
        0 => {
            describe(
                showcase,
                "第二阶段：点击自动基础分级，先核对草稿，再应用一次、撤销、重做及 XML 往返",
            );
            assert!(context.inspector.docked());
            assert!(!context.inspector.advanced_open && !context.inspector.targets_open);
            if !context.click(state, properties::Action::GenerateStaging) {
                return false;
            }
        }
        1 => {
            context.unchanged(state.assembled.as_ref().unwrap(), 8);
            assert_staging(context.draft(), AUTO, false);
            capture(commands, state, "editor-rocket-generated.png");
        }
        2 => {
            if !context.click(state, properties::Action::Apply) {
                return false;
            }
        }
        3 => {
            assert!(!context.inspector.is_open());
            assert_eq!(context.document.history.undo_len(), 9);
            assert_staging(staging(&context.document.ship).unwrap(), AUTO, false);
            state.automatic = Some(context.document.ship.clone());
            xml(&context.document.ship, "rocket-basic-staging.xml");
            context.key(KeyCode::KeyZ, true);
        }
        4 => {
            assert_eq!(
                Some(&context.document.ship),
                state.assembled.as_ref(),
                "一次 Undo 必须完整撤销自动分级"
            );
            assert_eq!(context.document.history.undo_len(), 8);
            assert_eq!(context.document.history.redo_len(), 1);
            context.key(KeyCode::KeyY, true);
        }
        5 => {
            context.unchanged(state.automatic.as_ref().unwrap(), 9);
            assert_rocket(&context.document);
            state.cases.push(
                serde_json::json!({"id":"basic-staging", "status":"passed", "metrics":{
                "steps":rows(staging(&context.document.ship).unwrap()), "current_stage":0,
                "all_moved_false":true, "draft_only_before_apply":true, "apply_history_delta":1,
                "undo_exact":true, "redo_exact":true, "xml_roundtrip":true}}),
            );
            // Undo 清除了选区，F2 的生产入口会回到唯一 Pod。
            context.key(KeyCode::F2, false);
            return next_phase(state, Phase::Manual);
        }
        _ => unreachable!(),
    }
    true
}

fn manual(
    context: &mut Context,
    state: &mut State,
    showcase: &mut Option<ResMut<Showcase>>,
    commands: &mut Commands,
) -> bool {
    if state.step <= 21 {
        context.unchanged(state.automatic.as_ref().unwrap(), 9);
    }
    let step = state.step;
    match step {
        0..=3 => {
            describe(
                showcase,
                "第三阶段：按下、保持越过阈值、移动、释放，把发动机 6 拖入分离器 7 卡；再拖整卡并回置",
            );
            if !drag(context, state, Drag::Activation, step) {
                return false;
            }
        }
        4 => {
            assert_staging(context.draft(), MANUAL, true);
            state.manual = Some(context.draft().clone());
        }
        5..=8 => {
            if !drag(context, state, Drag::CardDown, step - 5) {
                return false;
            }
        }
        9 => {
            let mut expected = state.manual.clone().unwrap();
            expected.steps.swap(0, 1);
            assert_eq!(context.draft(), &expected, "整卡拖放没有按最终下标重排");
            assert_eq!(inventory(context.draft()), vec![3, 4, 6, 7, 9]);
        }
        10..=13 => {
            if !drag(context, state, Drag::CardUp, step - 10) {
                return false;
            }
        }
        14 => {
            assert_eq!(
                Some(context.draft()),
                state.manual.as_ref(),
                "整卡回置没有恢复手动分级草稿"
            );
            capture(commands, state, "editor-rocket-manual.png");
        }
        15 => {
            if !context.click(state, properties::Action::Cancel) {
                return false;
            }
        }
        16 => {
            assert!(!context.inspector.is_open());
            context.unchanged(state.automatic.as_ref().unwrap(), 9);
            context.key(KeyCode::F2, false);
        }
        17..=20 => {
            if !drag(context, state, Drag::Activation, step - 17) {
                return false;
            }
        }
        21 => {
            assert_eq!(
                Some(context.draft()),
                state.manual.as_ref(),
                "取消后重做真实拖放应得到相同草稿"
            );
            if !context.click(state, properties::Action::Apply) {
                return false;
            }
        }
        22 => {
            assert!(!context.inspector.is_open());
            assert_eq!(staging(&context.document.ship), state.manual.as_ref());
            assert_eq!(context.document.history.undo_len(), 10);
            state.before = Some(context.document.ship.clone());
            context.key(KeyCode::KeyZ, true);
        }
        23 => {
            assert_eq!(Some(&context.document.ship), state.automatic.as_ref());
            assert_eq!(context.document.history.undo_len(), 9);
            assert_eq!(context.document.history.redo_len(), 1);
            context.key(KeyCode::KeyY, true);
        }
        24 => {
            assert_eq!(Some(&context.document.ship), state.before.as_ref());
            assert_eq!(context.document.history.undo_len(), 10);
            assert_eq!(context.document.history.redo_len(), 0);
            assert_staging(staging(&context.document.ship).unwrap(), MANUAL, true);
            assert_rocket(&context.document);
            xml(&context.document.ship, "rocket-manual-staging.xml");
            state.cases.push(
                serde_json::json!({"id":"manual-staging-drag", "status":"passed", "metrics":{
                "steps":rows(staging(&context.document.ship).unwrap()), "moved_ids":[6],
                "activation_multiset":[3,4,6,7,9], "draft_only_before_apply":true,
                "card_reordered":true, "card_restored":true, "cancel_unchanged":true,
                "apply_history_delta":1, "undo_exact":true, "redo_exact":true, "xml_roundtrip":true,
                "drags":state.traces}}),
            );
            state.phase = Phase::Finished;
        }
        _ => unreachable!(),
    }
    true
}

#[cfg(test)]
#[path = "tests/rocket_workflow.rs"]
mod tests;
