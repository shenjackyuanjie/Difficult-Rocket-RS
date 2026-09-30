use super::*;
use bevy::window::{FileDragAndDrop, WindowCloseRequested};
use std::path::{Path, PathBuf};

#[derive(Message, Clone, Debug)]
pub(crate) enum FileAction {
    New,
    OpenDialog,
    Open(PathBuf),
    Save,
    SaveAs,
    Exit,
}

#[derive(Clone, Copy)]
enum UnsavedChoice {
    Save,
    Discard,
    Cancel,
}

trait Dialogs {
    fn open(&mut self, paths: &EditorPaths) -> Option<PathBuf>;
    fn save(&mut self, paths: &EditorPaths) -> Option<PathBuf>;
    fn unsaved(&mut self) -> UnsavedChoice;
}

struct NativeDialogs;

fn initial_directory(paths: &EditorPaths) -> PathBuf {
    paths
        .ship
        .as_deref()
        .and_then(|path| Path::new(path).parent())
        .filter(|path| !path.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| Path::new(&paths.assets).join("ships"))
}

impl Dialogs for NativeDialogs {
    fn open(&mut self, paths: &EditorPaths) -> Option<PathBuf> {
        rfd::FileDialog::new()
            .set_title("打开 SR1 船体")
            .add_filter("SR1 船体 XML", &["xml"])
            .set_directory(initial_directory(paths))
            .pick_file()
    }

    fn save(&mut self, paths: &EditorPaths) -> Option<PathBuf> {
        let name = paths
            .ship
            .as_deref()
            .and_then(|path| Path::new(path).file_name())
            .and_then(|name| name.to_str())
            .unwrap_or("未命名.ship.xml");
        rfd::FileDialog::new()
            .set_title("保存 SR1 船体")
            .add_filter("SR1 船体 XML", &["xml"])
            .set_directory(initial_directory(paths))
            .set_file_name(name)
            .save_file()
            .map(|path| {
                if path.extension().is_none() {
                    path.with_extension("xml")
                } else {
                    path
                }
            })
    }

    fn unsaved(&mut self) -> UnsavedChoice {
        match rfd::MessageDialog::new()
            .set_title("保存未保存的修改？")
            .set_description("是：保存后继续\n否：放弃修改并继续\n取消：返回编辑器")
            .set_level(rfd::MessageLevel::Warning)
            .set_buttons(rfd::MessageButtons::YesNoCancel)
            .show()
        {
            rfd::MessageDialogResult::Yes => UnsavedChoice::Save,
            rfd::MessageDialogResult::No => UnsavedChoice::Discard,
            _ => UnsavedChoice::Cancel,
        }
    }
}

fn save_document(
    document: &mut EditorDocument,
    paths: &mut EditorPaths,
    dialogs: &mut impl Dialogs,
    save_as: bool,
) -> bool {
    let path = match (&paths.ship, save_as) {
        (Some(path), false) => PathBuf::from(path),
        _ => {
            let Some(path) = dialogs.save(paths) else {
                return false;
            };
            path
        }
    };
    match save_ship(&path, &document.ship) {
        Ok(()) => {
            paths.ship = Some(path.to_string_lossy().into_owned());
            document.saved_ship = document.ship.clone();
            document.refresh();
            document.status = format!("已保存：{}", path.display());
            true
        }
        Err(error) => {
            document.status = error.to_string();
            false
        }
    }
}

fn can_replace(
    document: &mut EditorDocument,
    paths: &mut EditorPaths,
    dialogs: &mut impl Dialogs,
) -> bool {
    if !document.dirty {
        return true;
    }
    match dialogs.unsaved() {
        UnsavedChoice::Save => save_document(document, paths, dialogs, false),
        UnsavedChoice::Discard => true,
        UnsavedChoice::Cancel => false,
    }
}

/// 所有文件入口共用这一流程；失败或取消不能替换文档、路径与撤销栈。
fn apply_action(
    action: FileAction,
    document: &mut EditorDocument,
    paths: &mut EditorPaths,
    dialogs: &mut impl Dialogs,
) -> bool {
    match action {
        FileAction::Save | FileAction::SaveAs => {
            save_document(
                document,
                paths,
                dialogs,
                matches!(action, FileAction::SaveAs),
            );
        }
        FileAction::Exit => return can_replace(document, paths, dialogs),
        FileAction::New => {
            if can_replace(document, paths, dialogs) {
                replace_document(document, Ship::default());
                paths.ship = None;
                document.status = "已新建船体".into();
            }
        }
        FileAction::OpenDialog => {
            if let Some(path) = dialogs.open(paths) {
                return apply_action(FileAction::Open(path), document, paths, dialogs);
            }
        }
        FileAction::Open(path) => {
            // 先验证输入，避免坏文件触发放弃当前文档的提示。
            match load_ship(&path) {
                Ok(_) => {
                    if can_replace(document, paths, dialogs) {
                        // 可能刚保存到同一路径，不能使用确认前读取的旧内容。
                        match load_ship(&path) {
                            Ok(ship) => {
                                replace_document(document, ship);
                                paths.ship = Some(path.to_string_lossy().into_owned());
                                document.status = format!("已打开：{}", path.display());
                            }
                            Err(error) => {
                                document.status = format!("打开失败（{}）：{error}", path.display())
                            }
                        }
                    }
                }
                Err(error) => document.status = format!("打开失败（{}）：{error}", path.display()),
            }
        }
    }
    false
}

fn replace_document(document: &mut EditorDocument, ship: Ship) {
    document.saved_ship = ship.clone();
    document.ship = ship;
    document.history = EditorHistory::with_limit(256);
    document.selected = None;
    document.refresh();
}

pub(crate) fn file_inputs(
    keys: Res<ButtonInput<KeyCode>>,
    mut drops: MessageReader<FileDragAndDrop>,
    mut closes: MessageReader<WindowCloseRequested>,
    mut actions: MessageWriter<FileAction>,
    mut document: ResMut<EditorDocument>,
) {
    if closes.read().next().is_some() {
        closes.clear();
        actions.write(FileAction::Exit);
        return;
    }
    let dropped: Vec<_> = drops
        .read()
        .filter_map(|event| {
            if let FileDragAndDrop::DroppedFile { path_buf, .. } = event {
                Some(path_buf.clone())
            } else {
                None
            }
        })
        .collect();
    match dropped.as_slice() {
        [path] => {
            actions.write(FileAction::Open(path.clone()));
        }
        [] => {}
        _ => document.status = "一次只能拖入一个船体文件".into(),
    }
    let control = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    if control {
        let action = if keys.just_pressed(KeyCode::KeyN) {
            Some(FileAction::New)
        } else if keys.just_pressed(KeyCode::KeyO) {
            Some(FileAction::OpenDialog)
        } else if keys.just_pressed(KeyCode::KeyS) {
            Some(if shift {
                FileAction::SaveAs
            } else {
                FileAction::Save
            })
        } else {
            None
        };
        if let Some(action) = action {
            actions.write(action);
        }
    }
}

pub(crate) fn file_actions(
    mut actions: MessageReader<FileAction>,
    mut document: ResMut<EditorDocument>,
    mut paths: ResMut<EditorPaths>,
    mut drag: ResMut<DragState>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut exit: MessageWriter<AppExit>,
) {
    for action in actions.read() {
        drag.id = None;
        if apply_action(
            action.clone(),
            &mut document,
            &mut paths,
            &mut NativeDialogs,
        ) {
            exit.write(AppExit::Success);
        }
        // 原生对话框关闭后不让触发按键或释放事件继续编辑画布。
        mouse.reset_all();
        keys.reset_all();
    }
}

#[derive(Component)]
pub(crate) struct FileButton(FileAction);

pub(crate) fn setup_file_toolbar(mut commands: Commands, assets: Res<AssetServer>) {
    let font = assets.load("fonts/HarmonyOS_Sans/HarmonyOS_Sans_SC/HarmonyOS_Sans_SC_Regular.ttf");
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: px(12),
                left: px(16),
                column_gap: px(6),
                ..default()
            },
            Name::new("文件工具栏"),
        ))
        .with_children(|root| {
            for (label, action) in [
                ("新建", FileAction::New),
                ("打开", FileAction::OpenDialog),
                ("保存", FileAction::Save),
                ("另存为", FileAction::SaveAs),
                ("退出", FileAction::Exit),
            ] {
                root.spawn((
                    Button,
                    Node {
                        padding: UiRect::axes(px(12), px(7)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.13, 0.18, 0.24)),
                    FileButton(action),
                ))
                .with_children(|button| {
                    button.spawn((
                        Text::new(label),
                        TextFont {
                            font: bevy::text::FontSource::Handle(font.clone()),
                            font_size: FontSize::Px(16.0),
                            ..default()
                        },
                        TextColor(Color::WHITE),
                    ));
                });
            }
        });
}

pub(crate) fn toolbar_actions(
    mut buttons: Query<(&Interaction, &FileButton, &mut BackgroundColor), Changed<Interaction>>,
    mut actions: MessageWriter<FileAction>,
) {
    for (interaction, button, mut color) in &mut buttons {
        color.0 = match interaction {
            Interaction::Pressed => {
                actions.write(button.0.clone());
                Color::srgb(0.3, 0.5, 0.65)
            }
            Interaction::Hovered => Color::srgb(0.22, 0.32, 0.43),
            Interaction::None => Color::srgb(0.13, 0.18, 0.24),
        };
    }
}

pub(crate) fn update_window_title(
    document: Res<EditorDocument>,
    paths: Res<EditorPaths>,
    mut windows: Query<&mut Window>,
) {
    if !document.is_changed() && !paths.is_changed() {
        return;
    }
    let name = paths
        .ship
        .as_deref()
        .and_then(|path| Path::new(path).file_name())
        .and_then(|name| name.to_str())
        .unwrap_or("未命名船体");
    for mut window in &mut windows {
        window.title = format!(
            "{}{} — Difficult Rocket Editor",
            if document.dirty { "* " } else { "" },
            name
        );
    }
}

/// 配合外部脚本验证真实原生对话框；通过同一消息入口启动，避免依赖桌面焦点。
pub(crate) fn native_dialog_test(
    mode: Res<SmokeTest>,
    mut phase: Local<u8>,
    mut original: Local<Option<Ship>>,
    mut document: ResMut<EditorDocument>,
    mut actions: MessageWriter<FileAction>,
) {
    if !mode.native_dialogs || mode.started.elapsed().as_secs() < 2 {
        return;
    }
    assert!(mode.started.elapsed().as_secs() < 90, "原生对话框自测超时");
    match *phase {
        0 => {
            *original = Some(document.ship.clone());
            actions.write(FileAction::OpenDialog);
        }
        1 => {
            assert_eq!(
                Some(&document.ship),
                original.as_ref(),
                "取消打开改变了文档"
            );
            actions.write(FileAction::SaveAs);
        }
        2 => {
            assert_eq!(
                Some(&document.ship),
                original.as_ref(),
                "取消另存为改变了文档"
            );
            let id = document.ship.parts.first().expect("自测船体必须有部件").id;
            assert!(document.execute(EditorCommand::Rotate(id)));
            assert!(document.dirty);
            actions.write(FileAction::Exit);
        }
        3 => {
            assert!(document.dirty, "取消退出丢失了未保存修改");
            actions.write(FileAction::Exit);
        }
        _ => return,
    }
    *phase += 1;
}

#[cfg(test)]
mod tests;
