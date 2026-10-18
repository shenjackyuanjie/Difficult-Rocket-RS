use super::*;
use crate::canvas_input::{keyboard_commands, mouse_editor};
use crate::view::camera_controls;
use bevy::ecs::system::SystemState;
use bevy::input::mouse::MouseWheel;
use bevy_egui::egui;

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
        snap_enabled: true,
        revision: 0,
        ship: Ship::default(),
        saved_ship: Ship::default(),
        catalog,
        selected: None,
        selection: BTreeSet::new(),
        dirty: false,
        history: EditorHistory::default(),
        status: String::new(),
        sfx: Vec::new(),
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
fn detailed_manifest_has_51_unique_explained_cases_and_unique_evidence() {
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
        "narrow-layout",
        "mobile-input",
        "performance",
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
    assert_eq!(ids.len(), 51);
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
fn coincident_parts_with_surface_mounts_still_have_a_nonzero_render_path() {
    let mut world = world();
    let mut params = SystemState::<Context>::new(&mut world);
    let mut ctx = params.get_mut(&mut world).unwrap();
    let mut state = Run::new("line-cases");
    setup(&mut state, &mut ctx, &cases("line-cases")[0]);
    let kind = ctx.document.catalog.get("fuselage-1").unwrap();
    ctx.document.ship.parts = vec![
        kind.instantiate(1, (2.0, -1.0)),
        kind.instantiate(2, (2.0, -1.0)),
    ];
    ctx.document.ship.connections = vec![Connection::Normal {
        parent: 1,
        child: 2,
        parent_attach: 1,
        child_attach: 1,
    }];
    let points = connection_path(&ctx.document);
    assert_eq!(points[0], points[3]);
    assert!(
        points
            .windows(2)
            .map(|pair| pair[0].distance(pair[1]))
            .sum::<f32>()
            > 0.0
    );
    assert!(connection_lines::along(&points, 0.5).is_some());
}

#[test]
fn zero_length_case_checks_real_mount_path_and_queues_no_gizmo_vertices() {
    let mut world = world();
    let index = cases("line-cases")
        .iter()
        .position(|case| matches!(case.scenario, Scenario::ZeroLength))
        .unwrap();
    let mut state = Run::new("line-cases");
    state.case_index = index;
    {
        let mut params = SystemState::<Context>::new(&mut world);
        let mut ctx = params.get_mut(&mut world).unwrap();
        setup(&mut state, &mut ctx, &cases("line-cases")[index]);
        let points = connection_path(&ctx.document);
        assert_ne!(points[0], Vec2::ZERO, "真实夹具不依赖世界原点");
        assert!(points.iter().all(|point| *point == points[0]));
        assert!(!interactions::advance(
            &mut state,
            &mut ctx,
            Scenario::ZeroLength
        ));
        assert!(ctx.document.catalog.get("demo-zero-length").is_some());
        assert!(
            !ctx.document
                .catalog
                .get("fuselage-1")
                .unwrap()
                .attach_points[0]
                .location
                .is_empty()
        );
    }
    world.resource_mut::<GizmoConfigStore>().insert(
        bevy::gizmos::config::GizmoConfig::default(),
        connection_lines::LineGizmos,
    );
    world.insert_resource(state);
    {
        let mut params = SystemState::<(
            ResMut<Run>,
            Res<EditorDocument>,
            Res<connection_lines::Settings>,
            Gizmos<connection_lines::LineGizmos>,
        )>::new(&mut world);
        let (state, document, lines, mut gizmos) = params.get_mut(&mut world).unwrap();
        // 正常路径确实会提交顶点，不能把未启用的 Gizmos 误当退化路径安全跳过。
        connection_lines::draw_path(&mut gizmos, &lines, &[Vec2::ZERO, Vec2::X * 60.0], 0.5, 1.0);
        let before = (gizmos.list_positions.len(), gizmos.strip_positions.len());
        assert!(before.0 > 0 && before.1 > 0);
        draw_degenerate(state, document, lines, gizmos);
        let (_, _, _, gizmos) = params.get_mut(&mut world).unwrap();
        assert_eq!(
            (gizmos.list_positions.len(), gizmos.strip_positions.len()),
            before
        );
    }
    let mut state = world.remove_resource::<Run>().unwrap();
    assert!(state.zero_path_checked);
    assert_eq!(state.metrics["path_point_count"], 4);
    assert_eq!(state.metrics["path_length"], 0.0);
    assert_eq!(state.metrics["queued_vertex_count"], 0);
    assert_eq!(state.metrics["direction_sample_count"], 0);
    assert_eq!(state.metrics["degenerate_path_safely_skipped"], true);
    assert!(
        state
            .metrics
            .get("zero_length_path_actually_drawn")
            .is_none()
    );
    let mut params = SystemState::<Context>::new(&mut world);
    let mut ctx = params.get_mut(&mut world).unwrap();
    assert!(interactions::advance(
        &mut state,
        &mut ctx,
        Scenario::ZeroLength
    ));
    unchanged(&state, &ctx.document);
    setup(&mut state, &mut ctx, &cases("free-cases")[0]);
    assert!(!state.zero_path_checked);
    assert!(ctx.document.catalog.get("demo-zero-length").is_none());
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
        .init_resource::<crate::properties::Inspector>()
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
fn populate_input(world: &mut World) {
    world.init_resource::<panels::UiPointer>();
    world.init_resource::<mobile_input::GestureInput>();
    world.init_resource::<Messages<bevy::input::touch::TouchInput>>();
    world.init_resource::<Messages<bevy::window::WindowFocused>>();
}

#[test]
fn narrow_layout_helpers_measure_canvas_and_project_part_centers() {
    let mut world = world();
    populate_input(&mut world);
    {
        let mut document = world.resource_mut::<EditorDocument>();
        let kind = document.catalog.get("fuselage-1").unwrap();
        document.ship = Ship {
            parts: vec![kind.instantiate(1, (0., 0.)), kind.instantiate(2, (2., 1.))],
            ..default()
        };
    }
    let mut ui = panels::egui_panel::UiState::default();
    ui.pixels_per_point = 1.0;
    ui.canvas_area = Some(egui::Rect::from_min_size(
        egui::pos2(0., 0.),
        egui::vec2(640., 304.),
    ));
    let window = Window {
        resolution: (640, 360).into(),
        ..default()
    };
    let canvas = layout::canvas_rect(&window, &ui).expect("画布应换算到窗口坐标");
    assert_eq!((canvas.width(), canvas.height()), (640., 304.));
    // 超出窗口的画布矩形不被当成真实布局。
    ui.canvas_area = Some(egui::Rect::from_min_size(
        egui::pos2(0., 0.),
        egui::vec2(700., 304.),
    ));
    assert!(layout::canvas_rect(&window, &ui).is_none());
    let center = layout::screen_point((0., 0.), Vec2::ZERO, 1., Vec2::new(640., 360.));
    assert_eq!(center, Vec2::new(320., 180.));
    let offset = layout::screen_point((2., 1.), Vec2::ZERO, 2., Vec2::new(640., 360.));
    assert_eq!(offset, Vec2::new(380., 150.));
    let document = world.resource::<EditorDocument>();
    let centers: Vec<_> = document
        .ship
        .keyed_parts()
        .map(|(_, part)| {
            layout::screen_point((part.x, part.y), Vec2::ZERO, 1., Vec2::new(640., 360.))
        })
        .collect();
    assert!(layout::centers_inside(document, &centers, canvas));
    let shifted = centers
        .iter()
        .map(|point| *point + Vec2::splat(400.))
        .collect::<Vec<_>>();
    assert!(!layout::centers_inside(document, &shifted, canvas));
}

#[test]
fn frame_stats_report_average_median_p95_and_max_of_real_samples() {
    let mut samples = vec![16.0, 4.0, 8.0, 32.0, 2.0];
    let (average, median, p95, max) = perf_drag::frame_stats(&mut samples);
    assert_eq!(samples, vec![2.0, 4.0, 8.0, 16.0, 32.0]);
    assert!((average - 12.4).abs() < 1e-9);
    assert_eq!(median, 8.0);
    assert_eq!(p95, 32.0);
    assert_eq!(max, 32.0);
}

fn mobile_frame(mut state: ResMut<Run>, mut ctx: Context) {
    if state.verified {
        return;
    }
    ctx.mouse.clear();
    ctx.keys.reset_all();
    let case = &cases("mobile-input")[state.case_index];
    if state.step == 0 {
        setup(&mut state, &mut ctx, case);
    } else if mobile::advance(&mut state, &mut ctx, case.scenario) {
        state.verified = true;
    }
}

fn replay_mobile(index: usize) -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default(), TransformPlugin))
        .init_asset::<Image>()
        .init_resource::<panels::Palette>()
        .add_message::<MouseWheel>();
    populate(app.world_mut());
    populate_input(app.world_mut());
    let mut state = Run::new("mobile-input");
    state.case_index = index;
    app.insert_resource(state)
        .init_resource::<crate::properties::Inspector>()
        .add_systems(
            Update,
            (
                mobile_frame,
                crate::mobile_input::input,
                panels::pointer_over_ui,
                mouse_editor,
                keyboard_commands,
            )
                .chain(),
        );
    for _ in 0..400 {
        app.update();
        if app.world().resource::<Run>().verified {
            return app;
        }
    }
    panic!("输入提交演示未结束：{}", cases("mobile-input")[index].id);
}

#[test]
fn touch_case_previews_on_press_and_commits_exactly_once_on_release() {
    let app = replay_mobile(0);
    let state = app.world().resource::<Run>();
    assert_eq!(state.metrics["committed_on"], "touch-release");
    assert_eq!(state.metrics["preview_committed"], false);
    assert_eq!(state.metrics["press_commits"], false);
    assert_eq!(state.metrics["release_commits"], 1);
    assert_eq!(state.metrics["duplicate_commits"], 0);
    assert_eq!(state.metrics["history_entries"], 1);
    let document = app.world().resource::<EditorDocument>();
    assert_eq!(document.ship.all_parts().count(), 5);
    assert_eq!(document.history.undo_len(), 1);
}

#[test]
fn mouse_case_commits_on_the_press_edge_without_a_second_commit() {
    let app = replay_mobile(1);
    let state = app.world().resource::<Run>();
    assert_eq!(state.metrics["committed_on"], "mouse-press");
    assert_eq!(state.metrics["release_pending"], true);
    assert_eq!(state.metrics["duplicate_commits"], 0);
    assert_eq!(state.metrics["history_entries"], 1);
    let document = app.world().resource::<EditorDocument>();
    assert_eq!(document.ship.all_parts().count(), 5);
    assert_eq!(document.history.undo_len(), 1);
}
fn bulk_frame(mut state: ResMut<Run>, mut ctx: Context) {
    if state.verified {
        return;
    }
    ctx.mouse.clear();
    ctx.keys.reset_all();
    let case = &cases("performance")[0];
    if state.step == 0 {
        setup(&mut state, &mut ctx, case);
    } else if perf_drag::advance(&mut state, &mut ctx, case.scenario) {
        state.verified = true;
    }
}

fn replay_bulk(count: usize) -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default(), TransformPlugin))
        .init_asset::<Image>()
        .init_resource::<panels::Palette>()
        .add_message::<MouseWheel>();
    populate(app.world_mut());
    populate_input(app.world_mut());
    {
        let mut document = app.world_mut().resource_mut::<EditorDocument>();
        let kind = document.catalog.get("fuselage-1").unwrap().clone();
        document.ship = Ship {
            parts: (0..count)
                .map(|index| {
                    let (column, row) = ((index % 40) as f64, (index / 40) as f64);
                    kind.instantiate(index as i64 + 1, (column * 6. - 120., row * 6. - 60.))
                })
                .collect(),
            ..default()
        };
    }
    let state = Run::new("performance");
    app.insert_resource(state)
        .init_resource::<crate::properties::Inspector>()
        .add_systems(
            Update,
            (bulk_frame, mouse_editor, keyboard_commands).chain(),
        );
    for _ in 0..400 {
        app.update();
        if app.world().resource::<Run>().verified {
            return app;
        }
    }
    panic!("1000 部件拖动演示未结束");
}

#[test]
fn bulk_drag_replays_one_thousand_parts_with_real_frame_samples_and_intact_connections() {
    let app = replay_bulk(1000);
    let state = app.world().resource::<Run>();
    assert_eq!(state.metrics["selected"], 1000);
    assert_eq!(state.metrics["connections_unchanged"], true);
    assert_eq!(state.metrics["all_selected_moved_together"], true);
    assert_eq!(state.metrics["unselected_unchanged"], true);
    assert_eq!(state.metrics["undo_restored"], true);
    assert_eq!(state.metrics["redo_restored"], true);
    assert!(state.metrics["frame_samples"].as_u64().unwrap() >= 32);
    let average = state.metrics["frame_average_ms"].as_f64().unwrap();
    assert!(average > 0.);
    let document = app.world().resource::<EditorDocument>();
    assert_eq!(document.history.undo_len(), 1);
    assert_eq!(document.ship.all_parts().count(), 1000);
}
#[test]
fn free_mode_press_probe_reports_when_the_center_hits_a_connection_point() {
    let mut world = world();
    {
        let mut document = world.resource_mut::<EditorDocument>();
        let kind = document.catalog.get("fuselage-1").unwrap().clone();
        document.ship = Ship {
            parts: vec![kind.instantiate(1, (0., 0.))],
            ..default()
        };
        document.free_mode = true;
    }
    let document = world.resource::<EditorDocument>();
    // 放大时连接点命中半径覆盖零件中心，按下会被当成选点。
    assert!(!perf_drag::press_is_plain_click(document, (0., 0.), 10.0));
    // 缩放到常规演示比例后中心不再是连接点。
    assert!(perf_drag::press_is_plain_click(document, (0., 0.), 0.2));
    let mut plain = world.resource_mut::<EditorDocument>();
    plain.free_mode = false;
    assert!(perf_drag::press_is_plain_click(&plain, (0., 0.), 10.0));
}
