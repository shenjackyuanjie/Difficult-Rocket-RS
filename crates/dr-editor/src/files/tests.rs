use super::*;
use std::collections::VecDeque;

#[derive(Default)]
struct TestDialogs {
    opens: VecDeque<Option<PathBuf>>,
    saves: VecDeque<Option<PathBuf>>,
    choices: VecDeque<UnsavedChoice>,
}

impl Dialogs for TestDialogs {
    fn open(&mut self, _: &EditorPaths) -> Option<PathBuf> {
        self.opens.pop_front().expect("意外打开对话框")
    }
    fn save(&mut self, _: &EditorPaths) -> Option<PathBuf> {
        self.saves.pop_front().expect("意外保存对话框")
    }
    fn unsaved(&mut self) -> UnsavedChoice {
        self.choices.pop_front().expect("意外未保存提示")
    }
}

fn context() -> (EditorDocument, EditorPaths, TestDialogs) {
    (
        crate::tests::document(),
        EditorPaths {
            ship: None,
            catalog: String::new(),
            assets: String::new(),
        },
        TestDialogs::default(),
    )
}

fn modify(document: &mut EditorDocument) {
    assert!(document.execute(EditorCommand::Move {
        id: 1,
        from: (0.0, 0.0),
        to: (3.0, 4.0)
    }));
}

#[test]
fn new_cancel_preserves_document_path_selection_and_history() {
    let (mut document, mut paths, mut dialogs) = context();
    paths.ship = Some("原文件.xml".into());
    modify(&mut document);
    document.selected = Some(PartKey::new(0, 1, 0));
    let before = document.ship.clone();
    dialogs.choices.push_back(UnsavedChoice::Cancel);
    assert!(!apply_action(
        FileAction::New,
        &mut document,
        &mut paths,
        &mut dialogs
    ));
    assert_eq!(document.ship, before);
    assert_eq!(paths.ship.as_deref(), Some("原文件.xml"));
    assert_eq!(document.selected, Some(PartKey::new(0, 1, 0)));
    assert!(document.dirty && document.history.can_undo());
}

#[test]
fn save_as_changes_path_only_after_a_successful_write() {
    let directory = tempfile::tempdir().unwrap();
    let destination = directory.path().join("另存.xml");
    let (mut document, mut paths, mut dialogs) = context();
    modify(&mut document);
    dialogs.saves.push_back(Some(destination.clone()));
    apply_action(FileAction::SaveAs, &mut document, &mut paths, &mut dialogs);
    assert_eq!(load_ship(&destination).unwrap(), document.ship);
    assert_eq!(paths.ship, Some(destination.to_string_lossy().into_owned()));
    assert!(!document.dirty);
    modify(&mut document);
    document.execute(EditorCommand::Rotate(1));
    let old_path = paths.ship.clone();
    let saved = document.saved_ship.clone();
    dialogs
        .saves
        .push_back(Some(directory.path().join("不存在/失败.xml")));
    apply_action(FileAction::SaveAs, &mut document, &mut paths, &mut dialogs);
    assert_eq!(paths.ship, old_path);
    assert_eq!(document.saved_ship, saved);
    assert!(document.dirty);
    assert!(document.status.contains("无法保存"));
    assert_eq!(load_ship(&destination).unwrap(), saved);
}

#[test]
fn open_failure_keeps_current_document_without_prompting_to_discard() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("损坏.xml");
    std::fs::write(&path, "<Ship><Parts></Ship>").unwrap();
    let (mut document, mut paths, mut dialogs) = context();
    modify(&mut document);
    let before = document.ship.clone();
    apply_action(
        FileAction::Open(path),
        &mut document,
        &mut paths,
        &mut dialogs,
    );
    assert_eq!(document.ship, before);
    assert!(document.dirty && document.history.can_undo());
    assert!(document.status.contains("打开失败"));
}

#[test]
fn discard_and_open_replaces_document_and_clears_history() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("新船体.xml");
    let next = Ship::default();
    save_ship(&path, &next).unwrap();
    let (mut document, mut paths, mut dialogs) = context();
    modify(&mut document);
    dialogs.choices.push_back(UnsavedChoice::Discard);
    dialogs.opens.push_back(Some(path.clone()));
    apply_action(
        FileAction::OpenDialog,
        &mut document,
        &mut paths,
        &mut dialogs,
    );
    assert_eq!(document.ship, next);
    assert!(!document.dirty);
    assert!(!document.history.can_undo());
    assert_eq!(paths.ship, Some(path.to_string_lossy().into_owned()));
}

#[test]
fn exit_save_cancel_and_failure_keep_editor_open() {
    let directory = tempfile::tempdir().unwrap();
    let (mut document, mut paths, mut dialogs) = context();
    modify(&mut document);
    dialogs.choices.push_back(UnsavedChoice::Save);
    dialogs.saves.push_back(None);
    assert!(!apply_action(
        FileAction::Exit,
        &mut document,
        &mut paths,
        &mut dialogs
    ));
    assert!(document.dirty);
    dialogs.choices.push_back(UnsavedChoice::Save);
    dialogs
        .saves
        .push_back(Some(directory.path().join("缺失目录/失败.xml")));
    assert!(!apply_action(
        FileAction::Exit,
        &mut document,
        &mut paths,
        &mut dialogs
    ));
    assert!(document.dirty);
    let path = directory.path().join("已保存.xml");
    dialogs.choices.push_back(UnsavedChoice::Save);
    dialogs.saves.push_back(Some(path.clone()));
    assert!(apply_action(
        FileAction::Exit,
        &mut document,
        &mut paths,
        &mut dialogs
    ));
    assert!(!document.dirty);
    assert_eq!(load_ship(path).unwrap(), document.ship);
}

#[test]
fn cancelled_open_and_save_as_are_no_ops() {
    let (mut document, mut paths, mut dialogs) = context();
    modify(&mut document);
    let before = document.ship.clone();
    dialogs.opens.push_back(None);
    dialogs.saves.push_back(None);
    apply_action(
        FileAction::OpenDialog,
        &mut document,
        &mut paths,
        &mut dialogs,
    );
    apply_action(FileAction::SaveAs, &mut document, &mut paths, &mut dialogs);
    assert_eq!(document.ship, before);
    assert!(document.dirty && document.history.can_undo());
    assert!(paths.ship.is_none());
}

#[test]
fn save_then_new_preserves_saved_ship_and_resets_document() {
    let directory = tempfile::tempdir().unwrap();
    let destination = directory.path().join("原船体.xml");
    let (mut document, mut paths, mut dialogs) = context();
    modify(&mut document);
    let before = document.ship.clone();
    dialogs.choices.push_back(UnsavedChoice::Save);
    dialogs.saves.push_back(Some(destination.clone()));
    apply_action(FileAction::New, &mut document, &mut paths, &mut dialogs);
    assert_eq!(load_ship(destination).unwrap(), before);
    assert_eq!(document.ship, Ship::default());
    assert!(paths.ship.is_none());
    assert!(!document.dirty);
    assert!(!document.history.can_undo());
}

#[test]
fn saving_before_reopening_same_path_does_not_restore_stale_contents() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("当前船体.xml");
    let (mut document, mut paths, mut dialogs) = context();
    save_ship(&path, &document.ship).unwrap();
    paths.ship = Some(path.to_string_lossy().into_owned());
    modify(&mut document);
    let edited = document.ship.clone();
    dialogs.choices.push_back(UnsavedChoice::Save);
    apply_action(
        FileAction::Open(path.clone()),
        &mut document,
        &mut paths,
        &mut dialogs,
    );
    assert_eq!(document.ship, edited);
    assert_eq!(load_ship(path).unwrap(), edited);
    assert!(!document.dirty);
}

fn file_app() -> (App, Entity) {
    let (document, paths, _) = context();
    let mut app = App::new();
    app.insert_resource(document)
        .insert_resource(paths)
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<ButtonInput<MouseButton>>()
        .init_resource::<DragState>()
        .add_message::<FileDragAndDrop>()
        .add_message::<WindowCloseRequested>()
        .add_message::<FileAction>()
        .add_message::<AppExit>()
        .add_systems(
            Update,
            (
                toolbar_actions,
                file_inputs,
                file_actions,
                update_window_title,
            )
                .chain(),
        );
    let window = app.world_mut().spawn(Window::default()).id();
    (app, window)
}

#[test]
fn dropped_file_reaches_the_document_through_bevy_messages() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("拖入.xml");
    let next = Ship::default();
    save_ship(&path, &next).unwrap();
    let (mut app, window) = file_app();
    app.world_mut().resource_mut::<DragState>().id = Some(PartKey::new(0, 1, 0));
    app.world_mut().write_message(FileDragAndDrop::DroppedFile {
        window,
        path_buf: path.clone(),
    });
    app.update();
    assert_eq!(app.world().resource::<EditorDocument>().ship, next);
    assert_eq!(
        app.world().resource::<EditorPaths>().ship,
        Some(path.to_string_lossy().into_owned())
    );
    assert!(app.world().resource::<DragState>().id.is_none());
    assert!(
        app.world()
            .get::<Window>(window)
            .unwrap()
            .title
            .contains("拖入.xml")
    );
}

#[test]
fn shortcut_save_and_clean_window_close_follow_the_same_workflow() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("快捷键.xml");
    let (mut app, window) = file_app();
    modify(&mut app.world_mut().resource_mut::<EditorDocument>());
    app.world_mut().resource_mut::<EditorPaths>().ship = Some(path.to_string_lossy().into_owned());
    let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
    keys.press(KeyCode::ControlLeft);
    keys.press(KeyCode::KeyS);
    app.update();
    let document = app.world().resource::<EditorDocument>();
    assert_eq!(load_ship(&path).unwrap(), document.ship);
    assert!(!document.dirty);
    app.world_mut()
        .write_message(WindowCloseRequested { window });
    app.update();
    assert!(!app.world().resource::<Messages<AppExit>>().is_empty());
}

#[test]
fn toolbar_press_routes_to_file_workflow() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("工具栏.xml");
    let (mut app, _) = file_app();
    app.world_mut().resource_mut::<EditorPaths>().ship = Some(path.to_string_lossy().into_owned());
    app.world_mut().spawn((
        FileButton(FileAction::Save),
        Interaction::Pressed,
        BackgroundColor(Color::BLACK),
    ));
    app.update();
    assert_eq!(
        load_ship(path).unwrap(),
        app.world().resource::<EditorDocument>().ship
    );
}
