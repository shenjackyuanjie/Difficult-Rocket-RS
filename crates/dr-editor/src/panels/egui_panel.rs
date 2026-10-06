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
    let response = ui.add_sized(
        [width, 52.0],
        egui::Button::new("")
            .selected(selected)
            .sense(egui::Sense::click_and_drag()),
    );
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

#[derive(Resource, Default)]
pub struct UiState {
    pub hits: Vec<(PanelButton, egui::Rect)>,
    pub areas: Vec<egui::Rect>,
    pub pixels_per_point: f32,
    pub scrolling: bool,
    pub browser_offset: f32,
    pub palette_offset: f32,
    pub browser_rows: usize,
    pub browser_scroll: Option<f32>,
    pub palette_scroll: Option<f32>,
    folder: PathBuf,
    category: Option<String>,
    selection: Option<usize>,
}

fn button(
    ui: &mut egui::Ui,
    action: PanelButton,
    selected: bool,
    text: impl Into<egui::WidgetText>,
    state: &mut UiState,
    actions: &mut MessageWriter<PanelButton>,
) {
    let response = ui.add(egui::Button::new(text).selected(selected));
    hit(ui, &response, action.clone(), state);
    if response.clicked() {
        response.surrender_focus();
        actions.write(action);
    }
}

fn hit(ui: &egui::Ui, response: &egui::Response, action: PanelButton, state: &mut UiState) {
    let rect = response.rect.intersect(ui.clip_rect());
    if response.enabled() && rect.is_positive() {
        state.hits.push((action, rect));
    }
}

#[allow(clippy::too_many_arguments)]
pub fn draw(
    mut contexts: EguiContexts,
    assets: Res<AssetServer>,
    image_assets: Res<Assets<Image>>,
    document: Res<EditorDocument>,
    paths: Res<EditorPaths>,
    palette: Res<Palette>,
    mut browser: ResMut<ShipBrowser>,
    cursor: Res<EditorCursor>,
    inspector: Res<properties::Inspector>,
    pending: Option<Res<files::PendingFileAction>>,
    mut topology: ResMut<crate::topology_ui::ConnectionEditor>,
    mut options: ResMut<view::ViewOptions>,
    mut state: ResMut<UiState>,
    mut actions: MessageWriter<PanelButton>,
    mut images: Local<std::collections::HashMap<String, egui::TextureId>>,
    mut counts: Local<std::collections::HashMap<String, usize>>,
) {
    let Ok(ctx) = contexts.ctx_mut() else { return };
    let mut viewport_ui = egui::Ui::new(
        ctx.clone(),
        egui::Id::new("directory_viewport"),
        egui::UiBuilder::new()
            .layer_id(egui::LayerId::background())
            .max_rect(ctx.viewport_rect()),
    );
    let ctx = ctx.clone();
    state.hits.clear();
    state.areas.clear();
    state.pixels_per_point = ctx.pixels_per_point();
    state.scrolling = ctx.input(|input| input.is_scrolling());
    let enabled = !inspector.is_open() && !pending.is_some_and(|state| state.is_blocked());
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
    // 给仍由 Bevy 绘制的文件工具栏留出空间。
    egui::Panel::top("directory_toolbar_space")
        .exact_size(52.0)
        .frame(egui::Frame::NONE)
        .show(&mut viewport_ui, |_| {});
    let left = egui::Panel::left("ship_browser_sidebar")
        .exact_size(248.0)
        .resizable(false)
        .show(&mut viewport_ui, |ui| {
            if !enabled {
                ui.disable();
            }
            ui.heading("船体列表");
            let folder = browser
                .folder
                .file_name()
                .unwrap_or(browser.folder.as_os_str())
                .to_string_lossy();
            ui.label(format!("{folder} · {} 个船体", browser.files.len()));
            ui.horizontal(|ui| {
                button(
                    ui,
                    PanelButton::Folder,
                    false,
                    "切换目录",
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
                ui.label(format!("已跳过 {} 个异常 XML", browser.rejected));
            }
            ui.separator();
            let mut scroll = egui::ScrollArea::vertical()
                .id_salt("ship_browser_files")
                .auto_shrink([false, false]);
            if let Some(offset) = state.browser_scroll.take() {
                scroll = scroll.vertical_scroll_offset(offset);
            }
            let row_height = 28.0;
            let output = scroll.show_rows(ui, row_height, browser.files.len(), |ui, rows| {
                state.browser_rows = rows.len();
                for index in rows {
                    let path = &browser.files[index];
                    let text = path.file_stem().unwrap_or_default().to_string_lossy();
                    let response = ui
                        .add_sized(
                            [ui.available_width(), row_height],
                            egui::Button::new(text.as_ref())
                                .selected(current.as_ref() == Some(path))
                                .truncate(),
                        )
                        .on_hover_text(path.display().to_string());
                    hit(ui, &response, PanelButton::Open(path.clone()), &mut state);
                    if response.clicked() {
                        response.surrender_focus();
                        actions.write(PanelButton::Open(path.clone()));
                    }
                }
            });
            state.browser_offset = output.state.offset.y;
        });
    state.areas.push(left.response.rect);
    let right = egui::Panel::right("part_palette_sidebar")
        .exact_size(296.0)
        .resizable(false)
        .show(&mut viewport_ui, |ui| {
            if !enabled {
                ui.disable();
            }
            ui.heading("部件目录");
            if ui.button("连接树 / 连接图 (F6)").clicked() {
                topology.open = true;
            }
            ui.horizontal_wrapped(|ui| {
                ui.label("框选按键");
                ui.selectable_value(
                    &mut options.box_select_button,
                    view::BoxSelectButton::Middle,
                    "中键",
                );
                ui.selectable_value(
                    &mut options.box_select_button,
                    view::BoxSelectButton::Left,
                    "左键",
                );
            });
            ui.small(match options.box_select_button {
                view::BoxSelectButton::Middle => "左键拖空白：移动视角 · 中键拖动：框选",
                view::BoxSelectButton::Left => "中键拖动：移动视角 · 左键拖空白：框选",
            });
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
            let mut scroll = egui::ScrollArea::vertical()
                .id_salt("part_palette_rows")
                .max_height((ui.available_height() - 215.0).max(50.0))
                .auto_shrink([false, false]);
            if let Some(offset) = state.palette_scroll.take() {
                scroll = scroll.vertical_scroll_offset(offset);
            }
            let indices = palette.indices(&document.catalog);
            let kinds: Vec<_> = document.catalog.visible().collect();
            let output = scroll.show(ui, |ui| {
                for index in indices {
                    let kind = kinds[index];
                    let texture = *images.entry(kind.sprite.clone()).or_insert_with(|| {
                        contexts.add_image(bevy_egui::EguiTextureHandle::Strong(
                            assets.load(format!("textures/parts/{}", kind.sprite)),
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
                    let response = part_row(
                        ui,
                        texture,
                        size,
                        format!("{}{limit}", kind.name),
                        index == cursor.catalog_index,
                        width,
                    )
                    .on_hover_text(&kind.description);
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
            ui.label("选部件后点击画布放置\nR 旋转 · X/Y 镜像\nTab 切换 · Esc/右键取消");
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
        });
    state.areas.push(right.response.rect);
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
