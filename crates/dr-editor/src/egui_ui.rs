//! Bevy 与 egui 的适配层：共享样式、贴图、输入隔离及属性界面接线。
//! 文本编辑、剪贴板和 IME 由 bevy_egui 处理；原生工具栏沿用 properties::setup/actions。
//! EguiPrimaryContextPass 由插件在 PostUpdate::EndPass 内执行，不应手动运行。

use super::*;
use bevy_egui::{EguiContexts, EguiPlugin, EguiPrimaryContextPass, egui};

/// 本次 egui pass 的逻辑坐标，供窗口自动化直接生成 egui 指针事件。
/// 若测试改为发送 Bevy 窗口事件，还需换算窗口缩放与 ctx.pixels_per_point()。
#[derive(Resource, Default)]
pub struct UiHits {
    pub actions: Vec<(properties::Action, egui::Rect)>,
    pub scrolling: bool,
    pub dock: Option<properties::egui_panel::Dock>,
    pub targets: Vec<(PartKey, egui::Rect)>,
    pub body: Option<egui::Rect>,
    /// 实际 Inspector 展开状态的本帧快照；自动化只通过真实切换按钮导航。
    pub advanced_open: bool,
    pub targets_open: bool,
    pub selected_activation: Option<(usize, usize)>,
}

impl UiHits {
    fn sync_navigation(&mut self, inspector: &properties::Inspector) {
        let open = inspector.is_open();
        self.advanced_open = open && (!inspector.docked() || inspector.advanced_open);
        self.targets_open = open && (!inspector.docked() || inspector.targets_open);
        self.selected_activation = if open {
            match inspector.preview {
                properties::Preview::Activation(stage, index) => Some((stage, index)),
                _ => None,
            }
        } else {
            None
        };
    }
}

/// 目录与分级动作共用 Bevy 贴图注册；尺寸每帧读取，避免缓存未加载时的回退值。
#[derive(Resource, Default)]
pub(crate) struct PartImages(std::collections::HashMap<String, egui::TextureId>);

impl PartImages {
    pub(crate) fn get(
        &mut self,
        contexts: &mut EguiContexts,
        assets: &AssetServer,
        images: &Assets<Image>,
        kind: &dr_core::PartType,
    ) -> (egui::TextureId, Vec2) {
        let texture = *self.0.entry(kind.sprite.clone()).or_insert_with(|| {
            contexts.add_image(bevy_egui::EguiTextureHandle::Strong(
                if kind.sprite.is_empty() {
                    Handle::<Image>::default()
                } else {
                    assets.load(format!("textures/parts/{}", kind.sprite))
                },
            ))
        });
        let size = render::image_size(&kind.sprite, assets, images)
            .unwrap_or_else(|| render::fallback_size(Some(kind)));
        (texture, size)
    }
}

/// 两个部件列表使用同一等比例缩略图绘制，图片不会拉伸或挤占文字列。
pub(crate) fn paint_part_image(
    ui: &egui::Ui,
    (texture, size): (egui::TextureId, Vec2),
    bounds: egui::Rect,
) {
    let size = egui::vec2(size.x, size.y);
    let scale = (bounds.width() / size.x.max(1.0)).min(bounds.height() / size.y.max(1.0));
    let rect = egui::Rect::from_center_size(bounds.center(), size * scale);
    egui::Image::new((texture, size))
        .maintain_aspect_ratio(true)
        .paint_at(ui, rect);
}

pub struct EditorEguiPlugin;

impl Plugin for EditorEguiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(EguiPlugin::default())
            .init_resource::<UiHits>()
            .init_resource::<PartImages>()
            .init_resource::<help::HelpState>()
            .init_resource::<files::PendingFileAction>()
            .init_resource::<files::FileMenu>()
            .init_resource::<topology_ui::ConnectionEditor>()
            .init_resource::<panels::egui_panel::UiState>()
            .add_systems(
                EguiPrimaryContextPass,
                (
                    configure,
                    panels::egui_panel::draw,
                    draw_native_toolbar,
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
    menu: Option<Res<files::FileMenu>>,
) -> bool {
    !inspector.is_some_and(|inspector| inspector.is_open())
        && !topology.is_some_and(|state| state.open)
        && !pending.is_some_and(|state| state.is_blocked())
        && !help.is_some_and(|state| state.open || state.suppress_frame)
        && !lines.is_some_and(|state| state.open)
        && !menu.is_some_and(|state| state.open)
}

/// 停靠分级打开/改尺寸时仅放行既有适配请求，仍不放行画布编辑快捷键。
pub fn canvas_or_fit_available(
    inspector: Option<Res<properties::Inspector>>,
    topology: Option<Res<topology_ui::ConnectionEditor>>,
    pending: Option<Res<files::PendingFileAction>>,
    help: Option<Res<help::HelpState>>,
    lines: Option<Res<connection_lines::Settings>>,
    ui: Option<Res<panels::egui_panel::UiState>>,
    menu: Option<Res<files::FileMenu>>,
) -> bool {
    let fit = inspector.as_ref().is_some_and(|state| state.docked())
        && ui.is_some_and(|state| state.fit_requested);
    canvas_input_available(
        if fit { None } else { inspector },
        topology,
        pending,
        help,
        lines,
        menu,
    )
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
    mut touch: Option<ResMut<mobile_input::GestureInput>>,
    menu: Option<Res<files::FileMenu>>,
) {
    if help.is_some_and(|state| state.open || state.suppress_frame) {
        if let Some(touch) = touch.as_mut() {
            touch.navigation = None;
        }
        canvas_input::cancel_preview(&mut drag, &mut cursor, &mut camera_drag);
        pointer.blocked = true;
        return;
    }
    let mut line_open = false;
    if let Some(mut lines) = lines {
        if lines.open && keys.just_pressed(KeyCode::Escape) {
            lines.open = false;
        }
        line_open = lines.open;
    }
    if pending.is_some_and(|state| state.is_blocked()) || menu.is_some_and(|state| state.open) {
        if let Some(touch) = touch.as_mut() {
            touch.navigation = None;
        }
        canvas_input::cancel_preview(&mut drag, &mut cursor, &mut camera_drag);
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
        if let Some(touch) = touch.as_mut() {
            touch.navigation = None;
        }
        canvas_input::cancel_preview(&mut drag, &mut cursor, &mut camera_drag);
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
            // 保留 egui 默认字形及等宽字体；原生顶栏也使用同一份中文字体。
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

pub(crate) fn ui_color(color: Color) -> egui::Color32 {
    let [r, g, b, a] = color.to_srgba().to_u8_array();
    egui::Color32::from_rgba_unmultiplied(r, g, b, a)
}

fn configure_theme(ctx: &egui::Context) {
    use crate::native_buttons::{
        ACCENT, BORDER, BUTTON, BUTTON_FONT_SIZE, HOVERED, PANEL, PRESSED, RADIUS, SELECTED, TEXT,
    };
    // 原生顶栏始终是深色，egui 不随系统主题变成另一套界面。
    ctx.set_theme(egui::Theme::Dark);
    ctx.style_mut_of(egui::Theme::Dark, |style| {
        for (kind, size) in [
            (egui::TextStyle::Body, 15.0),
            (egui::TextStyle::Button, BUTTON_FONT_SIZE),
            (egui::TextStyle::Heading, 20.0),
            (egui::TextStyle::Small, 12.0),
        ] {
            style
                .text_styles
                .insert(kind, egui::FontId::proportional(size));
        }
        style.spacing.item_spacing = egui::vec2(8.0, 6.0);
        style.spacing.button_padding = egui::vec2(10.0, 5.0);
        style.spacing.interact_size = egui::vec2(40.0, 28.0);
        style.spacing.window_margin = egui::Margin::same(12);
        let visuals = &mut style.visuals;
        visuals.panel_fill = ui_color(PANEL);
        visuals.window_fill = ui_color(PANEL);
        visuals.extreme_bg_color = ui_color(Color::srgb(0.08, 0.08, 0.08));
        visuals.faint_bg_color = ui_color(BUTTON);
        visuals.window_stroke = egui::Stroke::new(1.0, ui_color(BORDER));
        visuals.window_corner_radius = egui::CornerRadius::same(RADIUS);
        visuals.menu_corner_radius = egui::CornerRadius::same(RADIUS);
        visuals.weak_text_color = Some(ui_color(Color::srgb(0.62, 0.62, 0.62)));
        visuals.selection.bg_fill = ui_color(SELECTED);
        visuals.selection.stroke = egui::Stroke::new(1.0, ui_color(ACCENT));
        visuals.hyperlink_color = ui_color(ACCENT);
        for (widget, background) in [
            (&mut visuals.widgets.noninteractive, PANEL),
            (&mut visuals.widgets.inactive, BUTTON),
            (&mut visuals.widgets.hovered, HOVERED),
            (&mut visuals.widgets.active, PRESSED),
            (&mut visuals.widgets.open, SELECTED),
        ] {
            widget.bg_fill = ui_color(background);
            widget.weak_bg_fill = ui_color(background);
            widget.bg_stroke = egui::Stroke::new(
                1.0,
                ui_color(
                    if background == HOVERED || background == PRESSED || background == SELECTED {
                        ACCENT
                    } else {
                        BORDER
                    },
                ),
            );
            widget.fg_stroke = egui::Stroke::new(1.0, ui_color(TEXT));
            widget.corner_radius = egui::CornerRadius::same(RADIUS);
            widget.expansion = 0.0;
        }
    });
}

#[cfg(test)]
pub(crate) fn test_context() -> egui::Context {
    let ctx = egui::Context::default();
    configure_theme(&ctx);
    ctx
}

fn configure(mut contexts: EguiContexts, paths: Res<EditorPaths>, mut configured: Local<bool>) {
    if *configured {
        return;
    }
    let Ok(ctx) = contexts.ctx_mut() else { return };
    configure_theme(ctx);
    configure_chinese_font(ctx, &paths);
    *configured = true;
}

/// ComputedNode 与 UiGlobalTransform 已在物理像素空间；只换算 egui 的 DPI 一次。
fn native_button_rect(
    node: &ComputedNode,
    transform: &UiGlobalTransform,
    pixels_per_point: f32,
) -> Option<egui::Rect> {
    if !pixels_per_point.is_finite() || pixels_per_point <= 0.0 {
        return None;
    }
    let half = node.size() * 0.5;
    let mut min = Vec2::INFINITY;
    let mut max = Vec2::NEG_INFINITY;
    for corner in [
        -half,
        Vec2::new(half.x, -half.y),
        half,
        Vec2::new(-half.x, half.y),
    ] {
        let point = transform.transform_point2(corner) / pixels_per_point;
        if !point.is_finite() {
            return None;
        }
        min = min.min(point);
        max = max.max(point);
    }
    let rect = egui::Rect::from_min_max(egui::pos2(min.x, min.y), egui::pos2(max.x, max.y));
    rect.is_positive().then_some(rect)
}

/// 纯 Painter 不创建 Area / Response / Sense，原生 Button 继续拥有命中与焦点。
fn paint_native_toolbar(
    ctx: &egui::Context,
    buttons: impl IntoIterator<Item = (panels::ToolbarItem, egui::Rect)>,
) {
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("native_toolbar_icons"),
    ));
    for (item, rect) in buttons {
        let icon = match item {
            panels::ToolbarItem::Files => icons::Icon::Menu,
            panels::ToolbarItem::Properties => icons::Icon::Staging,
            panels::ToolbarItem::Help => icons::Icon::Help,
            panels::ToolbarItem::Status => continue,
        };
        icons::paint(
            &painter,
            rect.shrink(10.0),
            icon,
            ui_color(native_buttons::ACCENT),
        );
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn draw_native_toolbar(
    mut contexts: EguiContexts,
    buttons: Query<(&panels::ToolbarItem, &ComputedNode, &UiGlobalTransform), With<Button>>,
    mut state: ResMut<panels::egui_panel::UiState>,
    mut menu: ResMut<files::FileMenu>,
    inspector: Res<properties::Inspector>,
    pending: Res<files::PendingFileAction>,
    help: Res<help::HelpState>,
    topology: Res<topology_ui::ConnectionEditor>,
    lines: Option<Res<connection_lines::Settings>>,
    mut actions: MessageWriter<files::FileAction>,
) {
    let Ok(ctx) = contexts.ctx_mut() else { return };
    paint_native_toolbar(
        ctx,
        buttons.iter().filter_map(|(item, node, transform)| {
            native_button_rect(node, transform, ctx.pixels_per_point()).map(|rect| (*item, rect))
        }),
    );
    let blocked = state.drawer_open()
        || inspector.is_open()
        || pending.is_blocked()
        || help.open
        || help.suppress_frame
        || topology.open
        || lines.is_some_and(|state| state.open);
    if let Some(action) = files::show_menu(ctx, &mut menu, blocked) {
        actions.write(action);
    }
    if let Some(area) = menu.area {
        state.areas.push(area);
    }
}

type PreviewResources<'w, 's> = (
    Res<'w, AssetServer>,
    Res<'w, Assets<Image>>,
    Query<'w, 's, &'static Window, With<bevy::window::PrimaryWindow>>,
    Query<'w, 's, (&'static Transform, &'static Projection), With<Camera2d>>,
    ResMut<'w, PartImages>,
);

pub fn draw(
    mut contexts: EguiContexts,
    (paths, mut pending): (Res<EditorPaths>, ResMut<files::PendingFileAction>),
    mut inspector: ResMut<properties::Inspector>,
    mut document: ResMut<EditorDocument>,
    mut hits: ResMut<UiHits>,
    (mut state, mut last_canvas): (
        ResMut<panels::egui_panel::UiState>,
        Local<Option<egui::Rect>>,
    ),
    (assets, images, windows, cameras, mut part_images): PreviewResources,
) {
    hits.actions.clear();
    hits.dock = None;
    hits.targets.clear();
    hits.body = None;
    hits.scrolling = false;
    hits.sync_navigation(&inspector);
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };
    let ctx = ctx.clone();
    if pending.is_blocked() {
        files::show_confirmation(&ctx, &mut pending, &paths);
        return;
    }
    if !inspector.is_open() {
        *last_canvas = None;
        return;
    }
    hits.scrolling = ctx.input(|input| input.is_scrolling());
    let available = state.canvas_area.unwrap_or_else(|| ctx.content_rect());
    if let Some(shown) = properties::egui_panel::show(
        &ctx,
        &mut inspector,
        &document,
        &mut hits.actions,
        available,
        &mut |kind| part_images.get(&mut contexts, &assets, &images, kind),
    ) {
        hits.body = Some(shown.body);
        if let Some(action) = shown.action {
            properties::act(&action, &mut inspector, &mut document);
        }
        if let Some(dock) = shown.dock {
            hits.dock = Some(dock);
            state.areas.push(dock.panel);
            state.canvas_area = Some(dock.canvas);
            if *last_canvas != Some(dock.canvas) {
                state.fit_requested = true;
                *last_canvas = Some(dock.canvas);
            }
            if let (Ok(window), Ok((camera, projection))) = (windows.single(), cameras.single()) {
                hits.targets = properties::egui_panel::paint_targets(
                    &ctx,
                    &inspector,
                    &document,
                    dock.canvas,
                    (window, camera, projection),
                    |texture| render::image_size(texture, &assets, &images),
                );
            }
        } else {
            *last_canvas = None;
        }
    }
    hits.sync_navigation(&inspector);
}

/// 窗口自测共享驱动：通过真实 egui 指针/滚轮事件操作可见控件。
#[derive(Default)]
pub struct UiTestInput {
    settle: u8,
}

impl UiTestInput {
    pub fn tick(&mut self, input: &mut bevy_egui::EguiInput) -> bool {
        input.0.focused = true;
        if self.settle > 0 {
            self.settle -= 1;
            return true;
        }
        false
    }

    pub fn click_rect(&mut self, rect: egui::Rect, input: &mut bevy_egui::EguiInput) {
        // 一次短点击的两条边沿成对交付，如同系统在一帧间收到完整短按。
        // 不让观看停顿或慢帧把自测短点击变成长按；长按/拖动另用真实分帧输入。
        let pos = rect.center();
        input.0.events.extend([
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            },
        ]);
        self.settle = 2;
    }

    pub fn click(
        &mut self,
        action: properties::Action,
        hits: &UiHits,
        input: &mut bevy_egui::EguiInput,
    ) -> bool {
        use properties::{Action, Field};
        // 滚轮有平滑惯性；等正文稳定后再按下/释放，避免控件移出点击位置。
        if hits.scrolling {
            return false;
        }
        let expand = match &action {
            Action::Focus(Field::Name | Field::Fuel | Field::Throttle | Field::CurrentStage)
            | Action::Active
            | Action::OpenRepair
                if !hits.advanced_open =>
            {
                Some(Action::ToggleAdvanced)
            }
            Action::Focus(Field::Target) | Action::CycleTarget(_) | Action::AddActivation(_)
                if !hits.targets_open =>
            {
                Some(Action::ToggleTargets)
            }
            _ => None,
        };
        if let Some(expand) = expand {
            if let Some((_, rect)) = hits
                .actions
                .iter()
                .find(|(candidate, _)| candidate == &expand)
            {
                self.click_rect(*rect, input);
            } else {
                self.scroll(true, hits, input);
            }
            // 导航点击不是调用方请求的编辑动作；等待下一帧真实 Inspector 快照。
            return false;
        }
        if let Some((_, rect)) = hits
            .actions
            .iter()
            .find(|(candidate, _)| candidate == &action)
        {
            self.click_rect(*rect, input);
            return true;
        }
        if let Action::Moved(stage, index) | Action::RemoveActivation(stage, index) = &action
            && hits.selected_activation != Some((*stage, *index))
            && let Some((_, rect)) = hits
                .actions
                .iter()
                .find(|(candidate, _)| candidate == &Action::SelectActivation(*stage, *index))
        {
            self.click_rect(*rect, input);
            return false;
        }
        // 上下文在整组缩略图之后；已选中时只向正文导航，不反复点击缩略图。
        self.scroll(Self::scroll_up(&action, hits), hits, input);
        false
    }

    fn scroll_up(action: &properties::Action, hits: &UiHits) -> bool {
        use properties::{Action, Field};
        let field_order = |action: &Action| match action {
            Action::OpenRepair => Some(0),
            Action::Active => Some(1),
            Action::Focus(Field::Fuel) => Some(2),
            Action::Focus(Field::Name) => Some(3),
            Action::Focus(Field::Throttle) => Some(4),
            Action::Focus(Field::CurrentStage) => Some(5),
            Action::Focus(Field::Target) => Some(6),
            Action::CycleTarget(_) => Some(7),
            _ => None,
        };
        if let Some(target) = field_order(action) {
            return hits
                .actions
                .iter()
                .filter_map(|(visible, _)| field_order(visible))
                .min()
                .is_none_or(|visible| target < visible);
        }
        let stage = |action: &Action| match action {
            Action::SelectStage(stage)
            | Action::AddActivation(stage)
            | Action::RemoveStep(stage)
            | Action::MoveStep(stage, _)
            | Action::SelectActivation(stage, _)
            | Action::Moved(stage, _)
            | Action::RemoveActivation(stage, _) => Some(*stage),
            _ => None,
        };
        if let Some(target) = stage(action) {
            if hits
                .actions
                .iter()
                .filter_map(|(visible, _)| stage(visible))
                .min()
                .is_some_and(|visible| target < visible)
            {
                return true;
            }
            if let Action::Moved(stage, index) | Action::RemoveActivation(stage, index) = action
                && hits.selected_activation != Some((*stage, *index))
            {
                return hits
                    .actions
                    .iter()
                    .filter_map(|(visible, _)| match visible {
                        Action::SelectActivation(visible_stage, visible_index)
                            if visible_stage == stage =>
                        {
                            Some(*visible_index)
                        }
                        _ => None,
                    })
                    .min()
                    .is_some_and(|visible| *index < visible);
            }
        }
        false
    }

    fn scroll(&mut self, up: bool, hits: &UiHits, input: &mut bevy_egui::EguiInput) {
        // 使用本帧真实正文区；短横屏不能按应用按钮向上偏移固定距离猜位置。
        let Some(body) = hits.body.filter(|rect| rect.is_positive()) else {
            return;
        };
        let pos = body.center();
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
    }
}

#[cfg(test)]
mod tests {
    use super::{UiTestInput, configure_theme, egui, test_context, ui_color};
    use crate::native_buttons::{self, *};
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
                ctx: crate::egui_ui::test_context(),
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
    }

    #[test]
    fn shared_short_click_packet_survives_slow_frames_and_is_not_repeated() {
        for gap in [0.016, 1.0, 10.0] {
            let mut frames = ButtonFrames::new();
            let mut driver = UiTestInput::default();
            let mut input = EguiInput::default();
            driver.click_rect(frames.rect, &mut input);
            assert!(
                frames.frame(0.1 + gap, std::mem::take(&mut input.0.events)),
                "短点击应以完整事件对交给同一次 pass，不能被 {gap}s 的帧间隔变成长按"
            );
            for time in [0.2 + gap, 1.2 + gap] {
                assert!(driver.tick(&mut input));
                assert!(input.0.events.is_empty(), "等待帧不能重复注入释放");
                assert!(!frames.frame(time, std::mem::take(&mut input.0.events)));
            }
            assert!(!driver.tick(&mut input));
        }
    }

    #[test]
    fn scrolling_uses_the_actual_short_body_instead_of_a_fixed_footer_offset() {
        let body = egui::Rect::from_min_max(egui::pos2(8.0, 242.0), egui::pos2(310.0, 292.0));
        let hits = super::UiHits {
            body: Some(body),
            ..default()
        };
        let mut input = EguiInput::default();
        assert!(!UiTestInput::default().click(
            crate::properties::Action::Moved(63, 15),
            &hits,
            &mut input
        ));
        assert!(
            matches!(input.0.events.first(), Some(egui::Event::PointerMoved(pos)) if *pos == body.center())
        );
        assert!(
            input.0.events.iter().any(
                |event| matches!(event, egui::Event::MouseWheel { delta, .. } if delta.y < 0.0)
            )
        );
    }

    #[test]
    fn native_icons_follow_computed_buttons_with_one_dpi_conversion() {
        for scale in [1.0, 1.5, 2.0] {
            for left in [0.0, 48.0, 96.0] {
                let node = ComputedNode {
                    size: Vec2::splat(48.0 * scale),
                    ..default()
                };
                let transform =
                    UiGlobalTransform::from_xy((left + 24.0) * scale, (900.0 - 28.0) * scale);
                assert_eq!(
                    super::native_button_rect(&node, &transform, scale),
                    Some(egui::Rect::from_min_size(
                        egui::pos2(left, 848.0),
                        egui::vec2(48.0, 48.0),
                    ))
                );
                assert!(super::native_button_rect(&node, &transform, 0.0).is_none());
            }
        }
        assert!(super::native_button_rect(
            &ComputedNode::default(),
            &UiGlobalTransform::default(),
            1.0,
        ).is_none());
    }

    fn native_toolbar_rects() -> [(crate::panels::ToolbarItem, egui::Rect); 3] {
        use crate::panels::ToolbarItem;
        [
            (ToolbarItem::Files, 0.0),
            (ToolbarItem::Properties, 48.0),
            (ToolbarItem::Help, 96.0),
        ]
        .map(|(item, left)| {
            (
                item,
                egui::Rect::from_min_size(egui::pos2(left, 428.0), egui::vec2(48.0, 48.0)),
            )
        })
    }

    fn native_icon_frame(
        ctx: &egui::Context,
        paint: bool,
        time: f64,
        events: Vec<egui::Event>,
    ) -> egui::FullOutput {
        ctx.begin_pass(egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(640.0, 480.0),
            )),
            focused: true,
            time: Some(time),
            events,
            ..default()
        });
        if paint {
            super::paint_native_toolbar(ctx, native_toolbar_rects());
        }
        ctx.end_pass()
    }

    #[test]
    fn native_icon_painter_draws_without_owning_pointer_input() {
        for (_, rect) in native_toolbar_rects() {
            let baseline = test_context();
            let painted = test_context();
            // begin_pass 自动注册全屏 Background；没有 run_ui 根区时，空基线也 wants。
            // 对比独立但同输入的上下文，并跨帧检查命中，捕获下一帧才生效的 widget 注册。
            for (frame, pressed) in [None, Some(true), None, Some(false), None]
                .into_iter()
                .enumerate()
            {
                let mut events = vec![egui::Event::PointerMoved(rect.center())];
                if let Some(pressed) = pressed {
                    events.push(egui::Event::PointerButton {
                        pos: rect.center(),
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    });
                }
                let time = frame as f64 * 0.016;
                let plain_output = native_icon_frame(&baseline, false, time, events.clone());
                let painted_output = native_icon_frame(&painted, true, time, events);
                assert!(painted_output.shapes.len() > plain_output.shapes.len());
                assert_eq!(
                    painted.egui_wants_pointer_input(),
                    baseline.egui_wants_pointer_input(),
                    "叠绘不能增加背景基线之外的指针需求"
                );
                if frame == 0 {
                    assert!(
                        baseline.egui_wants_pointer_input(),
                        "核实旧式 pass 的背景基线"
                    );
                }
                for ctx in [&baseline, &painted] {
                    assert_eq!(
                        ctx.layer_id_at(rect.center()),
                        Some(egui::LayerId::background())
                    );
                    assert!(!ctx.egui_is_using_pointer(), "叠绘不能捕获按下或拖动");
                    ctx.interaction_snapshot(|snapshot| {
                        assert!(snapshot.hovered.is_empty());
                        assert!(snapshot.contains_pointer.is_empty());
                        assert_eq!(snapshot.clicked, None);
                        assert_eq!(snapshot.long_touched, None);
                        assert_eq!(snapshot.drag_started, None);
                        assert_eq!(snapshot.dragged, None);
                        assert_eq!(snapshot.drag_stopped, None);
                    });
                }
            }
        }
    }

    #[test]
    fn native_toolbar_painter_preserves_official_button_activation() {
        use bevy::input::ButtonState;
        use bevy::picking::backend::PointerHits;
        use bevy::picking::input::{mouse_pick_events, spawn_mouse_pointer};
        use bevy::picking::{InteractionPlugin, PickingPlugin, PickingSystems};
        use bevy::ui::Pressed;
        use bevy::ui_widgets::ButtonPlugin;
        use bevy::window::{PrimaryWindow, WindowEvent};

        #[derive(Resource, Default)]
        struct NativeActivations(Vec<Entity>);

        // 与无叠绘基线一样：真实窗口事件 -> 官方 picking -> Button -> Activate -> Activated。
        // 手动 backend 只提供已知原生按钮命中，不注入 Activate/Activated 或业务 Action。
        for paint in [false, true] {
            for (item, rect) in native_toolbar_rects() {
                let mut app = App::new();
                app.add_plugins((
                    MinimalPlugins,
                    PickingPlugin,
                    InteractionPlugin,
                    ButtonPlugin,
                    NativeButtonsPlugin,
                ))
                .init_resource::<NativeActivations>()
                .add_message::<WindowEvent>()
                .add_message::<PointerHits>()
                .add_systems(Startup, spawn_mouse_pointer)
                .add_systems(First, mouse_pick_events.in_set(PickingSystems::Input))
                .add_systems(
                    Update,
                    |mut messages: MessageReader<Activated>, mut log: ResMut<NativeActivations>| {
                        log.0.extend(messages.read().map(|message| message.entity));
                    },
                );
                let window = app
                    .world_mut()
                    .spawn((Window::default(), PrimaryWindow))
                    .id();
                let camera = app.world_mut().spawn_empty().id();
                let button = app.world_mut().spawn((press_triggered(), item)).id();
                app.add_systems(
                    PreUpdate,
                    (move |mut hits: MessageWriter<PointerHits>| {
                        hits.write(native_buttons::test_support::pointer_hits(button, camera));
                    })
                    .before(PickingSystems::Backend),
                );
                let ctx = test_context();
                native_icon_frame(
                    &ctx,
                    paint,
                    0.0,
                    vec![egui::Event::PointerMoved(rect.center())],
                );
                app.world_mut()
                    .write_message(native_buttons::test_support::cursor_moved_event(
                        window,
                        Vec2::new(rect.center().x, rect.center().y),
                    ));
                app.update();
                assert!(app.world().resource::<NativeActivations>().0.is_empty());
                for (frame, state) in [ButtonState::Pressed, ButtonState::Released]
                    .into_iter()
                    .enumerate()
                {
                    native_icon_frame(
                        &ctx,
                        paint,
                        (frame + 1) as f64 * 0.016,
                        vec![egui::Event::PointerButton {
                            pos: rect.center(),
                            button: egui::PointerButton::Primary,
                            pressed: state == ButtonState::Pressed,
                            modifiers: egui::Modifiers::NONE,
                        }],
                    );
                    app.world_mut().write_message(
                        native_buttons::test_support::mouse_button_event(
                            window,
                            MouseButton::Left,
                            state,
                        ),
                    );
                    app.update();
                    assert_eq!(app.world().get::<Pressed>(button).is_some(), frame == 0);
                    assert_eq!(app.world().resource::<NativeActivations>().0, vec![button]);
                }
                app.update();
                assert_eq!(app.world().resource::<NativeActivations>().0, vec![button]);
            }
        }
    }

    fn pod_document() -> crate::EditorDocument {
        let mut document = crate::test_support::document();
        document.ship = dr_core::ship_from_xml(
            r#"<Ship><Parts><Part id="1" partType="pod"><Pod name="测试" throttle="0"><Staging currentStage="0"/></Pod></Part></Parts></Ship>"#,
        ).unwrap();
        document.saved_ship = document.ship.clone();
        document
    }

    #[test]
    fn navigation_snapshot_tracks_actual_docked_inspector_state() {
        use crate::properties::{self, Inspector, Preview};
        let mut inspector = Inspector::default();
        properties::open(&mut inspector, &pod_document());
        assert!(inspector.docked());
        let mut hits = super::UiHits::default();
        hits.sync_navigation(&inspector);
        assert!(!hits.advanced_open && !hits.targets_open);
        assert_eq!(hits.selected_activation, None);
        inspector.advanced_open = true;
        inspector.targets_open = true;
        inspector.preview = Preview::Activation(2, 1);
        hits.sync_navigation(&inspector);
        assert!(hits.advanced_open && hits.targets_open);
        assert_eq!(hits.selected_activation, Some((2, 1)));
        inspector.draft = None;
        hits.sync_navigation(&inspector);
        assert!(!hits.advanced_open && !hits.targets_open);
        assert_eq!(hits.selected_activation, None);
    }

    #[test]
    fn driver_opens_real_fold_button_before_requested_action() {
        use crate::properties::{Action, Field};
        let toggle = egui::Rect::from_min_size(egui::pos2(12.0, 12.0), egui::vec2(44.0, 44.0));
        let target = toggle.translate(egui::vec2(0.0, 60.0));
        let advanced = [
            Action::Focus(Field::Name),
            Action::Focus(Field::Fuel),
            Action::Focus(Field::Throttle),
            Action::Focus(Field::CurrentStage),
            Action::Active,
            Action::OpenRepair,
        ];
        let targets = [
            Action::Focus(Field::Target),
            Action::CycleTarget(true),
            Action::AddActivation(0),
        ];
        for (action, navigation) in advanced
            .into_iter()
            .map(|a| (a, Action::ToggleAdvanced))
            .chain(targets.into_iter().map(|a| (a, Action::ToggleTargets)))
        {
            let mut hits = super::UiHits {
                actions: vec![(navigation, toggle), (action.clone(), target)],
                ..default()
            };
            let mut driver = UiTestInput::default();
            let mut input = EguiInput::default();
            assert!(!driver.click(action.clone(), &hits, &mut input));
            assert!(
                matches!(input.0.events.first(), Some(egui::Event::PointerMoved(pos)) if *pos == toggle.center())
            );
            assert_eq!(input.0.events.len(), 3, "导航仍走完整同帧短点击");
            input.0.events.clear();
            hits.advanced_open = true;
            hits.targets_open = true;
            assert!(driver.click(action, &hits, &mut input));
            assert!(
                matches!(input.0.events.first(), Some(egui::Event::PointerMoved(pos)) if *pos == target.center())
            );
        }
    }

    #[test]
    fn activation_context_selects_once_then_scrolls_actual_body() {
        use crate::properties::Action;
        let row = egui::Rect::from_min_size(egui::pos2(20.0, 100.0), egui::vec2(44.0, 44.0));
        let context = row.translate(egui::vec2(0.0, 80.0));
        let body = egui::Rect::from_min_size(egui::pos2(8.0, 90.0), egui::vec2(280.0, 220.0));
        for action in [Action::Moved(1, 0), Action::RemoveActivation(1, 0)] {
            let mut hits = super::UiHits {
                actions: vec![(Action::SelectActivation(1, 0), row)],
                body: Some(body),
                ..default()
            };
            let mut driver = UiTestInput::default();
            let mut input = EguiInput::default();
            assert!(!driver.click(action.clone(), &hits, &mut input));
            assert!(
                matches!(input.0.events.first(), Some(egui::Event::PointerMoved(pos)) if *pos == row.center())
            );
            assert_eq!(input.0.events.len(), 3);
            input.0.events.clear();
            hits.selected_activation = Some((1, 0));
            assert!(!driver.click(action.clone(), &hits, &mut input));
            assert!(
                matches!(input.0.events.first(), Some(egui::Event::PointerMoved(pos)) if *pos == body.center())
            );
            assert!(input.0.events.iter().any(
                |event| matches!(event, egui::Event::MouseWheel { delta, .. } if delta.y < 0.0)
            ));
            assert!(
                !input
                    .0
                    .events
                    .iter()
                    .any(|event| matches!(event, egui::Event::PointerButton { .. }))
            );
            input.0.events.clear();
            hits.actions.push((action.clone(), context));
            assert!(driver.click(action.clone(), &hits, &mut input));
            assert!(
                matches!(input.0.events.first(), Some(egui::Event::PointerMoved(pos)) if *pos == context.center())
            );
            input.0.events.clear();
            hits.scrolling = true;
            assert!(!driver.click(action, &hits, &mut input));
            assert!(input.0.events.is_empty());
        }
    }

    #[derive(Resource, Default)]
    struct CanvasPasses {
        editing: usize,
        fit: usize,
    }

    fn count_editing(mut passes: ResMut<CanvasPasses>) {
        passes.editing += 1;
    }
    fn count_fit(mut passes: ResMut<CanvasPasses>) {
        passes.fit += 1;
    }

    #[test]
    fn menu_blocks_canvas_and_fit_and_cancels_same_frame_previews() {
        use crate::{
            CameraDrag, DragState, EditorCursor, files, help, mobile_input, panels, properties,
        };
        let mut app = App::new();
        app.insert_resource(pod_document())
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<properties::Inspector>()
            .init_resource::<panels::UiPointer>()
            .init_resource::<panels::egui_panel::UiState>()
            .init_resource::<DragState>()
            .init_resource::<EditorCursor>()
            .init_resource::<CameraDrag>()
            .init_resource::<mobile_input::GestureInput>()
            .init_resource::<files::FileMenu>()
            .init_resource::<help::HelpState>()
            .init_resource::<CanvasPasses>()
            .add_systems(
                Update,
                (
                    super::prepare_input,
                    count_editing.run_if(super::canvas_input_available),
                    count_fit.run_if(super::canvas_or_fit_available),
                )
                    .chain(),
            );
        app.world_mut().resource_mut::<files::FileMenu>().open = true;
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::F2);
        app.world_mut().resource_mut::<DragState>().id = Some(dr_core::PartKey::new(0, 1, 0));
        app.world_mut().resource_mut::<EditorCursor>().placing = true;
        app.world_mut().resource_mut::<CameraDrag>().0 = Some(Vec2::ONE);
        app.world_mut()
            .resource_mut::<mobile_input::GestureInput>()
            .navigation = Some(mobile_input::NavigationDelta {
            from: Vec2::ZERO,
            to: Vec2::ONE,
            scale_ratio: 1.2,
        });
        app.update();
        assert!(app.world().resource::<panels::UiPointer>().blocked);
        assert!(app.world().resource::<DragState>().id.is_none());
        assert!(!app.world().resource::<EditorCursor>().placing);
        assert!(app.world().resource::<CameraDrag>().0.is_none());
        assert!(
            app.world()
                .resource::<mobile_input::GestureInput>()
                .navigation
                .is_none()
        );
        assert!(
            !app.world().resource::<properties::Inspector>().is_open(),
            "菜单打开时 F2 不穿透"
        );
        assert_eq!(app.world().resource::<CanvasPasses>().editing, 0);
        assert_eq!(app.world().resource::<CanvasPasses>().fit, 0);
        app.world_mut().resource_mut::<files::FileMenu>().close();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .clear();
        app.update();
        assert_eq!(app.world().resource::<CanvasPasses>().editing, 1);
        assert_eq!(app.world().resource::<CanvasPasses>().fit, 1);
        app.world_mut()
            .resource_scope(|world, mut inspector: Mut<properties::Inspector>| {
                properties::open(&mut inspector, world.resource::<crate::EditorDocument>());
            });
        app.world_mut()
            .resource_mut::<panels::egui_panel::UiState>()
            .fit_requested = true;
        app.update();
        assert_eq!(app.world().resource::<CanvasPasses>().editing, 1);
        assert_eq!(app.world().resource::<CanvasPasses>().fit, 2);
        app.world_mut().resource_mut::<files::FileMenu>().open = true;
        app.update();
        assert_eq!(app.world().resource::<CanvasPasses>().editing, 1);
        assert_eq!(
            app.world().resource::<CanvasPasses>().fit,
            2,
            "分级 fit 豁免仍受菜单门禁"
        );
        app.world_mut().resource_mut::<files::FileMenu>().close();
        app.world_mut()
            .resource_mut::<help::HelpState>()
            .suppress_frame = true;
        app.world_mut()
            .resource_mut::<mobile_input::GestureInput>()
            .navigation = Some(mobile_input::NavigationDelta {
            from: Vec2::ZERO,
            to: Vec2::ONE,
            scale_ratio: 1.1,
        });
        app.update();
        assert!(
            app.world()
                .resource::<mobile_input::GestureInput>()
                .navigation
                .is_none()
        );
        assert_eq!(
            app.world().resource::<CanvasPasses>().fit,
            2,
            "Help suppress 关闭帧仍不放行"
        );
    }

    #[test]
    fn egui_and_native_buttons_share_palette_font_and_corners() {
        let ctx = test_context();
        let style = ctx.style_of(egui::Theme::Dark);
        assert_eq!(style.visuals.widgets.inactive.bg_fill, ui_color(BUTTON));
        assert_eq!(style.visuals.widgets.hovered.bg_fill, ui_color(HOVERED));
        assert_eq!(style.visuals.widgets.active.bg_fill, ui_color(PRESSED));
        assert_eq!(style.visuals.selection.bg_fill, ui_color(SELECTED));
        assert_eq!(style.visuals.text_color(), ui_color(TEXT));
        assert_eq!(
            style.text_styles[&egui::TextStyle::Button].size,
            BUTTON_FONT_SIZE
        );
        assert_eq!(
            style.visuals.widgets.inactive.corner_radius,
            egui::CornerRadius::same(RADIUS)
        );
        let mut world = World::new();
        let entity = world
            .spawn((
                native_buttons::press_triggered(),
                Node {
                    border_radius: native_corners(),
                    ..default()
                },
            ))
            .id();
        assert_eq!(
            world.get::<Node>(entity).unwrap().border_radius,
            BorderRadius::all(px(f32::from(RADIUS)))
        );
        assert_eq!(
            world.get::<BorderColor>(entity),
            Some(&BorderColor::all(BORDER))
        );
    }

    #[test]
    fn sandbox_palette_is_neutral_with_lime_accent_and_small_corners() {
        let ctx = test_context();
        let style = ctx.style_of(egui::Theme::Dark);
        assert_eq!(style.visuals.panel_fill, egui::Color32::from_gray(33));
        assert_eq!(
            style.visuals.widgets.inactive.bg_fill,
            egui::Color32::from_gray(48)
        );
        assert_eq!(
            style.visuals.hyperlink_color,
            egui::Color32::from_rgb(179, 219, 5)
        );
        assert_eq!(
            style.visuals.widgets.hovered.bg_stroke.color,
            style.visuals.hyperlink_color
        );
        assert_eq!(
            style.visuals.widgets.inactive.corner_radius,
            egui::CornerRadius::same(2)
        );
    }

    #[test]
    fn theme_preserves_focus_and_shared_spacing() {
        let ctx = egui::Context::default();
        let focus = egui::Id::new("保留草稿焦点");
        ctx.memory_mut(|memory| memory.request_focus(focus));
        configure_theme(&ctx);
        assert_eq!(ctx.memory(|memory| memory.focused()), Some(focus));
        assert_eq!(
            ctx.style_of(egui::Theme::Dark).spacing.interact_size.y,
            28.0
        );
        assert_eq!(
            ctx.style_of(egui::Theme::Dark).spacing.item_spacing,
            egui::vec2(8.0, 6.0)
        );
    }
}
