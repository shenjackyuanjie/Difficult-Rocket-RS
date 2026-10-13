//! 两个目录边栏使用 egui 内置布局及虚拟滚动，不重建 Bevy UI 实体。
use super::*;
use bevy_egui::{EguiContexts, egui};

#[cfg(test)]
mod tests;

/// 缩略图容纳在 40×40 的区域内，但纹理尺寸必须使用真实像素宽高。
fn preview_image(texture: egui::TextureId, size: Vec2) -> egui::Image<'static> {
    egui::Image::new((texture, egui::vec2(size.x, size.y)))
        .maintain_aspect_ratio(true)
        .fit_to_exact_size(egui::Vec2::splat(40.0))
}

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
        [width, 52.0],
        egui::Button::new("").selected(selected).sense(sense),
    );
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), &name)
    });
    let rect = response.rect;
    let image_size = size / size.max_element().max(1.0) * 40.0;
    let image_rect = egui::Rect::from_center_size(
        egui::pos2(rect.left() + 28.0, rect.center().y),
        egui::vec2(image_size.x, image_size.y),
    );
    preview_image(texture, size).paint_at(ui, image_rect);
    let text_rect = egui::Rect::from_min_max(
        egui::pos2(rect.left() + 64.0, rect.top()),
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
                egui::Label::new(egui::RichText::new(name).color(color))
                    .halign(egui::Align::Min)
                    .selectable(false)
                    .truncate(),
            );
        },
    );
    ui.painter().vline(
        rect.left() + 56.0,
        rect.y_range(),
        ui.visuals().widgets.noninteractive.bg_stroke,
    );
    response
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
    drawer: Option<Drawer>,
    default_interact_size: Option<egui::Vec2>,
    /// 已有配置控件的可交互矩形；供窗口回归注入真实点击，不另造配置路径。
    pub box_middle: Option<egui::Rect>,
    pub box_left: Option<egui::Rect>,
    pub follow_children: Option<egui::Rect>,
    pub free_mode: Option<egui::Rect>,
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
    category: Option<String>,
    selection: Option<usize>,
    deletion_hint: Option<(u64, BTreeSet<PartKey>, String)>,
}

impl UiState {
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

/// 导航、事务按钮始终在正文滚动之外；窄屏默认收起目录保留画布。
#[allow(clippy::too_many_arguments)]
fn directory_header(
    ui: &mut egui::Ui,
    state: &mut UiState,
    compact: bool,
    enabled: bool,
    can_undo: bool,
    can_redo: bool,
    selected: bool,
    clipboard: bool,
    pending_edit: bool,
) -> Vec<PanelButton> {
    let mut actions = Vec::new();
    ui.spacing_mut().interact_size = egui::Vec2::splat(44.0);
    ui.horizontal_wrapped(|ui| {
        if !enabled {
            ui.disable();
        }
        if compact {
            for (drawer, label) in [(Drawer::Ships, "船体"), (Drawer::Parts, "部件")] {
                let response = ui.add(
                    egui::Button::new(label)
                        .selected(state.drawer == Some(drawer))
                        .min_size(egui::vec2(0.0, 44.0)),
                );
                if drawer == Drawer::Ships {
                    state.ship_toggle = Some(response.rect);
                } else {
                    state.part_toggle = Some(response.rect);
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
        }
        for (action, label, available) in [
            (PanelButton::Undo, "撤销", can_undo),
            (PanelButton::Redo, "重做", can_redo),
            (PanelButton::Delete, "删除", selected),
            (PanelButton::SelectAll, "全选", true),
            (PanelButton::Copy, "复制", selected),
            (PanelButton::Cut, "剪切", selected),
            (PanelButton::Paste, "粘贴", clipboard),
        ] {
            let response = ui.add_enabled(
                available,
                egui::Button::new(label).min_size(egui::vec2(0.0, 44.0)),
            );
            hit(ui, &response, action.clone(), state);
            if response.clicked() {
                response.surrender_focus();
                actions.push(action);
            }
        }
        if ui
            .add(egui::Button::new("适配视野").min_size(egui::vec2(0.0, 44.0)))
            .clicked()
        {
            state.fit_requested = true;
        }
        if pending_edit {
            let action = PanelButton::CancelPlacement;
            let response = ui.add(egui::Button::new("取消").min_size(egui::vec2(0.0, 44.0)));
            hit(ui, &response, action.clone(), state);
            if response.clicked() {
                response.surrender_focus();
                actions.push(action);
            }
        }
    });
    actions
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
        PanelButton::Category(_) => "筛选部件类别；全部显示所有可用部件。",
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

fn follow_toggle(ui: &mut egui::Ui, follow: &mut bool) -> egui::Response {
    ui.checkbox(follow, "↳ 子节点跟随")
        .on_hover_text("开启后拖动父节点时，所有后代一起移动和旋转，保留内部连接。关闭则只拖当前选中部件。按连接方向计算，不包含父节点；本次拖拽开始时确定范围。")
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

/// 只保留原生顶栏的布局和命中空隙，不绘制盖在 Bevy UI 上方的背景。
fn reserve_native_toolbar(ui: &mut egui::Ui, width: f32) -> egui::Response {
    egui::Panel::top("directory_toolbar_space")
        .exact_size(panels::toolbar_height(width))
        .frame(egui::Frame::NONE)
        .show(ui, |_| {})
        .response
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
    mut images: Local<std::collections::HashMap<String, egui::TextureId>>,
    mut counts: Local<std::collections::HashMap<String, usize>>,
) {
    let Ok(ctx) = contexts.ctx_mut() else { return };
    let ctx = ctx.clone();
    state.hits.clear();
    state.box_middle = None;
    state.box_left = None;
    state.follow_children = None;
    state.free_mode = None;
    state.rotate_left = None;
    state.rotate_right = None;
    state.connection_lines = None;
    state.areas.clear();
    state.palette_area = None;
    state.canvas_area = None;
    state.ship_toggle = None;
    state.part_toggle = None;
    state.pixels_per_point = ctx.pixels_per_point();
    state.scrolling = ctx.input(|input| input.is_scrolling());
    let enabled = !inspector.is_open()
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
    if state.category != palette.category {
        state.category = palette.category.clone();
        state.palette_scroll = Some(0.0);
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
    // 整个原生顶栏（包括空隙）均归 UI，触摸滚动不落到画布。
    let toolbar = reserve_native_toolbar(&mut viewport_ui, width);
    state.areas.push(toolbar.rect);
    let header = egui::Panel::top("directory_navigation").show(&mut viewport_ui, |ui| {
        directory_header(
            ui,
            &mut state,
            compact,
            enabled,
            document.history.can_undo(),
            document.history.can_redo(),
            !document.selected_keys().is_empty() && drag.id.is_none(),
            cursor.clipboard.is_some(),
            cursor.placing
                || cursor.paste.is_some()
                || cursor.manual_connection.is_some()
                || drag.id.is_some(),
        )
    });
    for action in header.inner {
        actions.write(action);
    }
    state.areas.push(header.response.rect);
    if !compact || state.drawer == Some(Drawer::Ships) {
        let left = egui::Panel::left("ship_browser_sidebar")
            .exact_size(sidebar_width(width, 248.0))
            .resizable(false)
            .show(&mut viewport_ui, |ui| {
                if !enabled {
                    ui.disable();
                }
                if compact {
                    ui.spacing_mut().interact_size.y = 44.0;
                }
                ui.heading("船体列表");
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
    }
    if !compact || state.drawer == Some(Drawer::Parts) {
        let right = egui::Panel::right("part_palette_sidebar")
        .exact_size(sidebar_width(width, 296.0))
        .resizable(false)
        .show(&mut viewport_ui, |ui| {
            if !enabled { ui.disable(); }
            if compact { ui.spacing_mut().interact_size.y = 44.0; }
            let palette_height = if compact { (ui.available_height() * 0.5).clamp(80.0, 240.0) }
                else { (ui.available_height() - 125.0).max(50.0) };
            let mut content = |ui: &mut egui::Ui| {
            if drag.id.is_some() {
                ui.colored_label(egui::Color32::LIGHT_RED, state.deletion_text(&document, &drag))
                    .on_hover_text("拖到此列表松手删除拖拽部件及整个相连分量（包括父节点、子节点和对接连接器），Ctrl+Z 一次撤销；Esc / 右键取消。");
                ui.disable();
            } else {
                state.deletion_hint = None;
                ui.heading("部件目录").on_hover_text("将画布部件拖到此列表松手，可删除它及所有相连部件；Ctrl+Z 撤销。");
            }
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
            let follow = follow_toggle(ui, &mut options.follow_children);
            state.follow_children = config_rect(ui, &follow);
            if let Some(settings) = line_settings.as_mut() {
                let response = ui.button("连接线设置…").on_hover_text("显示开关、颜色/透明度、粗细、实线/虚线/点线、呼吸/流动及方向箭头；只改变显示。");
                state.connection_lines = config_rect(ui, &response);
                if response.clicked() { response.surrender_focus(); settings.open = true; }
            }
            let mut free = document.free_mode;
            let response = ui.checkbox(&mut free, "自由模式")
                .on_hover_text("关闭自动吸附、碰撞判定和自动断连。依次点击两个连接点/边建立连接，重复点击这两个端点可断开；移动、旋转保留已有连接。Esc / 右键取消选点，Ctrl+Z 撤销。");
            state.free_mode = config_rect(ui, &response);
            if response.changed() {
                response.surrender_focus();
                free_mode::set_enabled(&mut document, &mut cursor, &mut drag, free);
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
            let mut categories: Vec<_> = document
                .catalog
                .visible()
                .map(|kind| category_name(kind).to_owned())
                .collect();
            categories.sort();
            categories.dedup();
            ui.horizontal_wrapped(|ui| {
                button(
                    ui,
                    PanelButton::Category(None),
                    palette.category.is_none(),
                    "全部",
                    &mut state,
                    &mut actions,
                );
                for category in categories {
                    button(
                        ui,
                        PanelButton::Category(Some(category.clone())),
                        palette.category.as_ref() == Some(&category),
                        category,
                        &mut state,
                        &mut actions,
                    );
                }
            });
            ui.separator();
            let scroll_id = ui.make_persistent_id(egui::IdSalt::new("part_palette_rows"));
            let mut scroll = egui::ScrollArea::vertical()
                .id_salt("part_palette_rows")
                .max_height(if compact { palette_height } else { (ui.available_height() - 125.0).max(50.0) })
                .auto_shrink([false, false]);
            if let Some(offset) = state.palette_scroll.take() {
                scroll = scroll.vertical_scroll_offset(offset);
            }
            let indices = palette.indices(&document.catalog);
            let kinds: Vec<_> = document.catalog.visible().collect();
            let output = scroll.show(ui, |ui| {
                touch_scroll_area(ui, scroll_id);
                for index in indices {
                    let kind = kinds[index];
                    let texture = *images.entry(kind.sprite.clone()).or_insert_with(|| {
                        contexts.add_image(bevy_egui::EguiTextureHandle::Strong(
                            if kind.sprite.is_empty() {
                                Handle::<Image>::default()
                            } else {
                                assets.load(format!("textures/parts/{}", kind.sprite))
                            },
                        ))
                    });
                    let size = render::image_size(&kind.sprite, &assets, &image_assets)
                        .unwrap_or_else(|| render::fallback_size(Some(kind)));
                    let count = counts.get(&kind.id).copied().unwrap_or(0);
                    let limit = kind
                        .max_occurrences
                        .map(|limit| format!(" {count}/{limit}"))
                        .unwrap_or_default();
                    let width = ui.available_width();
                    let response = ui.push_id(("palette_part", &kind.id), |ui| {
                        part_row(
                            ui,
                            texture,
                            size,
                            format!("{}{limit}", kind.name),
                            index == cursor.catalog_index,
                            width,
                        )
                    }).inner
                    .on_hover_text(format!("{}\n点击选择，再点击画布放置；也可直接拖到画布。\n{}", kind.name, kind.description))
                    .on_disabled_hover_text("请先应用或取消属性草稿，或处理文件确认，再选择部件。");
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
                        if compact { state.drawer = None; }
                    }
                }
            });
            state.palette_offset = output.state.offset.y;
            ui.separator();
            if let Some(kind) = kinds.get(cursor.catalog_index) {
                ui.strong(&kind.name);
                ui.label(format!("质量 {:.2}", kind.mass));
                ui.add(egui::Label::new(&kind.description).truncate())
                    .on_hover_text(&kind.description);
            }
            ui.horizontal_wrapped(|ui| {
                for (action, label) in [(PanelButton::Rotate, "旋转"), (PanelButton::FlipX, "镜像 X"), (PanelButton::FlipY, "镜像 Y")] {
                    button(ui, action, false, label, &mut state, &mut actions);
                }
            });
            ui.small("R 旋转 · X/Y 镜像")
                .on_hover_text("选部件后点击画布放置，或将目录部件拖到画布。R 旋转；X/Y 镜像；Tab 切换部件；Esc / 右键取消放置。");
            if cursor.placing {
                button(
                    ui,
                    PanelButton::CancelPlacement,
                    false,
                    "取消放置",
                    &mut state,
                    &mut actions,
                );
            }
            };
            if compact {
                egui::ScrollArea::vertical().id_salt("palette_controls_scroll")
                    .auto_shrink([false, false]).show(ui, content);
            } else { content(ui); }
        });
        state.areas.push(right.response.rect);
        if enabled {
            state.palette_area = Some(right.response.rect);
        }
    }
    let mut canvas = viewport_ui.available_rect_before_wrap();
    if !compact {
        canvas.min.y = canvas.min.y.max(220.0);
    }
    state.canvas_area = Some(canvas);
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
        let Some(rect) = state.areas.last() else {
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
