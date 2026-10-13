use super::*;
use std::path::{Path, PathBuf};

pub(crate) mod browser_smoke;
pub mod egui_panel;
pub(crate) mod smoke;

#[derive(Component)]
pub(crate) struct EditorPanel;
/// 原生顶栏只调整布局，不引入另一套移动界面。
#[derive(Component, Clone, Copy)]
pub(crate) enum ToolbarItem {
    Files,
    Properties,
    Help,
    Status,
}

pub(crate) fn compact(width: f32) -> bool {
    width < 960.0
}

pub(crate) fn file_toolbar_width(width: f32) -> f32 {
    (width - 32.0).clamp(160.0, 476.0)
}

pub(crate) fn file_toolbar_height(width: f32) -> f32 {
    let available = file_toolbar_width(width);
    let mut rows = 1;
    let mut used = 0.0;
    for item in [84.0, 84.0, 84.0, 116.0, 84.0] {
        let next = if used == 0.0 { item } else { used + 6.0 + item };
        if next > available {
            rows += 1;
            used = item;
        } else {
            used = next;
        }
    }
    rows as f32 * 44.0 + (rows - 1) as f32 * 6.0
}

pub(crate) fn toolbar_height(width: f32) -> f32 {
    if compact(width) {
        12.0 + file_toolbar_height(width) + 6.0 + 44.0 + 12.0
    } else {
        64.0
    }
}

pub(crate) fn layout_toolbars(
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mut items: Query<(&ToolbarItem, &mut Node)>,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    let width = window.width();
    for (item, mut node) in &mut items {
        let mut next = node.clone();
        let top = if compact(width) {
            12.0 + file_toolbar_height(width) + 6.0
        } else {
            12.0
        };
        match item {
            ToolbarItem::Files => {
                // 复用文件栏容器承载整条原生背景，按钮仍由左右 16 像素内边距排布。
                next.width = px(width);
                next.height = px(toolbar_height(width));
                next.top = px(0);
                next.left = px(0);
                next.flex_wrap = FlexWrap::Wrap;
                next.row_gap = px(6);
            }
            ToolbarItem::Properties => {
                next.top = px(top);
                next.left = px(if compact(width) { 16.0 } else { 500.0 });
                next.width = px(172);
                next.min_height = px(44);
            }
            ToolbarItem::Help => {
                next.top = px(top);
                next.right = px(16);
                next.width = px(100);
                next.min_height = px(44);
            }
            ToolbarItem::Status => {
                next.display = if compact(width) {
                    Display::None
                } else {
                    Display::Flex
                };
                next.top = px(126);
            }
        }
        node.set_if_neq(next);
    }
}

#[derive(Resource, Default)]
pub(crate) struct UiPointer {
    pub blocked: bool,
    pub palette_drop: bool,
}

#[derive(Resource, Default)]
pub(crate) struct Palette {
    collapsed: BTreeSet<String>,
}

impl Palette {
    /// 分组仅用于显示，保留目录中的可见索引及每组原始顺序，不修改 XML category。
    fn groups<'a>(&self, catalog: &'a PartCatalog) -> Vec<(&'a str, Vec<usize>)> {
        let mut groups = std::collections::BTreeMap::<&str, Vec<usize>>::new();
        for (index, kind) in catalog.visible().enumerate() {
            groups.entry(category_name(kind)).or_default().push(index);
        }
        let mut groups: Vec<_> = groups.into_iter().collect();
        groups.sort_by_key(|(name, _)| category_order(name));
        groups
    }

    fn indices(&self, catalog: &PartCatalog) -> Vec<usize> {
        self.groups(catalog)
            .into_iter()
            .filter(|(name, _)| !self.collapsed.contains(*name))
            .flat_map(|(_, indices)| indices)
            .collect()
    }
}

fn category_name(kind: &dr_core::PartType) -> &str {
    if !kind.category.is_empty() {
        return &kind.category;
    }
    match kind.kind {
        PartKind::Tank => "Fuel Tanks",
        PartKind::Engine => "Engines",
        _ => "Gizmos",
    }
}

fn category_order(name: &str) -> (u8, &str) {
    let order = match name {
        "Gizmos" => 0,
        "Fuel Tanks" => 1,
        "Engines" => 2,
        "Satellite" => 3,
        _ => 4,
    };
    (order, name)
}

fn category_label(name: &str) -> &str {
    match name {
        "Gizmos" => "通用组件",
        "Fuel Tanks" => "燃料罐",
        "Engines" => "发动机",
        "Satellite" => "卫星部件",
        _ => name,
    }
}

#[derive(Resource, Default)]
pub(crate) struct ShipBrowser {
    folder: PathBuf,
    files: Vec<PathBuf>,
    rejected: usize,
    error: Option<String>,
}

impl ShipBrowser {
    pub fn new(folder: PathBuf) -> Self {
        let mut browser = Self::default();
        browser.set_folder(folder);
        browser
    }

    fn set_folder(&mut self, folder: PathBuf) {
        match read_ship_folder(&folder) {
            Ok((files, rejected)) => {
                self.folder = folder;
                self.files = files;
                self.rejected = rejected;
                self.error = None;
            }
            Err(error) => self.error = Some(format!("无法读取目录：{error}")),
        }
    }
}

fn read_ship_folder(folder: &Path) -> std::io::Result<(Vec<PathBuf>, usize)> {
    let mut files = Vec::new();
    let mut rejected = 0;
    for entry in std::fs::read_dir(folder)? {
        let path = entry?.path();
        if !path.is_file()
            || !path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("xml"))
        {
            continue;
        }
        if load_ship(&path).is_ok() {
            files.push(std::fs::canonicalize(path)?);
        } else {
            rejected += 1;
        }
    }
    files.sort_by_key(|path| {
        path.file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_lowercase()
    });
    Ok((files, rejected))
}

#[derive(Message, Clone, Debug, PartialEq)]
pub enum PanelButton {
    Part(usize),
    DragPart(usize),
    ToggleCategory(String),
    ExpandCategories,
    Open(PathBuf),
    Folder,
    Refresh,
    CancelPlacement,
    Undo,
    Redo,
    Delete,
    SelectAll,
    Copy,
    Cut,
    Paste,
    Rotate,
    FlipX,
    FlipY,
}

pub(crate) fn over_ui(
    position: Vec2,
    panels: &Query<(&ComputedNode, &UiGlobalTransform), With<EditorPanel>>,
    egui_state: Option<&egui_panel::UiState>,
) -> bool {
    let over_egui = egui_state.is_some_and(|state| {
        let point = position / state.pixels_per_point.max(f32::EPSILON);
        state
            .areas
            .iter()
            .any(|rect| rect.contains(bevy_egui::egui::pos2(point.x, point.y)))
    });
    over_egui
        || panels
            .iter()
            .any(|(node, transform)| node.contains_point(*transform, position))
}

pub(crate) fn pointer_over_ui(
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    panels: Query<(&ComputedNode, &UiGlobalTransform), With<EditorPanel>>,
    mut pointer: ResMut<UiPointer>,
    egui_state: Option<Res<egui_panel::UiState>>,
    touch: Option<Res<mobile_input::GestureInput>>,
) {
    let touch = touch.as_deref().filter(|input| input.override_mouse);
    let position = windows.single().ok().and_then(|window| {
        touch.map_or_else(
            || window.physical_cursor_position(),
            |input| {
                input
                    .position
                    .map(|position| position * window.scale_factor())
            },
        )
    });
    let blocked = touch.is_some_and(|input| input.blocked())
        || position.is_some_and(|position| over_ui(position, &panels, egui_state.as_deref()));
    let palette_drop = position.is_some_and(|position| {
        egui_state.as_ref().is_some_and(|state| {
            let point = position / state.pixels_per_point.max(f32::EPSILON);
            state
                .palette_area
                .is_some_and(|rect| rect.contains(bevy_egui::egui::pos2(point.x, point.y)))
        })
    }) && !touch.is_some_and(|input| input.blocked());
    if pointer.blocked != blocked {
        pointer.blocked = blocked;
    }
    if pointer.palette_drop != palette_drop {
        pointer.palette_drop = palette_drop;
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn panel_actions(
    mut buttons: MessageReader<PanelButton>,
    mut palette: ResMut<Palette>,
    mut browser: ResMut<ShipBrowser>,
    mut cursor: ResMut<EditorCursor>,
    mut drag: ResMut<DragState>,
    mut document: ResMut<EditorDocument>,
    mut files: MessageWriter<files::FileAction>,
    pending: Option<Res<files::PendingFileAction>>,
    help: Option<Res<help::HelpState>>,
) {
    if help.is_some_and(|state| state.open || state.suppress_frame)
        || pending.is_some_and(|state| state.is_blocked())
    {
        buttons.clear();
        return;
    }
    for action in buttons.read() {
        match action {
            PanelButton::Part(index) | PanelButton::DragPart(index) => {
                if document.catalog.visible().nth(*index).is_some() {
                    cursor.catalog_index = *index;
                    cursor.placing = true;
                    cursor.palette_drag = matches!(action, PanelButton::DragPart(_));
                    cursor.valid = false;
                    cursor.paste = None;
                    cursor.rotation = 0;
                    cursor.fine_rotation = 0.0;
                    cursor.manual_connection = None;
                    cursor.flip_x = false;
                    cursor.flip_y = false;
                    drag.cancel();
                }
            }
            PanelButton::ToggleCategory(_) | PanelButton::ExpandCategories => {
                if let PanelButton::ToggleCategory(category) = action {
                    if !palette.collapsed.remove(category) {
                        palette.collapsed.insert(category.clone());
                    }
                } else {
                    palette.collapsed.clear();
                }
                let indices = palette.indices(&document.catalog);
                if !indices.contains(&cursor.catalog_index)
                    && let Some(index) = indices.first()
                {
                    cursor.catalog_index = *index;
                }
                cursor.cancel_placement();
                cursor.paste = None;
            }
            PanelButton::Open(path) => {
                files.write(files::FileAction::Open(path.clone()));
            }
            PanelButton::Folder => {
                if let Some(path) = rfd::FileDialog::new()
                    .set_title("选择船体目录")
                    .set_directory(&browser.folder)
                    .pick_folder()
                {
                    browser.set_folder(path);
                }
            }
            PanelButton::Refresh => {
                let folder = browser.folder.clone();
                browser.set_folder(folder);
            }
            PanelButton::CancelPlacement => {
                drag.cancel();
                cursor.cancel_placement();
                cursor.paste = None;
                cursor.manual_connection = None;
                cursor.valid = false;
            }
            PanelButton::Undo | PanelButton::Redo => {
                drag.cancel();
                cursor.cancel_placement();
                cursor.paste = None;
                cursor.manual_connection = None;
                if matches!(action, PanelButton::Undo) {
                    document.undo();
                } else {
                    document.redo();
                }
            }
            PanelButton::Delete => {
                let selected = document.selected_keys();
                if !selected.is_empty()
                    && drag.id.is_none()
                    && document.execute(EditorCommand::DeleteSelection(selected))
                {
                    document.clear_selection();
                }
            }
            PanelButton::SelectAll | PanelButton::Copy | PanelButton::Cut | PanelButton::Paste => {
                if drag.id.is_some() || drag.rectangle.is_some() {
                    continue;
                }
                let mut keys = ButtonInput::default();
                keys.press(KeyCode::ControlLeft);
                keys.press(match action {
                    PanelButton::SelectAll => KeyCode::KeyA,
                    PanelButton::Copy => KeyCode::KeyC,
                    PanelButton::Cut => KeyCode::KeyX,
                    PanelButton::Paste => KeyCode::KeyV,
                    _ => unreachable!(),
                });
                selection::keyboard(&mut document, &mut cursor, &keys, false);
                if matches!(action, PanelButton::Paste) && cursor.paste.is_some() {
                    cursor.manual_connection = None;
                    document.status =
                        "粘贴预览：指针定位，鼠标单击或触摸抬指放置，取消按钮 / Esc 取消".into();
                }
            }
            PanelButton::Rotate | PanelButton::FlipX | PanelButton::FlipY => {
                if drag.id.is_some() || drag.rectangle.is_some() {
                    continue;
                }
                let mut keys = ButtonInput::default();
                keys.press(match action {
                    PanelButton::Rotate => KeyCode::KeyR,
                    PanelButton::FlipX => KeyCode::KeyX,
                    PanelButton::FlipY => KeyCode::KeyY,
                    _ => unreachable!(),
                });
                if selection::keyboard(&mut document, &mut cursor, &keys, false) {
                    continue;
                }
                if cursor.placing {
                    match action {
                        PanelButton::Rotate => {
                            cursor.rotation = (cursor.rotation + 1).rem_euclid(4)
                        }
                        PanelButton::FlipX => cursor.flip_x = !cursor.flip_x,
                        PanelButton::FlipY => cursor.flip_y = !cursor.flip_y,
                        _ => unreachable!(),
                    }
                } else if let Some(id) = document.selected {
                    if matches!(action, PanelButton::Rotate)
                        && !document
                            .ship
                            .part_at(id)
                            .and_then(|part| document.catalog.get(&part.part_type))
                            .is_some_and(|kind| !kind.disable_editor_rotation)
                    {
                        continue;
                    }
                    let command = match action {
                        PanelButton::Rotate => EditorCommand::Rotate(id.id),
                        PanelButton::FlipX => EditorCommand::FlipX(id.id),
                        PanelButton::FlipY => EditorCommand::FlipY(id.id),
                        _ => unreachable!(),
                    };
                    placement::transform(&mut document, id, command);
                }
            }
        }
    }
}

/// Tab 按目录的分组顺序循环，跳过已收起的组；选择状态与鼠标目录共用。
pub(crate) fn cycle_part(
    document: &EditorDocument,
    palette: &Palette,
    cursor: &mut EditorCursor,
    reverse: bool,
) {
    let indices = palette.indices(&document.catalog);
    if indices.is_empty() {
        return;
    }
    let current = indices
        .iter()
        .position(|index| *index == cursor.catalog_index)
        .unwrap_or(0);
    let next = if reverse {
        (current + indices.len() - 1) % indices.len()
    } else {
        (current + 1) % indices.len()
    };
    cursor.catalog_index = indices[next];
    cursor.rotation = 0;
    cursor.fine_rotation = 0.0;
    cursor.manual_connection = None;
    cursor.flip_x = false;
    cursor.flip_y = false;
    cursor.placing = true;
    cursor.palette_drag = false;
    cursor.paste = None;
}

#[cfg(test)]
pub(crate) mod tests;
