//! Bevy 与 egui 的最小适配层；文本编辑、剪贴板和 IME 均由 bevy_egui 处理。
//!
//! 集成时保留 properties::setup/actions 工具栏；在 Update 原 properties::input
//! 的位置调用 prepare_input，移除旧 properties::input/render 系统。
//! EguiPrimaryContextPass 由插件在 PostUpdate::EndPass 内执行，不应手动运行。

use super::*;
use bevy_egui::{EguiContexts, EguiPlugin, EguiPrimaryContextPass, egui};

/// 本次 egui pass 的逻辑坐标，供窗口自动化直接生成 egui 指针事件。
/// 若测试改为发送 Bevy 窗口事件，还需换算窗口缩放与 ctx.pixels_per_point()。
#[derive(Resource, Default)]
pub struct UiHits(pub Vec<(properties::Action, egui::Rect)>, pub bool);

pub struct EditorEguiPlugin;

impl Plugin for EditorEguiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(EguiPlugin::default())
            .init_resource::<UiHits>()
            .init_resource::<help::HelpState>()
            .init_resource::<files::PendingFileAction>()
            .init_resource::<topology_ui::ConnectionEditor>()
            .init_resource::<panels::egui_panel::UiState>()
            .add_systems(
                EguiPrimaryContextPass,
                (
                    panels::egui_panel::draw,
                    draw,
                    topology_ui::draw,
                    connection_lines::ui,
                    help::draw,
                )
                    .chain(),
            );
    }
}

/// 边栏按钮焦点不独占画布快捷键；只有属性草稿中的文本/模态独占输入。
/// 直接检查事务状态，避免依赖 PostUpdate 才更新的上一帧焦点。
pub fn canvas_input_available(
    inspector: Option<Res<properties::Inspector>>,
    topology: Option<Res<topology_ui::ConnectionEditor>>,
    pending: Option<Res<files::PendingFileAction>>,
    help: Option<Res<help::HelpState>>,
    lines: Option<Res<connection_lines::Settings>>,
) -> bool {
    !inspector.is_some_and(|inspector| inspector.is_open())
        && !topology.is_some_and(|state| state.open)
        && !pending.is_some_and(|state| state.is_blocked())
        && !help.is_some_and(|state| state.open || state.suppress_frame)
        && !lines.is_some_and(|state| state.open)
}

/// 放在 pointer_over_ui、properties::actions 之后，画布输入系统之前。
/// 这里只管理编辑器模式及 F2，不消费/复制 bevy_egui 的任何文字或输入法事件。
#[allow(clippy::too_many_arguments)]
pub fn prepare_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut inspector: ResMut<properties::Inspector>,
    mut topology: Option<ResMut<topology_ui::ConnectionEditor>>,
    document: Res<EditorDocument>,
    mut pointer: ResMut<panels::UiPointer>,
    mut drag: ResMut<DragState>,
    mut cursor: ResMut<EditorCursor>,
    mut camera_drag: ResMut<CameraDrag>,
    pending: Option<Res<files::PendingFileAction>>,
    help: Option<Res<help::HelpState>>,
    lines: Option<ResMut<connection_lines::Settings>>,
) {
    if help.is_some_and(|state| state.open || state.suppress_frame) {
        return;
    }
    let mut line_open = false;
    if let Some(mut lines) = lines {
        if lines.open && keys.just_pressed(KeyCode::Escape) {
            lines.open = false;
        }
        line_open = lines.open;
    }
    if pending.is_some_and(|state| state.is_blocked()) {
        cursor.manual_connection = None;
        drag.cancel();
        cursor.cancel_placement();
        cursor.paste = None;
        camera_drag.0 = None;
        pointer.blocked = true;
        return;
    }
    if let Some(state) = topology.as_mut() {
        if !inspector.is_open() && keys.just_pressed(KeyCode::F6) {
            state.open = !state.open;
        }
        if state.open && keys.just_pressed(KeyCode::Escape) {
            state.open = false;
        }
        if inspector.is_open() {
            state.open = false;
        }
    }
    let topology_open = topology.as_ref().is_some_and(|state| state.open);
    if !inspector.is_open() && !topology_open && keys.just_pressed(KeyCode::F2) {
        properties::open(&mut inspector, &document);
    }
    if inspector.is_open() || topology_open || line_open {
        cursor.manual_connection = None;
        drag.cancel();
        cursor.cancel_placement();
        cursor.paste = None;
        camera_drag.0 = None;
    }
    // 沿用现有 Bevy 工具栏的命中结果，而不是覆盖它。
    pointer.blocked |= inspector.is_open() || topology_open || line_open;
}

fn configure_chinese_font(ctx: &egui::Context, paths: &EditorPaths) {
    let path = std::path::Path::new(&paths.assets)
        .join("fonts/HarmonyOS_Sans/HarmonyOS_Sans_SC/HarmonyOS_Sans_SC_Regular.ttf");
    match std::fs::read(&path) {
        Ok(bytes) => {
            let mut fonts = egui::FontDefinitions::default();
            let name = "HarmonyOS Sans SC".to_owned();
            fonts
                .font_data
                .insert(name.clone(), egui::FontData::from_owned(bytes).into());
            // 保留 egui 默认字形及等宽字体；只补入中文字体，不调整全局视觉主题。
            fonts
                .families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .insert(0, name.clone());
            fonts
                .families
                .entry(egui::FontFamily::Monospace)
                .or_default()
                .push(name);
            ctx.set_fonts(fonts);
        }
        Err(error) => warn!("无法读取 egui 中文字体 {}：{error}", path.display()),
    }
}

pub fn draw(
    mut contexts: EguiContexts,
    paths: Res<EditorPaths>,
    mut inspector: ResMut<properties::Inspector>,
    mut document: ResMut<EditorDocument>,
    mut hits: ResMut<UiHits>,
    mut font_configured: Local<bool>,
    mut pending: ResMut<files::PendingFileAction>,
) {
    hits.0.clear();
    hits.1 = false;
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };
    if !*font_configured {
        configure_chinese_font(ctx, &paths);
        *font_configured = true;
    }
    if pending.is_blocked() {
        files::show_confirmation(ctx, &mut pending, &paths);
        return;
    }
    if !inspector.is_open() {
        return;
    }
    hits.1 = ctx.input(|input| input.is_scrolling());
    if let Some(action) = properties::egui_panel::show(ctx, &mut inspector, &document, &mut hits.0)
    {
        properties::act(&action, &mut inspector, &mut document);
    }
}

/// 窗口自测共享驱动：通过真实 egui 指针/滚轮事件操作可见控件。
#[derive(Default)]
pub struct UiTestInput {
    release: Option<egui::Pos2>,
    settle: u8,
}

impl UiTestInput {
    pub fn tick(&mut self, input: &mut bevy_egui::EguiInput) -> bool {
        input.0.focused = true;
        if let Some(pos) = self.release.take() {
            input.0.events.extend([
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                },
            ]);
            self.settle = 1;
            return true;
        }
        if self.settle > 0 {
            self.settle -= 1;
            return true;
        }
        false
    }

    pub fn click_rect(&mut self, rect: egui::Rect, input: &mut bevy_egui::EguiInput) {
        let pos = rect.center();
        input.0.events.extend([
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ]);
        self.release = Some(pos);
    }

    pub fn click(
        &mut self,
        action: properties::Action,
        hits: &UiHits,
        input: &mut bevy_egui::EguiInput,
    ) -> bool {
        // 滚轮有平滑惯性；等正文稳定后再按下/释放，避免控件移出点击位置。
        if hits.1 {
            return false;
        }
        if let Some((_, rect)) = hits.0.iter().find(|(candidate, _)| *candidate == action) {
            let pos = rect.center();
            input.0.events.extend([
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ]);
            self.release = Some(pos);
            true
        } else {
            // 应用/取消固定在正文外，取其上方作为模态内的滚动位置。
            let Some((_, rect)) = hits
                .0
                .iter()
                .find(|(action, _)| *action == properties::Action::Apply)
            else {
                // Modal 首次测量/字体加载的 pass 还没有可交互控件。
                return false;
            };
            let pos = egui::pos2(rect.center().x, rect.top() - 80.0);
            let up = matches!(
                action,
                properties::Action::Focus(properties::Field::Name) | properties::Action::OpenRepair
            );
            input.0.events.extend([
                egui::Event::PointerMoved(pos),
                egui::Event::MouseWheel {
                    phase: egui::TouchPhase::Move,
                    unit: egui::MouseWheelUnit::Point,
                    delta: egui::vec2(0.0, if up { 350.0 } else { -350.0 }),
                    modifiers: egui::Modifiers::NONE,
                },
            ]);
            self.settle = 2;
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{UiTestInput, egui};
    use bevy::prelude::*;
    use bevy_egui::EguiInput;

    /// 显式推进 egui 的秒级时间，不 sleep，也不启动窗口或渲染插件。
    struct ButtonFrames {
        ctx: egui::Context,
        rect: egui::Rect,
    }

    impl ButtonFrames {
        fn new() -> Self {
            let mut frames = Self {
                ctx: egui::Context::default(),
                rect: egui::Rect::NOTHING,
            };
            for time in [0.0, 0.016, 0.032] {
                assert!(!frames.frame(time, vec![]));
            }
            frames
        }

        fn frame(&mut self, time: f64, events: Vec<egui::Event>) -> bool {
            self.ctx.begin_pass(egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(640.0, 480.0),
                )),
                time: Some(time),
                focused: true,
                events,
                ..Default::default()
            });
            let mut clicked = false;
            egui::Area::new(egui::Id::new("headless_undo_button"))
                .fixed_pos(egui::pos2(32.0, 32.0))
                .movable(false)
                .show(&self.ctx, |ui| {
                    // 固定绘制顺序、文案和布局，排除控件身份漂移这一独立变量。
                    let response = ui.button("Undo");
                    self.rect = response.rect;
                    clicked = response.clicked();
                });
            self.ctx.end_pass().textures_delta.clear();
            clicked
        }

        fn press(&mut self, driver: &mut UiTestInput) {
            let mut input = EguiInput::default();
            driver.click_rect(self.rect, &mut input);
            assert!(!self.frame(0.1, input.0.events));
        }

        fn release(&mut self, time: f64, driver: &mut UiTestInput) -> bool {
            let mut input = EguiInput::default();
            assert!(driver.tick(&mut input));
            self.frame(time, input.0.events)
        }
    }

    #[test]
    fn clean_450ms_hold_across_empty_frames_still_clicks() {
        let mut frames = ButtonFrames::new();
        let mut driver = UiTestInput::default();
        frames.press(&mut driver);
        // 真实界面在 smoke 的两个步骤之间仍持续执行 pass。
        for frame in 1..27 {
            assert!(!frames.frame(0.1 + f64::from(frame) / 60.0, vec![]));
        }
        assert!(
            frames.release(0.55, &mut driver),
            "干净的 450ms 按住低于 egui 默认 800ms 阈值，不应自行丢失点击"
        );
    }

    #[test]
    fn pointer_gone_in_an_intermediate_pass_cancels_click() {
        let mut frames = ButtonFrames::new();
        let mut driver = UiTestInput::default();
        frames.press(&mut driver);
        assert!(!frames.frame(0.116, vec![egui::Event::PointerGone]));
        assert!(!frames.frame(0.132, vec![]));
        assert!(
            !frames.release(0.55, &mut driver),
            "后续回到原点并释放不能恢复被 PointerGone 清掉的点击归属"
        );
    }

    #[test]
    fn distant_move_then_virtual_move_back_still_cancels_short_click() {
        let mut frames = ButtonFrames::new();
        let mut driver = UiTestInput::default();
        frames.press(&mut driver);
        let pos = frames.rect.center();
        assert!(!frames.frame(
            0.115,
            vec![
                egui::Event::PointerMoved(pos + egui::vec2(80.0, 0.0)),
                egui::Event::PointerMoved(pos),
            ],
        ));
        assert!(!frames.frame(0.13, vec![]));
        assert!(
            !frames.release(0.145, &mut driver),
            "即使只按住 45ms，末尾补虚拟坐标也不能撤销曾经超过点击距离的记录"
        );
    }

    #[test]
    fn demo_filter_then_virtual_move_and_45ms_release_clicks() {
        // 调用生产 restore_pointer，而不是在测试里复制过滤条件。
        let output = tempfile::tempdir().unwrap();
        let args = vec![
            "--demo-showcase".to_owned(),
            output.path().to_string_lossy().into_owned(),
        ];
        let showcase = crate::demo::Showcase::from_args(&args).unwrap().unwrap();
        let mut app = App::new();
        app.insert_resource(showcase)
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<ButtonInput<KeyCode>>()
            .add_systems(Update, crate::demo::restore_pointer);
        let entity = app.world_mut().spawn(EguiInput::default()).id();

        let mut frames = ButtonFrames::new();
        let mut driver = UiTestInput::default();
        frames.press(&mut driver);
        let pos = frames.rect.center();
        for time in [0.115, 0.13] {
            app.world_mut()
                .get_mut::<EguiInput>(entity)
                .unwrap()
                .0
                .events = vec![
                egui::Event::PointerMoved(pos + egui::vec2(80.0, 0.0)),
                egui::Event::PointerGone,
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                },
                egui::Event::Text("保留非指针事件".to_owned()),
            ];
            app.update();
            let mut input = app.world_mut().get_mut::<EguiInput>(entity).unwrap();
            assert!(
                input.0.events.is_empty(),
                "原生文本和误点都不能混入内部回放"
            );
            let mut events = std::mem::take(&mut input.0.events);
            // 与 demo 的执行顺序一致：先隔离原生输入，再补当帧虚拟位置。
            events.push(egui::Event::PointerMoved(pos));
            assert!(!frames.frame(time, events));
        }
        assert!(
            frames.release(0.145, &mut driver),
            "隔离原生指针后持续补虚拟 Move，45ms 短点击应成功"
        );
    }
}
