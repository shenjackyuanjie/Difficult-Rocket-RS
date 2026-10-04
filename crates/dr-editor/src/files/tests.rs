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

// 跨组重复编号是原版船体的合法输入，混合流程不能把 XML 编号当全局身份。
fn grouped_workflow_document() -> EditorDocument {
    let mut document = crate::tests::document();
    document.ship.parts[0].active = true;
    document.ship.parts[0].pod = Some(dr_core::PodState {
        name: "原始驾驶舱".into(),
        throttle: 0.25,
        staging: Some(dr_core::StagingState {
            current_stage: 0,
            steps: vec![dr_core::StageStep {
                activations: vec![dr_core::Activation {
                    id: 2,
                    moved: false,
                }],
            }],
        }),
    });
    let mut engine = document.ship.parts[0].clone();
    engine.id = 2;
    engine.part_type = "engine".into();
    engine.x = 2.0;
    engine.pod = None;
    document.ship.parts.push(engine);
    document.ship.connections.push(Connection::Normal {
        parent: 1,
        child: 2,
        parent_attach: 1,
        child_attach: 1,
    });
    let mut detached = dr_core::ShipGroup {
        parts: document.ship.parts.clone(),
        connections: document.ship.connections.clone(),
    };
    for part in &mut detached.parts {
        part.x += 10.0;
    }
    document.ship.disconnected.push(detached);
    document.saved_ship = document.ship.clone();
    document.refresh();
    document
}

fn copy_or_paste(document: &mut EditorDocument, cursor: &mut EditorCursor, key: KeyCode) {
    let mut keys = ButtonInput::default();
    keys.press(KeyCode::ControlLeft);
    keys.press(key);
    assert!(selection::keyboard(document, cursor, &keys, false));
}

// 原生对话框由 mock 替代；保留生产入口对粘贴预览的 Bevy 消息取消路径。
fn workflow_file_action(
    action: FileAction,
    document: &mut EditorDocument,
    paths: &mut EditorPaths,
    dialogs: &mut TestDialogs,
    cursor: &mut EditorCursor,
) {
    let mut app = App::new();
    app.insert_resource(std::mem::take(cursor))
        .add_message::<FileAction>()
        .add_systems(Update, placement::cancel_for_file_action);
    app.world_mut().write_message(action.clone());
    app.update();
    *cursor = app.world_mut().remove_resource::<EditorCursor>().unwrap();
    assert!(!apply_action(action, document, paths, dialogs));
}

#[test]
fn grouped_copy_properties_staging_save_history_and_reopen_preserve_references() {
    let directory = tempfile::tempdir().unwrap();
    let original_path = directory.path().join("原始多组.xml");
    let edited_path = directory.path().join("中文 & 另存.xml");
    let (_, mut paths, mut dialogs) = context();
    let mut document = grouped_workflow_document();
    let original = document.ship.clone();
    let mut cursor = EditorCursor::default();
    dialogs.saves.push_back(Some(original_path.clone()));
    workflow_file_action(
        FileAction::SaveAs,
        &mut document,
        &mut paths,
        &mut dialogs,
        &mut cursor,
    );

    document.selection = document.ship.keyed_parts().map(|(key, _)| key).collect();
    copy_or_paste(&mut document, &mut cursor, KeyCode::KeyC);
    let clipboard = cursor.clipboard.as_ref().unwrap().groups().to_vec();
    copy_or_paste(&mut document, &mut cursor, KeyCode::KeyV);
    cursor.world = (6.0, 20.0);
    cursor.valid = true;
    assert!(selection::commit_paste(&mut document, &mut cursor));
    assert_eq!(document.selected_keys().len(), 4);
    assert_eq!(document.history.undo_len(), 1);
    let pasted = document.ship.clone();
    assert_eq!(pasted.all_parts().count(), 8);
    assert_eq!(pasted.all_connections().count(), 4);
    for (_, parts, connections) in pasted.groups() {
        assert_eq!(parts.len(), 2);
        let pod = parts.iter().find(|part| part.pod.is_some()).unwrap();
        let engine = parts.iter().find(|part| part.pod.is_none()).unwrap();
        assert_eq!(
            connections[0],
            Connection::Normal {
                parent: pod.id,
                child: engine.id,
                parent_attach: 1,
                child_attach: 1,
            }
        );
        assert_eq!(
            pod.pod.as_ref().unwrap().staging.as_ref().unwrap().steps[0].activations[0].id,
            engine.id
        );
    }
    let pod_key = document
        .selected_keys()
        .into_iter()
        .find(|key| document.ship.part_at(*key).unwrap().pod.is_some())
        .unwrap();
    let engine_id = document
        .ship
        .groups()
        .find(|(group, _, _)| *group == pod_key.group)
        .unwrap()
        .1
        .iter()
        .find(|part| part.pod.is_none())
        .unwrap()
        .id;
    let staging = dr_core::StagingState {
        current_stage: 1,
        steps: vec![
            dr_core::StageStep::default(),
            dr_core::StageStep {
                activations: vec![dr_core::Activation {
                    id: engine_id,
                    moved: true,
                }],
            },
        ],
    };
    assert!(document.execute(EditorCommand::Scoped {
        part: pod_key,
        command: Box::new(EditorCommand::Batch(vec![
            EditorCommand::RenamePod(pod_key.id, "副本 <驾驶舱> & 分级".into()),
            EditorCommand::SetThrottle(pod_key.id, 0.75),
            EditorCommand::SetStaging(pod_key.id, Some(staging)),
        ])),
    }));
    let edited = document.ship.clone();
    assert_eq!(document.history.undo_len(), 2);
    dialogs.saves.push_back(Some(edited_path.clone()));
    workflow_file_action(
        FileAction::SaveAs,
        &mut document,
        &mut paths,
        &mut dialogs,
        &mut cursor,
    );
    assert_eq!(load_ship(&original_path).unwrap(), original);
    assert_eq!(load_ship(&edited_path).unwrap(), edited);
    assert_eq!(document.saved_ship, edited);
    assert!(!document.dirty);

    assert!(document.undo());
    assert_eq!(document.ship, pasted);
    assert!(document.dirty);
    assert!(document.redo());
    assert_eq!(document.ship, edited);
    assert!(!document.dirty);
    assert!(document.undo());
    assert!(document.undo());
    assert_eq!(document.ship, original);
    assert!(document.dirty);
    assert_eq!(document.history.redo_len(), 2);
    // 取消打开既不能丢弃未保存快照，也不能吃掉已存在的重做分支。
    document.select_only(Some(PartKey::new(1, 1, 0)));
    copy_or_paste(&mut document, &mut cursor, KeyCode::KeyV);
    dialogs.choices.push_back(UnsavedChoice::Cancel);
    workflow_file_action(
        FileAction::Open(edited_path.clone()),
        &mut document,
        &mut paths,
        &mut dialogs,
        &mut cursor,
    );
    assert_eq!(document.ship, original);
    assert_eq!(document.selected_keys(), vec![PartKey::new(1, 1, 0)]);
    assert_eq!(document.history.redo_len(), 2);
    assert_eq!(document.saved_ship, edited);
    assert!(cursor.paste.is_none());
    assert_eq!(cursor.clipboard.as_ref().unwrap().groups(), clipboard);

    dialogs.choices.push_back(UnsavedChoice::Discard);
    workflow_file_action(
        FileAction::Open(edited_path.clone()),
        &mut document,
        &mut paths,
        &mut dialogs,
        &mut cursor,
    );
    assert_eq!(document.ship, edited);
    assert!(!document.dirty);
    assert!(document.selected_keys().is_empty());
    assert!(!document.history.can_undo() && !document.history.can_redo());
    assert_eq!(paths.ship, Some(edited_path.to_string_lossy().into_owned()));
    assert_eq!(cursor.clipboard.as_ref().unwrap().groups(), clipboard);
    // 重开后应用内剪贴板仍可使用，且重新分配编号，不覆盖刚载入的副本。
    copy_or_paste(&mut document, &mut cursor, KeyCode::KeyV);
    cursor.world = (6.0, 40.0);
    cursor.valid = true;
    assert!(selection::commit_paste(&mut document, &mut cursor));
    assert_eq!(document.ship.all_parts().count(), 12);
    assert_eq!(document.ship.all_connections().count(), 6);
    assert!(document.undo());
    assert_eq!(document.ship, edited);
    assert!(!document.dirty);
}

#[test]
fn failed_save_during_new_preserves_cut_redo_and_clipboard_for_a_new_document() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("剪切前.xml");
    let (_, mut paths, mut dialogs) = context();
    let mut document = grouped_workflow_document();
    let original = document.ship.clone();
    let mut cursor = EditorCursor::default();
    dialogs.saves.push_back(Some(path.clone()));
    workflow_file_action(
        FileAction::SaveAs,
        &mut document,
        &mut paths,
        &mut dialogs,
        &mut cursor,
    );
    document.selection = [PartKey::new(1, 1, 0), PartKey::new(1, 2, 0)]
        .into_iter()
        .collect();
    copy_or_paste(&mut document, &mut cursor, KeyCode::KeyX);
    let cut = document.ship.clone();
    let clipboard = cursor.clipboard.as_ref().unwrap().groups().to_vec();
    assert_eq!(cut.all_parts().count(), 2);
    assert!(document.execute(EditorCommand::SetActive(1, false)));
    assert!(document.undo());
    assert_eq!(document.ship, cut);
    assert_eq!(document.history.redo_len(), 1);

    // 已有路径的保存失败不应误当作“放弃更改”而新建文档。
    paths.ship = Some(
        directory
            .path()
            .join("不存在/失败.xml")
            .to_string_lossy()
            .into_owned(),
    );
    let failed_path = paths.ship.clone();
    dialogs.choices.push_back(UnsavedChoice::Save);
    workflow_file_action(
        FileAction::New,
        &mut document,
        &mut paths,
        &mut dialogs,
        &mut cursor,
    );
    assert_eq!(document.ship, cut);
    assert_eq!(document.saved_ship, original);
    assert_eq!(paths.ship, failed_path);
    assert!(document.dirty);
    assert_eq!(document.history.undo_len(), 1);
    assert_eq!(document.history.redo_len(), 1);
    assert_eq!(load_ship(&path).unwrap(), original);
    assert_eq!(cursor.clipboard.as_ref().unwrap().groups(), clipboard);
    assert!(document.status.contains("无法保存"));

    assert!(document.redo());
    assert!(!document.ship.parts[0].active);
    assert!(document.undo());
    assert!(document.undo());
    assert_eq!(document.ship, original);
    assert!(!document.dirty);
    // 真正新建才清空两个历史分支，剪切得到的组可跨文档粘贴。
    workflow_file_action(
        FileAction::New,
        &mut document,
        &mut paths,
        &mut dialogs,
        &mut cursor,
    );
    assert_eq!(document.ship, Ship::default());
    assert!(paths.ship.is_none());
    assert!(!document.history.can_undo() && !document.history.can_redo());
    copy_or_paste(&mut document, &mut cursor, KeyCode::KeyV);
    cursor.world = (0.0, 0.0);
    cursor.valid = true;
    assert!(selection::commit_paste(&mut document, &mut cursor));
    let pod = document
        .ship
        .all_parts()
        .find(|part| part.pod.is_some())
        .unwrap();
    let engine = document
        .ship
        .all_parts()
        .find(|part| part.pod.is_none())
        .unwrap();
    assert_eq!(document.ship.all_parts().count(), 2);
    assert_eq!(document.ship.all_connections().count(), 1);
    assert_eq!(
        pod.pod.as_ref().unwrap().staging.as_ref().unwrap().steps[0].activations[0].id,
        engine.id
    );
    assert!(document.undo());
    assert_eq!(document.ship, Ship::default());
    assert!(!document.dirty);
}
