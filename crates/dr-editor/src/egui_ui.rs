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
            .init_resource::<topology_ui::ConnectionEditor>()
            .init_resource::<panels::egui_panel::UiState>()
            .add_systems(
                EguiPrimaryContextPass,
                (panels::egui_panel::draw, draw, topology_ui::draw).chain(),
            );
    }
}

/// 边栏按钮焦点不独占画布快捷键；只有属性草稿中的文本/模态独占输入。
/// 直接检查事务状态，避免依赖 PostUpdate 才更新的上一帧焦点。
pub fn canvas_input_available(
    inspector: Option<Res<properties::Inspector>>,
    topology: Option<Res<topology_ui::ConnectionEditor>>,
) -> bool {
    !inspector.is_some_and(|inspector| inspector.is_open())
        && !topology.is_some_and(|state| state.open)
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
) {
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
    if inspector.is_open() || topology_open {
        drag.cancel();
        cursor.cancel_placement();
        cursor.paste = None;
        camera_drag.0 = None;
    }
    // 沿用现有 Bevy 工具栏的命中结果，而不是覆盖它。
    pointer.blocked |= inspector.is_open() || topology_open;
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
