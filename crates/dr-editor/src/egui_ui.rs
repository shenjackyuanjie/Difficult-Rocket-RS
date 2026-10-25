//! Bevy 与 egui 的适配层：共享样式、贴图、输入隔离及属性界面接线。
//! 文本编辑、剪贴板和 IME 由 bevy_egui 处理；原生工具栏沿用 properties::setup/actions。
//! EguiPrimaryContextPass 由插件在 PostUpdate::EndPass 内执行，不应手动运行。

use crate::canvas_input;
use crate::connection_lines;

use crate::document::EditorDocument;
use crate::files;
use crate::help;
use crate::icons;
use crate::mobile_input;
use crate::native_buttons;
use crate::panels;
use crate::placement::EditorCursor;
use crate::properties;
use crate::render;
use crate::selection::DragState;
use crate::topology_ui;
use crate::view::CameraDrag;

use crate::window::EditorPaths;
use bevy::prelude::*;
use bevy::ui_widgets::Button;
use bevy_egui::{EguiContexts, EguiPlugin, EguiPrimaryContextPass, egui};
use dr_core::PartKey;

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
pub struct PartImages(std::collections::HashMap<String, egui::TextureId>);

impl PartImages {
    pub fn get(
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
                    assets.load(render::part_sprite_path(&kind.sprite))
                },
            ))
        });
        let size = render::image_size(&kind.sprite, assets, images)
            .unwrap_or_else(|| render::fallback_size(Some(kind)));
        (texture, size)
    }
}

/// 两个部件列表使用同一等比例缩略图绘制，图片不会拉伸或挤占文字列。
pub fn paint_part_image(
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

/// 停靠分级是左侧边栏，不独占画布、目录与快捷键；只有非停靠的属性窗口才是模态。
pub fn modal_properties(inspector: Option<&properties::Inspector>) -> bool {
    inspector.is_some_and(|inspector| inspector.is_open() && !inspector.docked())
}

pub fn properties_modal(inspector: Option<Res<properties::Inspector>>) -> bool {
    modal_properties(inspector.as_deref())
}

pub fn panel_actions_available(inspector: Option<Res<properties::Inspector>>) -> bool {
    !properties_modal(inspector)
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
    !modal_properties(inspector.as_deref())
        && !topology.is_some_and(|state| state.open)
        && !pending.is_some_and(|state| state.is_blocked())
        && !help.is_some_and(|state| state.open || state.suppress_frame)
        && !lines.is_some_and(|state| state.open)
        && !menu.is_some_and(|state| state.open)
}

/// 停靠分级共享画布输入，并保留打开/改尺寸时的既有适配请求。
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
    let modal = modal_properties(Some(&inspector));
    if let Some(state) = topology.as_mut() {
        if !modal && keys.just_pressed(KeyCode::F6) {
            state.open = !state.open;
        }
        if state.open && keys.just_pressed(KeyCode::Escape) {
            state.open = false;
        }
        if modal {
            state.open = false;
        }
    }
    let topology_open = topology.as_ref().is_some_and(|state| state.open);
    if !inspector.is_open() && !topology_open && keys.just_pressed(KeyCode::F2) {
        properties::open(&mut inspector, &document);
    }
    let modal = modal_properties(Some(&inspector));
    if modal || topology_open || line_open {
        if let Some(touch) = touch.as_mut() {
            touch.navigation = None;
        }
        canvas_input::cancel_preview(&mut drag, &mut cursor, &mut camera_drag);
    }
    // 沿用现有 Bevy 工具栏的命中结果，而不是覆盖它。
    pointer.blocked |= modal || topology_open || line_open;
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

pub fn ui_color(color: Color) -> egui::Color32 {
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
pub fn test_context() -> egui::Context {
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
    // 停靠草稿与画布并行编辑，每帧观察文本历史及外部修改的诊断，早退时也保持同步。
    inspector.refresh(&document);
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
mod tests;
