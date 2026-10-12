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
    world.init_resource::<CameraDrag>();
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
        Window {
            resolution: (1440, 900).into(),
            focused: true,
            ..default()
        },
        bevy::window::PrimaryWindow,
        EguiInput::default(),
        bevy_egui::PrimaryEguiContext,
    ));
    world.spawn((
        Camera2d,
        Transform::default(),
        GlobalTransform::default(),
        Projection::Orthographic(OrthographicProjection::default_2d()),
    ));
}

#[test]
fn detailed_manifest_has_45_unique_explained_cases_and_unique_evidence() {
    let mut ids = BTreeSet::new();
    for chapter in [
        "placement-cases",
        "geometry-cases",
        "free-cases",
        "group-cases",
        "line-cases",
        "interaction-cases",
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
    assert_eq!(ids.len(), 45);
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

fn gesture_frame(mut state: ResMut<Run>, mut ctx: Context) {
    if state.verified {
        return;
    }
    ctx.mouse.clear();
    ctx.keys.reset_all();
    let case = &cases("interaction-cases")[state.case_index];
    if state.step == 0 {
        setup(&mut state, &mut ctx, case);
    } else if gestures::advance(&mut state, &mut ctx, case.scenario) {
        gestures::report(&mut state);
        state.verified = true;
    }
}

fn replay_gesture(index: usize) -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default(), TransformPlugin))
        .init_asset::<Image>()
        .init_resource::<panels::Palette>()
        .init_resource::<panels::UiPointer>()
        .add_message::<MouseWheel>();
    populate(app.world_mut());
    let mut state = Run::new("interaction-cases");
    state.case_index = index;
    app.insert_resource(state)
        .add_systems(
            Update,
            (
                gesture_frame,
                camera_controls,
                mouse_editor,
                keyboard_commands,
                render::sync,
            )
                .chain(),
        )
        .add_systems(
            PostUpdate,
            gestures::verify.after(TransformSystems::Propagate),
        );
    for _ in 0..100 {
        app.update();
        if app.world().resource::<Run>().verified {
            return app;
        }
    }
    panic!("交互演示未结束：{}", cases("interaction-cases")[index].id);
}

#[test]
fn continuous_drag_demo_checks_actual_visuals_and_release_before_atomic_undo_redo() {
    let app = replay_gesture(0);
    let state = app.world().resource::<Run>();
    assert!(state.metrics["same_frame_checks"].as_u64().unwrap() >= 10);
    assert_eq!(state.metrics["release_frame_checks"], 1);
    assert!(state.metrics["max_world_pixel_error"].as_f64().unwrap() < 0.002);
    assert_eq!(
        app.world().resource::<EditorDocument>().history.undo_len(),
        1
    );
}

#[test]
fn background_pan_demo_checks_both_buttons_and_scales_without_editing_document() {
    let app = replay_gesture(1);
    let state = app.world().resource::<Run>();
    assert!(state.metrics["same_frame_checks"].as_u64().unwrap() >= 12);
    assert_eq!(state.metrics["release_frame_checks"], 2);
    assert_eq!(state.metrics["scales"], serde_json::json!([1, 2]));
    unchanged(state, app.world().resource::<EditorDocument>());
}

#[test]
fn single_hint_demo_commits_one_connection_from_multiple_geometric_candidates() {
    let app = replay_gesture(2);
    let state = app.world().resource::<Run>();
    assert!(state.metrics["geometric_candidates"].as_u64().unwrap() > 1);
    assert_eq!(state.metrics["displayed_pairs"], 1);
    assert_eq!(state.metrics["committed_connections"], 1);
    assert_eq!(state.metrics["hinted_connection_matches_commit"], true);
    assert_eq!(state.metrics["undo_redo"], true);
}

#[test]
fn manual_duplicate_demo_rejects_new_attach_pairs_in_both_directions_via_canvas_clicks() {
    let app = replay_gesture(3);
    let state = app.world().resource::<Run>();
    assert_eq!(state.metrics["different_attach_rejected"], true);
    assert_eq!(state.metrics["reverse_with_other_attaches_rejected"], true);
    assert_eq!(state.metrics["history_unchanged_on_rejection"], true);
    let document = app.world().resource::<EditorDocument>();
    assert_eq!(document.ship.connections.len(), 1);
    assert_eq!(document.history.undo_len(), 1);
    assert!(document.dirty);
}
