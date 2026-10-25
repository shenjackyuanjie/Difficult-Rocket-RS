//! 仅供窗口截图验收的真实输入回放；夹具初始化与外部冲突是仅有的文档脚本改动。
use crate::{
    DragState, EditorDocument, SmokeTest,
    properties::{self, Action, Inspector},
};
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk};
use bevy_egui::egui;
use dr_core::{Connection, EditorCommand, PartCatalog, PartKey, Ship};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

const STABLE_FRAMES: u8 = 12;
const NAMES: [&str; 6] = [
    "staging-empty",
    "staging-generated",
    "staging-palette-coexist",
    "staging-canvas-drag",
    "staging-card-drag",
    "staging-stale-target",
];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Phase {
    #[default]
    Init,
    InitialFit,
    Empty,
    Generate,
    Generated,
    Palette,
    Coexist,
    Fit,
    CanvasHover,
    CanvasPress,
    CanvasMove,
    CanvasShot,
    CanvasRelease,
    CardPress,
    CardMove,
    CardShot,
    CardRelease,
    Conflict,
    Stale,
    Done,
}
impl Phase {
    fn next(self) -> Self {
        use Phase::*;
        match self {
            Init => InitialFit,
            InitialFit => Empty,
            Empty => Generate,
            Generate => Generated,
            Generated => Palette,
            Palette => Coexist,
            Coexist => Fit,
            Fit => CanvasHover,
            CanvasHover => CanvasPress,
            CanvasPress => CanvasMove,
            CanvasMove => CanvasShot,
            CanvasShot => CanvasRelease,
            CanvasRelease => CardPress,
            CardPress => CardMove,
            CardMove => CardShot,
            CardShot => CardRelease,
            CardRelease => Conflict,
            Conflict => Stale,
            Stale | Done => Done,
        }
    }
    fn shot(self) -> Option<usize> {
        match self {
            Self::Empty => Some(0),
            Self::Generated => Some(1),
            Self::Coexist => Some(2),
            Self::CanvasShot => Some(3),
            Self::CardShot => Some(4),
            Self::Stale => Some(5),
            _ => None,
        }
    }
}

#[derive(Default)]
pub struct State {
    phase: Phase,
    wait: u8,
    input: crate::egui_ui::UiTestInput,
    pending: Option<Value>,
    entries: Vec<Value>,
    pointer: Option<egui::Pos2>,
    canvas_pointer: bool,
    source: Option<egui::Rect>,
    insertion: Option<egui::Rect>,
    owner: Option<PartKey>,
    deleted: Option<PartKey>,
    before: Option<(f64, f64)>,
}
impl State {
    fn advance(&mut self) {
        self.phase = self.phase.next();
        self.wait = STABLE_FRAMES;
    }
}

#[derive(Resource, Default)]
pub struct CaptureResult(Option<Value>);

pub fn ordinary_capture(mode: Res<SmokeTest>) -> bool {
    mode.staging_shots.is_none()
}

fn shot_path(directory: &Path, index: usize) -> PathBuf {
    directory.join(format!("{}.png", NAMES[index]))
}
/// 独立核对落盘 PNG 的 IHDR，不把请求尺寸当作实际像素尺寸。
fn png_size(bytes: &[u8]) -> Option<[u32; 2]> {
    if bytes.len() < 24 || !bytes.starts_with(b"\x89PNG\r\n\x1a\n") || &bytes[12..16] != b"IHDR" {
        return None;
    }
    let width = u32::from_be_bytes(bytes[16..20].try_into().ok()?);
    let height = u32::from_be_bytes(bytes[20..24].try_into().ok()?);
    (width > 0 && height > 0).then_some([width, height])
}
fn report(entries: &[Value], complete: bool) -> Value {
    json!({"schema_version": 1, "complete": complete, "coordinate_space": "egui points; physical pixels = points * pixels_per_point", "shots": entries})
}
fn rect_json(rect: Option<egui::Rect>) -> Value {
    rect.map_or(Value::Null, |r| json!({"min": [r.min.x, r.min.y], "max": [r.max.x, r.max.y], "width": r.width(), "height": r.height(), "positive": r.is_positive()}))
}
fn coexist(
    dock: Option<egui::Rect>,
    palette: Option<egui::Rect>,
    canvas: Option<egui::Rect>,
) -> bool {
    match (dock, palette, canvas) {
        (Some(d), Some(p), Some(c)) => {
            d.is_positive() && p.is_positive() && c.is_positive() && !d.intersect(p).is_positive()
        }
        _ => false,
    }
}

/// 使用正式目录的尺寸与贴图构造两级小船，不预填草稿分级。
fn fixture(catalog: &PartCatalog) -> Ship {
    let kinds = [
        "pod-1",
        "fueltank-0",
        "engine-0",
        "detacher-1",
        "fueltank-0",
        "engine-0",
    ];
    let mut top = 4.0;
    let mut parts = Vec::new();
    for (i, name) in kinds.iter().enumerate() {
        let kind = catalog
            .get(name)
            .unwrap_or_else(|| panic!("分级截图夹具缺少目录部件 {name}"));
        let half = kind.half_extents().1;
        top -= half;
        parts.push(kind.instantiate(i as i64 + 1, (0.0, top)));
        top -= half;
    }
    // 空分级明确从无分级的 Pod 开始，之后只点击正式生成控件。
    parts[0].pod.as_mut().expect("夹具 Pod").staging = None;
    let ship = Ship {
        parts,
        connections: (1..6)
            .map(|parent| Connection::Normal {
                parent,
                child: parent + 1,
                parent_attach: 0,
                child_attach: 1,
            })
            .collect(),
        ..default()
    };
    let generated = dr_core::generate_basic_staging(&ship, catalog, PartKey::new(0, 1, 0))
        .expect("夹具可生成基础分级");
    assert!(generated.steps.len() >= 3);
    ship
}
fn pointer(input: &mut bevy_egui::EguiInput, pos: egui::Pos2, pressed: Option<bool>) {
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
fn image_evidence(image: &Image, marker_region: Option<egui::Rect>) -> Value {
    let data = image.data.as_deref().unwrap_or_default();
    let first = data.first_chunk::<4>().copied().unwrap_or_default();
    let non_uniform = data.as_chunks::<4>().0.iter().any(|p| *p != first);
    let marker_pixels = data
        .as_chunks::<4>()
        .0
        .iter()
        .enumerate()
        .filter(|(index, p)| {
            let width = image.width().max(1) as usize;
            let point = egui::pos2((index % width) as f32, (index / width) as f32);
            marker_region.is_some_and(|region| region.contains(point))
                && p[1].abs_diff(225) <= 2
                && ((p[0].abs_diff(190) <= 2 && p[2].abs_diff(70) <= 2)
                    || (p[2].abs_diff(190) <= 2 && p[0].abs_diff(70) <= 2))
        })
        .count();
    json!({"pixel_size": [image.width(), image.height()], "nonempty": !data.is_empty(), "non_uniform": non_uniform, "insert_marker_pixels": marker_pixels})
}

#[allow(clippy::too_many_arguments)]
pub fn run(
    mode: Res<SmokeTest>,
    mut state: Local<State>,
    mut document: ResMut<EditorDocument>,
    mut inspector: ResMut<Inspector>,
    hits: Res<crate::egui_ui::UiHits>,
    ui: Res<crate::panels::egui_panel::UiState>,
    mut inputs: Query<&mut bevy_egui::EguiInput, With<bevy_egui::PrimaryEguiContext>>,
    mut windows: Query<&mut Window, With<bevy::window::PrimaryWindow>>,
    cameras: Query<(&Camera, &GlobalTransform), With<Camera2d>>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    drag: Res<DragState>,
    mut commands: Commands,
    mut captured: ResMut<CaptureResult>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(directory) = mode.staging_shots.as_deref().filter(|_| mode.enabled) else {
        return;
    };
    if mode.started.elapsed().as_secs() < 3 {
        return;
    }
    assert!(
        mode.started.elapsed().as_secs() < 120,
        "分级截图超时：{:?}；动作数={}；dock={:?}；palette={:?}",
        state.phase,
        hits.actions.len(),
        hits.dock.as_ref().map(|d| d.panel),
        ui.palette_area
    );
    let Ok(mut window) = windows.single_mut() else {
        return;
    };
    let Ok(mut input) = inputs.single_mut() else {
        return;
    };
    mouse.clear();
    input.0.focused = true;
    // 每帧保留脚本指针，截图异步回读期间也保持按住，直到对应 PNG 已落盘。
    if let Some(pos) = state.pointer {
        pointer(&mut input, pos, None);
        if state.canvas_pointer {
            let factor = ui.pixels_per_point / window.scale_factor();
            window.set_cursor_position(Some(Vec2::new(pos.x, pos.y) * factor));
        }
    }
    if mode
        .window_size
        .is_some_and(|size| !crate::window::smoke_window_matches(&window, size))
    {
        state.wait = STABLE_FRAMES;
        return;
    }
    if state.pending.is_some() {
        if let Some(image) = captured.0.take() {
            let index = state.phase.shot().unwrap();
            let path = shot_path(directory, index);
            let bytes = std::fs::read(&path).expect("截图回读后 PNG 已落盘");
            let actual_size = png_size(&bytes).expect("截图必须为正尺寸 PNG");
            assert_eq!(
                image["pixel_size"],
                json!(actual_size),
                "PNG 与回读像素尺寸应一致"
            );
            assert_eq!(image["nonempty"], true);
            assert_eq!(image["non_uniform"], true, "截图应包含实际画面");
            if index == 4 {
                assert!(
                    image["insert_marker_pixels"].as_u64().unwrap() > 0,
                    "动作卡拖动应出现绿色插入标记"
                );
            }
            let mut entry = state.pending.take().unwrap();
            entry["image"] = image;
            entry["bytes"] = json!(bytes.len());
            entry["png_pixel_size"] = json!(actual_size);
            assert_eq!(
                entry["physical_size"], entry["png_pixel_size"],
                "PNG 应匹配当前窗口物理尺寸"
            );
            state.entries.push(entry);
            std::fs::write(
                directory.join("staging-shots.json"),
                serde_json::to_vec_pretty(&report(&state.entries, index == 5)).unwrap(),
            )
            .expect("写分级截图报告");
            info!("分级截图已保存：{}", path.display());
            state.advance();
        }
        return;
    }
    if state.wait > 0 {
        state.wait -= 1;
        return;
    }
    if state.input.tick(&mut input) {
        return;
    }
    if let Some(index) = state.phase.shot() {
        if !inspector.docked() || hits.dock.is_none() || hits.scrolling {
            return;
        }
        let plan = inspector.draft.as_ref().and_then(|d| d.staging.as_ref());
        let stages = plan.map_or(0, |p| p.steps.len());
        let activations: usize =
            plan.map_or(0, |p| p.steps.iter().map(|s| s.activations.len()).sum());
        if index == 0 {
            assert_eq!(stages, 0);
        }
        if index > 0 {
            assert!(stages >= 3 && activations >= 3);
        }
        let layout_ok = coexist(
            hits.dock.as_ref().map(|d| d.panel),
            ui.palette_area,
            ui.canvas_area,
        );
        if index == 2 {
            assert!(layout_ok, "分级与部件目录应共存且不重叠，保留正画布区域");
        }
        if index == 3 {
            assert!(
                drag.id.is_some() && drag.preview != drag.origin,
                "停靠分级下画布实际拖动应生效"
            );
        }
        if index == 5 {
            assert!(
                inspector.error.contains("草稿已保留"),
                "外部冲突应显示保留草稿诊断：{}",
                inspector.error
            );
        }
        let entry = json!({
            "state": NAMES[index], "file": format!("{}.png", NAMES[index]),
            "logical_size": [window.width(), window.height()], "physical_size": [window.physical_width(), window.physical_height()],
            "scale_factor": window.scale_factor(), "pixels_per_point": ui.pixels_per_point,
            "stable_frames": STABLE_FRAMES, "docked": inspector.docked(), "draft_retained": inspector.draft.is_some(),
            "stage_count": stages, "activation_count": activations,
            "rectangles": {"dock": rect_json(hits.dock.as_ref().map(|d| d.panel)), "palette": rect_json(ui.palette_area), "canvas": rect_json(ui.canvas_area), "body": rect_json(hits.body), "card_source": rect_json(state.source), "insertion_target": rect_json(state.insertion)},
            "coexist_valid": layout_ok, "drag": {"active": drag.id.is_some(), "origin": drag.origin, "preview": drag.preview, "before_position": state.before, "pointer": state.pointer.map(|p| [p.x, p.y])},
            "diagnostic": inspector.error, "external_deleted_target": state.deleted.map(|k| k.to_string()),
        });
        state.pending = Some(entry);
        let path = shot_path(directory, index);
        // 只检查目标卡右缘的插入线，避开同色拖动图标。
        let marker_region = (index == 4).then(|| state.insertion).flatten().map(|rect| {
            egui::Rect::from_min_max(
                egui::pos2(
                    (rect.right() - 4.0) * ui.pixels_per_point,
                    rect.top() * ui.pixels_per_point,
                ),
                egui::pos2(
                    (rect.right() + 4.0) * ui.pixels_per_point,
                    rect.bottom() * ui.pixels_per_point,
                ),
            )
        });
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(path))
            .observe(
                move |event: On<ScreenshotCaptured>, mut result: ResMut<CaptureResult>| {
                    result.0 = Some(image_evidence(&event.image, marker_region));
                },
            );
        return;
    }
    match state.phase {
        Phase::Init => {
            std::fs::create_dir_all(directory).expect("创建分级截图目录");
            document.ship = fixture(&document.catalog);
            document.saved_ship = document.ship.clone();
            document.history = default();
            document.refresh();
            let owner = PartKey::new(0, 1, 0);
            document.select_only(Some(owner));
            state.owner = Some(owner);
            properties::act(&Action::Open, &mut inspector, &mut document);
        }
        Phase::Generate => {
            if !state
                .input
                .click(Action::GenerateStaging, &hits, &mut input)
            {
                return;
            }
        }
        Phase::Palette => {
            let Some(rect) = ui.part_toggle.filter(|r| r.is_positive()) else {
                return;
            };
            state.input.click_rect(rect, &mut input);
        }
        Phase::InitialFit | Phase::Fit => {
            let Some(rect) = ui.fit_button else {
                return;
            };
            state.input.click_rect(rect, &mut input);
        }
        Phase::CanvasHover => {
            let Ok((camera, transform)) = cameras.single() else {
                return;
            };
            // 拖动最下方发动机，不改变草稿 owner，后续动作卡仍可正常编辑。
            let part = document.ship.parts.last().unwrap();
            let Ok(pos) = camera.world_to_viewport(
                transform,
                Vec3::new(part.x as f32 * 60.0, part.y as f32 * 60.0, 0.0),
            ) else {
                return;
            };
            let factor = window.scale_factor() / ui.pixels_per_point;
            let pos = egui::pos2(pos.x * factor, pos.y * factor);
            assert!(
                ui.canvas_area.is_some_and(|r| r.contains(pos)),
                "夹具部件应在可见画布内，先等适配控件生效"
            );
            state.before = Some((part.x, part.y));
            state.pointer = Some(pos);
            state.canvas_pointer = true;
            pointer(&mut input, pos, None);
            window.set_cursor_position(Some(Vec2::new(pos.x, pos.y) / factor));
        }
        Phase::CanvasPress => {
            mouse.press(MouseButton::Left);
        }
        Phase::CanvasMove => {
            let canvas = ui.canvas_area.unwrap();
            let pos = state.pointer.unwrap();
            state.pointer = Some(egui::pos2(
                (pos.x + 70.0).min(canvas.right() - 8.0),
                pos.y - 25.0,
            ));
        }
        Phase::CanvasRelease => {
            // 先完成真实释放；只要求截图时的预览，不依赖吸附后的最终位置。
            mouse.release(MouseButton::Left);
            state.pointer = None;
            state.canvas_pointer = false;
        }
        Phase::CardPress => {
            let body = hits.body.unwrap();
            let cards: Vec<_> = hits
                .actions
                .iter()
                .filter_map(|(action, rect)| {
                    matches!(action, Action::SelectActivation(_, _)).then_some(*rect)
                })
                .filter(|r| r.is_positive() && body.contains_rect(*r))
                .collect();
            let Some(source) = cards.first().copied() else {
                input.0.events.extend([
                    egui::Event::PointerMoved(body.center()),
                    egui::Event::MouseWheel {
                        phase: egui::TouchPhase::Move,
                        unit: egui::MouseWheelUnit::Point,
                        delta: egui::vec2(0.0, -body.height() * 0.5),
                        modifiers: egui::Modifiers::NONE,
                    },
                ]);
                state.wait = STABLE_FRAMES;
                return;
            };
            let target = cards.get(1).copied().unwrap_or(source);
            state.source = Some(source);
            state.insertion = Some(target);
            state.pointer = Some(source.center());
            pointer(&mut input, source.center(), Some(true));
        }
        Phase::CardMove => {
            let target = state.insertion.unwrap();
            // 指向动作卡右半边，正式 drop_action 会绘制纵向绿色插入线。
            let pos = egui::pos2(target.right() - 3.0, target.center().y);
            state.pointer = Some(pos);
            pointer(&mut input, pos, None);
        }
        Phase::CardRelease => {
            pointer(&mut input, state.pointer.unwrap(), Some(false));
            state.pointer = None;
        }
        Phase::Conflict => {
            let target_id = inspector
                .draft
                .as_ref()
                .unwrap()
                .staging
                .as_ref()
                .unwrap()
                .steps
                .iter()
                .flat_map(|s| &s.activations)
                .next()
                .unwrap()
                .id;
            let key = document
                .ship
                .keyed_parts()
                .find(|(_, p)| p.id == target_id)
                .unwrap()
                .0;
            assert!(
                document.execute(EditorCommand::DeleteSelection(vec![key])),
                "脚本化外部删除应成功"
            );
            state.deleted = Some(key);
            // refresh 由正式界面每帧执行，此处等待其诊断而不覆写草稿。
        }
        Phase::Done => {
            exit.write(AppExit::Success);
            return;
        }
        _ => unreachable!(),
    }
    state.advance();
}

#[cfg(test)]
mod tests;
