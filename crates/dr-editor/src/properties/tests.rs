use super::*;

fn document() -> EditorDocument {
    let mut document = crate::tests::document();
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
    document.execute(EditorCommand::SetActive(2, true));
    act(&Action::Apply, &mut inspector, &mut document);
    assert!(inspector.error.contains("已变更"));
    assert_eq!(document.ship.part(2).unwrap().fuel, Some(10.0));
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
