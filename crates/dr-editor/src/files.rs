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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum UnsavedChoice {
    Save,
    Discard,
    Cancel,
}

trait Dialogs {
    fn open(&mut self, paths: &EditorPaths) -> Option<PathBuf>;
    fn save(&mut self, paths: &EditorPaths) -> Option<PathBuf>;
}

#[derive(Default)]
struct NativeDialogs {
    parent: Option<bevy::window::RawHandleWrapper>,
}

impl NativeDialogs {
    fn builder(&self, paths: &EditorPaths) -> rfd::FileDialog {
        let dialog = rfd::FileDialog::new()
            .add_filter("SR1 船体 XML", &["xml"])
            .set_directory(initial_directory(paths));
        #[cfg(windows)]
        if let Some(parent) = &self.parent {
            // Win32 的 HWND 可以用于工作线程的 COM 对话框 owner；wrapper 保持窗口存活。
            return dialog.set_parent(&unsafe { parent.get_handle() });
        }
        dialog
    }
}

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
        self.builder(paths).set_title("打开 SR1 船体").pick_file()
    }

    fn save(&mut self, paths: &EditorPaths) -> Option<PathBuf> {
        let name = paths
            .ship
            .as_deref()
            .and_then(|path| Path::new(path).file_name())
            .and_then(|name| name.to_str())
            .unwrap_or("未命名.ship.xml");
        self.builder(paths)
            .set_title("保存 SR1 船体")
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

/// 文件操作事务：等待主窗口确认/原生路径选择时不处理新的文件请求。
#[derive(Resource, Default)]
pub(crate) struct PendingFileAction {
    action: Option<FileAction>,
    choice: Option<UnsavedChoice>,
    native: Option<NativeSelection>,
    // 关闭确认的这一帧仍隔离输入，避免按钮释放/快捷键穿透。
    suppress_frame: bool,
    pub(crate) hits: Vec<(UnsavedChoice, bevy_egui::egui::Rect)>,
}

impl PendingFileAction {
    fn is_waiting(&self) -> bool {
        self.action.is_some() || self.native.is_some()
    }

    pub(crate) fn is_blocked(&self) -> bool {
        self.is_waiting() || self.suppress_frame
    }
}

/// 所有文件入口共用这一流程；确认跨帧等待，不调用原生消息框。
fn apply_action(
    action: FileAction,
    document: &mut EditorDocument,
    paths: &mut EditorPaths,
    dialogs: &mut impl Dialogs,
    pending: &mut PendingFileAction,
) -> bool {
    if pending.action.is_some() || pending.native.is_some() {
        return false;
    }
    if matches!(action, FileAction::OpenDialog) {
        return dialogs.open(paths).is_some_and(|path| {
            apply_action(FileAction::Open(path), document, paths, dialogs, pending)
        });
    }
    if let FileAction::Open(path) = &action
        && let Err(error) = load_ship(path)
    {
        document.status = format!("打开失败（{}）：{error}", path.display());
        return false;
    }
    if document.dirty
        && matches!(
            action,
            FileAction::New | FileAction::Open(_) | FileAction::Exit
        )
    {
        pending.action = Some(action);
        return false;
    }
    continue_action(action, document, paths, dialogs)
}

fn resolve_choice(
    choice: UnsavedChoice,
    document: &mut EditorDocument,
    paths: &mut EditorPaths,
    dialogs: &mut impl Dialogs,
    pending: &mut PendingFileAction,
) -> bool {
    let Some(action) = pending.action.take() else {
        return false;
    };
    pending.suppress_frame = true;
    match choice {
        UnsavedChoice::Cancel => false,
        UnsavedChoice::Save if !save_document(document, paths, dialogs, false) => {
            pending.action = Some(action);
            false
        }
        _ => continue_action(action, document, paths, dialogs),
    }
}

fn continue_action(
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
        FileAction::Exit => return true,
        FileAction::New => {
            replace_document(document, new_ship(&document.catalog));
            paths.ship = None;
            document.status = "已新建船体".into();
        }
        FileAction::Open(path) => {
            // 确认期间文件可能变化，也可能刚保存到同一路径，必须重新读取。
            match load_ship(&path) {
                Ok(ship) => {
                    replace_document(document, ship);
                    paths.ship = Some(path.to_string_lossy().into_owned());
                    document.status = format!("已打开：{}", path.display());
                }
                Err(error) => document.status = format!("打开失败（{}）：{error}", path.display()),
            }
        }
        FileAction::OpenDialog => unreachable!("打开选择应先转换为路径"),
    }
    false
}

// 原生文件选择也放在工作线程；Update 只非阻塞轮询结果。
struct NativeSelection {
    action: FileAction,
    choice: Option<UnsavedChoice>,
    result: std::sync::Mutex<std::sync::mpsc::Receiver<Option<PathBuf>>>,
}

impl NativeSelection {
    fn start(
        action: FileAction,
        choice: Option<UnsavedChoice>,
        paths: &EditorPaths,
        parent: Option<&bevy::window::RawHandleWrapper>,
    ) -> Self {
        let (sender, receiver) = std::sync::mpsc::channel();
        let paths = paths.clone();
        let open = matches!(action, FileAction::OpenDialog);
        let mut dialogs = NativeDialogs {
            parent: parent.cloned(),
        };
        std::thread::spawn(move || {
            let path = if open {
                dialogs.open(&paths)
            } else {
                dialogs.save(&paths)
            };
            let _ = sender.send(path);
        });
        Self {
            action,
            choice,
            result: std::sync::Mutex::new(receiver),
        }
    }
}

struct SelectedPath(Option<PathBuf>);
impl Dialogs for SelectedPath {
    fn open(&mut self, _: &EditorPaths) -> Option<PathBuf> {
        self.0.take()
    }
    fn save(&mut self, _: &EditorPaths) -> Option<PathBuf> {
        self.0.take()
    }
}

/// 仅绘制并记录选择，文件 I/O 与继续动作留给下一次 Update。
pub(crate) fn show_confirmation(
    ctx: &bevy_egui::egui::Context,
    pending: &mut PendingFileAction,
    paths: &EditorPaths,
) {
    use bevy_egui::egui;
    pending.hits.clear();
    let Some(action) = pending.action.as_ref() else {
        return;
    };
    if pending.native.is_some() || pending.choice.is_some() {
        return;
    }
    let next = match action {
        FileAction::New => "新建船体",
        FileAction::Open(_) => "打开另一船体",
        FileAction::Exit => "退出编辑器",
        _ => unreachable!("只有替换/退出操作需要确认"),
    };
    let mut choice = None;
    let response = egui::Modal::new(egui::Id::new("unsaved_file_confirmation")).show(ctx, |ui| {
        ui.set_max_width(440.0);
        ui.heading("保存未保存的修改？");
        ui.add_space(8.0);
        let name = paths
            .ship
            .as_deref()
            .and_then(|path| Path::new(path).file_name())
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "未命名船体".into());
        ui.label(format!("“{name}”有未保存的修改。继续{next}前是否保存？"));
        ui.label("放弃修改无法撤销；取消将返回当前文档。");
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            for (label, value) in [
                ("保存后继续", UnsavedChoice::Save),
                ("放弃修改并继续", UnsavedChoice::Discard),
                ("取消", UnsavedChoice::Cancel),
            ] {
                let response = ui.button(label);
                pending.hits.push((value, response.rect));
                if response.clicked() {
                    choice = Some(value);
                }
            }
        });
    });
    // 点击遮罩不放弃文档；Esc 等价于取消。不要把遮罩的关闭建议当成放弃。
    if choice.is_none()
        && response.is_top_modal
        && ctx.input(|input| input.key_pressed(egui::Key::Escape))
    {
        choice = Some(UnsavedChoice::Cancel);
    }
    pending.choice = choice;
}

fn replace_document(document: &mut EditorDocument, ship: Ship) {
    document.saved_ship = ship.clone();
    document.ship = ship;
    document.history = EditorHistory::with_limit(256);
    document.clear_selection();
    document.refresh();
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn file_inputs(
    keys: Res<ButtonInput<KeyCode>>,
    mut drops: MessageReader<FileDragAndDrop>,
    mut closes: MessageReader<WindowCloseRequested>,
    mut actions: MessageWriter<FileAction>,
    mut document: ResMut<EditorDocument>,
    mut inspector: Option<ResMut<properties::Inspector>>,
    pending: Option<Res<PendingFileAction>>,
    help: Option<Res<crate::help::HelpState>>,
) {
    if pending.is_some_and(|state| state.is_waiting()) {
        closes.clear();
        drops.clear();
        return;
    }
    if let Some(inspector) = inspector.as_mut()
        && inspector.is_open()
    {
        let attempted = !closes.is_empty() || !drops.is_empty();
        if attempted {
            inspector.guard_file_input();
        }
        closes.clear();
        drops.clear();
        return;
    }
    if closes.read().next().is_some() {
        closes.clear();
        actions.write(FileAction::Exit);
        return;
    }
    if help.is_some_and(|state| state.open || state.suppress_frame) {
        drops.clear();
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

#[allow(clippy::too_many_arguments)]
pub(crate) fn file_actions(
    mut actions: MessageReader<FileAction>,
    mut document: ResMut<EditorDocument>,
    mut paths: ResMut<EditorPaths>,
    mut pending: ResMut<PendingFileAction>,
    mut drag: ResMut<DragState>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut pointer: Option<ResMut<panels::UiPointer>>,
    parents: Query<&bevy::window::RawHandleWrapper, With<bevy::window::PrimaryWindow>>,
    mut camera_drag: Option<ResMut<CameraDrag>>,
    mut exit: MessageWriter<AppExit>,
) {
    pending.suppress_frame = false;
    let mut handled = false;
    let mut should_exit = false;
    if let Some(selection) = pending.native.as_ref() {
        let result = selection.result.lock().unwrap().try_recv();
        match result {
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
            result => {
                let selection = pending.native.take().unwrap();
                let mut dialogs = SelectedPath(result.ok().flatten());
                should_exit = if let Some(choice) = selection.choice {
                    resolve_choice(
                        choice,
                        &mut document,
                        &mut paths,
                        &mut dialogs,
                        &mut pending,
                    )
                } else {
                    apply_action(
                        selection.action,
                        &mut document,
                        &mut paths,
                        &mut dialogs,
                        &mut pending,
                    )
                };
                handled = true;
            }
        }
    } else if let Some(choice) = pending.choice.take() {
        if choice == UnsavedChoice::Save && paths.ship.is_none() {
            pending.native = Some(NativeSelection::start(
                FileAction::Save,
                Some(choice),
                &paths,
                parents.single().ok(),
            ));
        } else {
            should_exit = resolve_choice(
                choice,
                &mut document,
                &mut paths,
                &mut SelectedPath(None),
                &mut pending,
            );
        }
        handled = true;
    } else if !pending.is_blocked() {
        // 一帧只接受一个动作，防止排队请求覆盖待确认的操作。
        if let Some(action) = actions.read().next().cloned() {
            drag.cancel();
            if matches!(action, FileAction::OpenDialog | FileAction::SaveAs)
                || matches!(action, FileAction::Save) && paths.ship.is_none()
            {
                pending.native = Some(NativeSelection::start(
                    action,
                    None,
                    &paths,
                    parents.single().ok(),
                ));
            } else {
                should_exit = apply_action(
                    action,
                    &mut document,
                    &mut paths,
                    &mut SelectedPath(None),
                    &mut pending,
                );
            }
            handled = true;
        }
    }
    actions.clear();
    if should_exit {
        exit.write(AppExit::Success);
    }
    if handled {
        pending.suppress_frame = true;
        mouse.reset_all();
        keys.reset_all();
    }
    if pending.is_blocked() {
        drag.cancel();
        if let Some(camera_drag) = camera_drag.as_mut() {
            camera_drag.0 = None;
        }
        if let Some(pointer) = pointer.as_mut() {
            pointer.blocked = true;
        }
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
            panels::EditorPanel,
        ))
        .with_children(|root| {
            for (label, action) in [
                ("+ 新建", FileAction::New),
                ("↗ 打开", FileAction::OpenDialog),
                ("↓ 保存", FileAction::Save),
                ("↓+ 另存为", FileAction::SaveAs),
                ("× 退出", FileAction::Exit),
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
    pending: Option<Res<PendingFileAction>>,
    help: Option<Res<crate::help::HelpState>>,
) {
    if help.is_some_and(|state| state.open || state.suppress_frame)
        || pending.is_some_and(|state| state.is_waiting())
    {
        return;
    }
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

#[derive(Resource)]
pub(crate) struct ModalCaptured;

/// 保留旧 CLI 名称；用真实 egui 按下/释放确认，不直接写入内部 choice。
#[allow(clippy::too_many_arguments)]
pub(crate) fn native_dialog_test(
    mode: Res<SmokeTest>,
    mut phase: Local<u8>,
    mut settle: Local<u8>,
    mut original: Local<Option<Ship>>,
    mut driver: Local<egui_ui::UiTestInput>,
    mut inputs: Query<&mut bevy_egui::EguiInput, With<bevy_egui::PrimaryEguiContext>>,
    captured: Option<Res<ModalCaptured>>,
    mut document: ResMut<EditorDocument>,
    pending: Res<PendingFileAction>,
    mut actions: MessageWriter<FileAction>,
    mut commands: Commands,
) {
    if !mode.native_dialogs || mode.started.elapsed().as_secs() < 2 {
        return;
    }
    assert!(mode.started.elapsed().as_secs() < 90, "未保存确认自测超时");
    let Ok(mut input) = inputs.single_mut() else {
        return;
    };
    if driver.tick(&mut input) {
        return;
    }
    match *phase {
        0 => {
            let id = document.ship.parts.first().expect("自测船体必须有部件").id;
            assert!(document.execute(EditorCommand::Rotate(id)));
            assert!(document.dirty);
            *original = Some(document.ship.clone());
            actions.write(FileAction::Exit);
            *phase = 1;
        }
        1 if !pending.hits.is_empty() => {
            // 等待最终布局与淡入；截图回调仅标记完成，不直接取消事务。
            *settle += 1;
            if *settle < 20 {
                return;
            }
            use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk};
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk("target/editor-unsaved-modal.png"))
                .observe(|_: On<ScreenshotCaptured>, mut commands: Commands| {
                    commands.insert_resource(ModalCaptured);
                });
            *phase = 2;
        }
        2 if captured.is_some() => {
            let Some((_, rect)) = pending
                .hits
                .iter()
                .find(|(choice, _)| *choice == UnsavedChoice::Cancel)
            else {
                return;
            };
            driver.click_rect(*rect, &mut input);
            *phase = 5;
        }
        5 if pending.action.is_none() => {
            assert_eq!(
                Some(&document.ship),
                original.as_ref(),
                "取消退出改变了文档"
            );
            assert!(document.dirty, "取消退出丢失了未保存修改");
            actions.write(FileAction::Exit);
            *phase = 3;
        }
        3 if pending.action.is_some() => {
            let Some((_, rect)) = pending
                .hits
                .iter()
                .find(|(choice, _)| *choice == UnsavedChoice::Discard)
            else {
                return;
            };
            driver.click_rect(*rect, &mut input);
            info!(
                "主窗口未保存确认自测：稳定暗色模态截图、真实取消按钮保留文档、真实放弃按钮退出；原生文件选择未执行"
            );
            *phase = 4;
        }
        _ => {}
    }
}

#[derive(Default)]
pub(crate) struct NativeFileDialogTestState {
    phase: u8,
    frames: u64,
    opened_frame: u64,
    open_frames: u64,
    original: Option<Ship>,
    saved: Option<Ship>,
    path: Option<String>,
    dirty: bool,
    undo: usize,
    redo: usize,
}

/// 独立的原生路径选择验收；外部驱动取消窗口，本系统只记录主 Update 持续运行。
#[allow(clippy::too_many_arguments)]
pub(crate) fn native_file_dialog_test(
    mode: Res<SmokeTest>,
    mut state: Local<NativeFileDialogTestState>,
    document: Res<EditorDocument>,
    paths: Res<EditorPaths>,
    pending: Res<PendingFileAction>,
    mut actions: MessageWriter<FileAction>,
    mut exit: MessageWriter<AppExit>,
) {
    if !mode.native_file_dialogs || mode.started.elapsed().as_secs() < 2 {
        return;
    }
    assert!(
        mode.started.elapsed().as_secs() < 90,
        "原生路径选择自测超时"
    );
    state.frames += 1;
    match state.phase {
        0 => {
            state.original = Some(document.ship.clone());
            state.saved = Some(document.saved_ship.clone());
            state.path = paths.ship.clone();
            state.dirty = document.dirty;
            state.undo = document.history.undo_len();
            state.redo = document.history.redo_len();
            actions.write(FileAction::OpenDialog);
            state.phase = 1;
        }
        1 | 3 if pending.native.is_some() => {
            state.opened_frame = state.frames;
            state.phase += 1;
        }
        2 | 4 if !pending.is_waiting() => {
            let frames = state.frames - state.opened_frame;
            assert!(
                frames >= 3,
                "原生路径选择期间主 Update 未持续运行，或驱动过早取消"
            );
            assert_eq!(
                Some(&document.ship),
                state.original.as_ref(),
                "取消文件选择改变船体"
            );
            assert_eq!(
                Some(&document.saved_ship),
                state.saved.as_ref(),
                "取消文件选择改变保存快照"
            );
            assert_eq!(paths.ship, state.path, "取消文件选择改变路径");
            assert_eq!(document.dirty, state.dirty);
            assert_eq!(document.history.undo_len(), state.undo);
            assert_eq!(document.history.redo_len(), state.redo);
            if state.phase == 2 {
                state.open_frames = frames;
                actions.write(FileAction::SaveAs);
                state.phase = 3;
            } else {
                let report = format!(
                    "{{\"open_frames\":{},\"save_as_frames\":{},\"total_frames\":{},\"document_preserved\":true}}\n",
                    state.open_frames, frames, state.frames
                );
                std::fs::write("target/native-file-dialogs.json", report)
                    .expect("无法写入原生路径选择自测报告");
                info!("原生打开/另存为取消自测通过：文档与历史保留，主 Update 连续运行");
                exit.write(AppExit::Success);
                state.phase = 5;
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests;
