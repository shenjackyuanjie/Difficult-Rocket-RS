use crate::document::EditorDocument;
use crate::render::*;
use crate::selection::DragState;
use dr_core::Connection;
use dr_core::EditorCommand;
use dr_core::PartKey;

#[test]
fn free_connection_route_tracks_only_the_dragged_endpoint_after_mirrors() {
    let mut document = crate::test_support::document();
    document.catalog = crate::test_support::catalog();
    let kind = document.catalog.get("pod").unwrap();
    document.ship.parts = vec![
        kind.instantiate(1, (0.0, 0.0)),
        kind.instantiate(2, (3.0, 0.0)),
    ];
    document.free_mode = true;
    let before = document.ship.clone();
    let connection = Connection::Normal {
        parent: 1,
        child: 2,
        parent_attach: 2,
        child_attach: 1,
    };
    let drag = DragState {
        id: Some(PartKey::new(0, 1, 0)),
        preview: (2.0, 1.0),
        fine_rotation: 0.37,
        flip_x: true,
        ..default()
    };
    let points = dragged_connection_path(&document, &drag, 0, &connection).unwrap();
    let parent = drag.pose(PartKey::new(0, 1, 0), document.ship.part(1).unwrap());
    assert_eq!(
        points[0],
        Vec2::new(parent.x as f32, parent.y as f32) * 60.0
    );
    assert_eq!(points[3], Vec2::new(180.0, 0.0));
    let (a, b) = dr_core::connections::positions_between(
        &document.catalog,
        &connection,
        &parent,
        document.ship.part(2).unwrap(),
    )
    .unwrap();
    assert_eq!(points[1], Vec2::new(a.x as f32, a.y as f32) * 60.0);
    assert_eq!(points[2], Vec2::new(b.x as f32, b.y as f32) * 60.0);
    assert_eq!(document.ship, before);
}

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
        .init_asset::<Image>()
        .insert_resource(crate::test_support::document())
        .init_resource::<DragState>()
        .add_systems(Update, sync);
    app
}

fn visual(app: &mut App, id: i64) -> (Entity, Transform, Color) {
    app.world_mut()
        .query::<(Entity, &PartVisual, &Transform, &Sprite)>()
        .iter(app.world())
        .find(|(_, visual, _, _)| visual.id == id)
        .map(|(entity, _, transform, sprite)| (entity, *transform, sprite.color))
        .unwrap()
}

#[test]
fn main_group_islands_and_ambiguous_links_are_dimmed_without_cross_group_id_leaks() {
    let mut ship = crate::test_support::document().ship;
    let part = ship.parts[0].clone();
    for id in 2..=4 {
        let mut next = part.clone();
        next.id = id;
        ship.parts.push(next);
    }
    ship.connections = vec![
        Connection::Normal {
            parent: 1,
            child: 2,
            parent_attach: 1,
            child_attach: 1,
        },
        Connection::Dock {
            parent: 2,
            child: 3,
            dock: 4,
        },
    ];
    ship.disconnected.push(dr_core::ShipGroup {
        parts: vec![part.clone()],
        connections: vec![],
    });
    let linked = linked_parts(&ship);
    assert_eq!(linked.len(), 4);
    assert!(!linked.contains(&PartKey::new(1, 1, 0)));
    ship.parts.push(ship.parts[1].clone());
    assert_eq!(linked_parts(&ship), HashSet::from([PartKey::new(0, 1, 0)]));
    ship.connections.clear();
    assert_eq!(linked_parts(&ship), HashSet::from([PartKey::new(0, 1, 0)]));
}

#[test]
fn connecting_and_undo_redo_refresh_opacity_without_rebuilding_visuals() {
    let mut app = app();
    let mut part = app.world().resource::<EditorDocument>().ship.parts[0].clone();
    part.id = 2;
    part.x = 2.0;
    app.world_mut()
        .resource_mut::<EditorDocument>()
        .ship
        .parts
        .push(part);
    app.update();
    let initial = visual(&mut app, 2);
    assert_eq!(initial.2.alpha(), 100.0 / 255.0);
    {
        let mut document = app.world_mut().resource_mut::<EditorDocument>();
        let document = &mut *document;
        document
            .history
            .execute(
                &mut document.ship,
                EditorCommand::Connect(Connection::Normal {
                    parent: 1,
                    child: 2,
                    parent_attach: 1,
                    child_attach: 1,
                }),
            )
            .unwrap();
        document.refresh();
    }
    app.update();
    assert_eq!(visual(&mut app, 2).0, initial.0);
    assert_eq!(visual(&mut app, 2).2.alpha(), 1.0);
    assert!(app.world_mut().resource_mut::<EditorDocument>().undo());
    app.update();
    assert_eq!(visual(&mut app, 2).2.alpha(), initial.2.alpha());
    assert!(app.world_mut().resource_mut::<EditorDocument>().redo());
    app.update();
    assert_eq!(visual(&mut app, 2).0, initial.0);
    assert_eq!(visual(&mut app, 2).2.alpha(), 1.0);
}

#[test]
fn natural_image_size_and_odd_pixel_anchor_are_independent_of_physics() {
    let mut kind = crate::test_support::catalog().get("pod").unwrap().clone();
    kind.sprite = "DockingConnector.png".into();
    kind.width = 4;
    kind.height = 1;
    let part = kind.instantiate(1, (0.0, 0.0));
    let mut images = Assets::<Image>::default();
    let mut image = Image::default();
    image.texture_descriptor.size.width = 61;
    image.texture_descriptor.size.height = 29;
    let image = images.add(image);
    let (size, anchor) = sprite_geometry(Some(&kind), &image, &images, &part);
    assert_eq!(size, None);
    assert!(
        anchor
            .as_vec()
            .abs_diff_eq(Vec2::new(-0.5 / 61.0, -0.5 / 29.0), 1e-7)
    );
    assert_eq!(kind.half_extents(), (1.0, 0.25));
    assert_eq!(
        sprite_geometry(None, &image, &images, &part),
        (Some(Vec2::splat(30.0)), Anchor::CENTER)
    );
    images.remove(image.id());
    assert_eq!(
        sprite_geometry(Some(&kind), &image, &images, &part),
        (None, Anchor::CENTER)
    );
}

#[test]
fn mirrored_odd_pixels_rotate_about_original_floor_anchor() {
    let mut part = crate::test_support::document().ship.parts.remove(0);
    part.x = 3.0;
    part.y = -2.0;
    for size in [
        Vec2::new(61.0, 29.0),
        Vec2::new(84.0, 207.0),
        Vec2::new(60.0, 30.0),
    ] {
        for flip_x in [false, true] {
            for flip_y in [false, true] {
                for angle in [0.0, 0.37, std::f64::consts::FRAC_PI_2, -2.1] {
                    part.flip_x = flip_x;
                    part.flip_y = flip_y;
                    part.angle = angle;
                    let corners = image_corners(&part, size);
                    for uv in [Vec2::ZERO, Vec2::X, Vec2::ONE, Vec2::Y] {
                        let local = (uv * size - (size * 0.5).floor())
                            * Vec2::new(
                                if flip_x { -1.0 } else { 1.0 },
                                if flip_y { -1.0 } else { 1.0 },
                            );
                        let (sin, cos) = angle.sin_cos();
                        let expected = Vec2::new(
                            (local.x as f64 * cos - local.y as f64 * sin) as f32 + 180.0,
                            (local.x as f64 * sin + local.y as f64 * cos) as f32 - 120.0,
                        );
                        // 世界坐标约 291 时一个 f32 ULP 为 0.000030517578125，
                        // 原固定阈值 0.00003 小于可表示的最小步进。每轴最多补足
                        // 一个 ULP，不扩大为统一宽松阈值；半像素锚点符号错误仍会
                        // 产生约 1 像素差异。参考值继续独立使用 floor 锚点及 f64 旋转。
                        let one_ulp = |value: f32| {
                            let value = value.abs();
                            value.next_up() - value
                        };
                        let tolerance = Vec2::new(
                            3e-5_f32.max(one_ulp(expected.x)),
                            3e-5_f32.max(one_ulp(expected.y)),
                        );
                        assert!(
                            corners.iter().any(|corner| {
                                (*corner - expected).abs().cmple(tolerance).all()
                            }),
                            "size={size:?}, angle={angle}, flip=({flip_x},{flip_y}), uv={uv:?}, corners={corners:?}, expected={expected:?}, tolerance={tolerance:?}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn disconnected_alpha_survives_selection_and_collision_tints() {
    let mut document = crate::test_support::document();
    let part = document.ship.parts[0].clone();
    let key = PartKey::new(1, 1, 0);
    document.ship.disconnected.push(dr_core::ShipGroup {
        parts: vec![part.clone()],
        connections: vec![],
    });
    let mut drag = DragState::default();
    let ordinary = appearance(&document, &drag, key, &part, false).0;
    assert_eq!(ordinary, Color::WHITE.with_alpha(100.0 / 255.0));
    document.select_only(Some(key));
    let selected = appearance(&document, &drag, key, &part, false).0;
    assert_eq!(selected.alpha(), ordinary.alpha());
    assert_ne!(selected, ordinary);
    drag.id = Some(key);
    drag.members = [key, PartKey::new(0, 1, 0)].into_iter().collect();
    drag.blocked = true;
    let blocked = appearance(&document, &drag, key, &part, false).0;
    assert_eq!(blocked, Color::srgba(1.0, 0.2, 0.2, 100.0 / 255.0));
}

#[test]
fn selection_drag_cancel_and_document_changes_reuse_visual_entities() {
    let mut app = app();
    app.update();
    let initial = visual(&mut app, 1);
    app.world_mut().resource_mut::<EditorDocument>().selected = Some(PartKey::new(0, 1, 0));
    app.update();
    assert_eq!(visual(&mut app, 1).0, initial.0);
    assert_ne!(visual(&mut app, 1).2, initial.2);
    {
        let mut drag = app.world_mut().resource_mut::<DragState>();
        drag.id = Some(PartKey::new(0, 1, 0));
        drag.preview = (5.0, 3.0);
    }
    app.update();
    let moved = visual(&mut app, 1);
    assert_eq!(moved.0, initial.0);
    assert_eq!(moved.1.translation, Vec3::new(300.0, 180.0, 0.0));
    assert_eq!(
        app.world()
            .resource::<EditorDocument>()
            .ship
            .part(1)
            .unwrap()
            .x,
        0.0
    );
    app.world_mut().resource_mut::<DragState>().id = None;
    app.update();
    assert_eq!(visual(&mut app, 1).1, initial.1);
    app.world_mut()
        .resource_mut::<EditorDocument>()
        .execute(EditorCommand::FlipX(1));
    app.update();
    assert_eq!(visual(&mut app, 1).0, initial.0);
    assert!(app.world().get::<Sprite>(initial.0).unwrap().flip_x);
}

#[test]
fn deletion_only_removes_missing_ids_and_undo_restores_them() {
    let mut app = app();
    let mut part = app.world().resource::<EditorDocument>().ship.parts[0].clone();
    part.id = 2;
    part.x = 2.0;
    app.world_mut()
        .resource_mut::<EditorDocument>()
        .ship
        .parts
        .push(part);
    app.update();
    let keep = visual(&mut app, 1).0;
    let removed = visual(&mut app, 2).0;
    app.world_mut()
        .resource_mut::<EditorDocument>()
        .execute(EditorCommand::Delete(2));
    app.update();
    assert_eq!(visual(&mut app, 1).0, keep);
    assert!(app.world().get_entity(removed).is_err());
    app.world_mut().resource_mut::<EditorDocument>().undo();
    app.update();
    assert_eq!(visual(&mut app, 1).0, keep);
    assert_ne!(visual(&mut app, 2).0, removed);
    assert_eq!(visual(&mut app, 2).1.translation.x, 120.0);
}

#[test]
fn reordered_document_keeps_entities_but_updates_draw_order() {
    let mut app = app();
    let mut part = app.world().resource::<EditorDocument>().ship.parts[0].clone();
    part.id = 2;
    app.world_mut()
        .resource_mut::<EditorDocument>()
        .ship
        .parts
        .push(part);
    app.update();
    let first = visual(&mut app, 1);
    let second = visual(&mut app, 2);
    assert!(first.1.translation.z < second.1.translation.z);
    app.world_mut()
        .resource_mut::<EditorDocument>()
        .ship
        .parts
        .reverse();
    app.update();
    let a = visual(&mut app, 1);
    let b = visual(&mut app, 2);
    assert_eq!(a.0, first.0);
    assert_eq!(b.0, second.0);
    assert!(a.1.translation.z > b.1.translation.z);
}

#[test]
fn disconnected_groups_reusing_ids_keep_separate_visuals() {
    let mut app = app();
    let mut part = app.world().resource::<EditorDocument>().ship.parts[0].clone();
    part.x = 5.0;
    app.world_mut()
        .resource_mut::<EditorDocument>()
        .ship
        .disconnected
        .push(dr_core::ShipGroup {
            parts: vec![part],
            connections: vec![],
        });
    app.update();
    let snapshots = |app: &mut App| {
        app.world_mut()
            .query::<(Entity, &PartVisual, &Transform)>()
            .iter(app.world())
            .map(|(entity, visual, transform)| {
                ((visual.group, visual.id), (entity, transform.translation))
            })
            .collect::<HashMap<_, _>>()
    };
    let before = snapshots(&mut app);
    assert_eq!(before.len(), 2);
    assert_eq!(before[&(0, 1)].1.x, 0.0);
    assert_eq!(before[&(1, 1)].1.x, 300.0);
    app.world_mut().resource_mut::<EditorDocument>().status = "刷新状态".into();
    app.update();
    assert_eq!(snapshots(&mut app), before);
}

#[test]
fn repeated_ids_inside_one_group_are_rendered_without_leaking_entities() {
    let mut app = app();
    let mut part = app.world().resource::<EditorDocument>().ship.parts[0].clone();
    part.x = 5.0;
    app.world_mut()
        .resource_mut::<EditorDocument>()
        .ship
        .parts
        .push(part);
    app.update();
    assert_eq!(
        app.world_mut()
            .query::<&PartVisual>()
            .iter(app.world())
            .count(),
        2
    );
    app.world_mut().resource_mut::<EditorDocument>().status = "刷新".into();
    app.update();
    assert_eq!(
        app.world_mut()
            .query::<&PartVisual>()
            .iter(app.world())
            .count(),
        2
    );
    let positions: Vec<_> = app
        .world_mut()
        .query::<(&PartVisual, &Transform)>()
        .iter(app.world())
        .map(|(visual, transform)| (visual.occurrence, transform.translation.x))
        .collect();
    assert!(positions.contains(&(0, 0.0)));
    assert!(positions.contains(&(1, 300.0)));
}

#[test]
fn selection_and_drag_only_affect_the_exact_duplicate_instance() {
    let mut app = app();
    let part = app.world().resource::<EditorDocument>().ship.parts[0].clone();
    {
        let mut document = app.world_mut().resource_mut::<EditorDocument>();
        document.ship.parts.push(part.clone());
        document.ship.disconnected.push(dr_core::ShipGroup {
            parts: vec![part],
            connections: vec![],
        });
        document.selected = Some(PartKey::new(0, 1, 1));
    }
    app.update();
    {
        let mut drag = app.world_mut().resource_mut::<DragState>();
        drag.id = Some(PartKey::new(0, 1, 1));
        drag.preview = (5.0, 3.0);
    }
    app.update();
    for (visual, transform, sprite) in app
        .world_mut()
        .query::<(&PartVisual, &Transform, &Sprite)>()
        .iter(app.world())
    {
        if visual.group == 0 && visual.occurrence == 1 {
            assert_eq!(transform.translation.x, 300.0);
            assert_ne!(sprite.color, Color::WHITE);
        } else {
            assert_eq!(transform.translation.x, 0.0);
            assert_eq!(
                sprite.color,
                Color::WHITE.with_alpha(if visual.group == 0 {
                    1.0
                } else {
                    100.0 / 255.0
                })
            );
        }
    }
    app.world_mut().resource_mut::<DragState>().id = None;
    app.update();
    assert!(
        app.world_mut()
            .query::<&Transform>()
            .iter(app.world())
            .all(|transform| transform.translation.x == 0.0)
    );
}

#[test]
fn part_sprite_path_only_lowercases_the_final_extension() {
    assert_eq!(
        part_sprite_path("DockingConnector.PNG"),
        "textures/parts/DockingConnector.png"
    );
    assert_eq!(
        part_sprite_path("DockingConnector.png"),
        "textures/parts/DockingConnector.png"
    );
    assert_eq!(part_sprite_path("pod"), "textures/parts/pod");
    assert_eq!(part_sprite_path(""), "textures/parts/");
    assert_eq!(part_sprite_path("a.B.C"), "textures/parts/a.B.c");
}
