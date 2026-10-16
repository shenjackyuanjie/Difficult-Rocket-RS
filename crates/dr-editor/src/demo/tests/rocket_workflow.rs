//! 无窗口回归：目录消息/画布生产系统、真实 egui 控件与同一四帧拖放发生器。
use super::*;
use properties::Action;

fn document() -> EditorDocument {
    let mut document = crate::test_support::document();
    // 直接读取只读原版目录，安装点、尺寸、形状与窗口演示完全相同。
    document.catalog = dr_core::catalog_from_xml(
        &std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../Difficult-Rocket/assets/builtin/PartList.xml"
        ))
        .unwrap(),
    )
    .unwrap();
    document.ship = new_ship(&document.catalog);
    document.saved_ship = document.ship.clone();
    document
}

#[test]
fn hidden_pod_and_attachment_order_drive_the_read_only_placement_plan() {
    let mut document = document();
    assert!(document.catalog.visible().all(|kind| kind.id != "pod-1"));
    document
        .catalog
        .types
        .iter_mut()
        .find(|kind| kind.id == "pod-1")
        .unwrap()
        .attach_points
        .swap(0, 1);
    document.ship.parts[0].x = 3.25;
    document.ship.parts[0].y = 12.0;
    let before = document.ship.clone();
    let plan = placement(&document, 0);
    assert_eq!((plan.parent, plan.child), (1, 2));
    assert_eq!((plan.parent_attach, plan.child_attach), (1, 1));
    assert_eq!(plan.position, (3.25, 10.75));
    let parent = document.ship.part(1).unwrap();
    let child_kind = document.catalog.get("fueltank-0").unwrap();
    let child = child_kind.instantiate(plan.child, plan.position);
    let parent_endpoint = dr_core::part_world_attach(
        parent,
        &document.catalog.get("pod-1").unwrap().attach_points[0],
    );
    let child_endpoint = dr_core::part_world_attach(&child, &child_kind.attach_points[0]);
    assert!(parent_endpoint.distance(child_endpoint) < 1e-10);
    let bottom = assembly_bottom(&document);
    assert!(bottom.1 < plan.position.1);
    assert_eq!(document.ship, before);
    assert_eq!(document.revision, 0);
    assert_eq!(document.history.undo_len(), 0);
}

#[test]
fn screen_position_uses_current_camera_translation_and_zoom() {
    let window = Window {
        resolution: bevy::window::WindowResolution::new(1000, 800),
        ..default()
    };
    let camera = Transform::from_xyz(120.0, 60.0, 0.0);
    let mut projection = OrthographicProjection::default_2d();
    projection.scale = 2.0;
    assert_eq!(
        screen(
            &window,
            &camera,
            &Projection::Orthographic(projection),
            (4.0, -1.0)
        ),
        Vec2::new(560.0, 460.0)
    );
}

fn canvas_point(app: &mut App, position: (f64, f64)) {
    let world = app.world_mut();
    let mut windows = world.query_filtered::<&mut Window, With<bevy::window::PrimaryWindow>>();
    let mut window = windows.single_mut(world).unwrap();
    let mut projection = OrthographicProjection::default_2d();
    projection.scale = 10.0;
    let point = screen(
        &window,
        &Transform::IDENTITY,
        &Projection::Orthographic(projection),
        position,
    );
    window.set_cursor_position(Some(point));
}

/// 测试夹具走目录的生产消息消费者和生产画布入口；运行时 demo 另断言真实目录 hit。
fn assembled() -> EditorDocument {
    let mut app = crate::test_support::mouse_app();
    app.insert_resource(document())
        .init_resource::<crate::properties::Inspector>()
        .init_resource::<panels::Palette>()
        .init_resource::<panels::ShipBrowser>()
        .add_message::<panels::PanelButton>()
        .add_message::<files::FileAction>()
        .add_systems(
            Update,
            panels::panel_actions.before(canvas_input::mouse_editor),
        );
    // 与固定测试投影保持一致：缩小十倍，真实 SRB 高度也能完整落在画布内。
    {
        let world = app.world_mut();
        let mut cameras = world.query_filtered::<(&mut Camera, &mut Projection), With<Camera2d>>();
        let (mut camera, mut projection) = cameras.single_mut(world).unwrap();
        camera.computed.clip_from_view =
            Mat4::from_scale(Vec3::new(1.0 / 7200.0, 1.0 / 4500.0, 1.0));
        if let Projection::Orthographic(projection) = &mut *projection {
            projection.scale = 10.0;
        }
    }
    for (index, kind_id) in PARTS.iter().enumerate() {
        let document = app.world().resource::<EditorDocument>();
        let catalog_index = document
            .catalog
            .visible()
            .position(|kind| kind.id == *kind_id)
            .unwrap();
        let before = document.ship.clone();
        let revision = document.revision;
        let plan = placement(document, index);
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .reset_all();
        app.world_mut()
            .resource_mut::<Messages<panels::PanelButton>>()
            .write(panels::PanelButton::Part(catalog_index));
        canvas_point(&mut app, approach(plan, index));
        app.update();
        let document = app.world().resource::<EditorDocument>();
        let cursor = app.world().resource::<EditorCursor>();
        assert!(cursor.valid && cursor.placing && !cursor.palette_drag);
        assert_eq!(document.ship, before);
        assert_eq!(document.revision, revision);
        assert_eq!(document.history.undo_len(), index);
        let (part, connection, allowed) = placement::preview(document, cursor).unwrap();
        assert!(allowed);
        assert!(
            matches!(connection,
            Some(EditorCommand::ConnectParts { parent, child, kind: LinkKind::Normal { parent_attach, child_attach } })
            if (parent.id, child.id, parent_attach, child_attach) == (plan.parent, plan.child, plan.parent_attach, plan.child_attach)),
            "第 {} 件的生产预览未使用计划安装点：{:?}",
            index + 2,
            connection
        );
        assert!((part.x - plan.position.0).hypot(part.y - plan.position.1) < 1e-5);
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        let document = app.world().resource::<EditorDocument>();
        assert_eq!(document.ship.parts.len(), index + 2);
        assert_eq!(document.ship.connections.len(), index + 1);
        assert_eq!(document.history.undo_len(), index + 1);
        assert_eq!(
            dr_core::ship_from_xml(&dr_core::ship_to_xml(&document.ship).unwrap()).unwrap(),
            document.ship,
            "每次真实放置后的 XML 往返必须保留部件、连接和状态"
        );
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .clear();
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .release(MouseButton::Left);
        app.update();
    }
    app.world_mut().remove_resource::<EditorDocument>().unwrap()
}

#[test]
fn sixteen_real_catalog_parts_are_placed_with_fifteen_contact_tree_edges() {
    let document = assembled();
    assert!(assert_rocket(&document) < 1e-5);
    // 生产输入构成的同组无向树必须由真实核心算法接受，精确校验每张卡及标记。
    let generated =
        dr_core::generate_basic_staging(&document.ship, &document.catalog, PartKey::new(0, 1, 0))
            .unwrap();
    assert_staging(&generated, AUTO, false);
    assert!(staging(&document.ship).unwrap().steps.is_empty());
    assert_eq!(document.history.undo_len(), 15);
    assert_eq!(document.history.redo_len(), 0);
    assert_eq!(
        dr_core::ship_from_xml(&dr_core::ship_to_xml(&document.ship).unwrap()).unwrap(),
        document.ship
    );
}

struct Panel {
    context: egui::Context,
    inspector: properties::Inspector,
    document: EditorDocument,
    hits: Vec<(Action, egui::Rect)>,
}

impl Panel {
    fn new() -> Self {
        let mut document = assembled();
        document.clear_selection();
        let mut inspector = properties::Inspector::default();
        properties::open(&mut inspector, &document);
        let mut panel = Self {
            context: egui_ui::test_context(),
            inspector,
            document,
            hits: vec![],
        };
        for _ in 0..3 {
            panel.frame(vec![]);
        }
        panel
    }

    fn frame(&mut self, events: Vec<egui::Event>) {
        let available = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1440.0, 900.0));
        self.context.begin_pass(egui::RawInput {
            screen_rect: Some(available),
            events,
            focused: true,
            ..default()
        });
        self.hits.clear();
        if let Some(shown) = properties::egui_panel::show(
            &self.context,
            &mut self.inspector,
            &self.document,
            &mut self.hits,
            available,
            &mut |kind| {
                (
                    egui::TextureId::User(123),
                    render::fallback_size(Some(kind)),
                )
            },
        ) && let Some(action) = shown.action
        {
            properties::act(&action, &mut self.inspector, &mut self.document);
        }
        self.context.end_pass().textures_delta.clear();
    }

    fn rect(&self, action: Action) -> egui::Rect {
        self.hits
            .iter()
            .find(|(candidate, rect)| *candidate == action && rect.is_positive())
            .expect("真实生产控件未产生可见命中区")
            .1
    }

    fn click(&mut self, action: Action) {
        let point = self.rect(action).center();
        for pressed in [true, false] {
            let mut input = EguiInput::default();
            pointer(&mut input, point, Some(pressed));
            self.frame(input.0.events);
        }
        self.frame(vec![]);
    }

    fn draft(&self) -> &StagingState {
        self.inspector
            .draft
            .as_ref()
            .unwrap()
            .staging
            .as_ref()
            .unwrap()
    }

    fn drag(&mut self, source: Action, target: Action, after: Option<bool>) {
        let source = self.rect(source).center();
        let target_rect = self.rect(target);
        let target = match after {
            None => target_rect.center(),
            Some(true) => egui::pos2(target_rect.center().x, target_rect.bottom() - 2.0),
            Some(false) => egui::pos2(target_rect.center().x, target_rect.top() + 2.0),
        };
        let before = self.draft().clone();
        let ship = self.document.ship.clone();
        let history = self.document.history.undo_len();
        for step in 0..4 {
            let mut input = EguiInput::default();
            drag_frame(&mut input, source, target, step);
            self.frame(input.0.events);
            assert_eq!(self.document.ship, ship);
            assert_eq!(self.document.history.undo_len(), history);
            if step < 3 {
                assert_eq!(self.draft(), &before, "保持与预览帧只在释放时修改草稿");
            }
        }
        self.frame(vec![]);
    }
}

#[test]
fn generate_staging_button_is_draft_only_then_apply_is_one_undoable_transaction() {
    let mut panel = Panel::new();
    let before = panel.document.ship.clone();
    assert!(panel.inspector.docked());
    assert!(!panel.inspector.advanced_open && !panel.inspector.targets_open);
    panel.click(Action::GenerateStaging);
    assert_eq!(panel.document.ship, before);
    assert_eq!(panel.document.history.undo_len(), 15);
    assert_staging(panel.draft(), AUTO, false);
    panel.click(Action::Apply);
    assert!(!panel.inspector.is_open());
    assert_eq!(panel.document.history.undo_len(), 16);
    let applied = panel.document.ship.clone();
    assert_staging(staging(&applied).unwrap(), AUTO, false);
    assert!(panel.document.undo());
    assert_eq!(panel.document.ship, before);
    assert!(panel.document.redo());
    assert_eq!(panel.document.ship, applied);
}

#[test]
fn shared_drag_packets_keep_press_hold_and_release_in_separate_passes() {
    let source = egui::pos2(20.0, 30.0);
    let target = egui::pos2(160.0, 200.0);
    let labels = ["press", "held_threshold", "held_target", "release"];
    for (step, label) in labels.into_iter().enumerate() {
        let mut input = EguiInput::default();
        assert_eq!(drag_frame(&mut input, source, target, step as u8), label);
        let buttons: Vec<_> = input
            .0
            .events
            .iter()
            .filter_map(|event| match event {
                egui::Event::PointerButton { pressed, .. } => Some(*pressed),
                _ => None,
            })
            .collect();
        assert_eq!(
            buttons,
            match step {
                0 => vec![true],
                3 => vec![false],
                _ => vec![],
            }
        );
        if step == 1 {
            assert!(
                matches!(input.0.events[0], egui::Event::PointerMoved(point) if point.distance(source) >= 14.0)
            );
        }
    }
}

#[test]
fn real_action_and_card_drags_preserve_multiset_cancel_and_single_apply_history() {
    let mut panel = Panel::new();
    panel.click(Action::GenerateStaging);
    panel.click(Action::Apply);
    let automatic = panel.document.ship.clone();
    properties::open(&mut panel.inspector, &panel.document);
    for _ in 0..3 {
        panel.frame(vec![]);
    }
    panel.drag(Action::SelectActivation(1, 0), Action::SelectStage(2), None);
    assert_staging(panel.draft(), MANUAL, true);
    let manual = panel.draft().clone();
    panel.drag(Action::SelectStage(0), Action::SelectStage(1), Some(true));
    let mut reordered = manual.clone();
    reordered.steps.swap(0, 1);
    assert_eq!(panel.draft(), &reordered);
    assert_eq!(
        inventory(panel.draft()),
        vec![3, 4, 6, 7, 9, 10, 11, 13, 14, 16]
    );
    panel.drag(Action::SelectStage(1), Action::SelectStage(0), Some(false));
    assert_eq!(panel.draft(), &manual);
    panel.drag(Action::SelectActivation(2, 2), Action::SelectStage(1), None);
    assert_eq!(
        rows(panel.draft()),
        AUTO.iter().map(|row| row.to_vec()).collect::<Vec<_>>()
    );
    assert_eq!(panel.draft().steps[1].activations[0].id, 9);
    assert!(panel.draft().steps[1].activations[0].moved);
    panel.drag(Action::SelectActivation(1, 0), Action::SelectStage(2), None);
    assert_eq!(panel.draft(), &manual);
    panel.click(Action::Cancel);
    assert!(!panel.inspector.is_open());
    assert_eq!(panel.document.ship, automatic);
    assert_eq!(panel.document.history.undo_len(), 16);
    properties::open(&mut panel.inspector, &panel.document);
    for _ in 0..3 {
        panel.frame(vec![]);
    }
    panel.drag(Action::SelectActivation(1, 0), Action::SelectStage(2), None);
    assert_eq!(panel.draft(), &manual);
    panel.click(Action::Apply);
    assert_eq!(panel.document.history.undo_len(), 17);
    let applied = panel.document.ship.clone();
    assert_staging(staging(&applied).unwrap(), MANUAL, true);
    assert_eq!(
        dr_core::ship_from_xml(&dr_core::ship_to_xml(&applied).unwrap()).unwrap(),
        applied
    );
    assert!(panel.document.undo());
    assert_eq!(panel.document.ship, automatic);
    assert!(panel.document.redo());
    assert_eq!(panel.document.ship, applied);
}

/// 纯身份检查夹具：不创建渲染器、不模拟截图，也不替代上面的生产拼装测试。
fn visual_fixture() -> (Ship, Vec<VisualObservation>) {
    let mut document = document();
    for (index, kind) in PARTS.iter().enumerate() {
        document.ship.parts.push(
            document
                .catalog
                .get(kind)
                .unwrap()
                .instantiate(index as i64 + 2, (0.5, -(index as f64))),
        );
    }
    let mut world = World::new();
    let mut images = Assets::<Image>::default();
    let image = images.add(Image::default()).id();
    let observations = document
        .ship
        .keyed_parts()
        .map(|(key, part)| {
            let position = Vec3::new(part.x as f32 * 60.0, part.y as f32 * 60.0, 0.0);
            VisualObservation {
                entity: world.spawn_empty().id(),
                key,
                expected_image: Some(image),
                image,
                image_size: Some(UVec2::new(32, 64)),
                load_failed: false,
                material_image: Some(image),
                local: position,
                global: position,
                visible: true,
                in_view: Some(true),
                record: serde_json::Value::Null,
            }
        })
        .collect();
    (document.ship, observations)
}

#[test]
fn visual_identity_accepts_exactly_sixteen_unique_matching_loaded_parts() {
    let (ship, observations) = visual_fixture();
    assert_eq!(observations.len(), 16);
    assert_eq!(check_visuals(&ship, &observations), Ok(()));
}

#[test]
fn visual_identity_rejects_duplicate_keys_entities_and_stale_parts() {
    let (ship, observations) = visual_fixture();
    let mut bad = observations.clone();
    bad[8].key = bad[0].key;
    assert!(matches!(
        check_visuals(&ship, &bad),
        Err(VisualIssue::Invalid(_))
    ));
    let mut bad = observations.clone();
    bad[8].entity = bad[0].entity;
    assert!(matches!(
        check_visuals(&ship, &bad),
        Err(VisualIssue::Invalid(_))
    ));
    let mut bad = observations.clone();
    bad[8].key = PartKey::new(1, 9, 0);
    assert!(matches!(
        check_visuals(&ship, &bad),
        Err(VisualIssue::Invalid(_))
    ));
    assert!(matches!(
        check_visuals(&ship, &observations[..8]),
        Err(VisualIssue::Pending(_))
    ));
}

#[test]
fn visual_identity_rejects_catalog_or_material_image_mismatch_even_while_loading() {
    let (ship, observations) = visual_fixture();
    let wrong = Handle::<Image>::default().id();
    let mut bad = observations.clone();
    bad[8].image = wrong;
    assert!(matches!(
        check_visuals(&ship, &bad),
        Err(VisualIssue::Invalid(_))
    ));
    let mut bad = observations.clone();
    bad[0].image_size = None;
    bad[8].material_image = Some(wrong);
    assert!(matches!(
        check_visuals(&ship, &bad),
        Err(VisualIssue::Invalid(_))
    ));
}

#[test]
fn visual_identity_waits_only_for_missing_resources_and_commands() {
    let (ship, observations) = visual_fixture();
    for field in 0..4 {
        let mut pending = observations.clone();
        match field {
            0 => pending[0].expected_image = None,
            1 => pending[0].image_size = None,
            2 => pending[0].material_image = None,
            3 => pending[0].in_view = None,
            _ => unreachable!(),
        }
        assert!(matches!(
            check_visuals(&ship, &pending),
            Err(VisualIssue::Pending(_))
        ));
    }
    let mut bad = observations.clone();
    bad[0].load_failed = true;
    assert!(matches!(
        check_visuals(&ship, &bad),
        Err(VisualIssue::Invalid(_))
    ));
    let mut bad = observations.clone();
    bad[0].image_size = Some(UVec2::ZERO);
    assert!(matches!(
        check_visuals(&ship, &bad),
        Err(VisualIssue::Invalid(_))
    ));
}

#[test]
fn visual_identity_rejects_local_global_visibility_and_nonfinite_mismatch() {
    let (ship, observations) = visual_fixture();
    for field in 0..5 {
        let mut bad = observations.clone();
        match field {
            0 => bad[8].local.x += 1.0,
            1 => bad[8].global.y += 1.0,
            2 => bad[8].visible = false,
            3 => bad[8].in_view = Some(false),
            4 => bad[8].global.x = f32::NAN,
            _ => unreachable!(),
        }
        assert!(matches!(
            check_visuals(&ship, &bad),
            Err(VisualIssue::Invalid(_))
        ));
    }
}
