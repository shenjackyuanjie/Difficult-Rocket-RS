use super::*;
use bevy::ecs::system::SystemState;

fn world() -> World {
    let mut world = World::new();
    populate(&mut world);
    world
}

fn populate(world: &mut World) {
    let catalog = dr_core::catalog_from_xml(
        r#"<PartTypes>
      <PartType id="fuselage-1" width="4" height="4"><AttachPoints>
        <AttachPoint location="TopCenter"/><AttachPoint location="BottomCenter"/>
        <AttachPoint location="LeftSide"/><AttachPoint location="RightSide"/>
      </AttachPoints></PartType>
      <PartType id="nosecone-1" width="4" height="2"/>
      <PartType id="dock-1" type="dockconnector" width="4" height="1"><AttachPoints>
        <AttachPoint location="BottomCenter"/>
      </AttachPoints></PartType>
    </PartTypes>"#,
    )
    .unwrap();
    world.insert_resource(EditorDocument {
        free_mode: false,
        revision: 0,
        ship: Ship::default(),
        saved_ship: Ship::default(),
        catalog,
        selected: None,
        selection: BTreeSet::new(),
        dirty: false,
        history: EditorHistory::default(),
        status: String::new(),
    });
    world.init_resource::<EditorCursor>();
    world.init_resource::<DragState>();
    world.init_resource::<view::ViewOptions>();
    world.init_resource::<connection_lines::Settings>();
    world.init_resource::<connection_lines::Controls>();
    world.init_resource::<bevy::gizmos::config::GizmoConfigStore>();
    world.init_resource::<topology_ui::ConnectionEditor>();
    world.init_resource::<properties::Inspector>();
    world.init_resource::<egui_ui::UiHits>();
    world.init_resource::<files::PendingFileAction>();
    world.insert_resource(EditorPaths {
        ship: None,
        catalog: String::new(),
        assets: String::new(),
    });
    world.init_resource::<ButtonInput<MouseButton>>();
    world.init_resource::<ButtonInput<KeyCode>>();
    world.init_resource::<Messages<files::FileAction>>();
    world.spawn((
        Window::default(),
        bevy::window::PrimaryWindow,
        EguiInput::default(),
        bevy_egui::PrimaryEguiContext,
    ));
}

#[test]
fn detailed_manifest_has_41_unique_explained_cases_and_unique_evidence() {
    let mut ids = BTreeSet::new();
    for chapter in [
        "placement-cases",
        "geometry-cases",
        "free-cases",
        "group-cases",
        "line-cases",
        "history-cases",
        "topology-cases",
        "file-cases",
    ] {
        let list = cases(chapter);
        assert!(!list.is_empty());
        let files = artifacts(chapter);
        assert_eq!(files.len(), list.len());
        for (case, filename) in list.iter().zip(files) {
            assert!(ids.insert(case.id), "重复的样例身份");
            assert!(
                !case.title.is_empty() && !case.expected.is_empty() && !case.input_path.is_empty()
            );
            assert_eq!(std::path::Path::new(&filename).components().count(), 1);
        }
        let mut state = Run::new(chapter);
        assert!(state.caption().unwrap().contains("预期："));
        state.case_index = list.len();
        assert!(state.caption().is_none());
    }
    assert_eq!(ids.len(), 41);
    assert!(Run::new("transforms").caption().is_none());
}

#[test]
fn exact_geometry_demo_fixtures_execute_real_transactions_and_keep_failed_history_clean() {
    for case in cases("geometry-cases") {
        let mut world = world();
        let mut params = SystemState::<Context>::new(&mut world);
        let mut ctx = params.get_mut(&mut world).unwrap();
        let mut state = Run::new("geometry-cases");
        setup(&mut state, &mut ctx, case);
        assert!(!ctx.document.free_mode);
        assert!(!geometry::advance(&mut state, &mut ctx, case.scenario));
        let accepted = state.metrics["accepted"].as_bool().unwrap();
        let done = geometry::advance(&mut state, &mut ctx, case.scenario);
        assert_eq!(done, !accepted);
        if accepted {
            assert_eq!(ctx.document.history.undo_len(), 1);
            assert!(ctx.document.undo());
            unchanged(&state, &ctx.document);
            assert!(ctx.document.redo());
            assert_eq!(state.after.as_ref(), Some(&ctx.document.ship));
        }
    }
}

#[test]
fn independent_cases_reset_configuration_selection_and_document_history() {
    let mut world = world();
    let mut params = SystemState::<Context>::new(&mut world);
    let mut ctx = params.get_mut(&mut world).unwrap();
    let mut state = Run::new("line-cases");
    setup(&mut state, &mut ctx, &cases("line-cases")[2]);
    assert!(!ctx.lines.enabled);
    ctx.document.select_only(Some(key(1)));
    assert!(ctx.document.execute(EditorCommand::Rotate(1)));
    ctx.document.status = "前一夹具的错误不能带入下一例".into();
    setup(&mut state, &mut ctx, &cases("free-cases")[0]);
    assert_eq!(*ctx.lines, connection_lines::Settings::default());
    assert!(ctx.document.selected_keys().is_empty());
    assert!(ctx.document.free_mode);
    assert!(ctx.document.status.is_empty());
    unchanged(&state, &ctx.document);
}

#[test]
fn explicit_docking_probe_rejects_normal_endpoints_without_changing_original_mount_points() {
    let mut world = world();
    let mut params = SystemState::<Context>::new(&mut world);
    let mut ctx = params.get_mut(&mut world).unwrap();
    let mut state = Run::new("free-cases");
    setup(&mut state, &mut ctx, &cases("free-cases")[5]);
    assert!(!ctx.document.catalog.get("dock-1").unwrap().attach_points[0].dock);
    assert!(
        ctx.document
            .catalog
            .get("demo-explicit-dock")
            .unwrap()
            .attach_points[0]
            .dock
    );
    let a = ctx.endpoint(1, 0);
    let b = ctx.endpoint(2, 0);
    assert!(free_mode::click(&mut ctx.document, &mut ctx.cursor, a, 0.1));
    assert!(free_mode::click(&mut ctx.document, &mut ctx.cursor, b, 0.1));
    unchanged(&state, &ctx.document);
    assert!(ctx.document.status.contains("对接"));
    setup(&mut state, &mut ctx, &cases("free-cases")[6]);
    assert!(ctx.document.catalog.get("demo-explicit-dock").is_none());
}

#[test]
fn an_asserted_case_does_not_advance_or_exit_until_its_screenshot_is_captured() {
    let mut app = App::new();
    populate(app.world_mut());
    let mut state = Run::new("geometry-cases");
    state.verified = true;
    app.insert_resource(state)
        .init_resource::<Captured>()
        .init_resource::<Messages<AppExit>>()
        .add_systems(Update, run);
    app.update();
    app.update();
    let state = app.world().resource::<Run>();
    assert_eq!(state.case_index, 0);
    assert!(state.results.is_empty());
    assert!(state.capturing);
    assert!(app.world().resource::<Messages<AppExit>>().is_empty());
    app.world_mut()
        .resource_mut::<Captured>()
        .0
        .insert("overlap-below".into());
    app.update();
    let state = app.world().resource::<Run>();
    assert_eq!(state.case_index, 1);
    assert_eq!(state.results.len(), 1);
    assert!(!state.verified);
    assert!(!state.capturing);
    assert_eq!(state.results[0]["status"], "passed");
}
