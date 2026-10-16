use super::*;
use dr_core::load_catalog;

fn document() -> EditorDocument {
    let mut document = crate::test_support::document();
    document.ship = dr_core::ship_from_xml(r#"<Ship><Parts>
        <Part id="1" partType="pod"><Pod name="初始" throttle="0"><Staging currentStage="0"/></Pod></Part>
        <Part id="2" partType="tank"><Tank fuel="10"/></Part>
        </Parts></Ship>"#).unwrap();
    document.saved_ship = document.ship.clone();
    document
}

#[test]
fn duplicate_repair_ui_requires_all_assignments_and_applies_as_one_undo() {
    let mut document = document();
    let duplicate = document.ship.parts[1].clone();
    document.ship.parts.push(duplicate);
    document.ship.connections.push(Connection::Normal {
        parent: 1,
        child: 2,
        parent_attach: 1,
        child_attach: 1,
    });
    document.selected = Some(PartKey::new(0, 2, 1));
    let before = document.ship.clone();
    let mut inspector = Inspector::default();
    act(&Action::Open, &mut inspector, &mut document);
    act(&Action::OpenRepair, &mut inspector, &mut document);
    assert_eq!(inspector.repair.as_ref().unwrap().unassigned(), 1);
    act(&Action::Apply, &mut inspector, &mut document);
    assert!(inspector.is_open());
    assert!(!inspector.error.is_empty());
    assert_eq!(document.ship, before);
    act(&Action::RepairTarget(0), &mut inspector, &mut document);
    act(&Action::RepairTarget(0), &mut inspector, &mut document);
    let new_id = inspector.repair.as_ref().unwrap().new_ids()[1];
    act(&Action::Apply, &mut inspector, &mut document);
    assert!(!inspector.is_open(), "{}", inspector.error);
    assert_eq!(document.ship.parts[2].id, new_id);
    assert!(
        matches!(document.ship.connections[0], Connection::Normal { child, .. } if child == new_id)
    );
    assert!(document.undo());
    assert_eq!(document.ship, before);
    assert!(!document.history.can_undo());
}

#[test]
fn repair_does_not_discard_property_drafts_and_cancel_does_not_renumber() {
    let mut document = document();
    document.ship.parts.push(document.ship.parts[1].clone());
    document.selected = Some(PartKey::new(0, 2, 1));
    let before = document.ship.clone();
    let mut inspector = Inspector::default();
    act(&Action::Open, &mut inspector, &mut document);
    act(&Action::Active, &mut inspector, &mut document);
    act(&Action::OpenRepair, &mut inspector, &mut document);
    assert!(inspector.repair.is_none());
    assert!(!inspector.error.is_empty());
    assert!(inspector.draft.as_ref().unwrap().active);
    act(&Action::Active, &mut inspector, &mut document);
    act(&Action::OpenRepair, &mut inspector, &mut document);
    assert!(inspector.repair.is_some());
    act(&Action::BackToProperties, &mut inspector, &mut document);
    assert!(inspector.is_open());
    assert!(inspector.repair.is_none());
    act(&Action::OpenRepair, &mut inspector, &mut document);
    act(&Action::Cancel, &mut inspector, &mut document);
    assert_eq!(document.ship, before);
    assert!(!document.history.can_undo());
}

#[test]
fn duplicate_pod_draft_targets_its_group_and_limits_staging_choices() {
    let mut document = document();
    document.ship.disconnected.push(dr_core::ShipGroup {
        parts: document.ship.parts.clone(),
        connections: vec![],
    });
    let mut outside = document.ship.parts[1].clone();
    outside.id = 99;
    document.ship.parts.push(outside);
    let before = document.ship.clone();
    document.selected = Some(PartKey::new(1, 1, 0));
    let mut inspector = Inspector::default();
    act(&Action::Open, &mut inspector, &mut document);
    assert_eq!(inspector.draft.as_ref().unwrap().key, PartKey::new(1, 1, 0));
    act(&Action::AddStep, &mut inspector, &mut document);
    inspector.draft.as_mut().unwrap().target = "99".into();
    act(&Action::AddActivation(0), &mut inspector, &mut document);
    assert!(!inspector.error.is_empty());
    act(&Action::CycleTarget(true), &mut inspector, &mut document);
    assert_eq!(inspector.draft.as_ref().unwrap().target, "1");
    act(&Action::CycleTarget(true), &mut inspector, &mut document);
    assert_eq!(inspector.draft.as_ref().unwrap().target, "2");
    act(&Action::AddActivation(0), &mut inspector, &mut document);
    act(&Action::Active, &mut inspector, &mut document);
    act(&Action::Apply, &mut inspector, &mut document);
    assert!(!inspector.is_open(), "{}", inspector.error);
    assert_eq!(document.ship.parts, before.parts);
    assert!(document.ship.disconnected[0].parts[0].active);
    assert_eq!(
        document.ship.disconnected[0].parts[0]
            .pod
            .as_ref()
            .unwrap()
            .staging
            .as_ref()
            .unwrap()
            .steps[0]
            .activations[0]
            .id,
        2
    );
    assert!(document.undo());
    assert_eq!(document.ship, before);
}

#[test]
fn unique_target_choices_keep_source_order_and_cycle_within_their_group() {
    let mut document = document();
    let pod = document.ship.parts[0].clone();
    let tank = document.ship.parts[1].clone();
    let parts = |ids: &[i64]| {
        ids.iter()
            .map(|id| {
                let mut part = tank.clone();
                part.id = *id;
                part
            })
            .collect::<Vec<_>>()
    };
    document.ship.parts = parts(&[9, 4, 2, 4]);
    document.ship.parts.push(pod);
    document.ship.disconnected.push(dr_core::ShipGroup {
        parts: parts(&[4, 2, 2, 8]),
        connections: vec![],
    });
    assert_eq!(
        unique_target_parts(&document.ship.parts)
            .map(|part| part.id)
            .collect::<Vec<_>>(),
        vec![9, 2, 1]
    );
    assert_eq!(
        unique_target_parts(&document.ship.disconnected[0].parts)
            .map(|part| part.id)
            .collect::<Vec<_>>(),
        vec![4, 8]
    );
    let before = document.ship.clone();
    document.selected = Some(PartKey::new(0, 1, 0));
    let mut inspector = Inspector::default();
    open(&mut inspector, &document);
    for (forward, targets) in [(true, ["9", "2", "1", "9"]), (false, ["1", "2", "9", "1"])] {
        inspector.draft.as_mut().unwrap().target.clear();
        for target in targets {
            act(&Action::CycleTarget(forward), &mut inspector, &mut document);
            assert_eq!(inspector.draft.as_ref().unwrap().target, target);
            assert_eq!(inspector.preview, Preview::Target);
        }
    }
    // 循环操作仍直接 parse；不把 UI 目标显示使用的 trim 顺带引入此处。
    for target in ["4", "999", " 2 "] {
        for (forward, expected) in [(true, "9"), (false, "1")] {
            inspector.draft.as_mut().unwrap().target = target.into();
            act(&Action::CycleTarget(forward), &mut inspector, &mut document);
            assert_eq!(inspector.draft.as_ref().unwrap().target, expected);
        }
    }
    document.selected = Some(PartKey::new(1, 4, 0));
    open(&mut inspector, &document);
    for target in ["4", "8", "4"] {
        act(&Action::CycleTarget(true), &mut inspector, &mut document);
        assert_eq!(inspector.draft.as_ref().unwrap().target, target);
    }
    assert_eq!(document.ship, before);
    assert!(!document.dirty && !document.history.can_undo());
}

#[test]
fn cycling_empty_or_ambiguous_target_choices_keeps_the_draft_target() {
    let mut document = document();
    let pod = document.ship.parts[0].clone();
    for parts in [vec![], vec![pod.clone(), pod.clone()]] {
        document.ship.parts = parts;
        assert!(unique_target_parts(&document.ship.parts).next().is_none());
        let before = document.ship.clone();
        let mut inspector = Inspector {
            draft: Some(Draft::new(PartKey::new(0, 1, 0), &pod)),
            ..default()
        };
        inspector.draft.as_mut().unwrap().target = " 2 ".into();
        for forward in [true, false] {
            inspector.preview = Preview::Owner;
            act(&Action::CycleTarget(forward), &mut inspector, &mut document);
            assert_eq!(inspector.draft.as_ref().unwrap().target, " 2 ");
            assert_eq!(inspector.preview, Preview::Target);
            assert_eq!(document.ship, before);
            assert!(!document.dirty && !document.history.can_undo());
        }
    }
}

#[test]
fn staged_changes_cancel_or_apply_and_undo_together() {
    let mut document = document();
    let before = document.ship.clone();
    let mut inspector = Inspector::default();
    act(&Action::Open, &mut inspector, &mut document);
    act(&Action::Focus(Field::Name), &mut inspector, &mut document);
    inspector.draft.as_mut().unwrap().name = "复刻 & 火箭".into();
    act(&Action::Active, &mut inspector, &mut document);
    act(&Action::AddStep, &mut inspector, &mut document);
    inspector.draft.as_mut().unwrap().target = "2".into();
    act(&Action::AddActivation(0), &mut inspector, &mut document);
    act(&Action::Moved(0, 0), &mut inspector, &mut document);
    assert_eq!(document.ship, before);
    act(&Action::Apply, &mut inspector, &mut document);
    assert!(inspector.draft.is_none());
    assert!(document.dirty);
    let pod = document.ship.part(1).unwrap().pod.as_ref().unwrap();
    assert_eq!(pod.name, "复刻 & 火箭");
    assert_eq!(
        pod.staging.as_ref().unwrap().steps[0].activations,
        vec![Activation { id: 2, moved: true }]
    );
    let edited = document.ship.clone();
    assert_eq!(
        dr_core::ship_from_xml(&dr_core::ship_to_xml(&edited).unwrap()).unwrap(),
        edited
    );
    assert!(document.undo());
    assert_eq!(document.ship, before);
    assert!(!document.dirty);
    assert!(!document.history.can_undo());
    assert!(document.redo());
    assert_eq!(document.ship, edited);
    act(&Action::Open, &mut inspector, &mut document);
    act(&Action::Active, &mut inspector, &mut document);
    act(&Action::Cancel, &mut inspector, &mut document);
    assert_eq!(document.ship, edited);
}

#[test]
fn stages_reorder_and_remove_without_losing_activation_flags() {
    let mut document = document();
    let mut inspector = Inspector::default();
    act(&Action::Open, &mut inspector, &mut document);
    for _ in 0..2 {
        act(&Action::AddStep, &mut inspector, &mut document);
    }
    inspector.draft.as_mut().unwrap().target = "99".into();
    act(&Action::AddActivation(0), &mut inspector, &mut document);
    assert!(!inspector.error.is_empty());
    inspector.draft.as_mut().unwrap().target = "2".into();
    act(&Action::AddActivation(0), &mut inspector, &mut document);
    act(&Action::AddActivation(0), &mut inspector, &mut document);
    assert!(!inspector.error.is_empty());
    act(&Action::Moved(0, 0), &mut inspector, &mut document);
    act(&Action::MoveStep(0, true), &mut inspector, &mut document);
    act(&Action::RemoveStep(0), &mut inspector, &mut document);
    let steps = &inspector
        .draft
        .as_ref()
        .unwrap()
        .staging
        .as_ref()
        .unwrap()
        .steps;
    assert_eq!(steps.len(), 1);
    assert_eq!(
        steps[0].activations,
        vec![Activation { id: 2, moved: true }]
    );
    act(
        &Action::RemoveActivation(0, 0),
        &mut inspector,
        &mut document,
    );
    assert!(
        inspector
            .draft
            .as_ref()
            .unwrap()
            .staging
            .as_ref()
            .unwrap()
            .steps[0]
            .activations
            .is_empty()
    );
}

#[test]
fn bad_fuel_and_stale_drafts_preserve_document() {
    let mut document = document();
    document.selected = Some(PartKey::new(0, 2, 0));
    let before = document.ship.clone();
    let mut inspector = Inspector::default();
    act(&Action::Open, &mut inspector, &mut document);
    for value in ["NaN", "-1", "abc"] {
        inspector.draft.as_mut().unwrap().fuel = value.into();
        act(&Action::Apply, &mut inspector, &mut document);
        assert!(inspector.draft.is_some());
        assert!(!inspector.error.is_empty());
        assert_eq!(document.ship, before);
        assert!(!document.history.can_undo());
    }
    inspector.draft.as_mut().unwrap().fuel = "5".into();
    assert!(document.execute(EditorCommand::SetFuel(2, 7.0)));
    let external = document_snapshot(&document);
    act(&Action::Apply, &mut inspector, &mut document);
    assert!(inspector.error.contains("已变更"));
    assert!(inspector.error.contains("冲突"));
    assert_eq!(document.ship.part(2).unwrap().fuel, Some(7.0));
    assert_eq!(document_snapshot(&document), external);
    assert_eq!(inspector.draft.as_ref().unwrap().fuel, "5");
}

#[test]
fn closing_or_dropping_a_file_keeps_unapplied_draft() {
    use bevy::window::{FileDragAndDrop, WindowCloseRequested};
    let document = document();
    let mut inspector = Inspector::default();
    open(&mut inspector, &document);
    inspector.draft.as_mut().unwrap().name = "还未应用".into();
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_resource(document)
        .insert_resource(inspector)
        .init_resource::<ButtonInput<KeyCode>>()
        .add_message::<WindowCloseRequested>()
        .add_message::<FileDragAndDrop>()
        .add_message::<files::FileAction>()
        .add_systems(Update, files::file_inputs);
    let window = app.world_mut().spawn_empty().id();
    app.world_mut()
        .write_message(WindowCloseRequested { window });
    app.world_mut().write_message(FileDragAndDrop::DroppedFile {
        window,
        path_buf: "other.xml".into(),
    });
    app.update();
    assert!(
        app.world()
            .resource::<Messages<files::FileAction>>()
            .is_empty()
    );
    let inspector = app.world().resource::<Inspector>();
    assert_eq!(inspector.draft.as_ref().unwrap().name, "还未应用");
    assert!(!inspector.error.is_empty());
}

#[test]
fn fuel_capacity_is_checked_and_valid_change_undoes() {
    let mut document = document();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("PartList.xml");
    std::fs::write(
        &path,
        r#"<PartTypes><PartType id="tank" type="tank"><Tank fuel="10"/></PartType></PartTypes>"#,
    )
    .unwrap();
    document.catalog = load_catalog(path).unwrap();
    let kind = document
        .catalog
        .types
        .iter()
        .find(|kind| kind.tank.is_some())
        .unwrap();
    let max = kind.tank.as_ref().unwrap().fuel;
    document.selected = Some(PartKey::new(0, 2, 0));
    document.saved_ship = document.ship.clone();
    let mut inspector = Inspector::default();
    act(&Action::Open, &mut inspector, &mut document);
    inspector.draft.as_mut().unwrap().fuel = (max + 1.0).to_string();
    act(&Action::Apply, &mut inspector, &mut document);
    assert!(inspector.error.contains("容量"));
    assert!(!document.dirty);
    inspector.draft.as_mut().unwrap().fuel = (max / 2.0).to_string();
    act(&Action::Apply, &mut inspector, &mut document);
    assert!(inspector.draft.is_none());
    assert_eq!(document.ship.part(2).unwrap().fuel, Some(max / 2.0));
    assert!(document.undo());
    assert!(!document.dirty);
    assert_eq!(document.ship.part(2).unwrap().fuel, Some(max));
}

fn action_app() -> App {
    let mut app = App::new();
    app.add_plugins(native_buttons::NativeButtonsPlugin)
        .insert_resource(document())
        .init_resource::<Inspector>()
        .init_resource::<DragState>()
        .init_resource::<EditorCursor>()
        .init_resource::<files::PendingFileAction>()
        .init_resource::<help::HelpState>()
        .init_resource::<mobile_input::GestureInput>()
        .add_systems(Update, actions);
    app
}

#[test]
fn widget_activation_routes_property_actions_and_preserves_single_undo() {
    let mut app = action_app();
    let before = app.world().resource::<EditorDocument>().ship.clone();
    let open_button = app.world_mut().spawn(button(Action::Open)).id();
    let cancel_button = app.world_mut().spawn(button(Action::Cancel)).id();
    let apply_button = app.world_mut().spawn(button(Action::Apply)).id();
    let other = app.world_mut().spawn_empty().id();
    app.world_mut()
        .trigger(bevy::ui_widgets::Activate { entity: other });
    app.update();
    assert!(!app.world().resource::<Inspector>().is_open());
    app.world_mut().resource_mut::<DragState>().id = Some(PartKey::new(0, 1, 0));
    app.world_mut().resource_mut::<EditorCursor>().placing = true;
    app.world_mut().trigger(bevy::ui_widgets::Activate {
        entity: open_button,
    });
    app.update();
    assert!(app.world().resource::<Inspector>().is_open());
    assert!(app.world().resource::<DragState>().id.is_none());
    assert!(!app.world().resource::<EditorCursor>().placing);
    assert_eq!(app.world().resource::<EditorDocument>().ship, before);
    assert!(!app.world().resource::<EditorDocument>().history.can_undo());
    app.world_mut()
        .resource_mut::<Inspector>()
        .draft
        .as_mut()
        .unwrap()
        .name = "取消的草稿".into();
    app.world_mut().trigger(bevy::ui_widgets::Activate {
        entity: cancel_button,
    });
    app.update();
    assert!(!app.world().resource::<Inspector>().is_open());
    assert_eq!(app.world().resource::<EditorDocument>().ship, before);
    assert!(!app.world().resource::<EditorDocument>().history.can_undo());
    app.world_mut().trigger(bevy::ui_widgets::Activate {
        entity: open_button,
    });
    app.update();
    app.world_mut()
        .resource_mut::<Inspector>()
        .draft
        .as_mut()
        .unwrap()
        .name = "按钮提交".into();
    app.world_mut().trigger(bevy::ui_widgets::Activate {
        entity: apply_button,
    });
    app.update();
    assert!(!app.world().resource::<Inspector>().is_open());
    assert_eq!(
        app.world().resource::<EditorDocument>().ship.parts[0]
            .pod
            .as_ref()
            .unwrap()
            .name,
        "按钮提交"
    );
    let mut document = app.world_mut().resource_mut::<EditorDocument>();
    assert!(document.undo());
    assert_eq!(document.ship, before);
    assert!(!document.history.can_undo(), "应用按钮只提交一个事务");
}

#[test]
fn property_actions_drop_help_blocked_activations_before_resuming() {
    for open in [true, false] {
        let mut app = action_app();
        let entity = app.world_mut().spawn(button(Action::Open)).id();
        *app.world_mut().resource_mut::<help::HelpState>() = help::HelpState {
            open,
            suppress_frame: !open,
        };
        app.world_mut()
            .trigger(bevy::ui_widgets::Activate { entity });
        app.update();
        assert!(!app.world().resource::<Inspector>().is_open());
        app.insert_resource(help::HelpState::default());
        app.update();
        assert!(
            !app.world().resource::<Inspector>().is_open(),
            "帮助关闭不应补发旧激活"
        );
        app.world_mut()
            .trigger(bevy::ui_widgets::Activate { entity });
        app.update();
        assert!(app.world().resource::<Inspector>().is_open());
    }
}

#[test]
fn native_property_click_opens_once_and_drops_compatible_mouse_activation() {
    let mut app = action_app();
    let before = document_snapshot(app.world().resource::<EditorDocument>());
    let entity = app.world_mut().spawn(button(Action::Open)).id();
    app.world_mut().resource_mut::<DragState>().id = Some(PartKey::new(0, 1, 0));
    app.world_mut().resource_mut::<EditorCursor>().placing = true;
    {
        let mut touch = app.world_mut().resource_mut::<mobile_input::GestureInput>();
        touch.override_mouse = true;
        touch.native_click = Some(mobile_input::NativeClick::Properties);
    }
    app.world_mut()
        .trigger(bevy::ui_widgets::Activate { entity });
    app.update();
    assert!(app.world().resource::<Inspector>().is_open());
    assert!(app.world().resource::<DragState>().id.is_none());
    assert!(!app.world().resource::<EditorCursor>().placing);
    app.world_mut()
        .resource_mut::<Inspector>()
        .draft
        .as_mut()
        .unwrap()
        .name = "保持未提交草稿".into();
    app.world_mut()
        .resource_mut::<mobile_input::GestureInput>()
        .native_click = None;
    app.world_mut()
        .trigger(bevy::ui_widgets::Activate { entity });
    app.update();
    assert_eq!(
        app.world()
            .resource::<Inspector>()
            .draft
            .as_ref()
            .unwrap()
            .name,
        "保持未提交草稿"
    );
    app.world_mut()
        .resource_mut::<mobile_input::GestureInput>()
        .override_mouse = false;
    app.update();
    assert_eq!(
        app.world()
            .resource::<Inspector>()
            .draft
            .as_ref()
            .unwrap()
            .name,
        "保持未提交草稿",
        "接管解除不能补发旧消息并重置草稿"
    );
    app.world_mut()
        .trigger(bevy::ui_widgets::Activate { entity });
    app.update();
    let inspector = app.world().resource::<Inspector>();
    assert!(inspector.is_open(), "有修改的草稿保持打开");
    assert_eq!(
        inspector.draft.as_ref().unwrap().name,
        "保持未提交草稿",
        "再次真实激活不能静默丢弃草稿"
    );
    assert!(!inspector.error.is_empty());
    assert!(
        inspector.error.contains("草稿有修改"),
        "{}",
        inspector.error
    );
    assert!(
        inspector.error.contains("请先应用或取消"),
        "{}",
        inspector.error
    );
    assert_eq!(
        document_snapshot(app.world().resource::<EditorDocument>()),
        before,
        "原生属性入口及兼容鼠标激活都不修改文档、选择、脏标记或文档历史"
    );
    assert!(!app.world().resource::<EditorDocument>().history.can_undo());
}

#[test]
fn staging_preview_resolves_only_current_unique_group_targets_without_a_transaction() {
    let mut document = document();
    document.ship.disconnected.push(dr_core::ShipGroup {
        parts: vec![document.ship.parts[1].clone()],
        ..default()
    });
    document.select_only(Some(PartKey::new(0, 1, 0)));
    let before = document.ship.clone();
    let selection = document.selected_keys();
    let mut inspector = Inspector::default();
    open(&mut inspector, &document);
    inspector.draft.as_mut().unwrap().staging = Some(StagingState {
        steps: vec![StageStep {
            activations: vec![Activation {
                id: 2,
                moved: false,
            }],
        }],
        ..default()
    });
    act(
        &Action::SelectActivation(0, 0),
        &mut inspector,
        &mut document,
    );
    assert_eq!(
        inspector.preview_keys(&document),
        vec![PartKey::new(0, 2, 0)]
    );
    act(&Action::SelectStage(0), &mut inspector, &mut document);
    assert_eq!(
        inspector.preview_keys(&document),
        vec![PartKey::new(0, 2, 0)]
    );
    assert_eq!(document.ship, before);
    assert_eq!(document.selected_keys(), selection);
    assert!(!document.dirty && !document.history.can_undo());
    inspector.preview = Preview::Target;
    inspector.draft.as_mut().unwrap().target = "999".into();
    assert!(inspector.preview_keys(&document).is_empty());
    inspector.draft.as_mut().unwrap().target = "2".into();
    document.ship.parts.push(document.ship.parts[1].clone());
    assert!(
        inspector.preview_keys(&document).is_empty(),
        "同组重号不能猜测实例"
    );
    document.ship.parts.pop();
    document.ship.parts[0].active = !document.ship.parts[0].active;
    assert_eq!(
        inspector.preview_keys(&document),
        vec![PartKey::new(0, 2, 0)],
        "其他属性的外部变化不改变预览身份"
    );
    document.ship.parts[0].part_type = "changed-type".into();
    assert!(
        inspector.preview_keys(&document).is_empty(),
        "类型变化后停止预览"
    );
}

#[test]
fn stage_reorder_and_action_edits_keep_preview_on_the_edited_target_and_one_undo() {
    let mut document = document();
    let before = document.ship.clone();
    let mut inspector = Inspector::default();
    open(&mut inspector, &document);
    inspector.draft.as_mut().unwrap().target = "2".into();
    for _ in 0..2 {
        act(&Action::AddStep, &mut inspector, &mut document);
    }
    act(&Action::AddActivation(0), &mut inspector, &mut document);
    assert_eq!(
        inspector.preview_keys(&document),
        vec![PartKey::new(0, 2, 0)]
    );
    act(&Action::MoveStep(0, true), &mut inspector, &mut document);
    assert_eq!(inspector.preview, Preview::Stage(1));
    assert_eq!(
        inspector.preview_keys(&document),
        vec![PartKey::new(0, 2, 0)]
    );
    act(&Action::Moved(1, 0), &mut inspector, &mut document);
    assert_eq!(inspector.preview, Preview::Activation(1, 0));
    assert_eq!(document.ship, before);
    act(&Action::Apply, &mut inspector, &mut document);
    assert!(!inspector.is_open());
    assert_eq!(document.history.undo_len(), 1);
    assert!(document.undo());
    assert_eq!(document.ship, before);
}

fn staging_document() -> EditorDocument {
    let mut document = document();
    document.catalog = dr_core::catalog_from_xml(
        r#"<PartTypes>
            <PartType id="pod" type="pod" hidden="true"/>
            <PartType id="tank" type="tank"><Tank fuel="20"/></PartType>
            <PartType id="engine" type="engine"><Engine/></PartType>
            <PartType id="detacher" type="detacher"/>
            <PartType id="parachute" type="parachute"/>
        </PartTypes>"#,
    )
    .unwrap();
    document.ship = dr_core::Ship {
        parts: [
            (1, "pod"),
            (2, "tank"),
            (3, "engine"),
            (4, "detacher"),
            (5, "tank"),
            (6, "engine"),
            (7, "detacher"),
            (8, "tank"),
            (9, "engine"),
            (10, "parachute"),
        ]
        .into_iter()
        .map(|(id, kind)| {
            document
                .catalog
                .get(kind)
                .unwrap()
                .instantiate(id, (0.0, 0.0))
        })
        .collect(),
        connections: (1..9)
            .map(|id| staging_connection(id, id + 1))
            .chain([staging_connection(1, 10)])
            .collect(),
        ..default()
    };
    document.saved_ship = document.ship.clone();
    document.select_only(Some(PartKey::new(0, 1, 0)));
    document
}

fn staging_connection(parent: i64, child: i64) -> Connection {
    Connection::Normal {
        parent,
        child,
        parent_attach: 0,
        child_attach: 1,
    }
}

fn editable_plan() -> StagingState {
    StagingState {
        current_stage: 2,
        steps: [
            vec![(3, false), (6, true), (9, false)],
            vec![(4, false)],
            vec![(7, true), (10, false)],
        ]
        .into_iter()
        .map(|activations| StageStep {
            activations: activations
                .into_iter()
                .map(|(id, moved)| Activation { id, moved })
                .collect(),
        })
        .collect(),
    }
}

fn staging_inspector() -> (EditorDocument, Inspector) {
    let document = staging_document();
    let mut inspector = Inspector::default();
    open(&mut inspector, &document);
    let draft = inspector.draft.as_mut().unwrap();
    draft.current_stage = "2".into();
    draft.staging = Some(editable_plan());
    (document, inspector)
}

fn draft_plan(inspector: &Inspector) -> &StagingState {
    inspector.draft.as_ref().unwrap().staging.as_ref().unwrap()
}

fn assert_same_draft(actual: &Draft, before: &Draft) {
    assert_eq!(actual.key, before.key);
    assert_eq!(actual.original, before.original);
    assert_eq!(actual.name, before.name);
    assert_eq!(actual.fuel, before.fuel);
    assert_eq!(actual.throttle, before.throttle);
    assert_eq!(actual.current_stage, before.current_stage);
    assert_eq!(actual.target, before.target);
    assert_eq!(actual.active, before.active);
    assert_eq!(actual.staging, before.staging);
}

fn document_snapshot(
    document: &EditorDocument,
) -> (dr_core::Ship, Vec<PartKey>, bool, usize, usize, u64) {
    (
        document.ship.clone(),
        document.selected_keys(),
        document.dirty,
        document.history.undo_len(),
        document.history.redo_len(),
        document.revision,
    )
}

#[test]
fn generate_staging_only_replaces_plan_and_current_stage_and_is_idempotent() {
    let (mut document, mut inspector) = staging_inspector();
    assert!(document.execute(EditorCommand::SetActive(2, true)));
    assert!(document.undo());
    document.select_only(Some(PartKey::new(0, 1, 0)));
    let before = document_snapshot(&document);
    let draft = inspector.draft.as_mut().unwrap();
    draft.name = "未提交名称".into();
    draft.fuel = "保留原输入".into();
    draft.throttle = "0.75".into();
    draft.target = "10".into();
    draft.active = !draft.active;
    draft.current_stage = "不合法的旧输入".into();
    let mut expected = draft.clone();
    expected.staging = Some(
        dr_core::generate_basic_staging(&document.ship, &document.catalog, expected.key).unwrap(),
    );
    expected.current_stage = "0".into();
    inspector.preview = Preview::Target;
    inspector.hovered = Some(Preview::Activation(2, 0));
    inspector.focus = Some(Field::Name);
    inspector.advanced_open = true;
    inspector.targets_open = true;
    inspector.error = "旧错误".into();

    for _ in 0..2 {
        act(&Action::GenerateStaging, &mut inspector, &mut document);
        assert_same_draft(inspector.draft.as_ref().unwrap(), &expected);
        assert!(inspector.error.is_empty());
        assert_eq!(inspector.preview, Preview::Stage(0));
        assert_eq!(inspector.hovered, None);
        assert_eq!(
            inspector.preview_keys(&document),
            vec![PartKey::new(0, 9, 0)]
        );
        assert_eq!(inspector.focus, Some(Field::Name));
        assert!(inspector.advanced_open && inspector.targets_open);
        assert_eq!(document_snapshot(&document), before);
    }
    assert_eq!(document.history.redo_len(), 1);
}

#[test]
fn generate_staging_accepts_empty_plan_and_previews_owner() {
    let mut document = staging_document();
    document.ship.parts.truncate(1);
    document.ship.connections.clear();
    document.saved_ship = document.ship.clone();
    let before = document_snapshot(&document);
    let mut inspector = Inspector::default();
    open(&mut inspector, &document);
    inspector.draft.as_mut().unwrap().current_stage = "99".into();
    inspector.preview = Preview::Activation(1, 0);
    act(&Action::GenerateStaging, &mut inspector, &mut document);
    assert!(inspector.error.is_empty());
    assert_eq!(draft_plan(&inspector), &StagingState::default());
    assert_eq!(inspector.draft.as_ref().unwrap().current_stage, "0");
    assert_eq!(inspector.preview, Preview::Owner);
    assert_eq!(
        inspector.preview_keys(&document),
        vec![PartKey::new(0, 1, 0)]
    );
    assert_eq!(document_snapshot(&document), before);
}

#[test]
fn generation_failures_keep_all_draft_fields_preview_and_history() {
    for invalid in 0..4 {
        let (mut document, mut inspector) = staging_inspector();
        match invalid {
            0 => document.catalog = PartCatalog::default(),
            1 => document.ship.parts.push(document.ship.parts[2].clone()),
            2 => document.ship.connections.push(staging_connection(9, 1)),
            _ => document.ship.parts[0].part_type = "changed-type".into(),
        }
        inspector.draft.as_mut().unwrap().name = "保留手工草稿".into();
        inspector.draft.as_mut().unwrap().current_stage = "旧分级输入".into();
        inspector.preview = Preview::Activation(2, 1);
        inspector.hovered = Some(Preview::Stage(1));
        let before_draft = inspector.draft.as_ref().unwrap().clone();
        let before_document = document_snapshot(&document);
        act(&Action::GenerateStaging, &mut inspector, &mut document);
        assert!(!inspector.error.is_empty(), "错误场景 {invalid}");
        assert_same_draft(inspector.draft.as_ref().unwrap(), &before_draft);
        assert_eq!(inspector.preview, Preview::Activation(2, 1));
        assert_eq!(inspector.hovered, Some(Preview::Stage(1)));
        assert_eq!(document_snapshot(&document), before_document);
    }
}

#[test]
fn reorder_step_inserts_at_final_index_and_preserves_flags_and_current_stage() {
    let (mut document, mut inspector) = staging_inspector();
    let before_document = document_snapshot(&document);
    let original = editable_plan();
    act(
        &Action::ReorderStep { from: 0, to: 2 },
        &mut inspector,
        &mut document,
    );
    assert_eq!(
        draft_plan(&inspector).steps,
        vec![
            original.steps[1].clone(),
            original.steps[2].clone(),
            original.steps[0].clone(),
        ]
    );
    assert_eq!(inspector.preview, Preview::Stage(2));
    act(
        &Action::ReorderStep { from: 2, to: 0 },
        &mut inspector,
        &mut document,
    );
    assert_eq!(draft_plan(&inspector), &original);
    act(
        &Action::ReorderStep { from: 1, to: 1 },
        &mut inspector,
        &mut document,
    );
    assert_eq!(draft_plan(&inspector), &original);
    assert_eq!(inspector.draft.as_ref().unwrap().current_stage, "2");
    assert_eq!(inspector.preview, Preview::Stage(1));
    assert_eq!(document_snapshot(&document), before_document);
}

#[test]
fn reorder_step_rejects_invalid_indices_and_missing_plan_atomically() {
    for action in [
        Action::ReorderStep { from: 3, to: 0 },
        Action::ReorderStep { from: 0, to: 3 },
        Action::ReorderStep {
            from: usize::MAX,
            to: usize::MAX,
        },
    ] {
        let (mut document, mut inspector) = staging_inspector();
        inspector.preview = Preview::Target;
        inspector.hovered = Some(Preview::Owner);
        let before = inspector.draft.as_ref().unwrap().clone();
        act(&action, &mut inspector, &mut document);
        assert!(!inspector.error.is_empty());
        assert_same_draft(inspector.draft.as_ref().unwrap(), &before);
        assert_eq!(inspector.preview, Preview::Target);
        assert_eq!(inspector.hovered, Some(Preview::Owner));
        assert_eq!(document.history.undo_len(), 0);
    }
    let (mut document, mut inspector) = staging_inspector();
    inspector.draft.as_mut().unwrap().staging = None;
    act(
        &Action::ReorderStep { from: 0, to: 0 },
        &mut inspector,
        &mut document,
    );
    assert!(!inspector.error.is_empty());
    assert!(inspector.draft.as_ref().unwrap().staging.is_none());
}

#[test]
fn move_activation_between_cards_marks_moved_and_keeps_empty_source_card() {
    let (mut document, mut inspector) = staging_inspector();
    let before_document = document_snapshot(&document);
    inspector.hovered = Some(Preview::Owner);
    act(
        &Action::MoveActivation {
            from_stage: 0,
            index: 2,
            to_stage: 1,
            to_index: 0,
        },
        &mut inspector,
        &mut document,
    );
    assert!(inspector.error.is_empty());
    assert_eq!(
        draft_plan(&inspector).steps[0].activations,
        vec![
            Activation {
                id: 3,
                moved: false
            },
            Activation { id: 6, moved: true },
        ]
    );
    assert_eq!(
        draft_plan(&inspector).steps[1].activations,
        vec![
            Activation { id: 9, moved: true },
            Activation {
                id: 4,
                moved: false
            },
        ]
    );
    assert_eq!(inspector.preview, Preview::Activation(1, 0));
    assert_eq!(inspector.hovered, None);
    assert_eq!(
        inspector.preview_keys(&document),
        vec![PartKey::new(0, 9, 0)]
    );
    act(
        &Action::MoveActivation {
            from_stage: 1,
            index: 1,
            to_stage: 2,
            to_index: 2,
        },
        &mut inspector,
        &mut document,
    );
    assert_eq!(
        draft_plan(&inspector).steps[2].activations.last(),
        Some(&Activation { id: 4, moved: true })
    );
    act(
        &Action::MoveActivation {
            from_stage: 1,
            index: 0,
            to_stage: 0,
            to_index: 0,
        },
        &mut inspector,
        &mut document,
    );
    assert!(inspector.error.is_empty());
    assert!(draft_plan(&inspector).steps[1].activations.is_empty());
    assert_eq!(draft_plan(&inspector).steps.len(), 3);
    assert_eq!(draft_plan(&inspector).current_stage, 2);
    assert_eq!(inspector.draft.as_ref().unwrap().current_stage, "2");
    assert_eq!(document_snapshot(&document), before_document);
}

#[test]
fn move_activation_within_card_uses_post_removal_index_and_preserves_flags() {
    let (mut document, mut inspector) = staging_inspector();
    let original = editable_plan();
    act(
        &Action::MoveActivation {
            from_stage: 0,
            index: 0,
            to_stage: 0,
            to_index: 2,
        },
        &mut inspector,
        &mut document,
    );
    assert_eq!(
        draft_plan(&inspector).steps[0].activations,
        vec![
            Activation { id: 6, moved: true },
            Activation {
                id: 9,
                moved: false
            },
            Activation {
                id: 3,
                moved: false
            },
        ]
    );
    assert_eq!(inspector.preview, Preview::Activation(0, 2));
    act(
        &Action::MoveActivation {
            from_stage: 0,
            index: 2,
            to_stage: 0,
            to_index: 0,
        },
        &mut inspector,
        &mut document,
    );
    for index in 0..2 {
        act(
            &Action::MoveActivation {
                from_stage: 0,
                index,
                to_stage: 0,
                to_index: index,
            },
            &mut inspector,
            &mut document,
        );
        assert!(inspector.error.is_empty());
        assert_eq!(draft_plan(&inspector), &original);
    }
    assert_eq!(document.history.undo_len(), 0);
}

#[test]
fn invalid_activation_moves_preserve_source_destination_and_preview() {
    for (from_stage, index, to_stage, to_index) in [
        (3, 0, 0, 0),
        (0, 3, 1, 0),
        (0, 0, 3, 0),
        (0, 0, 1, 2),
        (0, 0, 0, 3),
        (usize::MAX, 0, 0, 0),
        (0, usize::MAX, 1, 0),
        (0, 0, usize::MAX, 0),
        (0, 0, 1, usize::MAX),
    ] {
        let (mut document, mut inspector) = staging_inspector();
        inspector.preview = Preview::Stage(2);
        inspector.hovered = Some(Preview::Target);
        let before = inspector.draft.as_ref().unwrap().clone();
        let before_document = document_snapshot(&document);
        act(
            &Action::MoveActivation {
                from_stage,
                index,
                to_stage,
                to_index,
            },
            &mut inspector,
            &mut document,
        );
        assert!(!inspector.error.is_empty());
        assert_same_draft(inspector.draft.as_ref().unwrap(), &before);
        assert_eq!(inspector.preview, Preview::Stage(2));
        assert_eq!(inspector.hovered, Some(Preview::Target));
        assert_eq!(document_snapshot(&document), before_document);
    }
    let (mut document, mut inspector) = staging_inspector();
    inspector.draft.as_mut().unwrap().staging = None;
    act(
        &Action::MoveActivation {
            from_stage: 0,
            index: 0,
            to_stage: 0,
            to_index: 0,
        },
        &mut inspector,
        &mut document,
    );
    assert!(!inspector.error.is_empty());
    assert!(inspector.draft.as_ref().unwrap().staging.is_none());
}

#[test]
fn move_activation_rejects_destination_duplicate_without_cleaning_legacy_cross_step_ids() {
    let (mut document, mut inspector) = staging_inspector();
    inspector
        .draft
        .as_mut()
        .unwrap()
        .staging
        .as_mut()
        .unwrap()
        .steps[1]
        .activations
        .push(Activation { id: 3, moved: true });
    let before = inspector.draft.as_ref().unwrap().clone();
    act(
        &Action::MoveActivation {
            from_stage: 0,
            index: 0,
            to_stage: 1,
            to_index: 0,
        },
        &mut inspector,
        &mut document,
    );
    assert!(inspector.error.contains("该级已有"));
    assert_same_draft(inspector.draft.as_ref().unwrap(), &before);
    act(
        &Action::MoveActivation {
            from_stage: 0,
            index: 2,
            to_stage: 2,
            to_index: 2,
        },
        &mut inspector,
        &mut document,
    );
    assert!(inspector.error.is_empty());
    assert_eq!(draft_plan(&inspector).steps[0].activations[0].id, 3);
    assert_eq!(
        draft_plan(&inspector).steps[1].activations[1],
        Activation { id: 3, moved: true }
    );
    assert_eq!(
        draft_plan(&inspector)
            .steps
            .iter()
            .flat_map(|step| &step.activations)
            .filter(|activation| activation.id == 3)
            .count(),
        2
    );
    act(&Action::Apply, &mut inspector, &mut document);
    assert!(!inspector.is_open(), "{}", inspector.error);
    assert_eq!(document.history.undo_len(), 1);
    let xml = dr_core::ship_to_xml(&document.ship).unwrap();
    assert_eq!(dr_core::ship_from_xml(&xml).unwrap(), document.ship);
}

#[test]
fn move_activation_rejects_existing_duplicate_within_same_card_atomically() {
    let (mut document, mut inspector) = staging_inspector();
    inspector
        .draft
        .as_mut()
        .unwrap()
        .staging
        .as_mut()
        .unwrap()
        .steps[0]
        .activations
        .push(Activation { id: 3, moved: true });
    let before = inspector.draft.as_ref().unwrap().clone();
    act(
        &Action::MoveActivation {
            from_stage: 0,
            index: 0,
            to_stage: 0,
            to_index: 2,
        },
        &mut inspector,
        &mut document,
    );
    assert!(inspector.error.contains("该级已有"));
    assert_same_draft(inspector.draft.as_ref().unwrap(), &before);
}

#[test]
fn generated_and_manual_changes_cancel_without_touching_document_or_redo() {
    let (mut document, mut inspector) = staging_inspector();
    assert!(document.execute(EditorCommand::SetActive(2, true)));
    assert!(document.undo());
    let before = document_snapshot(&document);
    act(&Action::GenerateStaging, &mut inspector, &mut document);
    act(
        &Action::MoveActivation {
            from_stage: 0,
            index: 0,
            to_stage: 1,
            to_index: 1,
        },
        &mut inspector,
        &mut document,
    );
    act(
        &Action::ReorderStep { from: 1, to: 0 },
        &mut inspector,
        &mut document,
    );
    act(&Action::ToggleAdvanced, &mut inspector, &mut document);
    act(&Action::ToggleTargets, &mut inspector, &mut document);
    inspector.draft.as_mut().unwrap().name = "仅草稿".into();
    act(&Action::Cancel, &mut inspector, &mut document);
    assert!(!inspector.is_open());
    assert!(!inspector.advanced_open && !inspector.targets_open);
    assert_eq!(document_snapshot(&document), before);
    assert!(document.redo());
    assert!(document.ship.part(2).unwrap().active);
}

#[test]
fn generated_manual_and_property_edits_apply_as_one_xml_roundtrippable_undo() {
    let (mut document, mut inspector) = staging_inspector();
    let before = document.ship.clone();
    act(&Action::GenerateStaging, &mut inspector, &mut document);
    act(
        &Action::MoveActivation {
            from_stage: 0,
            index: 0,
            to_stage: 1,
            to_index: 1,
        },
        &mut inspector,
        &mut document,
    );
    act(
        &Action::ReorderStep { from: 1, to: 0 },
        &mut inspector,
        &mut document,
    );
    let expected_plan = draft_plan(&inspector).clone();
    let draft = inspector.draft.as_mut().unwrap();
    draft.name = "自动 + 手动 & 三级".into();
    draft.throttle = "0.6".into();
    draft.active = !draft.active;
    assert_eq!(document.ship, before);
    assert_eq!(document.history.undo_len(), 0);
    act(&Action::Apply, &mut inspector, &mut document);
    assert!(!inspector.is_open(), "{}", inspector.error);
    assert_eq!(document.history.undo_len(), 1);
    assert!(document.dirty);
    let pod = document.ship.part(1).unwrap().pod.as_ref().unwrap();
    assert_eq!(pod.name, "自动 + 手动 & 三级");
    assert_eq!(pod.throttle, 0.6);
    assert_eq!(pod.staging.as_ref(), Some(&expected_plan));
    assert_eq!(
        expected_plan.steps[0].activations[1],
        Activation { id: 9, moved: true }
    );
    assert!(expected_plan.steps[1].activations.is_empty());
    let after = document.ship.clone();
    assert_eq!(
        dr_core::ship_from_xml(&dr_core::ship_to_xml(&after).unwrap()).unwrap(),
        after
    );
    assert!(document.undo());
    assert_eq!(document.ship, before);
    assert!(!document.dirty);
    assert_eq!(document.history.undo_len(), 0);
    assert!(document.redo());
    assert_eq!(document.ship, after);
    assert_eq!(document.history.undo_len(), 1);
}

#[test]
fn generating_unchanged_plan_and_applying_preserves_redo_without_new_history() {
    let mut document = staging_document();
    let plan =
        dr_core::generate_basic_staging(&document.ship, &document.catalog, PartKey::new(0, 1, 0))
            .unwrap();
    document.ship.parts[0].pod.as_mut().unwrap().staging = Some(plan);
    document.saved_ship = document.ship.clone();
    assert!(document.execute(EditorCommand::SetActive(2, true)));
    assert!(document.undo());
    let before = document.ship.clone();
    let mut inspector = Inspector::default();
    open(&mut inspector, &document);
    act(&Action::GenerateStaging, &mut inspector, &mut document);
    act(&Action::GenerateStaging, &mut inspector, &mut document);
    act(&Action::Apply, &mut inspector, &mut document);
    assert!(!inspector.is_open(), "{}", inspector.error);
    assert_eq!(document.ship, before);
    assert!(!document.dirty);
    assert_eq!(document.history.undo_len(), 0);
    assert_eq!(document.history.redo_len(), 1);
    assert!(document.redo());
    assert!(document.ship.part(2).unwrap().active);
}

#[test]
fn inspector_drawer_toggles_only_change_ui_state() {
    let (mut document, mut inspector) = staging_inspector();
    inspector.error = "需保留的提示".into();
    inspector.focus = Some(Field::Name);
    inspector.preview = Preview::Activation(0, 1);
    inspector.hovered = Some(Preview::Stage(2));
    let before_document = document_snapshot(&document);
    let before_draft = inspector.draft.as_ref().unwrap().clone();
    assert!(!inspector.advanced_open && !inspector.targets_open);
    for expected in [true, false] {
        act(&Action::ToggleAdvanced, &mut inspector, &mut document);
        act(&Action::ToggleTargets, &mut inspector, &mut document);
        assert_eq!(inspector.advanced_open, expected);
        assert_eq!(inspector.targets_open, expected);
        assert_eq!(inspector.error, "需保留的提示");
        assert_eq!(inspector.focus, Some(Field::Name));
        assert_eq!(inspector.preview, Preview::Activation(0, 1));
        assert_eq!(inspector.hovered, Some(Preview::Stage(2)));
        assert_same_draft(inspector.draft.as_ref().unwrap(), &before_draft);
        assert_eq!(document_snapshot(&document), before_document);
    }
}

#[test]
fn open_toggles_clean_drafts_and_keeps_modified_drafts_until_apply_or_cancel() {
    let mut document = document();
    let before = document_snapshot(&document);
    let mut inspector = Inspector::default();
    act(&Action::Open, &mut inspector, &mut document);
    inspector.draft.as_mut().unwrap().target = "2".into();
    act(&Action::ToggleAdvanced, &mut inspector, &mut document);
    act(&Action::Open, &mut inspector, &mut document);
    assert!(!inspector.is_open(), "导航状态不构成未应用修改");
    act(&Action::Open, &mut inspector, &mut document);
    inspector.draft.as_mut().unwrap().name = "保留名称".into();
    let draft = inspector.draft.as_ref().unwrap().clone();
    act(&Action::Open, &mut inspector, &mut document);
    assert_same_draft(inspector.draft.as_ref().unwrap(), &draft);
    assert!(inspector.error.contains("草稿"));
    assert!(inspector.undo_draft());
    act(&Action::Open, &mut inspector, &mut document);
    assert!(!inspector.is_open(), "撤销至原值后可以收起");
    assert_eq!(document_snapshot(&document), before);
    assert!(!inspector.can_undo() && !inspector.can_redo());
}

#[test]
fn local_history_tracks_direct_text_and_staging_actions_without_document_transactions() {
    let mut document = document();
    let before = document_snapshot(&document);
    let mut inspector = Inspector::default();
    open(&mut inspector, &document);
    assert!(!inspector.can_undo() && !inspector.can_redo());
    inspector.draft.as_mut().unwrap().target = "2".into();
    inspector.draft.as_mut().unwrap().name = "草稿名称".into();
    assert!(inspector.can_undo(), "直接文本编辑在 refresh 前也可撤销");
    act(&Action::Active, &mut inspector, &mut document);
    act(&Action::AddStep, &mut inspector, &mut document);
    act(&Action::AddActivation(0), &mut inspector, &mut document);
    act(&Action::Moved(0, 0), &mut inspector, &mut document);
    let edited = inspector.draft.as_ref().unwrap().clone();
    assert!(inspector.undo_draft());
    assert!(!draft_plan(&inspector).steps[0].activations[0].moved);
    assert!(inspector.undo_draft());
    assert!(draft_plan(&inspector).steps[0].activations.is_empty());
    assert!(inspector.undo_draft());
    assert!(draft_plan(&inspector).steps.is_empty());
    assert!(inspector.undo_draft());
    assert!(!inspector.draft.as_ref().unwrap().active);
    assert!(inspector.undo_draft());
    assert_eq!(inspector.draft.as_ref().unwrap().name, "初始");
    assert_eq!(inspector.draft.as_ref().unwrap().target, "2");
    assert!(!inspector.undo_draft() && !inspector.can_undo());
    for _ in 0..5 {
        assert!(inspector.redo_draft());
    }
    assert_same_draft(inspector.draft.as_ref().unwrap(), &edited);
    assert!(!inspector.redo_draft() && !inspector.can_redo());
    assert_eq!(document_snapshot(&document), before);
    act(&Action::Apply, &mut inspector, &mut document);
    assert!(!inspector.is_open(), "{}", inspector.error);
    assert_eq!(document.history.undo_len(), 1);
    assert!(document.undo());
    assert_eq!(document.ship, before.0);
}

#[test]
fn local_history_ignores_noops_navigation_and_failures_and_clears_redo_on_real_edits() {
    let mut document = document();
    let before = document_snapshot(&document);
    let mut inspector = Inspector::default();
    open(&mut inspector, &document);
    act(&Action::AddStep, &mut inspector, &mut document);
    assert!(inspector.undo_draft() && inspector.can_redo());
    for action in [
        Action::Focus(Field::Name),
        Action::CycleTarget(true),
        Action::ToggleAdvanced,
        Action::ToggleTargets,
        Action::SelectStage(0),
        Action::SelectActivation(0, 0),
        Action::RemoveStep(99),
        Action::MoveStep(99, true),
        Action::ReorderStep { from: 99, to: 0 },
        Action::AddActivation(99),
    ] {
        act(&action, &mut inspector, &mut document);
        assert!(!inspector.can_undo(), "非编辑动作不新增历史：{action:?}");
        assert!(inspector.can_redo(), "非编辑动作保留 redo：{action:?}");
    }
    inspector.draft.as_mut().unwrap().throttle = "未完成数字".into();
    assert!(inspector.can_undo());
    assert!(!inspector.can_redo());
    assert!(!inspector.redo_draft(), "新文本分支清空旧 redo");
    assert!(inspector.undo_draft());
    assert_eq!(inspector.draft.as_ref().unwrap().throttle, "0");
    assert!(inspector.redo_draft());
    assert_eq!(inspector.draft.as_ref().unwrap().throttle, "未完成数字");
    assert_eq!(document_snapshot(&document), before);
    act(&Action::Cancel, &mut inspector, &mut document);
    assert!(!inspector.can_undo() && !inspector.can_redo());
    assert_eq!(document_snapshot(&document), before);
}

#[test]
fn local_history_exhaustion_leaves_document_undo_and_redo_available() {
    let mut document = document();
    assert!(document.execute(EditorCommand::SetActive(2, true)));
    let mut inspector = Inspector::default();
    open(&mut inspector, &document);
    act(&Action::Active, &mut inspector, &mut document);
    let external = document_snapshot(&document);
    assert!(inspector.undo_draft());
    assert!(!inspector.undo_draft());
    assert_eq!(document_snapshot(&document), external);
    assert!(document.undo());
    inspector.refresh(&document);
    assert!(inspector.can_redo());
    let undone_document = document_snapshot(&document);
    assert!(inspector.redo_draft());
    assert!(!inspector.redo_draft());
    assert_eq!(document_snapshot(&document), undone_document);
    assert!(document.redo());
    inspector.refresh(&document);
    assert!(inspector.error.is_empty());
    assert!(document.ship.parts[1].active);
    assert!(inspector.draft.as_ref().unwrap().active);
}

fn assert_one_staging_command(draft: &Draft, document: &EditorDocument) {
    let EditorCommand::Scoped { command, part } = draft.command(document).unwrap() else {
        panic!("属性事务必须限定明确实例");
    };
    assert_eq!(part, draft.key);
    let EditorCommand::Batch(commands) = *command else {
        panic!("属性事务必须保持单个 Batch");
    };
    assert_eq!(
        commands
            .iter()
            .filter(|command| matches!(command, EditorCommand::SetStaging(..)))
            .count(),
        1
    );
}

#[test]
fn apply_merges_draft_fields_with_parallel_geometry_and_unrelated_properties_as_one_transaction() {
    let mut document = document();
    let mut inspector = Inspector::default();
    open(&mut inspector, &document);
    inspector.draft.as_mut().unwrap().name = "本地名称".into();
    inspector.draft.as_mut().unwrap().target = "2".into();
    act(&Action::AddStep, &mut inspector, &mut document);
    act(&Action::AddActivation(0), &mut inspector, &mut document);
    assert!(document.execute(EditorCommand::Batch(vec![
        EditorCommand::Move {
            id: 1,
            from: (0.0, 0.0),
            to: (12.0, 7.0)
        },
        EditorCommand::Rotate(1),
        EditorCommand::SetActive(1, true),
        EditorCommand::SetThrottle(1, 0.6),
        EditorCommand::SetFuel(2, 7.0),
    ])));
    let external = document.ship.clone();
    let history_len = document.history.undo_len();
    inspector.refresh(&document);
    assert!(inspector.error.is_empty());
    inspector.preview = Preview::Activation(0, 0);
    assert_eq!(
        inspector.preview_keys(&document),
        vec![PartKey::new(0, 2, 0)]
    );
    assert_one_staging_command(inspector.draft.as_ref().unwrap(), &document);
    let mut expected = external.clone();
    let pod = expected.parts[0].pod.as_mut().unwrap();
    pod.name = "本地名称".into();
    pod.staging = inspector.draft.as_ref().unwrap().staging.clone();
    act(&Action::Apply, &mut inspector, &mut document);
    assert!(!inspector.is_open(), "{}", inspector.error);
    assert_eq!(
        document.ship, expected,
        "位置、姿态、外部字段及无关部件全部保留"
    );
    assert_eq!(document.history.undo_len(), history_len + 1);
    assert!(document.undo());
    assert_eq!(document.ship, external);
    assert_eq!(document.history.undo_len(), history_len);
    assert!(document.redo());
    assert_eq!(document.ship, expected);
}

#[test]
fn semantic_numeric_noops_preserve_external_fields_and_convergent_edits_add_no_transaction() {
    let mut document = document();
    document.select_only(Some(PartKey::new(0, 2, 0)));
    let mut inspector = Inspector::default();
    open(&mut inspector, &document);
    inspector.draft.as_mut().unwrap().fuel = "10.000".into();
    assert!(document.execute(EditorCommand::Batch(vec![
        EditorCommand::SetFuel(2, 7.0),
        EditorCommand::SetActive(2, true),
    ])));
    let external = document.ship.clone();
    let history_len = document.history.undo_len();
    act(&Action::Apply, &mut inspector, &mut document);
    assert!(!inspector.is_open(), "{}", inspector.error);
    assert_eq!(document.ship, external);
    assert_eq!(document.history.undo_len(), history_len);
    document.select_only(Some(PartKey::new(0, 1, 0)));
    open(&mut inspector, &document);
    inspector.draft.as_mut().unwrap().name = "相同名称".into();
    assert!(document.execute(EditorCommand::RenamePod(1, "相同名称".into())));
    let external = document.ship.clone();
    let history_len = document.history.undo_len();
    act(&Action::Apply, &mut inspector, &mut document);
    assert!(!inspector.is_open());
    assert_eq!(document.ship, external);
    assert_eq!(document.history.undo_len(), history_len);
}

#[test]
fn staging_steps_and_current_stage_merge_independently_with_exactly_one_set_staging() {
    for edit_steps in [true, false] {
        let mut document = document();
        let mut inspector = Inspector::default();
        open(&mut inspector, &document);
        let mut external_plan = StagingState::default();
        if edit_steps {
            act(&Action::AddStep, &mut inspector, &mut document);
            external_plan.current_stage = 4;
        } else {
            inspector.draft.as_mut().unwrap().current_stage = "3".into();
            external_plan.steps.push(StageStep {
                activations: vec![Activation { id: 2, moved: true }],
            });
        }
        assert!(document.execute(EditorCommand::SetStaging(1, Some(external_plan.clone()))));
        let external = document.ship.clone();
        assert_one_staging_command(inspector.draft.as_ref().unwrap(), &document);
        act(&Action::Apply, &mut inspector, &mut document);
        assert!(!inspector.is_open(), "{}", inspector.error);
        let plan = document.ship.parts[0]
            .pod
            .as_ref()
            .unwrap()
            .staging
            .as_ref()
            .unwrap();
        assert_eq!(plan.current_stage, if edit_steps { 4 } else { 3 });
        assert_eq!(
            plan.steps,
            if edit_steps {
                vec![StageStep::default()]
            } else {
                external_plan.steps
            }
        );
        assert_eq!(document.history.undo_len(), 2);
        assert!(document.undo());
        assert_eq!(document.ship, external);
    }
}

#[test]
fn same_field_conflicts_preserve_the_complete_draft_document_and_history() {
    for conflict in 0..5 {
        let mut document = document();
        let mut inspector = Inspector::default();
        open(&mut inspector, &document);
        let command = match conflict {
            0 => {
                inspector.draft.as_mut().unwrap().name = "本地名称".into();
                EditorCommand::RenamePod(1, "外部名称".into())
            }
            1 => {
                inspector.draft.as_mut().unwrap().throttle = "0.2".into();
                EditorCommand::SetThrottle(1, 0.7)
            }
            2 => {
                inspector.draft.as_mut().unwrap().current_stage = "2".into();
                EditorCommand::SetStaging(
                    1,
                    Some(StagingState {
                        current_stage: 3,
                        ..default()
                    }),
                )
            }
            3 => {
                act(&Action::AddStep, &mut inspector, &mut document);
                EditorCommand::SetStaging(
                    1,
                    Some(StagingState {
                        steps: vec![StageStep::default(); 2],
                        ..default()
                    }),
                )
            }
            _ => {
                inspector.draft.as_mut().unwrap().staging = None;
                EditorCommand::SetStaging(
                    1,
                    Some(StagingState {
                        steps: vec![StageStep::default()],
                        ..default()
                    }),
                )
            }
        };
        assert!(document.execute(command));
        let draft = inspector.draft.as_ref().unwrap().clone();
        let external = document_snapshot(&document);
        act(&Action::Apply, &mut inspector, &mut document);
        assert!(
            inspector.error.contains("冲突"),
            "场景 {conflict}: {}",
            inspector.error
        );
        assert_same_draft(inspector.draft.as_ref().unwrap(), &draft);
        assert_eq!(document_snapshot(&document), external);
        assert!(inspector.can_undo());
        assert!(document.undo());
        inspector.refresh(&document);
        act(&Action::Apply, &mut inspector, &mut document);
        assert!(
            !inspector.is_open(),
            "冲突撤销后应能应用：{}",
            inspector.error
        );
        assert_eq!(document.history.undo_len(), 1);
    }
}

#[test]
fn deleted_or_retyped_owner_keeps_draft_and_blocks_apply_generation_and_preview() {
    for invalid in 0..3 {
        let mut document = staging_document();
        let mut inspector = Inspector::default();
        open(&mut inspector, &document);
        inspector.draft.as_mut().unwrap().name = "保留草稿".into();
        match invalid {
            0 => assert!(document.execute(EditorCommand::Delete(1))),
            1 => document.ship.parts[0].part_type = "tank".into(),
            _ => document.ship.parts[0].pod = None,
        }
        let draft = inspector.draft.as_ref().unwrap().clone();
        let external = document_snapshot(&document);
        inspector.refresh(&document);
        assert!(!inspector.error.is_empty());
        assert!(inspector.preview_keys(&document).is_empty());
        for action in [Action::Apply, Action::GenerateStaging] {
            act(&action, &mut inspector, &mut document);
            assert!(!inspector.error.is_empty());
            assert_same_draft(inspector.draft.as_ref().unwrap(), &draft);
            assert_eq!(document_snapshot(&document), external);
        }
    }
}

#[test]
fn removed_or_ambiguous_targets_are_retained_in_draft_and_block_apply_until_resolved() {
    for duplicate in [false, true] {
        let mut document = document();
        document.ship.parts[0]
            .pod
            .as_mut()
            .unwrap()
            .staging
            .as_mut()
            .unwrap()
            .steps = vec![StageStep {
            activations: vec![Activation { id: 2, moved: true }],
        }];
        let original = document.ship.clone();
        let mut inspector = Inspector::default();
        open(&mut inspector, &document);
        inspector.draft.as_mut().unwrap().name = "保留未提交名称".into();
        if duplicate {
            document.ship.parts.push(document.ship.parts[1].clone());
            document.refresh();
        } else {
            assert!(document.execute(EditorCommand::Delete(2)));
        }
        let external = document_snapshot(&document);
        let draft = inspector.draft.as_ref().unwrap().clone();
        inspector.preview = Preview::Activation(0, 0);
        inspector.refresh(&document);
        assert!(inspector.error.contains("目标 2"));
        assert!(inspector.preview_keys(&document).is_empty());
        act(&Action::Apply, &mut inspector, &mut document);
        assert_same_draft(inspector.draft.as_ref().unwrap(), &draft);
        assert_eq!(document_snapshot(&document), external);
        if duplicate {
            document.ship = original;
            document.refresh();
        } else {
            assert!(document.undo());
        }
        inspector.refresh(&document);
        assert!(inspector.error.is_empty(), "目标恢复后清除本轮诊断");
        assert_same_draft(inspector.draft.as_ref().unwrap(), &draft);
        act(&Action::Apply, &mut inspector, &mut document);
        assert!(!inspector.is_open(), "{}", inspector.error);
    }
}

#[test]
fn original_missing_targets_remain_compatible_for_unrelated_property_changes() {
    let mut document = document();
    document.ship.parts[0]
        .pod
        .as_mut()
        .unwrap()
        .staging
        .as_mut()
        .unwrap()
        .steps = vec![StageStep {
        activations: vec![Activation {
            id: 999,
            moved: false,
        }],
    }];
    let original = document.ship.clone();
    let mut inspector = Inspector::default();
    open(&mut inspector, &document);
    assert!(inspector.error.is_empty());
    inspector.draft.as_mut().unwrap().name = "仅改名称".into();
    act(&Action::Apply, &mut inspector, &mut document);
    assert!(!inspector.is_open(), "{}", inspector.error);
    assert_eq!(
        document.ship.parts[0].pod.as_ref().unwrap().staging,
        original.parts[0].pod.as_ref().unwrap().staging
    );
    assert_eq!(document.history.undo_len(), 1);
    assert!(document.undo());
    assert_eq!(document.ship, original);
}

#[test]
fn newly_invalid_target_references_cannot_be_applied_and_can_be_undone_locally() {
    let mut document = document();
    let before = document_snapshot(&document);
    let mut inspector = Inspector::default();
    open(&mut inspector, &document);
    inspector
        .draft
        .as_mut()
        .unwrap()
        .staging
        .as_mut()
        .unwrap()
        .steps
        .push(StageStep {
            activations: vec![Activation {
                id: 999,
                moved: false,
            }],
        });
    inspector.refresh(&document);
    assert!(inspector.error.contains("目标 999"));
    act(&Action::Apply, &mut inspector, &mut document);
    assert!(inspector.is_open());
    assert_eq!(document_snapshot(&document), before);
    assert!(inspector.undo_draft());
    inspector.refresh(&document);
    assert!(inspector.error.is_empty());
    assert!(draft_plan(&inspector).steps.is_empty());
    assert_eq!(document_snapshot(&document), before);
}

#[test]
fn generate_staging_accepts_parallel_owner_geometry_and_property_changes() {
    let mut document = staging_document();
    let mut inspector = Inspector::default();
    open(&mut inspector, &document);
    inspector.draft.as_mut().unwrap().name = "保留本地名称".into();
    let owner = &mut document.ship.parts[0];
    owner.x = 12.0;
    owner.y = -4.0;
    owner.angle = 1.5;
    owner.editor_angle = 90;
    owner.active = !owner.active;
    let expected =
        dr_core::generate_basic_staging(&document.ship, &document.catalog, PartKey::new(0, 1, 0))
            .unwrap();
    let external = document_snapshot(&document);
    act(&Action::GenerateStaging, &mut inspector, &mut document);
    assert!(inspector.error.is_empty(), "{}", inspector.error);
    assert_eq!(draft_plan(&inspector), &expected);
    assert_eq!(inspector.draft.as_ref().unwrap().name, "保留本地名称");
    assert!(!inspector.preview_keys(&document).is_empty());
    assert_eq!(document_snapshot(&document), external);
}
