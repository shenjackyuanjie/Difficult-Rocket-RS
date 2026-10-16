//! 底部图标工具栏与互斥目录 / 设置抽屉，保留正式 egui 输入和语义消息。
use super::*;
use crate::icons::{self, Icon};
use bevy_egui::{EguiContexts, egui};

#[cfg(test)]
mod tests;

const PREVIEW_SIZE: f32 = 48.0;
const PART_ROW_HEIGHT: f32 = 64.0;

/// 内置 Button 负责整行交互，图片和 Label 放入各自固定的列区域。
fn part_row(
    ui: &mut egui::Ui,
    texture: egui::TextureId,
    size: Vec2,
    name: String,
    selected: bool,
    width: f32,
) -> egui::Response {
    // 触摸拖动由 ScrollArea 处理；鼠标仍支持从目录拖出放置。
    let sense = if ui.input(|input| input.any_touches()) {
        egui::Sense::click()
    } else {
        egui::Sense::click_and_drag()
    };
    let response = ui.add_sized(
        [width, PART_ROW_HEIGHT],
        egui::Button::new("").selected(selected).sense(sense),
    );
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), &name)
    });
    let rect = response.rect;
    let image_rect = egui::Rect::from_center_size(
        egui::pos2(rect.left() + 32.0, rect.center().y),
        egui::Vec2::splat(PREVIEW_SIZE),
    );
    crate::egui_ui::paint_part_image(ui, (texture, size), image_rect);
    let text_rect = egui::Rect::from_min_max(
        egui::pos2(rect.left() + 72.0, rect.top()),
        egui::pos2(rect.right() - 8.0, rect.bottom()),
    );
    let color = ui
        .style()
        .interact_selectable(&response, selected)
        .text_color();
    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(text_rect)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
        |ui| {
            ui.add(
                egui::Label::new(egui::RichText::new(name).size(18.0).color(color))
                    .halign(egui::Align::Min)
                    .selectable(false)
                    .truncate(),
            );
        },
    );
    ui.painter().vline(
        rect.left() + 64.0,
        rect.y_range(),
        ui.visuals().widgets.noninteractive.bg_stroke,
    );
    response
}

/// 展开状态由 Palette 唯一维护；egui 只负责标题、箭头和裁剪动画。
fn category_group<R>(
    ui: &mut egui::Ui,
    category: &str,
    open: bool,
    add_body: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::CollapsingResponse<R> {
    ui.visuals_mut().collapsing_header_frame = true;
    let text: egui::WidgetText = egui::RichText::new(category_label(category))
        .color(crate::egui_ui::ui_color(crate::native_buttons::ACCENT))
        .into();
    let text = text.into_galley(
        ui,
        Some(egui::TextWrapMode::Truncate),
        (ui.available_width() - ui.spacing().indent - ui.spacing().button_padding.x).max(0.0),
        egui::TextStyle::Button,
    );
    egui::CollapsingHeader::new(text)
        .id_salt(("palette_category", category))
        .open(Some(open))
        .show_unindented(ui, |ui| ui.add_enabled_ui(open, add_body).inner)
}

/// 首指按下时把拖拽候选交回已存在的滚动区，避免上一帧目录行的鼠标 Sense 抢占。
/// 点击候选仍由 egui 保留：轻点选部件，滑动由 ScrollArea 原生滚动与惯性处理。
fn touch_scroll_area(ui: &egui::Ui, scroll_id: egui::Id) {
    let clip = ui.clip_rect();
    let started_inside = ui.is_enabled()
        && ui.input(|input| {
            input.any_touches()
                && input.pointer.button_pressed(egui::PointerButton::Primary)
                && input
                    .pointer
                    .interact_pos()
                    .is_some_and(|point| clip.contains(point))
        });
    if started_inside {
        ui.ctx().set_dragged_id(scroll_id.with("area"));
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Drawer {
    Ships,
    Parts,
    Tools,
}

#[derive(Resource, Default)]
pub struct UiState {
    pub hits: Vec<(PanelButton, egui::Rect)>,
    pub areas: Vec<egui::Rect>,
    pub palette_area: Option<egui::Rect>,
    pub canvas_area: Option<egui::Rect>,
    pub fit_requested: bool,
    pub ship_toggle: Option<egui::Rect>,
    pub part_toggle: Option<egui::Rect>,
    pub more_toggle: Option<egui::Rect>,
    pub settings_toggle: Option<egui::Rect>,
    pub fit_button: Option<egui::Rect>,
    pub browser_area: Option<egui::Rect>,
    pub tools_area: Option<egui::Rect>,
    drawer: Option<Drawer>,
    default_interact_size: Option<egui::Vec2>,
    /// 已有配置控件的可交互矩形；供窗口回归注入真实点击，不另造配置路径。
    pub box_middle: Option<egui::Rect>,
    pub box_left: Option<egui::Rect>,
    pub follow_children: Option<egui::Rect>,
    pub free_mode: Option<egui::Rect>,
    pub snap_enabled: Option<egui::Rect>,
    pub rotate_left: Option<egui::Rect>,
    pub rotate_right: Option<egui::Rect>,
    pub connection_lines: Option<egui::Rect>,
    pub pixels_per_point: f32,
    pub scrolling: bool,
    pub browser_offset: f32,
    pub browser_max_scroll: f32,
    pub palette_offset: f32,
    pub browser_rows: usize,
    pub browser_scroll: Option<f32>,
    pub palette_scroll: Option<f32>,
    folder: PathBuf,
    selection: Option<usize>,
    deletion_hint: Option<(u64, BTreeSet<PartKey>, String)>,
}

impl UiState {
    /// 原生文件菜单 / 帮助入口可以与 egui 抽屉共用互斥关闭合同。
    pub fn close_drawer(&mut self) {
        self.drawer = None;
    }

    pub fn drawer_open(&self) -> bool {
        self.drawer.is_some()
    }

    fn deletion_text(&mut self, document: &EditorDocument, drag: &DragState) -> &str {
        let roots = drag.keys();
        if self
            .deletion_hint
            .as_ref()
            .is_none_or(|(revision, previous, _)| {
                *revision != document.revision || *previous != roots
            })
        {
            let text = match selection::connected_keys(
                &document.ship,
                &roots.iter().copied().collect::<Vec<_>>(),
            ) {
                Ok(parts) => format!("× 拖入此处删除 {} 个相连部件", parts.len()),
                Err(_) => "× 连接引用有歧义，无法删除".into(),
            };
            // 预览只改变姿态，不改变连接分量；文档提交/撤销/打开及拖拽范围变化时重算。
            self.deletion_hint = Some((document.revision, roots, text));
        }
        &self.deletion_hint.as_ref().expect("删除提示已建立").2
    }
}

/// 图标只绘制外观，入口的命中、焦点与点击仍来自正式 Button / Response。
fn drawer_toggle(ui: &mut egui::Ui, state: &mut UiState, drawer: Drawer) {
    let (icon, hint) = match drawer {
        Drawer::Ships => (Icon::Ships, "船体目录"),
        Drawer::Parts => (Icon::Parts, "部件目录"),
        Drawer::Tools => (Icon::More, "更多编辑工具与设置"),
    };
    let response = icons::selected_button(ui, icon, hint, state.drawer == Some(drawer));
    let rect = config_rect(ui, &response);
    match drawer {
        Drawer::Ships => state.ship_toggle = rect,
        Drawer::Parts => state.part_toggle = rect,
        Drawer::Tools => {
            state.more_toggle = rect;
            state.settings_toggle = rect;
        }
    }
    if response.clicked() {
        response.surrender_focus();
        state.drawer = if state.drawer == Some(drawer) {
            None
        } else {
            Some(drawer)
        };
    }
}

fn action_button(
    ui: &mut egui::Ui,
    state: &mut UiState,
    action: PanelButton,
    icon: Icon,
    available: bool,
    selected: bool,
) -> bool {
    let response = ui
        .add_enabled_ui(available, |ui| {
            icons::selected_button(ui, icon, panel_hint(&action), selected)
        })
        .inner;
    hit(ui, &response, action, state);
    if response.clicked() {
        response.surrender_focus();
        true
    } else {
        false
    }
}

fn fit_button(ui: &mut egui::Ui, state: &mut UiState) {
    let response = icons::button(ui, Icon::Fit, "适配视野");
    state.fit_button = config_rect(ui, &response);
    if response.clicked() {
        response.surrender_focus();
        state.fit_requested = true;
    }
}

/// 左侧 144 像素交给三个原生入口；其余空间只放一行 44 像素按钮。
#[allow(clippy::too_many_arguments)]
fn directory_header(
    ui: &mut egui::Ui,
    state: &mut UiState,
    enabled: bool,
    can_undo: bool,
    can_redo: bool,
    pending_edit: bool,
) -> Vec<PanelButton> {
    let mut actions = Vec::new();
    let rect = ui.max_rect();
    let tools = egui::Rect::from_min_max(
        egui::pos2(rect.left() + panels::NATIVE_TOOLBAR_WIDTH, rect.top()),
        rect.max,
    );
    if !tools.is_positive() {
        return actions;
    }
    ui.painter().rect_filled(
        tools,
        0.0,
        crate::egui_ui::ui_color(crate::native_buttons::PANEL),
    );
    ui.painter().hline(
        rect.x_range(),
        tools.top(),
        egui::Stroke::new(1.0, crate::egui_ui::ui_color(crate::native_buttons::ACCENT)),
    );
    let row = tools.shrink2(egui::vec2(2.0, 6.0));
    let slots = ((row.width() + 4.0) / 48.0).floor() as usize;
    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(row)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
        |ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            ui.spacing_mut().interact_size = egui::Vec2::splat(44.0);
            if !enabled {
                ui.disable();
            }
            if slots >= 2 {
                drawer_toggle(ui, state, Drawer::Parts);
            }
            if slots >= 3
                && action_button(ui, state, PanelButton::Undo, Icon::Undo, can_undo, false)
            {
                actions.push(PanelButton::Undo);
            }
            if slots >= 4
                && action_button(ui, state, PanelButton::Redo, Icon::Redo, can_redo, false)
            {
                actions.push(PanelButton::Redo);
            }
            if slots >= 5 {
                fit_button(ui, state);
            }
            if slots >= 6 {
                drawer_toggle(ui, state, Drawer::Ships);
            }
            if slots >= 7
                && pending_edit
                && action_button(
                    ui,
                    state,
                    PanelButton::CancelPlacement,
                    Icon::Cancel,
                    true,
                    false,
                )
            {
                actions.push(PanelButton::CancelPlacement);
            }
            if slots >= 1 {
                drawer_toggle(ui, state, Drawer::Tools);
            }
        },
    );
    actions
}

fn drawer_header(ui: &mut egui::Ui, title: &str, state: &mut UiState) {
    ui.horizontal(|ui| {
        ui.heading(title);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let response = icons::button(ui, Icon::Cancel, "收起抽屉");
            if response.clicked() {
                response.surrender_focus();
                state.drawer = None;
            }
        });
    });
}

fn tools_navigation(ui: &mut egui::Ui, state: &mut UiState) {
    ui.horizontal(|ui| {
        drawer_toggle(ui, state, Drawer::Ships);
        drawer_toggle(ui, state, Drawer::Parts);
        fit_button(ui, state);
    });
}

fn sidebar_width(width: f32, preferred: f32) -> f32 {
    if panels::compact(width) {
        preferred.min(width * 0.7)
    } else {
        preferred
    }
}

fn button(
    ui: &mut egui::Ui,
    action: PanelButton,
    selected: bool,
    text: impl Into<egui::WidgetText>,
    state: &mut UiState,
    actions: &mut MessageWriter<PanelButton>,
) {
    let response = directory_button(ui, &action, selected, text);
    hit(ui, &response, action.clone(), state);
    if response.clicked() {
        response.surrender_focus();
        actions.write(action);
    }
}

fn directory_button(
    ui: &mut egui::Ui,
    action: &PanelButton,
    selected: bool,
    text: impl Into<egui::WidgetText>,
) -> egui::Response {
    let hint = panel_hint(action);
    ui.push_id(("directory_action", format!("{action:?}")), |ui| {
        ui.add(egui::Button::new(text).selected(selected))
    })
    .inner
    .on_hover_text(hint)
    .on_disabled_hover_text(
        "请先应用或取消属性草稿，或处理文件确认，再操作目录。 ".to_owned() + hint,
    )
}

fn panel_hint(action: &PanelButton) -> &'static str {
    match action {
        PanelButton::Folder => "选择并切换船体目录。",
        PanelButton::Refresh => "重新扫描当前目录的船体 XML 文件。",
        PanelButton::ToggleCategory(_) => "展开或收起此类部件；Tab 跳过已收起的类别。",
        PanelButton::ExpandCategories => "展开所有部件类别。",
        PanelButton::CancelPlacement => {
            "取消尚未提交的拖动、放置或选点，不修改文档（Esc / 右键）。"
        }
        PanelButton::Undo => "撤销上次文档事务（Ctrl+Z）。",
        PanelButton::Redo => "重做上次撤销（Ctrl+Y）。",
        PanelButton::Delete => "删除选中部件，可一次撤销。",
        PanelButton::SelectAll => "选择全部部件（Ctrl+A）。",
        PanelButton::Copy => "复制选中部件（Ctrl+C）。",
        PanelButton::Cut => "剪切选中部件，可一次撤销（Ctrl+X）。",
        PanelButton::Paste => "预览剪贴板部件，定位后放置（Ctrl+V）。",
        PanelButton::Rotate => "旋转 90°（R）。",
        PanelButton::FlipX => "水平镜像（X）。",
        PanelButton::FlipY => "垂直镜像（Y）。",
        _ => "选择船体或部件；部件可拖到画布放置。",
    }
}

const FOLLOW_HINT: &str = "子节点跟随：开启后拖动父节点时，所有后代一起移动和旋转，保留内部连接。关闭则只拖当前选中部件。按连接方向计算，不包含父节点；本次拖拽开始时确定范围。";

fn icon_toggle(ui: &mut egui::Ui, icon: Icon, hint: &str, value: &mut bool) -> egui::Response {
    let mut response = icons::selected_button(ui, icon, hint, *value);
    if response.clicked() {
        *value = !*value;
        response.mark_changed();
        response.surrender_focus();
    }
    response
}

/// 显式按可用宽度分行，固定尺寸按钮不会把抽屉撑宽。
fn tool_action_rows(
    ui: &mut egui::Ui,
    state: &mut UiState,
    entries: &[(PanelButton, Icon, bool)],
) -> Vec<PanelButton> {
    let mut actions = Vec::new();
    let columns = (((ui.available_width() + 4.0) / 48.0).floor() as usize).max(1);
    for row in entries.chunks(columns) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            for (action, icon, available) in row {
                if action_button(ui, state, action.clone(), *icon, *available, false) {
                    actions.push(action.clone());
                }
            }
        });
    }
    actions
}

fn config_rect(ui: &egui::Ui, response: &egui::Response) -> Option<egui::Rect> {
    let rect = response.rect.intersect(ui.clip_rect());
    (response.enabled() && rect.is_positive()).then_some(rect)
}

fn hit(ui: &egui::Ui, response: &egui::Response, action: PanelButton, state: &mut UiState) {
    let rect = response.rect.intersect(ui.clip_rect());
    if response.enabled() && rect.is_positive() {
        state.hits.push((action, rect));
    }
}

/// 复用 egui 按钮的真实文字、边距与描边尺寸，虚拟列表不能把末行挤出滚动范围。
fn browser_row_height(ui: &egui::Ui) -> f32 {
    let button = ui.style().button_style(
        &egui::widget_style::Classes::default(),
        egui::widget_style::WidgetState::Inactive,
    );
    ui.spacing()
        .interact_size
        .y
        .max(ui.text_style_height(&egui::TextStyle::Button) + button.frame.total_margin().sum().y)
        .ceil()
}

/// 单条底部 Panel 占据布局和命中空间，背景只画在原生入口以右。
#[allow(clippy::too_many_arguments)]
fn bottom_toolbar(
    ui: &mut egui::Ui,
    state: &mut UiState,
    enabled: bool,
    can_undo: bool,
    can_redo: bool,
    pending_edit: bool,
) -> (egui::Rect, Vec<PanelButton>) {
    let toolbar = egui::Panel::bottom("editor_bottom_toolbar")
        .exact_size(panels::TOOLBAR_HEIGHT)
        .resizable(false)
        .frame(egui::Frame::NONE)
        .show(ui, |ui| {
            directory_header(ui, state, enabled, can_undo, can_redo, pending_edit)
        });
    (toolbar.response.rect, toolbar.inner)
}

// 将可选资源合并为一个参数，保持在 Bevy 系统参数数量限制内。
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn draw(
    mut contexts: EguiContexts,
    assets: Res<AssetServer>,
    image_assets: Res<Assets<Image>>,
    mut document: ResMut<EditorDocument>,
    paths: Res<EditorPaths>,
    palette: Res<Palette>,
    mut browser: ResMut<ShipBrowser>,
    mut cursor: ResMut<EditorCursor>,
    inspector: Res<properties::Inspector>,
    (pending, help): (
        Option<Res<files::PendingFileAction>>,
        Option<Res<help::HelpState>>,
    ),
    mut topology: ResMut<crate::topology_ui::ConnectionEditor>,
    (mut options, mut drag, mut line_settings): (
        ResMut<view::ViewOptions>,
        ResMut<DragState>,
        Option<ResMut<connection_lines::Settings>>,
    ),
    mut state: ResMut<UiState>,
    mut actions: MessageWriter<PanelButton>,
    mut images: ResMut<crate::egui_ui::PartImages>,
    mut counts: Local<std::collections::HashMap<String, usize>>,
) {
    let Ok(ctx) = contexts.ctx_mut() else { return };
    let ctx = ctx.clone();
    state.hits.clear();
    state.box_middle = None;
    state.box_left = None;
    state.follow_children = None;
    state.free_mode = None;
    state.snap_enabled = None;
    state.rotate_left = None;
    state.rotate_right = None;
    state.connection_lines = None;
    state.areas.clear();
    state.palette_area = None;
    state.canvas_area = None;
    state.ship_toggle = None;
    state.part_toggle = None;
    state.more_toggle = None;
    state.settings_toggle = None;
    state.fit_button = None;
    state.browser_area = None;
    state.tools_area = None;
    state.pixels_per_point = ctx.pixels_per_point();
    state.scrolling = ctx.input(|input| input.is_scrolling());
    let enabled = !crate::egui_ui::modal_properties(Some(&inspector))
        && !pending.is_some_and(|state| state.is_blocked())
        && !help.is_some_and(|state| state.open || state.suppress_frame)
        && !topology.open
        && !line_settings.as_ref().is_some_and(|state| state.open);
    if document.is_changed() {
        counts.clear();
        for part in document.ship.all_parts() {
            *counts.entry(part.part_type.clone()).or_default() += 1;
        }
    }
    let current = paths
        .ship
        .as_deref()
        .and_then(|path| std::fs::canonicalize(path).ok());
    if paths.is_changed()
        && let Some(path) = &current
        && !browser.files.contains(path)
        && std::fs::canonicalize(&browser.folder).ok().as_deref() == path.parent()
    {
        let folder = browser.folder.clone();
        browser.set_folder(folder);
    }
    if state.folder != browser.folder {
        state.folder = browser.folder.clone();
        state.browser_scroll = Some(0.0);
    }
    let selection_changed = state.selection != Some(cursor.catalog_index);
    state.selection = Some(cursor.catalog_index);
    let width = ctx.viewport_rect().width();
    let compact = panels::compact(width);
    let original = *state
        .default_interact_size
        .get_or_insert(ctx.style_of(ctx.theme()).spacing.interact_size);
    let interact_size = if compact {
        original.max(egui::Vec2::splat(44.0))
    } else {
        original
    };
    if ctx.style_of(ctx.theme()).spacing.interact_size != interact_size {
        ctx.style_mut_of(ctx.theme(), |style| {
            style.spacing.interact_size = interact_size
        });
    }
    let mut viewport_ui = egui::Ui::new(
        ctx.clone(),
        egui::Id::new("directory_viewport"),
        egui::UiBuilder::new()
            .layer_id(egui::LayerId::background())
            .max_rect(ctx.viewport_rect()),
    );
    let pending_edit = cursor.placing
        || cursor.paste.is_some()
        || cursor.manual_connection.is_some()
        || drag.id.is_some();
    let (toolbar, requested) = bottom_toolbar(
        &mut viewport_ui,
        &mut state,
        enabled,
        document.history.can_undo() || inspector.can_undo(),
        document.history.can_redo() || inspector.can_redo(),
        pending_edit,
    );
    actions.write_batch(requested);
    // 包含原生三入口和条内空白；滚动与点按不会穿透到底层画布。
    state.areas.push(toolbar);
    if !crate::egui_ui::modal_properties(Some(&inspector)) && state.drawer == Some(Drawer::Ships) {
        let left = egui::Panel::left("ship_browser_sidebar")
            .exact_size(sidebar_width(width, 248.0))
            .resizable(false)
            .show(&mut viewport_ui, |ui| {
                if !enabled {
                    ui.disable();
                }
                ui.spacing_mut().interact_size.y = 44.0;
                drawer_header(ui, "船体目录", &mut state);
                let folder = browser
                    .folder
                    .file_name()
                    .unwrap_or(browser.folder.as_os_str())
                    .to_string_lossy();
                ui.label(format!("{folder} · {}", browser.files.len()))
                    .on_hover_text(format!(
                        "{}\n{} 个船体",
                        browser.folder.display(),
                        browser.files.len()
                    ));
                ui.horizontal(|ui| {
                    button(
                        ui,
                        PanelButton::Folder,
                        false,
                        "目录…",
                        &mut state,
                        &mut actions,
                    );
                    button(
                        ui,
                        PanelButton::Refresh,
                        false,
                        "刷新",
                        &mut state,
                        &mut actions,
                    );
                });
                if let Some(error) = &browser.error {
                    ui.colored_label(egui::Color32::LIGHT_RED, error);
                }
                if browser.rejected > 0 {
                    ui.label(format!("异常 XML · {}", browser.rejected))
                        .on_hover_text("扫描时跳过了无法解析的 XML，未修改这些文件。");
                }
                ui.separator();
                let mut scroll = egui::ScrollArea::vertical()
                    .id_salt("ship_browser_files")
                    .auto_shrink([false, false]);
                if let Some(offset) = state.browser_scroll.take() {
                    scroll = scroll.vertical_scroll_offset(offset);
                }
                let row_height = browser_row_height(ui);
                let output = scroll.show_rows(ui, row_height, browser.files.len(), |ui, rows| {
                    state.browser_rows = rows.len();
                    for index in rows {
                        let path = &browser.files[index];
                        let text = path.file_stem().unwrap_or_default().to_string_lossy();
                        let response = ui
                            .push_id(("ship_file", path), |ui| {
                                ui.add_sized(
                                    [ui.available_width(), row_height],
                                    egui::Button::new(text.as_ref())
                                        .selected(current.as_ref() == Some(path))
                                        .truncate(),
                                )
                            })
                            .inner
                            .on_hover_text(path.display().to_string())
                            .on_disabled_hover_text(
                                "请先应用或取消属性草稿，或处理文件确认，再打开船体。",
                            );
                        hit(ui, &response, PanelButton::Open(path.clone()), &mut state);
                        if response.clicked() {
                            response.surrender_focus();
                            actions.write(PanelButton::Open(path.clone()));
                            if compact {
                                state.drawer = None;
                            }
                        }
                    }
                });
                state.browser_offset = output.state.offset.y;
                state.browser_max_scroll =
                    (output.content_size.y - output.inner_rect.height()).max(0.0);
            });
        state.areas.push(left.response.rect);
        if enabled {
            state.browser_area = Some(left.response.rect);
        }
    }
    if !crate::egui_ui::modal_properties(Some(&inspector)) && state.drawer == Some(Drawer::Parts) {
        let right = egui::Panel::right("part_palette_sidebar")
            .exact_size(sidebar_width(width, 296.0))
            .resizable(false)
            .show(&mut viewport_ui, |ui| {
                if !enabled {
                    ui.disable();
                }
                ui.spacing_mut().interact_size.y = 44.0;
                if drag.id.is_some() {
                    ui.colored_label(
                        egui::Color32::LIGHT_RED,
                        state.deletion_text(&document, &drag),
                    )
                    .on_hover_text(
                        "拖到此列表松手删除整个相连分量；Ctrl+Z 一次撤销；Esc / 右键取消。",
                    );
                    ui.disable();
                } else {
                    state.deletion_hint = None;
                    drawer_header(ui, "部件目录", &mut state);
                }
                button(
                    ui,
                    PanelButton::ExpandCategories,
                    palette.collapsed.is_empty(),
                    "全部展开",
                    &mut state,
                    &mut actions,
                );
                ui.separator();
                let scroll_id = ui.make_persistent_id(egui::IdSalt::new("part_palette_rows"));
                let mut scroll = egui::ScrollArea::vertical()
                    .id_salt("part_palette_rows")
                    .max_height((ui.available_height() - 100.0).max(50.0))
                    .auto_shrink([false, false]);
                if let Some(offset) = state.palette_scroll.take() {
                    scroll = scroll.vertical_scroll_offset(offset);
                }
                let groups = palette.groups(&document.catalog);
                let kinds: Vec<_> = document.catalog.visible().collect();
                let output = scroll.show(ui, |ui| {
                    touch_scroll_area(ui, scroll_id);
                    ui.spacing_mut().item_spacing.y = 0.0;
                    for (category, indices) in groups {
                        let open = !palette.collapsed.contains(category);
                        let group = category_group(ui, category, open, |ui| {
                            for index in indices {
                                let kind = kinds[index];
                                let (texture, size) =
                                    images.get(&mut contexts, &assets, &image_assets, kind);
                                let count = counts.get(&kind.id).copied().unwrap_or(0);
                                let limit = kind
                                    .max_occurrences
                                    .map(|limit| format!(" {count}/{limit}"))
                                    .unwrap_or_default();
                                let width = ui.available_width();
                                let response = ui
                                    .push_id(("palette_part", &kind.id), |ui| {
                                        part_row(
                                            ui,
                                            texture,
                                            size,
                                            format!("{}{limit}", kind.name),
                                            index == cursor.catalog_index,
                                            width,
                                        )
                                    })
                                    .inner
                                    .on_hover_text(format!(
                                        "{}\n点击选择，再点击画布放置；也可直接拖到画布。\n{}",
                                        kind.name, kind.description
                                    ))
                                    .on_disabled_hover_text(
                                        "请先应用或取消属性草稿，或处理文件确认，再选择部件。",
                                    );
                                hit(ui, &response, PanelButton::Part(index), &mut state);
                                if selection_changed && index == cursor.catalog_index {
                                    response.scroll_to_me(Some(egui::Align::Center));
                                }
                                if response.drag_started() {
                                    response.surrender_focus();
                                    actions.write(PanelButton::DragPart(index));
                                } else if response.clicked() {
                                    response.surrender_focus();
                                    actions.write(PanelButton::Part(index));
                                    if compact {
                                        state.drawer = None;
                                    }
                                }
                            }
                        });
                        let action = PanelButton::ToggleCategory(category.to_owned());
                        let response = group.header_response.on_hover_text(panel_hint(&action));
                        hit(ui, &response, action.clone(), &mut state);
                        if response.clicked() {
                            response.surrender_focus();
                            actions.write(action);
                        }
                    }
                });
                state.palette_offset = output.state.offset.y;
                ui.separator();
                if let Some(kind) = kinds.get(cursor.catalog_index) {
                    ui.add(egui::Label::new(&kind.name).truncate())
                        .on_hover_text(format!("质量 {:.2}\n{}", kind.mass, kind.description));
                }
                ui.horizontal(|ui| {
                    for (action, icon) in [
                        (PanelButton::Rotate, Icon::Rotate),
                        (PanelButton::FlipX, Icon::FlipX),
                        (PanelButton::FlipY, Icon::FlipY),
                    ] {
                        if action_button(ui, &mut state, action.clone(), icon, true, false) {
                            actions.write(action);
                        }
                    }
                    if cursor.placing
                        && action_button(
                            ui,
                            &mut state,
                            PanelButton::CancelPlacement,
                            Icon::Cancel,
                            true,
                            false,
                        )
                    {
                        actions.write(PanelButton::CancelPlacement);
                    }
                });
            });
        state.areas.push(right.response.rect);
        if enabled {
            state.palette_area = Some(right.response.rect);
        }
    }
    if !crate::egui_ui::modal_properties(Some(&inspector)) && state.drawer == Some(Drawer::Tools) {
        let tools = egui::Panel::right("editor_tools_drawer")
            .exact_size(sidebar_width(width, 296.0))
            .resizable(false)
            .show(&mut viewport_ui, |ui| {
                if !enabled { ui.disable(); }
                ui.spacing_mut().interact_size = egui::Vec2::splat(44.0);
                drawer_header(ui, "编辑工具", &mut state);
                let scroll_id = ui.make_persistent_id(egui::IdSalt::new("editor_tools_scroll"));
                let scroll = egui::ScrollArea::vertical()
                    .id_salt("editor_tools_scroll")
                    .auto_shrink([false, false]);
                scroll.show(ui, |ui| {
                    ui.set_max_width(ui.available_width());
                    touch_scroll_area(ui, scroll_id);
                    tools_navigation(ui, &mut state);
                    ui.separator();
                    let selected = !document.selected_keys().is_empty() && drag.id.is_none();
                    let mut entries = vec![
                        (PanelButton::Undo, Icon::Undo, document.history.can_undo() || inspector.can_undo()),
                        (PanelButton::Redo, Icon::Redo, document.history.can_redo() || inspector.can_redo()),
                        (PanelButton::Delete, Icon::Delete, selected),
                        (PanelButton::SelectAll, Icon::SelectAll, true),
                        (PanelButton::Copy, Icon::Copy, selected),
                        (PanelButton::Cut, Icon::Cut, selected),
                        (PanelButton::Paste, Icon::Paste, cursor.clipboard.is_some()),
                        (PanelButton::Rotate, Icon::Rotate, true),
                        (PanelButton::FlipX, Icon::FlipX, true),
                        (PanelButton::FlipY, Icon::FlipY, true),
                    ];
                    if pending_edit {
                        entries.push((PanelButton::CancelPlacement, Icon::Cancel, true));
                    }
                    for action in tool_action_rows(ui, &mut state, &entries) {
                        actions.write(action);
                    }
                    ui.separator();
                    ui.strong("编辑设置");
                    if ui.button("树 / 图 · F6")
                        .on_hover_text("打开连接树 / 有向连接图；选择子树、连通部件和编辑连接（F6）。")
                        .on_disabled_hover_text("请先应用或取消属性草稿，或处理文件确认，再编辑连接。")
                        .clicked() {
                        topology.open = true;
                    }
                    ui.horizontal_wrapped(|ui| {
                        ui.label("框选").on_hover_text("框选多个部件；选择左键框选时，中键用于移动视角。默认中键框选、左键拖空白移动视角。");
                        let middle = ui.selectable_value(
                            &mut options.box_select_button,
                            view::BoxSelectButton::Middle,
                            "中键",
                        )
                        .on_hover_text("中键拖动框选；左键拖空白移动视角。")
                        .on_disabled_hover_text("请先应用或取消属性草稿，或处理文件确认，再切换框选按键。");
                        state.box_middle = config_rect(ui, &middle);
                        let left = ui.selectable_value(
                            &mut options.box_select_button,
                            view::BoxSelectButton::Left,
                            "左键",
                        )
                        .on_hover_text("左键拖空白框选；中键拖动移动视角。")
                        .on_disabled_hover_text("请先应用或取消属性草稿，或处理文件确认，再切换框选按键。");
                        state.box_left = config_rect(ui, &left);
                    });
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        let follow = icon_toggle(ui, Icon::Follow, FOLLOW_HINT, &mut options.follow_children);
                        state.follow_children = config_rect(ui, &follow);
                        let snap = icon_toggle(ui, Icon::Snap, "吸附：开启后放置/拖动会对齐连接点并自动建立连接，关闭后完全按指针位置放置、已有连接保持不变", &mut document.snap_enabled);
                        state.snap_enabled = config_rect(ui, &snap);
                        let mut free = document.free_mode;
                        let response = icon_toggle(ui, Icon::Free, "自由模式：关闭自动吸附、碰撞判定和自动断连。依次点击两个连接点/边建立连接，重复点击这两个端点可断开；移动、旋转保留已有连接。Esc / 右键取消选点，Ctrl+Z 撤销。", &mut free);
                        state.free_mode = config_rect(ui, &response);
                        if response.changed() {
                            free_mode::set_enabled(&mut document, &mut cursor, &mut drag, free);
                        }
                    });
                    if let Some(settings) = line_settings.as_mut() {
                        let response = ui.button("连接线设置…").on_hover_text("显示开关、颜色/透明度、粗细、实线/虚线/点线、呼吸/流动及方向箭头；只改变显示。");
                        state.connection_lines = config_rect(ui, &response);
                        if response.clicked() { response.surrender_focus(); settings.open = true; }
                    }
                    let selected = document.selected_keys();
                    if let Some(part) = document.selected.and_then(|key| document.ship.part_at(key)) {
                        let angle = part.angle.to_degrees();
                        ui.horizontal_wrapped(|ui| {
                            ui.label(format!("角度 {angle:.1}°"));
                            for (degrees, label) in [(-15.0_f64, "−15°"), (15.0_f64, "+15°")] {
                                let response = ui.button(label).on_hover_text("围绕选区中心精细旋转；连接点和连接边跟随角度。R 仍为 90°。");
                                if degrees < 0.0 { state.rotate_left = config_rect(ui, &response); }
                                else { state.rotate_right = config_rect(ui, &response); }
                                if response.clicked() {
                                    response.surrender_focus();
                                    let center = selection::center(&document, &selected);
                                    document.execute(EditorCommand::TransformSelection {
                                        parts: selected.clone(),
                                        transform: SelectionTransform::RotateBy { center, radians: degrees.to_radians() },
                                    });
                                }
                            }
                        });
                    }
                });
            });
        state.areas.push(tools.response.rect);
        if enabled {
            state.tools_area = Some(tools.response.rect);
        }
    }
    state.canvas_area = Some(viewport_ui.available_rect_before_wrap());
}

impl crate::egui_ui::UiTestInput {
    pub fn click_panel(
        &mut self,
        action: PanelButton,
        state: &UiState,
        input: &mut bevy_egui::EguiInput,
    ) -> bool {
        if state.scrolling {
            return false;
        }
        if let Some((_, rect)) = state
            .hits
            .iter()
            .find(|(candidate, _)| *candidate == action)
        {
            self.click_rect(*rect, input);
            return true;
        }
        let wanted = match &action {
            PanelButton::Part(_)
            | PanelButton::DragPart(_)
            | PanelButton::ToggleCategory(_)
            | PanelButton::ExpandCategories => Drawer::Parts,
            PanelButton::Open(_) | PanelButton::Folder | PanelButton::Refresh => Drawer::Ships,
            _ => Drawer::Tools,
        };
        if state.drawer != Some(wanted) {
            let toggle = match wanted {
                Drawer::Parts => state.part_toggle,
                Drawer::Ships => state.ship_toggle.or(state.more_toggle),
                Drawer::Tools => state.more_toggle,
            };
            if let Some(rect) = toggle {
                self.click_rect(rect, input);
            }
            return false;
        }
        let area = match wanted {
            Drawer::Parts => state.palette_area,
            Drawer::Ships => state.browser_area,
            Drawer::Tools => state.tools_area,
        };
        let Some(rect) = area else {
            return false;
        };
        input.0.events.extend([
            egui::Event::PointerMoved(rect.center()),
            egui::Event::MouseWheel {
                phase: egui::TouchPhase::Move,
                unit: egui::MouseWheelUnit::Point,
                delta: egui::vec2(0.0, -350.0),
                modifiers: egui::Modifiers::NONE,
            },
        ]);
        false
    }
}
