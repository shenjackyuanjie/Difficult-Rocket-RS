use super::*;
use std::path::{Path, PathBuf};

pub(crate) mod browser_smoke;
pub mod egui_panel;
pub(crate) mod smoke;

#[derive(Component)]
pub(crate) struct EditorPanel;
#[derive(Resource, Default)]
pub(crate) struct UiPointer {
    pub blocked: bool,
}

#[derive(Resource, Default)]
pub(crate) struct Palette {
    category: Option<String>,
}

impl Palette {
    fn indices(&self, catalog: &PartCatalog) -> Vec<usize> {
        catalog
            .visible()
            .enumerate()
            .filter(|(_, kind)| {
                self.category
                    .as_deref()
                    .is_none_or(|category| category == category_name(kind))
            })
            .map(|(index, _)| index)
            .collect()
    }
}

fn category_name(kind: &dr_core::PartType) -> &str {
    if kind.category.is_empty() {
        "常规"
    } else {
        &kind.category
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
    Category(Option<String>),
    Open(PathBuf),
    Folder,
    Refresh,
    CancelPlacement,
}

pub(crate) fn pointer_over_ui(
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    panels: Query<(&ComputedNode, &UiGlobalTransform), With<EditorPanel>>,
    mut pointer: ResMut<UiPointer>,
    egui_state: Option<Res<egui_panel::UiState>>,
) {
    let position = windows
        .single()
        .ok()
        .and_then(Window::physical_cursor_position);
    let blocked = position.is_some_and(|position| {
        let over_egui = egui_state.as_ref().is_some_and(|state| {
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
    });
    if pointer.blocked != blocked {
        pointer.blocked = blocked;
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn panel_actions(
    mut buttons: MessageReader<PanelButton>,
    mut palette: ResMut<Palette>,
    mut browser: ResMut<ShipBrowser>,
    mut cursor: ResMut<EditorCursor>,
    mut drag: ResMut<DragState>,
    document: Res<EditorDocument>,
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
                    cursor.flip_x = false;
                    cursor.flip_y = false;
                    drag.cancel();
                }
            }
            PanelButton::Category(category) => {
                palette.category = category.clone();
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
            PanelButton::CancelPlacement => cursor.placing = false,
        }
    }
}

/// Tab 沿当前分类循环；选择状态与鼠标目录共用。
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
    cursor.flip_x = false;
    cursor.flip_y = false;
    cursor.placing = true;
    cursor.palette_drag = false;
    cursor.paste = None;
}

#[cfg(test)]
pub(crate) mod tests;
