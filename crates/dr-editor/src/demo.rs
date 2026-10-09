//! 单窗口章节编排：复用有断言的 UI 自测，延迟的是注入步骤而非编辑器帧循环。
use super::*;
use std::{
    io::Write,
    path::PathBuf,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

struct Chapter {
    id: &'static str,
    title: &'static str,
    description: &'static str,
    artifacts: &'static [&'static str],
}

impl Chapter {
    fn evidence(&self) -> Vec<String> {
        self.artifacts
            .iter()
            .map(|name| (*name).to_owned())
            .chain(demo_cases::artifacts(self.id))
            .collect()
    }
}
const CHAPTERS: &[Chapter] = &[
    Chapter {
        id: "panels",
        title: "部件目录与放置",
        description: "目录选择 → 旋转/镜像预览 → 碰撞拒绝 → 放置及撤销 → 分类筛选",
        artifacts: &[
            "editor-placement-preview.png",
            "editor-collision-preview.png",
            "editor-panels-smoke.png",
        ],
    },
    Chapter {
        id: "connections",
        title: "连接点与吸附",
        description: "紫色候选连接点 → 长梁沿边吸附 → 连接/撤销 → XML 往返",
        artifacts: &[
            "editor-connection-hints.png",
            "editor-connections-smoke.png",
            "connections-smoke.xml",
        ],
    },
    Chapter {
        id: "transforms",
        title: "旋转镜像、自由连接与连线设置",
        description: "非直角吸附 → 组合镜像 → 自由双点连/断 → 连线样式 → 树/连通/框选/后代整组变换",
        artifacts: &[
            "editor-angled-connection.png",
            "editor-combined-transform.png",
            "editor-free-connections.png",
            "editor-free-placement.png",
            "editor-transforms-smoke.png",
            "editor-lines-hidden.png",
            "editor-lines-flow.png",
            "editor-lines-pulse.png",
            "editor-tree-transforms.png",
            "editor-component-transforms.png",
            "editor-box-transforms.png",
            "editor-descendant-transforms.png",
            "transforms-smoke.xml",
            "transforms-smoke.json",
        ],
    },
    Chapter {
        id: "selection",
        title: "选择与编辑",
        description: "左键选取/拖动，中键框选，右键取消 → 切换框选按键 → 后代跟随 → 删除及撤销",
        artifacts: &[
            "editor-selection-preview.png",
            "editor-selection-smoke.png",
            "editor-drag-rotation.png",
            "selection-smoke.xml",
        ],
    },
    Chapter {
        id: "view",
        title: "视角与显示",
        description: "F 整船 / Shift+F 选区适配 → F3 调试文字 → F4 船体显隐 → F1 帮助",
        artifacts: &["editor-view-smoke.png", "editor-help.png"],
    },
    Chapter {
        id: "staging",
        title: "属性与分级",
        description: "F2 属性草稿 → 分级长列表 → 修改并应用 → 撤销重做",
        artifacts: &[
            "editor-staging-draft.png",
            "editor-staging-smoke.png",
            "staging-smoke.xml",
        ],
    },
    Chapter {
        id: "topology",
        title: "连接树与连接图",
        description: "树/图切换 → 换父节点 → 环路拒绝 → 断开连接及撤销",
        artifacts: &[
            "editor-topology-tree.png",
            "editor-topology-graph.png",
            "editor-topology-smoke.png",
            "topology-smoke.xml",
            "topology-smoke.json",
        ],
    },
    Chapter {
        id: "repair",
        title: "原版重号船体修复",
        description: "只读加载 Heronb → 分配真实歧义引用 → 应用修复 → 撤销重做",
        artifacts: &[
            "editor-repair-draft.png",
            "editor-repair-smoke.png",
            "repair-smoke.xml",
        ],
    },
    Chapter {
        id: "browser",
        title: "船体目录与虚拟滚动",
        description: "扫描临时 1000 船体 → 跳过坏 XML → 滚动末尾 → 点击打开",
        artifacts: &["editor-browser-smoke.png", "editor-browser-smoke.json"],
    },
    Chapter {
        id: "unsaved",
        title: "未保存确认",
        description: "窗口内暗色模态 → 取消保留文档 → 放弃修改；不覆盖原版样本",
        artifacts: &["editor-unsaved-modal.png"],
    },
];

const DETAILED_CHAPTERS: &[(&str, Chapter)] = &[
    (
        "panels",
        Chapter {
            id: "placement-cases",
            title: "放置边界：取消、上限与禁用旋转",
            description: "每个夹具独立初始化；预览/拒绝操作不应改写文档。",
            artifacts: &["placement-cases.json"],
        },
    ),
    (
        "connections",
        Chapter {
            id: "geometry-cases",
            title: "精确重叠边界：4.9% / 5% / 5.1%",
            description: "精确事务参数避免鼠标坐标量化；红/绿轮廓是被拒绝/允许的候选。",
            artifacts: &["geometry-cases.json"],
        },
    ),
    (
        "transforms",
        Chapter {
            id: "free-cases",
            title: "自由连接边界：取消、自连接与类型",
            description: "真实端点点击与快捷键；自由模式依然校验引用和端点类型。",
            artifacts: &["free-cases.json"],
        },
    ),
    (
        "transforms",
        Chapter {
            id: "group-cases",
            title: "整组变换边界：空选区、顺序与后代",
            description: "不相连的多选仍是刚体；旋转/镜像顺序不能交换。",
            artifacts: &["group-cases.json"],
        },
    ),
    (
        "transforms",
        Chapter {
            id: "line-cases",
            title: "连接线边界：上下限、重置与退化路径",
            description: "会话级显示设置不生成文档历史；隐藏线不隐藏手动端点。",
            artifacts: &["line-cases.json"],
        },
    ),
    (
        "staging",
        Chapter {
            id: "history-cases",
            title: "历史与属性边界：回滚、分支与非法草稿",
            description: "失败操作保持完整快照；草稿参数直接设置，不进行系统文字输入专项。",
            artifacts: &["history-cases.json"],
        },
    ),
    (
        "topology",
        Chapter {
            id: "topology-cases",
            title: "拓扑边界：自连接、环路与跨组重号",
            description: "树与图的环路规则不同；跨组合并保持实例与引用归属。",
            artifacts: &["topology-cases.json"],
        },
    ),
    (
        "unsaved",
        Chapter {
            id: "file-cases",
            title: "文件边界：坏 XML、保存失败与模态隔离",
            description: "仅使用 target 临时输入，不覆盖或恢复写入原版样本。",
            artifacts: &["file-cases.json"],
        },
    ),
];

const DETAILED_STAGING: Chapter = Chapter {
    id: "staging",
    title: "属性与长分级：64 级 / 1024 动作",
    description: "滚动至最后一级最后一个动作 → 修改草稿 → 应用 → 撤销重做 → XML 往返",
    artifacts: &[
        "editor-staging-draft.png",
        "editor-staging-smoke.png",
        "staging-smoke.xml",
        "editor-staging-performance.json",
    ],
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Mode {
    #[default]
    Brief,
    Detailed,
}

impl Mode {
    fn name(self) -> &'static str {
        match self {
            Self::Brief => "brief",
            Self::Detailed => "detailed",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Brief => "简略",
            Self::Detailed => "详细",
        }
    }

    fn chapters(self) -> Vec<&'static Chapter> {
        let mut chapters = Vec::new();
        for chapter in CHAPTERS {
            chapters.push(if self == Self::Detailed && chapter.id == "staging" {
                &DETAILED_STAGING
            } else {
                chapter
            });
            if self == Self::Detailed {
                chapters.extend(
                    DETAILED_CHAPTERS
                        .iter()
                        .filter(|(after, _)| *after == chapter.id)
                        .map(|(_, extra)| extra),
                );
            }
        }
        chapters
    }
}

fn staging_fixture(document: &mut EditorDocument, mode: Mode) {
    let (stages, last_id) = match mode {
        Mode::Brief => (8, 7),
        Mode::Detailed => (64, 17),
    };
    let kind = document.catalog.get("detacher-1").unwrap();
    document.ship.parts.extend((2..=last_id).map(|id| {
        kind.instantiate(
            id,
            (
                ((id - 2) % 3) as f64 * 3. - 3.,
                -((id - 2) / 3) as f64 * 3. - 2.,
            ),
        )
    }));
    document.ship.parts[0].pod.as_mut().unwrap().staging = Some(dr_core::StagingState {
        current_stage: stages / 2,
        steps: (0..stages)
            .map(|_| dr_core::StageStep {
                activations: (2..=last_id)
                    .map(|id| dr_core::Activation { id, moved: false })
                    .collect(),
            })
            .collect(),
    });
    document.saved_ship = document.ship.clone();
}

/// 提示寿命独立于演示节拍：fast 模式也有可见的停留与淡出。
const KEY_HINT_HOLD: Duration = Duration::from_millis(250);
const KEY_HINT_FADE: Duration = Duration::from_millis(650);

struct KeyHint {
    text: String,
    applied: Instant,
}

impl KeyHint {
    fn opacity(&self, now: Instant) -> f32 {
        let elapsed = now.saturating_duration_since(self.applied);
        if elapsed <= KEY_HINT_HOLD {
            1.
        } else {
            (1. - (elapsed - KEY_HINT_HOLD).as_secs_f32() / KEY_HINT_FADE.as_secs_f32())
                .clamp(0., 1.)
        }
    }
}

#[derive(Default)]
struct KeyHints {
    latest: Option<KeyHint>,
    // 只记已采集的边沿；即使 ButtonInput 在下一帧仍保留 just_pressed，也不重置寿命。
    seen_edges: std::collections::HashSet<KeyCode>,
}

fn is_modifier(name: &str) -> bool {
    matches!(
        name,
        "ControlLeft"
            | "ControlRight"
            | "ShiftLeft"
            | "ShiftRight"
            | "AltLeft"
            | "AltRight"
            | "SuperLeft"
            | "SuperRight"
    )
}

fn button_key_name(key: KeyCode) -> String {
    use bevy_egui::egui;
    let name = format!("{key:?}");
    let name = name
        .strip_prefix("Key")
        .or_else(|| name.strip_prefix("Digit"))
        .unwrap_or(&name);
    egui::Key::from_name(name).map_or_else(|| name.to_owned(), |key| key.name().to_owned())
}

fn key_combination(name: &str, modifiers: bevy_egui::egui::Modifiers) -> String {
    let mut parts = Vec::new();
    if modifiers.ctrl || (modifiers.command && !modifiers.mac_cmd) {
        parts.push("Ctrl");
    }
    if modifiers.alt {
        parts.push("Alt");
    }
    if modifiers.shift {
        parts.push("Shift");
    }
    if modifiers.mac_cmd {
        parts.push("Cmd");
    }
    parts.push(name);
    parts.join("+")
}

impl KeyHints {
    /// 必须在 Motion 的键与事件已重播之后调用；不读取待应用的 Motion 快照。
    fn collect(
        &mut self,
        keys: &ButtonInput<KeyCode>,
        events: &[bevy_egui::egui::Event],
        now: Instant,
    ) -> Vec<String> {
        use bevy_egui::egui;
        let mut labels = BTreeSet::new();
        let mut event_keys = BTreeSet::new();
        for event in events {
            if let egui::Event::Key {
                key,
                physical_key,
                pressed,
                repeat,
                modifiers,
            } = event
            {
                // egui 的逻辑键用于显示，物理键用于和 ButtonInput 去重。
                // repeat/release 也占据该键，防止 fallback 把重复事件重新当作新按键。
                event_keys.insert(key.name().to_owned());
                event_keys.insert(physical_key.unwrap_or(*key).name().to_owned());
                if *pressed && !*repeat && !is_modifier(key.name()) {
                    labels.insert(key_combination(key.name(), *modifiers));
                }
            }
        }
        let modifiers = egui::Modifiers {
            ctrl: keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight),
            alt: keys.pressed(KeyCode::AltLeft) || keys.pressed(KeyCode::AltRight),
            shift: keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight),
            mac_cmd: keys.pressed(KeyCode::SuperLeft) || keys.pressed(KeyCode::SuperRight),
            ..Default::default()
        };
        self.seen_edges.retain(|key| keys.just_pressed(*key));
        for key in keys.get_just_pressed() {
            let fresh = self.seen_edges.insert(*key);
            let name = button_key_name(*key);
            if fresh && !is_modifier(&name) && !event_keys.contains(&name) {
                labels.insert(key_combination(&name, modifiers));
            }
        }
        let labels: Vec<_> = labels.into_iter().collect();
        if !labels.is_empty() {
            self.latest = Some(KeyHint {
                text: labels.join(" · "),
                applied: now,
            });
        }
        labels
    }
}

#[derive(Default)]
struct MouseHints {
    latest: Option<KeyHint>,
    seen_edges: std::collections::HashSet<MouseButton>,
}

fn mouse_label(button: MouseButton) -> Option<(&'static str, &'static str)> {
    match button {
        MouseButton::Left => Some(("left", "左键")),
        MouseButton::Middle => Some(("middle", "中键")),
        MouseButton::Right => Some(("right", "右键")),
        _ => None,
    }
}

impl MouseHints {
    fn collect(
        &mut self,
        mouse: &ButtonInput<MouseButton>,
        events: &[bevy_egui::egui::Event],
        now: Instant,
    ) -> Vec<(&'static str, String)> {
        use bevy_egui::egui;
        let mut presses = BTreeSet::new();
        let mut event_buttons = BTreeSet::new();
        let mut actions = Vec::new();
        for event in events {
            match event {
                egui::Event::PointerButton {
                    button, pressed, ..
                } => {
                    let mapped = match button {
                        egui::PointerButton::Primary => Some(MouseButton::Left),
                        egui::PointerButton::Middle => Some(MouseButton::Middle),
                        egui::PointerButton::Secondary => Some(MouseButton::Right),
                        _ => None,
                    };
                    if let Some((kind, label)) = mapped.and_then(mouse_label) {
                        event_buttons.insert(kind);
                        if *pressed {
                            presses.insert((kind, label));
                        }
                    }
                }
                egui::Event::MouseWheel { delta, .. } if delta.length_sq() > 0. => {
                    actions.push(("scroll", format!("滚轮 ({:.0}, {:.0})", delta.x, delta.y)));
                }
                _ => {}
            }
        }
        self.seen_edges.retain(|button| mouse.just_pressed(*button));
        for button in mouse.get_just_pressed() {
            let fresh = self.seen_edges.insert(*button);
            if let Some((kind, label)) = mouse_label(*button)
                && fresh
                && !event_buttons.contains(kind)
            {
                presses.insert((kind, label));
            }
        }
        actions.extend(
            presses
                .into_iter()
                .map(|(kind, label)| (kind, label.to_owned())),
        );
        if !actions.is_empty() {
            self.latest = Some(KeyHint {
                text: actions
                    .iter()
                    .map(|(_, label)| label.as_str())
                    .collect::<Vec<_>>()
                    .join(" · "),
                applied: now,
            });
        }
        actions
    }
}

/// 仅向 egui pass 的图层添加形状，不创建 Area/控件，也不请求焦点或捕获输入。
fn paint_key_hint(ctx: &bevy_egui::egui::Context, hint: &KeyHint, now: Instant, below_help: bool) {
    paint_input_hint(ctx, hint, now, below_help, false);
}

fn paint_input_hint(
    ctx: &bevy_egui::egui::Context,
    hint: &KeyHint,
    now: Instant,
    below_help: bool,
    mouse: bool,
) {
    use bevy_egui::egui;
    let opacity = hint.opacity(now);
    if opacity <= 0. {
        return;
    }
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Tooltip,
        egui::Id::new(if mouse {
            "demo-mouse-hint"
        } else {
            "demo-key-hint"
        }),
    ));
    let color = egui::Color32::from_rgb(239, 226, 255).gamma_multiply(opacity);
    let galley = painter.layout_no_wrap(hint.text.clone(), egui::FontId::proportional(24.), color);
    let center = egui::pos2(
        ctx.content_rect().center().x,
        if below_help {
            ctx.content_rect().bottom()
                - 12.
                - (galley.size().y + 16.) / 2.
                - if mouse { 50. } else { 0. }
        } else {
            ctx.content_rect().top() + if mouse { 122. } else { 72. }
        },
    );
    let rect = egui::Rect::from_center_size(center, galley.size() + egui::vec2(28., 16.));
    painter.rect_filled(
        rect,
        7.,
        egui::Color32::from_rgb(25, 23, 34).gamma_multiply(opacity),
    );
    painter.galley(rect.min + egui::vec2(14., 8.), galley, color);
    ctx.request_repaint();
}

/// 节拍只给真实动作留阅读时间，轮询/截图等技术等待不冒充重要操作。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Action {
    Technical,
    Navigation,
    Click,
    Key,
    Drag,
}

fn fast(step: Duration) -> bool {
    step <= Duration::from_millis(60)
}

fn action_pause(step: Duration, action: Action) -> Duration {
    if fast(step) {
        return step;
    }
    let (base_ms, min_ms, max_ms) = match action {
        Action::Technical => (80., 20., 160.),
        Action::Navigation => (120., 60., 240.),
        Action::Click => (650., 500., 900.),
        Action::Key => (700., 500., 900.),
        Action::Drag => (200., 100., 400.),
    };
    // 450ms 为观看基准，CLI 调节相对节奏，但重要操作仍保证可读范围。
    Duration::from_millis(
        (base_ms * step.as_secs_f64() / 0.45)
            .clamp(min_ms, max_ms)
            .round() as u64,
    )
}

fn input_hold(step: Duration) -> Duration {
    if fast(step) {
        step / 2
    } else {
        Duration::from_millis(45)
    }
}

fn chapter_pause(step: Duration, intro: bool) -> Duration {
    if fast(step) {
        step
    } else {
        Duration::from_millis(if intro { 1200 } else { 900 })
    }
}

fn movement_duration(step: Duration, distance: f32) -> Duration {
    if fast(step) {
        Duration::ZERO
    } else {
        let millis = distance as f64 / 1.3 * step.as_secs_f64() / 0.45;
        Duration::from_millis(millis.clamp(200., 800.).round() as u64)
    }
}

fn classify_action(
    moved: bool,
    before_mouse: &ButtonInput<MouseButton>,
    mouse: &ButtonInput<MouseButton>,
    keys: &ButtonInput<KeyCode>,
    events: &[bevy_egui::egui::Event],
) -> Action {
    use bevy_egui::egui;
    if keys
        .get_just_pressed()
        .any(|key| !is_modifier(&format!("{key:?}")))
        || events.iter().any(|event| {
            matches!(
                event,
                egui::Event::Key {
                    pressed: true,
                    repeat: false,
                    ..
                } | egui::Event::Text(_)
            )
        })
    {
        Action::Key
    } else if before_mouse.get_pressed().next().is_some() && moved {
        Action::Drag
    } else if mouse.get_just_pressed().next().is_some()
        || mouse.get_just_released().next().is_some()
        || events
            .iter()
            .any(|event| matches!(event, egui::Event::PointerButton { pressed: true, .. }))
    {
        Action::Click
    } else if moved
        || events
            .iter()
            .any(|event| matches!(event, egui::Event::MouseWheel { .. }))
    {
        Action::Navigation
    } else {
        Action::Technical
    }
}

fn unix_ms(time: SystemTime) -> u64 {
    time.duration_since(UNIX_EPOCH)
        .expect("系统时间早于 UNIX_EPOCH")
        .as_millis()
        .try_into()
        .expect("时间戳溢出")
}

fn append_timeline(output: &std::path::Path, event: &str, chapter: &Chapter, time: SystemTime) {
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open(output.join("demo-timeline.jsonl"))
        .expect("无法打开演示时间线");
    serde_json::to_writer(&mut file, &serde_json::json!({"event": event, "id": chapter.id, "title": chapter.title, "unix_ms": unix_ms(time)})).expect("无法写演示时间线");
    file.write_all(b"\n").expect("无法结束时间线记录");
    file.flush().expect("无法刷新演示时间线");
}

/// 只在边沿真正应用后写入；与章节时间线分离，移动帧和松手不制造音效点。
fn append_actions(
    output: &std::path::Path,
    chapter: &Chapter,
    actions: &[(&str, String)],
    time: SystemTime,
) {
    if actions.is_empty() {
        return;
    }
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open(output.join("demo-actions.jsonl"))
        .expect("无法打开演示动作日志");
    let timestamp = unix_ms(time);
    for (kind, detail) in actions
        .iter()
        .filter(|(kind, _)| matches!(*kind, "left" | "middle" | "right" | "key" | "scroll"))
    {
        serde_json::to_writer(&mut file, &serde_json::json!({
            "event": "action", "id": chapter.id, "chapter_id": chapter.id, "title": chapter.title,
            "unix_ms": timestamp, "kind": kind, "detail": detail,
        })).expect("无法写演示动作日志");
        file.write_all(
            b"
",
        )
        .expect("无法结束动作记录");
    }
    file.flush().expect("无法刷新演示动作日志");
}

/// 各章节复用已有步骤的说明，不增加虚构设置或重复操作。
pub(crate) fn describe(showcase: &mut Option<ResMut<Showcase>>, text: &'static str) {
    if let Some(showcase) = showcase
        && showcase.description != Some(text)
    {
        showcase.description = Some(text);
        showcase.description_changed = true;
    }
}

struct Motion {
    from: Vec2,
    to: Vec2,
    started: Instant,
    duration: Duration,
    ui: bool,
    action: Action,
    events: Vec<bevy_egui::egui::Event>,
    mouse: ButtonInput<MouseButton>,
    keys: ButtonInput<KeyCode>,
}

/// 只保存演示自身消费过的输入，下一帧不接受桌面误点或按键覆盖。
struct ReplayInput {
    mouse: ButtonInput<MouseButton>,
    keys: ButtonInput<KeyCode>,
    focused: bool,
}

impl Default for ReplayInput {
    fn default() -> Self {
        Self {
            mouse: default(),
            keys: default(),
            focused: true,
        }
    }
}

impl ReplayInput {
    fn restore(&self, mouse: &mut ButtonInput<MouseButton>, keys: &mut ButtonInput<KeyCode>) {
        *mouse = self.mouse.clone();
        *keys = self.keys.clone();
        mouse.clear();
        keys.clear();
    }
}

#[derive(Resource)]
pub(crate) struct Showcase {
    output: PathBuf,
    mode: Mode,
    chapters: Vec<&'static Chapter>,
    step: Duration,
    exit_on_complete: bool,
    pub(crate) presentation: bool,
    chapter_start_unix_ms: u64,
    finishing: Option<Instant>,
    key_release: Option<(Instant, Vec<KeyCode>)>,
    index: usize,
    initialize: bool,
    next_tick: Instant,
    chapter_started: Instant,
    artifact_since: SystemTime,
    reports: Vec<serde_json::Value>,
    completed: bool,
    pointer: Option<Vec2>,
    ui_pointer: Option<bevy_egui::egui::Pos2>,
    motion: Option<Motion>,
    before_mouse: ButtonInput<MouseButton>,
    before_keys: ButtonInput<KeyCode>,
    replay: ReplayInput,
    display_pointer: Option<Vec2>,
    click_release: Option<(Instant, bevy_egui::egui::Pos2)>,
    key_hints: KeyHints,
    mouse_hints: MouseHints,
    description: Option<&'static str>,
    description_changed: bool,
}

impl Showcase {
    pub fn from_args(args: &[String]) -> anyhow::Result<Option<Self>> {
        let Some(output) = args.windows(2).find(|w| w[0] == "--demo-showcase") else {
            return Ok(None);
        };
        let step_ms = args
            .windows(2)
            .find(|w| w[0] == "--demo-step-ms")
            .map(|w| w[1].parse::<u64>())
            .transpose()?
            .unwrap_or(450);
        anyhow::ensure!(step_ms > 0, "演示步骤间隔必须大于零");
        let mode = if let Some(index) = args.iter().position(|arg| arg == "--demo-mode") {
            match args.get(index + 1).map(String::as_str) {
                Some("brief") => Mode::Brief,
                Some("detailed") => Mode::Detailed,
                _ => anyhow::bail!("--demo-mode 必须为 brief（简略）或 detailed（详细）"),
            }
        } else {
            Mode::Brief
        };
        let output = std::path::absolute(&output[1])?;
        std::fs::create_dir_all(&output)?;
        anyhow::ensure!(
            !output.join("demo-report.json").exists(),
            "演示目录已有报告，请使用新目录"
        );
        // 启动时就发布文件，父录制进程可在第一章初始化完成之前订阅。
        std::fs::File::create(output.join("demo-timeline.jsonl"))?;
        std::fs::File::create(output.join("demo-actions.jsonl"))?;
        Ok(Some(Self {
            output,
            mode,
            chapters: mode.chapters(),
            step: Duration::from_millis(step_ms),
            exit_on_complete: args.iter().any(|arg| arg == "--demo-exit-on-complete"),
            presentation: args.iter().any(|arg| arg == "--demo-presentation"),
            chapter_start_unix_ms: 0,
            finishing: None,
            key_release: None,
            index: 0,
            initialize: true,
            next_tick: Instant::now(),
            chapter_started: Instant::now(),
            artifact_since: SystemTime::now(),
            reports: vec![],
            completed: false,
            pointer: None,
            ui_pointer: None,
            motion: None,
            before_mouse: default(),
            before_keys: default(),
            replay: default(),
            display_pointer: None,
            click_release: None,
            key_hints: KeyHints::default(),
            mouse_hints: MouseHints::default(),
            description: None,
            description_changed: false,
        }))
    }

    fn chapter(&self) -> &'static Chapter {
        self.chapters[self.index]
    }

    pub(crate) fn case_result_pause(&self) -> Duration {
        if self.mode == Mode::Detailed && !fast(self.step) {
            self.step * 4
        } else {
            Duration::ZERO
        }
    }
}

/// 详细演示不以旧专项的固定时长截断样例；显式超时由外部驱动管理。
pub(crate) fn within_timeout(showcase: Option<&Showcase>, started: Instant, seconds: u64) -> bool {
    showcase.is_some_and(|showcase| showcase.mode == Mode::Detailed)
        || started.elapsed().as_secs() < seconds
}

/// 非演示模式不影响任何原有自测；实际编辑器输入、绘制及通道轮询始终每帧运行。
pub(crate) fn advance_ready(showcase: Option<Res<Showcase>>) -> bool {
    showcase.is_none_or(|showcase| {
        !showcase.completed
            && !showcase.initialize
            && showcase.finishing.is_none()
            && showcase.motion.is_none()
            && showcase.click_release.is_none()
            && showcase.key_release.is_none()
            && Instant::now() >= showcase.next_tick
    })
}

pub(crate) fn begin(world: &mut World) {
    let Some(showcase) = world.get_resource::<Showcase>() else {
        return;
    };
    if showcase.completed || !showcase.initialize {
        return;
    }
    if Instant::now() < showcase.next_tick {
        return;
    }
    let index = showcase.index;
    let chapter = showcase.chapter();
    let presentation = showcase.presentation;
    let demo_mode = showcase.mode;
    let started = SystemTime::now();
    append_timeline(&showcase.output, "chapter_started", chapter, started);
    {
        let mut showcase = world.resource_mut::<Showcase>();
        showcase.chapter_started = Instant::now();
        showcase.chapter_start_unix_ms = unix_ms(started);
        showcase.artifact_since = started;
    }
    let paths = world.resource::<EditorPaths>().clone();
    let sample = match chapter.id {
        "panels" | "unsaved" => Some("Test.xml"),
        "repair" => Some("Heronb.xml"),
        _ => None,
    };
    let sample = sample.map(|name| {
        std::path::Path::new(&paths.assets)
            .join("ships")
            .join(name)
            .to_string_lossy()
            .into_owned()
    });
    let mut document =
        load_document(sample.as_deref(), &paths.catalog).expect("无法只读加载演示样本");
    // 简略保留原夹具；详细用真实长列表，复用相同的 UI/历史断言而不缩减数据。
    if chapter.id == "staging" {
        staging_fixture(&mut document, demo_mode);
    }
    document.revision = world.resource::<EditorDocument>().revision.wrapping_add(1);
    world.insert_resource(document);
    world.insert_resource(demo_cases::Run::new(chapter.id));
    world.resource_mut::<EditorPaths>().ship = sample;
    world.insert_resource(DragState::default());
    world.insert_resource(EditorCursor::default());
    world.insert_resource(CameraDrag::default());
    world.insert_resource(view::ViewOptions::default());
    world.insert_resource(properties::Inspector::default());
    world.insert_resource(topology_ui::ConnectionEditor::default());
    world.insert_resource(files::PendingFileAction::default());
    world.insert_resource(panels::UiPointer::default());
    world.insert_resource(panels::ShipBrowser::new(
        std::path::Path::new(&paths.assets).join("ships"),
    ));
    world.resource_mut::<ButtonInput<KeyCode>>().reset_all();
    world.resource_mut::<ButtonInput<MouseButton>>().reset_all();
    world.resource_mut::<Messages<files::FileAction>>().clear();
    for (mut transform, mut projection) in world
        .query_filtered::<(&mut Transform, &mut Projection), With<Camera2d>>()
        .iter_mut(world)
    {
        transform.translation.x = 0.;
        transform.translation.y = 0.;
        if let Projection::Orthographic(projection) = &mut *projection {
            projection.scale = 1.;
        }
    }
    // presentation 的物理 1080p 与 scale factor 由 main 启动窗口负责。
    if !presentation {
        for mut window in world.query::<&mut Window>().iter_mut(world) {
            window.resolution.set(1440., 900.);
        }
    }
    {
        let mut mode = world.resource_mut::<SmokeTest>();
        let id = chapter.id;
        mode.panels = id == "panels";
        mode.connections = id == "connections";
        mode.transforms = id == "transforms";
        mode.selection = id == "selection";
        mode.view = id == "view";
        mode.staging = id == "staging";
        mode.topology = id == "topology";
        mode.repair = id == "repair";
        mode.browser = id == "browser";
        mode.native_dialogs = id == "unsaved";
        mode.started = Instant::now();
    }
    let mut showcase = world.resource_mut::<Showcase>();
    showcase.initialize = false;
    showcase.pointer = None;
    showcase.ui_pointer = None;
    showcase.motion = None;
    showcase.replay = ReplayInput::default();
    showcase.click_release = None;
    showcase.key_hints = KeyHints::default();
    showcase.mouse_hints = MouseHints::default();
    showcase.description = None;
    showcase.description_changed = false;
    showcase.key_release = None;
    showcase.next_tick = Instant::now() + chapter_pause(showcase.step, true);
    info!(
        "演示 {}/{}：{}",
        index + 1,
        showcase.chapters.len(),
        chapter.title
    );
}

/// 在所有 Update 注入之后消费节拍。截图观察者不受节拍限制。
pub(crate) fn pace(
    mut showcase: Option<ResMut<Showcase>>,
    windows: Query<&Window>,
    inputs: Query<&bevy_egui::EguiInput>,
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
) {
    if let Some(showcase) = showcase.as_mut() {
        showcase.pointer = windows.iter().next().and_then(Window::cursor_position);
        showcase.replay.mouse = mouse.clone();
        showcase.replay.keys = keys.clone();
        if let Some(window) = windows.iter().next() {
            showcase.replay.focused = window.focused;
        }
        for input in &inputs {
            for event in &input.0.events {
                if let bevy_egui::egui::Event::PointerMoved(pos) = event {
                    showcase.ui_pointer = Some(*pos);
                }
            }
        }
    }
    if let Some(showcase) = showcase.as_mut()
        && !showcase.initialize
        && showcase.finishing.is_none()
        && showcase.motion.is_none()
        && Instant::now() >= showcase.next_tick
    {
        // animate 已为实际动作或到达设置停顿；这里只消费无输入的技术步骤。
        showcase.next_tick = Instant::now() + action_pause(showcase.step, Action::Technical);
    }
}

/// 注入坐标只服务本帧编辑器；PostUpdate 后设 None，阻止 Winit 在 Last 移动系统光标。
/// 下一帧 Update 再恢复坐标，因此无需把桌面前台或系统鼠标当作演示先决条件。
pub(crate) fn restore_pointer(
    showcase: Option<ResMut<Showcase>>,
    mut windows: Query<&mut Window>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut inputs: Query<&mut bevy_egui::EguiInput>,
    wheels: Option<ResMut<Messages<bevy::input::mouse::MouseWheel>>>,
) {
    if let Some(mut showcase) = showcase
        && !showcase.completed
    {
        showcase.replay.restore(&mut mouse, &mut keys);
        showcase.before_mouse = mouse.clone();
        showcase.before_keys = keys.clone();
        // 原生鼠标、按键、文本和滚轮不得混入内部回放；章节稍后注入自己的输入。
        for mut input in &mut inputs {
            input.0.events.clear();
            input.0.focused = showcase.replay.focused;
        }
        if let Some(mut wheels) = wheels {
            wheels.clear();
        }
        for mut window in &mut windows {
            window.set_cursor_position(showcase.pointer);
            window.focused = showcase.replay.focused;
        }
    }
}
/// 线性插值的是编辑器实际使用的指针；按键/按钮边沿在到达目标时再应用。
/// egui 点击采用短按，观看节拍只控制下一动作，不把按钮一直按到下一节拍。
#[allow(clippy::too_many_arguments)]
pub(crate) fn animate(
    showcase: Option<ResMut<Showcase>>,
    mut windows: Query<&mut Window, With<bevy::window::PrimaryWindow>>,
    mut inputs: Query<&mut bevy_egui::EguiInput, With<bevy_egui::PrimaryEguiContext>>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
) {
    use bevy_egui::egui;
    let Some(mut showcase) = showcase else {
        return;
    };
    if showcase.completed || showcase.initialize {
        return;
    }
    let (Ok(mut window), Ok(mut input)) = (windows.single_mut(), inputs.single_mut()) else {
        return;
    };
    let now = Instant::now();
    let mut events = std::mem::take(&mut input.0.events);
    let mut applied_action = None;
    if let Some((deadline, held)) = &showcase.key_release
        && now >= *deadline
    {
        for key in held {
            keys.release(*key);
        }
        showcase.key_release = None;
    }
    if showcase.motion.is_none() {
        let ui_target = events.iter().rev().find_map(|event| match event {
            egui::Event::PointerMoved(point) => Some(Vec2::new(point.x, point.y)),
            _ => None,
        });
        // 显式 egui 坐标始终优先，包括在原位置释放的 PointerMoved。
        // smoke 保存的窗口坐标可能仍在画布/滚轮处，不能在短按期间回退过去。
        let ui = ui_target.is_some();
        let target = ui_target.or_else(|| window.cursor_position());
        if let Some(to) = target {
            let from = showcase
                .display_pointer
                .unwrap_or(Vec2::new(window.width() / 2., window.height() / 2.));
            let moved = from.distance(to) > 1.;
            let action = classify_action(moved, &showcase.before_mouse, &mouse, &keys, &events);
            if moved && !fast(showcase.step) {
                showcase.motion = Some(Motion {
                    from,
                    to,
                    started: now,
                    duration: movement_duration(showcase.step, from.distance(to)),
                    ui,
                    action,
                    events: std::mem::take(&mut events),
                    mouse: mouse.clone(),
                    keys: keys.clone(),
                });
                *mouse = showcase.before_mouse.clone();
                *keys = showcase.before_keys.clone();
            } else {
                // fast/原位点击也必须同步实际窗口指针，pace 才不会保存旧画布坐标。
                window.set_cursor_position(Some(to));
                showcase.display_pointer = Some(to);
                if now >= showcase.next_tick {
                    applied_action = Some(action);
                }
            }
        } else if now >= showcase.next_tick {
            applied_action = Some(classify_action(
                false,
                &showcase.before_mouse,
                &mouse,
                &keys,
                &events,
            ));
        }
    }
    if let Some(motion) = showcase.motion.as_ref() {
        let t =
            (motion.started.elapsed().as_secs_f32() / motion.duration.as_secs_f32()).clamp(0., 1.);
        let point = motion.from.lerp(motion.to, t);
        window.set_cursor_position(Some(point));
        showcase.display_pointer = Some(point);
        if t >= 1. {
            let motion = showcase.motion.take().unwrap();
            events.extend(motion.events);
            *mouse = motion.mouse;
            *keys = motion.keys;
            applied_action = Some(motion.action);
            if motion.ui {
                showcase.ui_pointer = Some(egui::pos2(point.x, point.y));
            }
        }
    }
    let point = showcase.display_pointer.or(window.cursor_position());
    if let Some(point) = point {
        // 每一帧保持 egui 的内部指针连续，不只更新说明区的装饰圆环。
        input
            .0
            .events
            .push(egui::Event::PointerMoved(egui::pos2(point.x, point.y)));
        showcase.ui_pointer = Some(egui::pos2(point.x, point.y));
    }
    if let Some((deadline, point)) = showcase.click_release
        && now >= deadline
    {
        input.0.events.push(egui::Event::PointerButton {
            pos: point,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        });
        showcase.click_release = None;
    }
    for event in &events {
        if let egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: true,
            ..
        } = event
        {
            showcase.click_release = Some((now + input_hold(showcase.step), *pos));
        }
    }
    if let Some(action) = applied_action {
        // 停顿从边沿实际重播/指针抵达后开始，而不是从注入帧开始。
        let pause_action = if action == Action::Technical && showcase.description_changed {
            Action::Key
        } else {
            action
        };
        showcase.next_tick = now + action_pause(showcase.step, pause_action);
        showcase.description_changed = false;
        let held: Vec<_> = keys
            .get_just_pressed()
            .copied()
            .filter(|key| !is_modifier(&format!("{key:?}")))
            .collect();
        if !held.is_empty() {
            showcase.key_release = Some((now + input_hold(showcase.step), held));
        }
    }
    let key_actions = showcase.key_hints.collect(&keys, &events, now);
    let mut actions = showcase.mouse_hints.collect(&mouse, &events, now);
    actions.extend(key_actions.into_iter().map(|label| ("key", label)));
    append_actions(
        &showcase.output,
        showcase.chapter(),
        &actions,
        SystemTime::now(),
    );
    input.0.events.extend(events);
}

pub(crate) fn isolate_pointer(showcase: Option<Res<Showcase>>, mut windows: Query<&mut Window>) {
    if showcase.is_some_and(|showcase| !showcase.completed) {
        for mut window in &mut windows {
            window.set_cursor_position(None);
        }
    }
}

/// Last 截获章节自测的成功退出；失败退出永不转为通过。
pub(crate) fn finish(world: &mut World) {
    let Some(showcase) = world.get_resource::<Showcase>() else {
        return;
    };
    if showcase.completed || showcase.initialize {
        return;
    }
    // 成果停留期间出现的失败也不得发布 finished 成功事件。
    if world
        .resource::<Messages<AppExit>>()
        .iter_current_update_messages()
        .any(|exit| !matches!(exit, AppExit::Success))
    {
        return;
    }
    if showcase.finishing.is_none() {
        let exits: Vec<_> = world
            .resource::<Messages<AppExit>>()
            .iter_current_update_messages()
            .cloned()
            .collect();
        if exits.is_empty() || exits.iter().any(|exit| !matches!(exit, AppExit::Success)) {
            return;
        }
        // 用户关闭窗口也产生 Success，只有本章所有新鲜证据齐全才接受。
        let chapter = showcase.chapter();
        let evidence = chapter.evidence();
        let fresh = evidence.iter().all(|name| {
            std::fs::metadata(format!("target/{name}")).is_ok_and(|metadata| {
                metadata.len() > 0
                    && metadata
                        .modified()
                        .is_ok_and(|mtime| mtime >= showcase.artifact_since)
            })
        });
        if !fresh {
            return;
        }
        let output = showcase.output.clone();
        for name in &evidence {
            std::fs::copy(format!("target/{name}"), output.join(name)).expect("无法复制演示证据");
        }
        world.resource_mut::<Messages<AppExit>>().clear();
        let mut showcase = world.resource_mut::<Showcase>();
        showcase.finishing = Some(Instant::now() + chapter_pause(showcase.step, false));
        return;
    }
    if Instant::now() < showcase.finishing.unwrap() {
        return;
    }
    let chapter = showcase.chapter();
    let ended = SystemTime::now();
    append_timeline(&showcase.output, "chapter_finished", chapter, ended);
    let output = showcase.output.clone();
    let elapsed = showcase.chapter_started.elapsed().as_secs_f64();
    let report = serde_json::json!({"id":chapter.id, "title":chapter.title, "status":"passed", "elapsed_seconds":elapsed, "artifacts":chapter.evidence(), "start_unix_ms":showcase.chapter_start_unix_ms, "end_unix_ms":unix_ms(ended)});
    let mut showcase = world.resource_mut::<Showcase>();
    showcase.finishing = None;
    showcase.reports.push(report);
    showcase.index += 1;
    if showcase.index < showcase.chapters.len() {
        showcase.initialize = true;
        showcase.next_tick = Instant::now();
        return;
    }
    let report = serde_json::json!({"completed":true, "mode":showcase.mode.name(), "step_ms":showcase.step.as_millis(), "input_mode":"内部 UI/画布输入注入；不是系统键鼠验收", "chapters":showcase.reports});
    let temporary = output.join("demo-report.json.tmp");
    std::fs::write(&temporary, serde_json::to_vec_pretty(&report).unwrap())
        .expect("无法写演示报告");
    std::fs::rename(temporary, output.join("demo-report.json")).expect("无法发布完整演示报告");
    showcase.completed = true;
    let exit = showcase.exit_on_complete;
    let label = showcase.mode.label();
    world.resource_mut::<SmokeTest>().native_dialogs = false;
    info!(
        "{}模式演示完成，报告：{}",
        label,
        output.join("demo-report.json").display()
    );
    if exit {
        world
            .resource_mut::<Messages<AppExit>>()
            .write(AppExit::Success);
    }
}

/// 说明区不捕获指针；避免改变被演示控件的焦点与命中。
#[allow(clippy::too_many_arguments)]
pub(crate) fn overlay(
    mut contexts: bevy_egui::EguiContexts,
    showcase: Option<Res<Showcase>>,
    inspector: Res<properties::Inspector>,
    topology: Res<topology_ui::ConnectionEditor>,
    pending: Res<files::PendingFileAction>,
    help: Res<help::HelpState>,
    mouse: Res<ButtonInput<MouseButton>>,
    details: Option<Res<demo_cases::Run>>,
) {
    use bevy_egui::egui;
    let Some(showcase) = showcase else {
        return;
    };
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };
    if let Some(hint) = &showcase.key_hints.latest {
        paint_key_hint(ctx, hint, Instant::now(), help.open);
    }
    let held: Vec<_> = [MouseButton::Left, MouseButton::Middle, MouseButton::Right]
        .into_iter()
        .filter(|button| mouse.pressed(*button))
        .filter_map(mouse_label)
        .map(|(_, label)| label)
        .collect();
    if !held.is_empty() {
        let now = Instant::now();
        paint_input_hint(
            ctx,
            &KeyHint {
                text: format!("{} · 按住", held.join(" · ")),
                applied: now,
            },
            now,
            help.open,
            true,
        );
    } else if let Some(hint) = &showcase.mouse_hints.latest {
        paint_input_hint(ctx, hint, Instant::now(), help.open, true);
    }
    if !showcase.completed {
        let pointer = if inspector.is_open() || topology.open || pending.is_blocked() {
            showcase.ui_pointer
        } else {
            showcase
                .pointer
                .map(|point| egui::pos2(point.x, point.y))
                .or(showcase.ui_pointer)
        };
        if let Some(point) = pointer {
            let painter = ctx.layer_painter(egui::LayerId::new(
                egui::Order::Tooltip,
                egui::Id::new("demo-pointer"),
            ));
            painter.circle_stroke(
                point,
                9.,
                egui::Stroke::new(2., egui::Color32::from_rgb(222, 185, 255)),
            );
            painter.circle_filled(point, 2., egui::Color32::WHITE);
        }
    }
    // 帮助需要完整阅读：暂停章节横幅，并把按键提示放在窗口下方空隙。
    if help.open {
        return;
    }
    if showcase.presentation {
        // 录制模式额外提供顶部短章名；下方仍保留十章说明框，字幕在视频独立底栏。
        let title = if showcase.completed {
            "演示完成"
        } else {
            showcase.chapter().title
        };
        let painter = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Tooltip,
            egui::Id::new("demo-chapter"),
        ));
        painter.text(
            egui::pos2(
                ctx.content_rect().center().x,
                ctx.content_rect().top() + 14.,
            ),
            egui::Align2::CENTER_TOP,
            title,
            egui::FontId::proportional(20.),
            egui::Color32::from_rgb(221, 203, 255),
        );
    }
    let (title, description) = if showcase.completed {
        (
            format!(
                "{1}演示完成：{0}/{0} 章节通过",
                showcase.chapters.len(),
                showcase.mode.label()
            ),
            "原版样本未保存覆盖。截图和 XML 副本已写入演示目录；可以关闭窗口。",
        )
    } else {
        let chapter = showcase.chapter();
        (
            format!(
                "{}演示 {}/{} · {}",
                showcase.mode.label(),
                showcase.index + 1,
                showcase.chapters.len(),
                chapter.title
            ),
            showcase.description.unwrap_or(chapter.description),
        )
    };
    egui::Area::new(egui::Id::new("demo-caption"))
        .anchor(egui::Align2::CENTER_BOTTOM, egui::vec2(0., -14.))
        .order(egui::Order::Tooltip)
        .interactable(false)
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(egui::Color32::from_rgb(25, 23, 34))
                .stroke(egui::Stroke::new(
                    1.,
                    egui::Color32::from_rgb(142, 116, 199),
                ))
                .corner_radius(6.)
                .inner_margin(10.)
                .show(ui, |ui| {
                    ui.set_max_width(620.);
                    ui.label(
                        egui::RichText::new(title)
                            .size(18.)
                            .color(egui::Color32::from_rgb(221, 203, 255)),
                    );
                    ui.label(description);
                    if let Some(caption) = details.as_ref().and_then(|details| details.caption()) {
                        ui.separator();
                        ui.label(caption);
                    }
                    ui.label(
                        egui::RichText::new(format!(
                            "节奏基准 {}ms · 内部输入注入 · 窗口单次启动",
                            showcase.step.as_millis()
                        ))
                        .size(12.)
                        .weak(),
                    );
                });
        });
}

#[cfg(test)]
#[path = "demo_pacing_tests.rs"]
mod pacing_tests;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn detailed_staging_uses_the_full_64_stage_1024_action_fixture_while_brief_stays_small() {
        for (mode, stages, activations, parts) in
            [(Mode::Brief, 8, 48, 7), (Mode::Detailed, 64, 1024, 17)]
        {
            let mut document = crate::tests::document();
            document.catalog = dr_core::catalog_from_xml(
                r#"<PartTypes>
              <PartType id="pod-1" type="pod" width="4" height="3"/>
              <PartType id="detacher-1" width="4" height="1"/>
            </PartTypes>"#,
            )
            .unwrap();
            document.ship = new_ship(&document.catalog);
            staging_fixture(&mut document, mode);
            let staging = document.ship.parts[0]
                .pod
                .as_ref()
                .unwrap()
                .staging
                .as_ref()
                .unwrap();
            assert_eq!(staging.steps.len(), stages);
            assert_eq!(
                staging
                    .steps
                    .iter()
                    .map(|stage| stage.activations.len())
                    .sum::<usize>(),
                activations
            );
            assert_eq!(document.ship.parts.len(), parts);
            assert_eq!(document.ship, document.saved_ship);
            assert!(
                staging
                    .steps
                    .iter()
                    .flat_map(|stage| &stage.activations)
                    .all(|activation| document.ship.part(activation.id).is_some())
            );
            let chapter = mode
                .chapters()
                .into_iter()
                .find(|chapter| chapter.id == "staging")
                .unwrap();
            assert_eq!(
                chapter
                    .artifacts
                    .contains(&"editor-staging-performance.json"),
                mode == Mode::Detailed
            );
        }
    }
    #[test]
    fn modes_are_explicit_and_only_detailed_removes_fixed_smoke_deadlines() {
        let old = Instant::now() - Duration::from_secs(3600);
        assert!(!within_timeout(None, old, 60));
        for (args_mode, expected) in [
            (None, Mode::Brief),
            (Some("brief"), Mode::Brief),
            (Some("detailed"), Mode::Detailed),
        ] {
            let folder = tempfile::tempdir().unwrap();
            let mut args = vec![
                "editor".into(),
                "--demo-showcase".into(),
                folder.path().to_string_lossy().into_owned(),
            ];
            if let Some(mode) = args_mode {
                args.extend(["--demo-mode".into(), mode.into()]);
            }
            let showcase = Showcase::from_args(&args).unwrap().unwrap();
            assert_eq!(showcase.mode, expected);
            assert_eq!(showcase.step, Duration::from_millis(450));
            assert_eq!(
                showcase.case_result_pause(),
                if expected == Mode::Detailed {
                    Duration::from_millis(1800)
                } else {
                    Duration::ZERO
                }
            );
            assert_eq!(
                within_timeout(Some(&showcase), old, 60),
                expected == Mode::Detailed
            );
            assert_eq!(showcase.chapter().id, "panels");
        }
        for tail in [vec!["--demo-mode"], vec!["--demo-mode", "unknown"]] {
            let folder = tempfile::tempdir().unwrap();
            let mut args = vec![
                "editor".into(),
                "--demo-showcase".into(),
                folder.path().to_string_lossy().into_owned(),
            ];
            args.extend(tail.into_iter().map(String::from));
            assert!(Showcase::from_args(&args).is_err());
        }
    }
    #[test]
    fn chapters_are_unique_and_evidence_is_relative() {
        let ids: BTreeSet<_> = CHAPTERS.iter().map(|chapter| chapter.id).collect();
        assert_eq!(ids.len(), 10);
        for chapter in CHAPTERS {
            assert!(!chapter.artifacts.is_empty());
            for name in chapter.artifacts {
                assert_eq!(std::path::Path::new(name).components().count(), 1);
            }
        }
        let detailed = Mode::Detailed.chapters();
        assert_eq!(detailed.len(), 18);
        assert_eq!(
            detailed
                .iter()
                .map(|chapter| chapter.id)
                .collect::<BTreeSet<_>>()
                .len(),
            18
        );
        assert_eq!(
            detailed
                .iter()
                .filter(|chapter| demo_cases::cases(chapter.id).is_empty())
                .map(|chapter| chapter.id)
                .collect::<Vec<_>>(),
            CHAPTERS
                .iter()
                .map(|chapter| chapter.id)
                .collect::<Vec<_>>()
        );
        let evidence: Vec<_> = detailed
            .iter()
            .flat_map(|chapter| chapter.evidence())
            .collect();
        assert_eq!(evidence.len(), 90);
        assert_eq!(evidence.iter().collect::<BTreeSet<_>>().len(), 90);
    }
    #[test]
    fn failed_and_unproven_success_exit_are_not_swallowed() {
        let folder = tempfile::tempdir().unwrap();
        let args = vec![
            "editor".into(),
            "--demo-showcase".into(),
            folder.path().to_string_lossy().into_owned(),
        ];
        for exit in [
            AppExit::Success,
            AppExit::Error(std::num::NonZeroU8::new(1).unwrap()),
        ] {
            let mut showcase = Showcase::from_args(&args).unwrap().unwrap();
            showcase.initialize = false;
            showcase.artifact_since = SystemTime::now() + Duration::from_secs(3600);
            let mut world = World::new();
            world.insert_resource(showcase);
            let mut exits = Messages::<AppExit>::default();
            exits.write(exit.clone());
            world.insert_resource(exits);
            finish(&mut world);
            assert_eq!(world.resource::<Showcase>().index, 0);
            assert!(world.resource::<Showcase>().reports.is_empty());
            assert_eq!(
                world
                    .resource::<Messages<AppExit>>()
                    .iter_current_update_messages()
                    .next(),
                Some(&exit)
            );
            assert!(!folder.path().join("demo-report.json").exists());
            assert!(
                std::fs::read_to_string(folder.path().join("demo-timeline.jsonl"))
                    .unwrap()
                    .is_empty()
            );
        }
    }

    #[test]
    fn demo_pointer_is_removed_before_native_window_sync() {
        let folder = tempfile::tempdir().unwrap();
        let args = vec![
            "editor".into(),
            "--demo-showcase".into(),
            folder.path().to_string_lossy().into_owned(),
        ];
        let mut showcase = Showcase::from_args(&args).unwrap().unwrap();
        showcase.pointer = Some(Vec2::new(100., 200.));
        let mut app = App::new();
        app.init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<ButtonInput<KeyCode>>();
        app.insert_resource(showcase)
            .add_systems(Update, (restore_pointer, isolate_pointer).chain());
        let window = app.world_mut().spawn(Window::default()).id();
        app.update();
        assert_eq!(
            app.world()
                .get::<Window>(window)
                .unwrap()
                .physical_cursor_position(),
            None
        );
        assert_eq!(
            app.world().resource::<Showcase>().pointer,
            Some(Vec2::new(100., 200.))
        );
    }

    #[test]
    fn ordinary_window_cursor_is_not_modified() {
        let mut app = App::new();
        app.init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<ButtonInput<KeyCode>>()
            .add_systems(Update, (restore_pointer, isolate_pointer).chain());
        let mut window = Window::default();
        window.set_cursor_position(Some(Vec2::new(100., 200.)));
        let entity = app.world_mut().spawn(window).id();
        app.update();
        assert_eq!(
            app.world().get::<Window>(entity).unwrap().cursor_position(),
            Some(Vec2::new(100., 200.))
        );
    }

    #[test]
    fn motion_interpolates_real_cursor_and_delays_button_edge() {
        let folder = tempfile::tempdir().unwrap();
        let args = vec![
            "editor".into(),
            "--demo-showcase".into(),
            folder.path().to_string_lossy().into_owned(),
        ];
        let mut showcase = Showcase::from_args(&args).unwrap().unwrap();
        showcase.initialize = false;
        showcase.display_pointer = Some(Vec2::ZERO);
        let mut app = App::new();
        app.insert_resource(showcase)
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<ButtonInput<KeyCode>>()
            .add_systems(Update, animate);
        let mut window = Window::default();
        window.set_cursor_position(Some(Vec2::new(100., 0.)));
        let entity = app
            .world_mut()
            .spawn((
                window,
                bevy::window::PrimaryWindow,
                bevy_egui::EguiInput::default(),
                bevy_egui::PrimaryEguiContext,
            ))
            .id();
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        assert!(
            app.world()
                .get::<Window>(entity)
                .unwrap()
                .cursor_position()
                .unwrap()
                .length()
                < 1.,
            "首帧允许实际计时造成的亚像素移动"
        );
        assert!(
            !app.world()
                .resource::<ButtonInput<MouseButton>>()
                .pressed(MouseButton::Left)
        );
        {
            let mut showcase = app.world_mut().resource_mut::<Showcase>();
            let motion = showcase.motion.as_mut().unwrap();
            motion.started = Instant::now() - motion.duration / 2;
        }
        app.update();
        let x = app
            .world()
            .get::<Window>(entity)
            .unwrap()
            .cursor_position()
            .unwrap()
            .x;
        assert!((45. ..55.).contains(&x), "中间帧应线性移动：{x}");
        assert!(
            !app.world()
                .resource::<ButtonInput<MouseButton>>()
                .pressed(MouseButton::Left)
        );
        {
            let mut showcase = app.world_mut().resource_mut::<Showcase>();
            let motion = showcase.motion.as_mut().unwrap();
            motion.started = Instant::now() - motion.duration;
        }
        app.update();
        assert_eq!(
            app.world().get::<Window>(entity).unwrap().cursor_position(),
            Some(Vec2::new(100., 0.))
        );
        assert!(
            app.world()
                .resource::<ButtonInput<MouseButton>>()
                .just_pressed(MouseButton::Left)
        );
        assert!(app.world().resource::<Showcase>().motion.is_none());
    }

    fn key_event(key: bevy_egui::egui::Key, pressed: bool, repeat: bool) -> bevy_egui::egui::Event {
        use bevy_egui::egui;
        egui::Event::Key {
            key,
            physical_key: Some(key),
            pressed,
            repeat,
            modifiers: egui::Modifiers {
                ctrl: true,
                command: true,
                ..Default::default()
            },
        }
    }

    pub(super) fn animation_app(step_ms: u64, target: Vec2) -> (App, Entity, tempfile::TempDir) {
        let folder = tempfile::tempdir().unwrap();
        let args = vec![
            "editor".into(),
            "--demo-showcase".into(),
            folder.path().to_string_lossy().into_owned(),
            "--demo-step-ms".into(),
            step_ms.to_string(),
        ];
        let mut showcase = Showcase::from_args(&args).unwrap().unwrap();
        showcase.initialize = false;
        showcase.display_pointer = Some(Vec2::ZERO);
        let mut app = App::new();
        app.insert_resource(showcase)
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<ButtonInput<KeyCode>>()
            .add_systems(Update, animate);
        let mut window = Window::default();
        window.set_cursor_position(Some(target));
        let entity = app
            .world_mut()
            .spawn((
                window,
                bevy::window::PrimaryWindow,
                bevy_egui::EguiInput::default(),
                bevy_egui::PrimaryEguiContext,
            ))
            .id();
        (app, entity, folder)
    }

    fn inject_undo(app: &mut App, entity: Entity, keyboard: bool, egui: bool) {
        if keyboard {
            let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            keys.press(KeyCode::ControlLeft);
            keys.press(KeyCode::KeyZ);
        }
        if egui {
            app.world_mut()
                .get_mut::<bevy_egui::EguiInput>(entity)
                .unwrap()
                .0
                .events
                .extend([
                    key_event(bevy_egui::egui::Key::Z, true, false),
                    key_event(bevy_egui::egui::Key::Z, false, false),
                ]);
        }
    }

    #[test]
    fn key_hints_wait_for_motion_replay_for_each_input_source() {
        for (keyboard, egui) in [(true, false), (false, true), (true, true)] {
            let (mut app, entity, _folder) = animation_app(450, Vec2::new(100., 0.));
            inject_undo(&mut app, entity, keyboard, egui);
            app.update();
            assert!(
                app.world()
                    .resource::<Showcase>()
                    .key_hints
                    .latest
                    .is_none()
            );
            assert!(
                !app.world()
                    .resource::<ButtonInput<KeyCode>>()
                    .just_pressed(KeyCode::KeyZ)
            );
            assert!(
                !app.world()
                    .get::<bevy_egui::EguiInput>(entity)
                    .unwrap()
                    .0
                    .events
                    .iter()
                    .any(|event| matches!(event, bevy_egui::egui::Event::Key { .. }))
            );
            {
                let mut showcase = app.world_mut().resource_mut::<Showcase>();
                let motion = showcase.motion.as_mut().unwrap();
                // 无 sleep：明确保持中途帧未到达，然后强制到达。
                motion.started = Instant::now();
                motion.duration = Duration::from_secs(60);
            }
            app.update();
            assert!(
                app.world()
                    .resource::<Showcase>()
                    .key_hints
                    .latest
                    .is_none()
            );
            {
                let mut showcase = app.world_mut().resource_mut::<Showcase>();
                let motion = showcase.motion.as_mut().unwrap();
                motion.started = Instant::now() - motion.duration;
            }
            let replay_start = Instant::now();
            app.update();
            let showcase = app.world().resource::<Showcase>();
            let hint = showcase.key_hints.latest.as_ref().unwrap();
            assert_eq!(hint.text, "Ctrl+Z");
            assert!(hint.applied >= replay_start);
            assert!(showcase.motion.is_none());
            let applied = hint.applied;
            assert_eq!(
                app.world()
                    .resource::<ButtonInput<KeyCode>>()
                    .just_pressed(KeyCode::KeyZ),
                keyboard
            );
            let input = app.world().get::<bevy_egui::EguiInput>(entity).unwrap();
            assert_eq!(
                input
                    .0
                    .events
                    .iter()
                    .filter(|event| matches!(event, bevy_egui::egui::Event::Key { .. }))
                    .count(),
                if egui { 2 } else { 0 }
            );
            // 模拟 egui pass 消费事件，保留 ButtonInput 边沿检验不逐帧刷新。
            app.world_mut()
                .get_mut::<bevy_egui::EguiInput>(entity)
                .unwrap()
                .0
                .events
                .clear();
            app.update();
            assert_eq!(
                app.world()
                    .resource::<Showcase>()
                    .key_hints
                    .latest
                    .as_ref()
                    .unwrap()
                    .applied,
                applied
            );
        }
    }

    #[test]
    fn key_hints_capture_direct_inputs_in_fast_and_stationary_modes() {
        for (step, target) in [(1, Vec2::new(100., 0.)), (450, Vec2::ZERO)] {
            for (keyboard, egui) in [(true, false), (false, true), (true, true)] {
                let (mut app, entity, _folder) = animation_app(step, target);
                inject_undo(&mut app, entity, keyboard, egui);
                app.update();
                let showcase = app.world().resource::<Showcase>();
                assert!(showcase.motion.is_none());
                assert_eq!(showcase.key_hints.latest.as_ref().unwrap().text, "Ctrl+Z");
                assert!(showcase.click_release.is_none());
            }
        }
    }

    #[test]
    fn key_hint_opacity_holds_then_fades_linearly_and_expires() {
        let now = Instant::now();
        let hint = KeyHint {
            text: "F2".into(),
            applied: now,
        };
        assert_eq!(hint.opacity(now), 1.);
        assert_eq!(hint.opacity(now + KEY_HINT_HOLD), 1.);
        assert!((hint.opacity(now + KEY_HINT_HOLD + KEY_HINT_FADE / 4) - 0.75).abs() < 0.001);
        assert!((hint.opacity(now + KEY_HINT_HOLD + KEY_HINT_FADE / 2) - 0.5).abs() < 0.001);
        assert_eq!(hint.opacity(now + KEY_HINT_HOLD + KEY_HINT_FADE), 0.);
        assert_eq!(hint.opacity(now + Duration::from_secs(10)), 0.);
    }

    #[test]
    fn key_hints_ignore_modifiers_repeats_and_held_edges_but_accept_repress() {
        use bevy_egui::egui;
        let now = Instant::now();
        let mut hints = KeyHints::default();
        let mut keys = ButtonInput::default();
        for key in [
            KeyCode::ControlRight,
            KeyCode::ShiftRight,
            KeyCode::AltRight,
            KeyCode::SuperRight,
        ] {
            keys.press(key);
        }
        hints.collect(
            &keys,
            &[key_event(egui::Key::ControlLeft, true, false)],
            now,
        );
        assert!(hints.latest.is_none());
        keys.press(KeyCode::KeyZ);
        hints.collect(&keys, &[key_event(egui::Key::Z, true, true)], now);
        assert!(
            hints.latest.is_none(),
            "repeat 不得经由 ButtonInput fallback 重新出现"
        );
        hints.collect(&keys, &[], now);
        assert!(hints.latest.is_none());
        keys.clear();
        hints.collect(&keys, &[], now);
        keys.release(KeyCode::KeyZ);
        keys.press(KeyCode::KeyZ);
        hints.collect(&keys, &[], now);
        assert_eq!(hints.latest.as_ref().unwrap().text, "Ctrl+Alt+Shift+Cmd+Z");
        let later = now + Duration::from_millis(500);
        hints.collect(&keys, &[], later);
        assert_eq!(hints.latest.as_ref().unwrap().applied, now);
        keys.clear();
        hints.collect(&keys, &[], later);
        keys.release(KeyCode::KeyZ);
        keys.press(KeyCode::KeyZ);
        hints.collect(&keys, &[], later);
        assert_eq!(hints.latest.as_ref().unwrap().applied, later);
    }

    #[test]
    fn key_hints_use_logical_names_and_deduplicate_physical_keys() {
        use bevy_egui::egui;
        let mut hints = KeyHints::default();
        let mut keys = ButtonInput::default();
        keys.press(KeyCode::KeyY);
        let mut event = key_event(egui::Key::Z, true, false);
        if let egui::Event::Key {
            physical_key,
            modifiers,
            ..
        } = &mut event
        {
            *physical_key = Some(egui::Key::Y);
            modifiers.shift = true;
        }
        hints.collect(&keys, &[event], Instant::now());
        assert_eq!(hints.latest.as_ref().unwrap().text, "Ctrl+Shift+Z");
        for (key, expected) in [
            (KeyCode::KeyR, "R"),
            (KeyCode::Digit1, "1"),
            (KeyCode::F2, "F2"),
            (KeyCode::Escape, "Escape"),
            (KeyCode::ArrowLeft, "Left"),
            (KeyCode::NumpadEnter, "Enter"),
        ] {
            assert_eq!(button_key_name(key), expected);
        }
    }

    #[test]
    fn key_hint_painter_does_not_block_underlying_input_and_stops_after_expiry() {
        use bevy_egui::egui;
        let ctx = egui::Context::default();
        let now = Instant::now();
        let hint = KeyHint {
            text: "Ctrl+Z".into(),
            applied: now,
        };
        let rect = egui::Rect::from_center_size(egui::pos2(400., 72.), egui::vec2(120., 40.));
        let raw = || egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(800., 600.),
            )),
            ..Default::default()
        };
        let mut clicked = false;
        // 首帧布局，第二帧把点击送到提示覆盖的按钮。
        for click in [false, true] {
            let mut input = raw();
            if click {
                input.events.push(egui::Event::PointerMoved(rect.center()));
                for pressed in [true, false] {
                    input.events.push(egui::Event::PointerButton {
                        pos: rect.center(),
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    });
                }
                input.events.push(key_event(egui::Key::Z, true, false));
            }
            let mut output = ctx.run_ui(input, |ui| {
                clicked |= ui.put(rect, egui::Button::new("下层按钮")).clicked();
                let ctx = ui.ctx();
                let before = ctx.input(|input| input.events.clone());
                let focus = ctx.memory(|memory| memory.focused());
                paint_key_hint(ctx, &hint, now, false);
                assert_eq!(ctx.input(|input| input.events.clone()), before);
                assert_eq!(ctx.memory(|memory| memory.focused()), focus);
            });
            // 无头测试不上传 GPU 字体图集，确认丢弃纹理增量。
            output.textures_delta.clear();
            assert!(output.shapes.iter().any(|shape| matches!(&shape.shape, egui::epaint::Shape::Text(text) if text.galley.text() == "Ctrl+Z")));
        }
        assert!(clicked, "提示所在位置的下层按钮必须仍收到点击");
        ctx.begin_pass(raw());
        paint_key_hint(&ctx, &hint, now + KEY_HINT_HOLD + KEY_HINT_FADE, false);
        let mut output = ctx.end_pass();
        output.textures_delta.clear();
        assert!(output.shapes.is_empty());
    }

    #[test]
    fn key_hint_below_help_avoids_minimum_window_sheet() {
        use bevy_egui::egui;
        let ctx = egui::Context::default();
        let now = Instant::now();
        let hint = KeyHint {
            text: "F4 · R".into(),
            applied: now,
        };
        ctx.begin_pass(egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(960., 640.),
            )),
            ..Default::default()
        });
        paint_key_hint(&ctx, &hint, now, true);
        let mut output = ctx.end_pass();
        output.textures_delta.clear();
        let rect = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::epaint::Shape::Rect(shape) => Some(shape.rect),
                _ => None,
            })
            .unwrap();
        assert!(rect.top() >= 580. && rect.bottom() <= 628., "{rect:?}");
    }

    #[test]
    fn cli_requires_positive_step_and_fresh_report() {
        let folder = tempfile::tempdir().unwrap();
        let args = vec![
            "editor".into(),
            "--demo-showcase".into(),
            folder.path().to_string_lossy().into_owned(),
            "--demo-step-ms".into(),
            "0".into(),
        ];
        assert!(Showcase::from_args(&args).is_err());
        let args = args[..3].to_vec();
        let showcase = Showcase::from_args(&args).unwrap().unwrap();
        assert_eq!(showcase.step, Duration::from_millis(450));
        std::fs::write(folder.path().join("demo-report.json"), "{}").unwrap();
        assert!(Showcase::from_args(&args).is_err());
    }
}
#[test]
fn replay_keeps_its_own_held_inputs_and_discards_native_clicks_without_repeating_edges() {
    let mut snapshot = ReplayInput::default();
    snapshot.mouse.press(MouseButton::Left);
    snapshot.keys.press(KeyCode::KeyR);
    let mut mouse = ButtonInput::default();
    mouse.press(MouseButton::Right);
    let mut keys = ButtonInput::default();
    keys.press(KeyCode::Escape);
    snapshot.restore(&mut mouse, &mut keys);
    assert!(mouse.pressed(MouseButton::Left));
    assert!(!mouse.pressed(MouseButton::Right));
    assert!(!mouse.just_pressed(MouseButton::Left));
    assert!(keys.pressed(KeyCode::KeyR));
    assert!(!keys.pressed(KeyCode::Escape));
    assert!(!keys.just_pressed(KeyCode::KeyR));
    snapshot.mouse.release(MouseButton::Left);
    snapshot.keys.release(KeyCode::KeyR);
    snapshot.restore(&mut mouse, &mut keys);
    assert!(!mouse.pressed(MouseButton::Left));
    assert!(!keys.pressed(KeyCode::KeyR));
    assert!(!mouse.just_released(MouseButton::Left));
}
