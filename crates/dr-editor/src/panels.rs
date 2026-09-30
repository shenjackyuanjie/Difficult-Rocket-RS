use super::*;
use std::path::{Path, PathBuf};

pub(crate) mod smoke;

#[derive(Component)]
pub(crate) struct EditorPanel;
#[derive(Component)]
pub(crate) struct ScrollArea;
#[derive(Component)]
pub(crate) struct PaletteRoot;
#[derive(Component)]
pub(crate) struct BrowserRoot;

#[derive(Component)]
pub(crate) struct PaletteScroll;
#[derive(Component)]
pub(crate) struct BrowserScroll;

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

#[derive(Component, Clone)]
pub(crate) enum PanelButton {
    Part(usize),
    Category(Option<String>),
    Open(PathBuf),
    Folder,
    Refresh,
    CancelPlacement,
}

fn text_style(value: impl Into<String>, font: &Handle<Font>, size: f32) -> impl Bundle {
    (
        Text::new(value),
        TextFont {
            font: bevy::text::FontSource::Handle(font.clone()),
            font_size: FontSize::Px(size),
            ..default()
        },
        TextColor(Color::srgb(0.9, 0.94, 1.0)),
    )
}

fn button_style(action: PanelButton, selected: bool) -> impl Bundle {
    let part_row = matches!(action, PanelButton::Part(_));
    let file_row = matches!(action, PanelButton::Open(_));
    (
        Button,
        Node {
            padding: UiRect::all(px(6)),
            min_height: px(30),
            height: if part_row {
                px(52)
            } else if file_row {
                px(30)
            } else {
                Val::Auto
            },
            overflow: Overflow::clip(),
            flex_shrink: 0.0,
            ..default()
        },
        BackgroundColor(if selected {
            Color::srgb(0.2, 0.42, 0.53)
        } else {
            Color::srgb(0.12, 0.17, 0.23)
        }),
        action,
    )
}

pub(crate) fn pointer_over_ui(
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    panels: Query<(&ComputedNode, &UiGlobalTransform), With<EditorPanel>>,
    mut pointer: ResMut<UiPointer>,
) {
    let position = windows
        .single()
        .ok()
        .and_then(Window::physical_cursor_position);
    let blocked = position.is_some_and(|position| {
        panels
            .iter()
            .any(|(node, transform)| node.contains_point(*transform, position))
    });
    if pointer.blocked != blocked {
        pointer.blocked = blocked;
    }
}

pub(crate) fn scroll_panels(
    mut wheels: MessageReader<MouseWheel>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mut areas: Query<
        (
            &ComputedNode,
            &UiGlobalTransform,
            &mut ScrollPosition,
            Has<properties::InspectorScroll>,
        ),
        With<ScrollArea>,
    >,
    inspector: Option<Res<properties::Inspector>>,
) {
    let position = windows
        .single()
        .ok()
        .and_then(Window::physical_cursor_position);
    let modal = !properties::closed(inspector);
    for wheel in wheels.read() {
        let Some(position) = position else {
            continue;
        };
        for (node, transform, mut scroll, is_inspector) in &mut areas {
            if modal && !is_inspector {
                continue;
            }
            if node.contains_point(*transform, position) {
                let delta = match wheel.unit {
                    MouseScrollUnit::Line => wheel.y * 40.0,
                    MouseScrollUnit::Pixel => wheel.y,
                };
                let max = ((node.content_size().y - node.size().y) * node.inverse_scale_factor())
                    .max(0.0);
                scroll.y = (scroll.y - delta).clamp(0.0, max);
            }
        }
    }
}

pub(crate) fn panel_actions(
    buttons: Query<(&Interaction, &PanelButton), Changed<Interaction>>,
    mut palette: ResMut<Palette>,
    mut browser: ResMut<ShipBrowser>,
    mut cursor: ResMut<EditorCursor>,
    mut drag: ResMut<DragState>,
    document: Res<EditorDocument>,
    mut files: MessageWriter<files::FileAction>,
) {
    for (interaction, action) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match action {
            PanelButton::Part(index) => {
                if document.catalog.visible().nth(*index).is_some() {
                    cursor.catalog_index = *index;
                    cursor.placing = true;
                    cursor.rotation = 0;
                    cursor.flip_x = false;
                    cursor.flip_y = false;
                    drag.id = None;
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
                cursor.placing = false;
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
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn render_palette(
    mut commands: Commands,
    document: Res<EditorDocument>,
    palette: Res<Palette>,
    cursor: Res<EditorCursor>,
    assets: Res<AssetServer>,
    roots: Query<Entity, With<PaletteRoot>>,
    scrolling: Query<(&ScrollPosition, &ComputedNode), With<PaletteScroll>>,
    mut last_selection: Local<Option<(usize, bool)>>,
) {
    let selection = (cursor.catalog_index, cursor.placing);
    if !document.is_changed() && !palette.is_changed() && *last_selection == Some(selection) {
        return;
    }
    let index_changed = last_selection
        .as_ref()
        .is_none_or(|previous| previous.0 != selection.0);
    *last_selection = Some(selection);
    let mut offset = if palette.is_changed() {
        0.0
    } else {
        scrolling
            .single()
            .map(|(scroll, _)| scroll.y)
            .unwrap_or(0.0)
    };
    if index_changed
        && let Some(index) = palette
            .indices(&document.catalog)
            .iter()
            .position(|index| *index == cursor.catalog_index)
        && let Ok((_, node)) = scrolling.single()
    {
        let height = node.size().y * node.inverse_scale_factor();
        let top = index as f32 * 56.0;
        if top < offset {
            offset = top;
        } else if top + 56.0 > offset + height {
            offset = (top + 56.0 - height).max(0.0);
        }
    }
    for entity in &roots {
        commands.entity(entity).despawn();
    }
    let font = assets.load("fonts/HarmonyOS_Sans/HarmonyOS_Sans_SC/HarmonyOS_Sans_SC_Regular.ttf");
    let mut categories: Vec<_> = document
        .catalog
        .visible()
        .map(|kind| category_name(kind).to_owned())
        .collect();
    categories.sort();
    categories.dedup();
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: px(16),
                top: px(60),
                bottom: px(16),
                width: px(280),
                padding: UiRect::all(px(10)),
                row_gap: px(8),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            BackgroundColor(Color::srgba(0.035, 0.055, 0.08, 0.97)),
            EditorPanel,
            PaletteRoot,
            Name::new("部件目录"),
        ))
        .with_children(|root| {
            root.spawn(text_style("部件目录", &font, 20.0));
            root.spawn((Node {
                flex_wrap: FlexWrap::Wrap,
                column_gap: px(4),
                row_gap: px(4),
                ..default()
            },))
                .with_children(|tabs| {
                    tabs.spawn(button_style(
                        PanelButton::Category(None),
                        palette.category.is_none(),
                    ))
                    .with_children(|button| {
                        button.spawn(text_style("全部", &font, 14.0));
                    });
                    for category in categories {
                        tabs.spawn(button_style(
                            PanelButton::Category(Some(category.clone())),
                            palette.category.as_ref() == Some(&category),
                        ))
                        .with_children(|button| {
                            button.spawn(text_style(category, &font, 14.0));
                        });
                    }
                });
            root.spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    flex_grow: 1.0,
                    min_height: px(0),
                    overflow: Overflow::scroll_y(),
                    row_gap: px(4),
                    ..default()
                },
                ScrollArea,
                ScrollPosition(Vec2::new(0.0, offset)),
                PaletteScroll,
            ))
            .with_children(|list| {
                for index in palette.indices(&document.catalog) {
                    let kind = document.catalog.visible().nth(index).unwrap();
                    list.spawn(button_style(
                        PanelButton::Part(index),
                        cursor.catalog_index == index,
                    ))
                    .with_children(|row| {
                        row.spawn((
                            ImageNode::new(assets.load(format!("textures/parts/{}", kind.sprite))),
                            Node {
                                width: px(40),
                                height: px(40),
                                margin: UiRect::right(px(8)),
                                flex_shrink: 0.0,
                                ..default()
                            },
                        ));
                        let count = document.ship.count_type(&kind.id);
                        let limit = kind
                            .max_occurrences
                            .map(|limit| format!(" {count}/{limit}"))
                            .unwrap_or_default();
                        row.spawn((
                            text_style(format!("{}{limit}", kind.name), &font, 14.0),
                            Node {
                                flex_shrink: 1.0,
                                min_width: px(0),
                                width: px(190),
                                ..default()
                            },
                        ));
                    });
                }
            });
            if let Some(kind) = document.catalog.visible().nth(cursor.catalog_index) {
                root.spawn((
                    text_style(
                        format!("{}\n质量 {:.2}\n{}", kind.name, kind.mass, kind.description),
                        &font,
                        14.0,
                    ),
                    Node {
                        max_height: px(155),
                        overflow: Overflow::clip(),
                        flex_shrink: 0.0,
                        ..default()
                    },
                ));
            }
            root.spawn(text_style(
                "选部件后点击画布放置\nR 旋转 · X/Y 镜像\nTab 切换 · Esc/右键取消",
                &font,
                14.0,
            ));
            if cursor.placing {
                root.spawn(button_style(PanelButton::CancelPlacement, false))
                    .with_children(|button| {
                        button.spawn(text_style("取消放置", &font, 14.0));
                    });
            }
        });
}

pub(crate) fn render_browser(
    mut commands: Commands,
    mut browser: ResMut<ShipBrowser>,
    paths: Res<EditorPaths>,
    assets: Res<AssetServer>,
    roots: Query<Entity, With<BrowserRoot>>,
    scrolling: Query<&ScrollPosition, With<BrowserScroll>>,
) {
    if !browser.is_changed() && !paths.is_changed() {
        return;
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
    let offset = if browser.is_changed() {
        0.0
    } else {
        scrolling.single().map(|scroll| scroll.y).unwrap_or(0.0)
    };
    for entity in &roots {
        commands.entity(entity).despawn();
    }
    let font = assets.load("fonts/HarmonyOS_Sans/HarmonyOS_Sans_SC/HarmonyOS_Sans_SC_Regular.ttf");
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(16),
                top: px(220),
                bottom: px(16),
                width: px(232),
                padding: UiRect::all(px(10)),
                row_gap: px(8),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            BackgroundColor(Color::srgba(0.035, 0.055, 0.08, 0.97)),
            EditorPanel,
            BrowserRoot,
            Name::new("船体列表"),
        ))
        .with_children(|root| {
            root.spawn(text_style("船体列表", &font, 20.0));
            let folder = browser
                .folder
                .file_name()
                .unwrap_or(browser.folder.as_os_str())
                .to_string_lossy();
            root.spawn(text_style(
                format!("{} · {} 个船体", folder, browser.files.len()),
                &font,
                14.0,
            ));
            root.spawn((Node {
                column_gap: px(6),
                ..default()
            },))
                .with_children(|row| {
                    for (label, action) in [
                        ("切换目录", PanelButton::Folder),
                        ("刷新", PanelButton::Refresh),
                    ] {
                        row.spawn(button_style(action, false))
                            .with_children(|button| {
                                button.spawn(text_style(label, &font, 14.0));
                            });
                    }
                });
            if let Some(error) = &browser.error {
                root.spawn(text_style(error, &font, 14.0));
            }
            if browser.rejected > 0 {
                root.spawn(text_style(
                    format!("已跳过 {} 个异常 XML", browser.rejected),
                    &font,
                    13.0,
                ));
            }
            root.spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    flex_grow: 1.0,
                    min_height: px(0),
                    overflow: Overflow::scroll_y(),
                    row_gap: px(4),
                    ..default()
                },
                ScrollArea,
                ScrollPosition(Vec2::new(0.0, offset)),
                BrowserScroll,
            ))
            .with_children(|list| {
                for path in &browser.files {
                    let selected = current.as_ref() == Some(path);
                    list.spawn(button_style(PanelButton::Open(path.clone()), selected))
                        .with_children(|button| {
                            button.spawn((
                                text_style(
                                    path.file_stem().unwrap_or_default().to_string_lossy(),
                                    &font,
                                    14.0,
                                ),
                                TextLayout::default().with_no_wrap(),
                                Node {
                                    min_width: px(0),
                                    max_width: px(190),
                                    width: px(190),
                                    ..default()
                                },
                            ));
                        });
                }
            });
        });
}

#[cfg(test)]
mod tests;
