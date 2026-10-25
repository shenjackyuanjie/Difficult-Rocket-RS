
use super::*;
use crate::model::{Part, PartKind};
fn p(id: i64, x: f64) -> Part {
    Part {
        id,
        part_type: "x".into(),
        x,
        y: 0.0,
        angle: 0.0,
        editor_angle: 0,
        angle_v: 0.0,
        flip_x: false,
        flip_y: false,
        active: false,
        exploded: false,
        fuel: None,
        fuel_kind: None,
        extension: None,
        parachute: Default::default(),
        lander: Default::default(),
        pod: None,
    }
}
fn t(points: Vec<AttachPoint>) -> PartType {
    PartType {
        id: "x".into(),
        name: "x".into(),
        description: String::new(),
        sprite: String::new(),
        kind: PartKind::Strut,
        mass: 1.0,
        width: 1,
        height: 1,
        category: String::new(),
        hidden: false,
        ignore_editor_intersections: false,
        disable_editor_rotation: false,
        max_occurrences: None,
        friction: None,
        can_explode: None,
        cover_height: None,
        sandbox_only: None,
        drag: None,
        buoyancy: None,
        damage: None,
        rcs: None,
        solar: None,
        lander: None,
        tank: None,
        engine: None,
        attach_points: points,
        shapes: vec![],
    }
}
#[test]
fn cached_collision_set_matches_direct_shapes_and_exemptions() {
    let mut triangle = t(vec![]);
    triangle.id = "triangle".into();
    triangle.shapes = vec![crate::PolygonShape {
        // 轮廓超出贴图边界，不能按 width/height 剔除。
        vertices: vec![(4.0, -2.0), (8.0, -2.0), (4.0, 2.0)],
        sensor: false,
    }];
    let mut wheel = t(vec![]);
    wheel.id = "wheel".into();
    wheel.kind = PartKind::Wheel;
    wheel.width = 4;
    wheel.height = 4;
    let mut ignored = t(vec![]);
    ignored.id = "ignored".into();
    ignored.ignore_editor_intersections = true;
    let catalog = crate::PartCatalog::new("测试", vec![t(vec![]), triangle, wheel, ignored]);
    let targets: Vec<_> = catalog
        .types
        .iter()
        .enumerate()
        .map(|(i, kind)| {
            let mut part = kind.instantiate(i as i64 + 1, (i as f64 * 5.0, 0.0));
            part.angle = 0.37;
            part.flip_x = true;
            part
        })
        .collect();
    let cached = CollisionSet::new(targets.iter(), &catalog);
    for kind in &catalog.types {
        for angle in [0.0, 0.71, std::f64::consts::FRAC_PI_2] {
            for x in -20..=80 {
                let mut part = kind.instantiate(100, (x as f64 / 4.0, 0.13));
                part.angle = angle;
                part.flip_y = true;
                let direct = targets.iter().any(|other| {
                    intersects(&part, kind, other, catalog.get(&other.part_type).unwrap())
                });
                assert_eq!(
                    cached.intersects(&part, kind),
                    direct,
                    "{} {angle} {x}",
                    kind.id
                );
            }
        }
    }
}

#[test]
fn sorted_collision_set_matches_unsorted_duplicates_and_large_offset_shapes() {
    let mut far_shape = t(vec![]);
    far_shape.id = "far".into();
    far_shape.shapes = vec![crate::PolygonShape {
        vertices: vec![(50.0, -4.0), (54.0, -4.0), (54.0, 4.0), (50.0, 4.0)],
        sensor: false,
    }];
    let mut wheel = t(vec![]);
    wheel.id = "wheel".into();
    wheel.kind = PartKind::Wheel;
    wheel.width = 8;
    wheel.height = 8;
    let mut ignored = t(vec![]);
    ignored.id = "ignored".into();
    ignored.ignore_editor_intersections = true;
    let catalog = crate::PartCatalog::new("测试", vec![t(vec![]), far_shape, wheel, ignored]);
    let targets: Vec<_> = (0..240)
        .map(|i| {
            let kind = &catalog.types[i % catalog.types.len()];
            let mut part =
                kind.instantiate(1, (((i * 73) % 241) as f64 - 120.0, (i % 9) as f64 - 4.0));
            part.angle = (i % 5) as f64 * 0.37;
            part.flip_x = i % 2 == 0;
            part.flip_y = i % 3 == 0;
            part
        })
        .collect();
    let cached = CollisionSet::new(targets.iter(), &catalog);
    assert!(
        cached
            .parts
            .windows(2)
            .all(|pair| pair[0].0.x <= pair[1].0.x)
    );
    for kind in &catalog.types {
        for x in -170..=170 {
            for y in [-10.0, -0.25, 0.0, 7.0] {
                let mut part = kind.instantiate(1, (x as f64 + 0.125, y));
                part.angle = 0.71;
                part.flip_y = true;
                let direct = targets.iter().any(|other| {
                    intersects(&part, kind, other, catalog.get(&other.part_type).unwrap())
                });
                assert_eq!(
                    cached.intersects(&part, kind),
                    direct,
                    "{} {x} {y}",
                    kind.id
                );
            }
        }
    }
    let empty = CollisionSet::new(std::iter::empty(), &catalog);
    assert!(!empty.intersects(&p(1, 0.0), &catalog.types[0]));
}

#[test]
fn sorted_collision_set_keeps_boundary_contacts_non_colliding() {
    let catalog = crate::PartCatalog::new("测试", vec![t(vec![])]);
    let targets = [p(1, 2.0), p(1, -2.0)];
    let cached = CollisionSet::new(targets.iter(), &catalog);
    let kind = &catalog.types[0];
    for x in [-2.5, -1.5, 1.5, 2.5] {
        assert!(!cached.intersects(&p(1, x), kind));
    }
    for x in [-2.49999, -1.50001, 1.50001, 2.49999] {
        assert!(cached.intersects(&p(1, x), kind));
    }
}

#[test]
fn rectangle_selection_uses_shape_and_ignores_collision_exemption() {
    let mut kind = t(vec![]);
    kind.ignore_editor_intersections = true;
    kind.shapes = vec![crate::PolygonShape {
        vertices: vec![(-2.0, -2.0), (2.0, -2.0), (-2.0, 2.0)],
        sensor: false,
    }];
    let part = p(1, 0.0);
    assert!(intersects_rect(
        &part,
        &kind,
        Vec2d { x: -1.1, y: -0.2 },
        Vec2d { x: -0.8, y: 0.2 }
    ));
    assert!(!intersects_rect(
        &part,
        &kind,
        Vec2d { x: 0.5, y: 0.5 },
        Vec2d { x: 0.9, y: 0.9 }
    ));
    kind.kind = PartKind::Wheel;
    kind.shapes.clear();
    kind.width = 4;
    kind.height = 4;
    assert!(!intersects_rect(
        &part,
        &kind,
        Vec2d { x: 0.8, y: 0.8 },
        Vec2d { x: 1.0, y: 1.0 }
    ));
    assert!(intersects_rect(
        &part,
        &kind,
        Vec2d { x: 1.1, y: 0.2 },
        Vec2d { x: 0.9, y: -0.2 }
    ));
}
#[test]
fn overlap_respects_flag() {
    let mut a = t(vec![]);
    let b = t(vec![]);
    assert!(intersects(&p(1, 0.0), &a, &p(2, 0.1), &b));
    a.ignore_editor_intersections = true;
    assert!(!intersects(&p(1, 0.0), &a, &p(2, 0.1), &b));
}

#[test]
fn rotated_hit_and_collision_match_rendered_size() {
    let mut kind = t(vec![]);
    kind.width = 8;
    kind.height = 2;
    let mut part = p(1, 0.0);
    part.angle = std::f64::consts::FRAC_PI_2;
    assert!(contains_point(&part, &kind, Vec2d { x: 0.4, y: 1.9 }));
    assert!(!contains_point(&part, &kind, Vec2d { x: 1.0, y: 0.0 }));
    assert!(!intersects(&part, &kind, &p(2, 1.0), &t(vec![])));
    assert!(intersects(&part, &kind, &p(2, 0.6), &t(vec![])));
    part.angle = std::f64::consts::FRAC_PI_4;
    assert!(contains_point(&part, &kind, Vec2d { x: 1.0, y: 1.0 }));
    assert!(!contains_point(&part, &kind, Vec2d { x: 1.0, y: -1.0 }));
}

#[test]
fn attachment_uses_ship_units_rotation_and_mirroring() {
    let attach = AttachPoint {
        location: String::new(),
        flip_y: false,
        x: 2.0,
        y: 1.0,
        dock: false,
        fuel_line: false,
        flip_x: false,
        group: None,
        order: None,
        break_angle: None,
        break_force: None,
    };
    let mut part = p(1, 3.0);
    part.y = 4.0;
    part.flip_x = true;
    part.angle = std::f64::consts::FRAC_PI_2;
    let result = part_world_attach(&part, &attach);
    assert!((result.x - 2.5).abs() < 1e-10);
    assert!((result.y - 3.0).abs() < 1e-10);
}

#[test]
fn triangle_hit_follows_rotation_and_both_mirrors() {
    let mut kind = t(vec![]);
    kind.shapes.push(crate::PolygonShape {
        vertices: vec![(-2.0, -2.0), (2.0, -2.0), (-2.0, 2.0)],
        sensor: false,
    });
    let mut part = p(1, 3.0);
    part.y = 4.0;
    assert!(!contains_point(&part, &kind, Vec2d { x: 3.8, y: 4.8 }));
    assert!(contains_point(&part, &kind, Vec2d { x: 2.2, y: 3.2 }));
    part.flip_x = true;
    assert!(contains_point(&part, &kind, Vec2d { x: 3.8, y: 3.2 }));
    part.flip_y = true;
    part.angle = std::f64::consts::FRAC_PI_2;
    assert!(contains_point(&part, &kind, Vec2d { x: 2.2, y: 4.8 }));
    assert!(!contains_point(&part, &kind, Vec2d { x: 3.8, y: 3.2 }));
}

#[test]
fn compound_shape_keeps_holes_and_excludes_sensors() {
    let mut kind = t(vec![]);
    for (min, max, sensor) in [(-4.0, -2.0, false), (2.0, 4.0, false), (-2.0, 2.0, true)] {
        kind.shapes.push(crate::PolygonShape {
            vertices: vec![(min, -2.0), (max, -2.0), (max, 2.0), (min, 2.0)],
            sensor,
        });
    }
    assert!(!contains_point(&p(1, 0.0), &kind, Vec2d { x: 0.0, y: 0.0 }));
    assert!(contains_point(&p(1, 0.0), &kind, Vec2d { x: 1.5, y: 0.0 }));
    assert!(!intersects(&p(1, 0.0), &kind, &p(2, 0.0), &t(vec![])));
    assert!(intersects(&p(1, 0.0), &kind, &p(2, 1.5), &t(vec![])));
}

#[test]
fn circle_and_polygon_use_actual_outline_and_allow_touching() {
    let mut wheel = t(vec![]);
    wheel.kind = PartKind::Wheel;
    wheel.width = 4;
    wheel.height = 4;
    let circle = p(1, 0.0);
    assert!(contains_point(&circle, &wheel, Vec2d { x: 1.0, y: 0.0 }));
    assert!(!contains_point(&circle, &wheel, Vec2d { x: 0.9, y: 0.9 }));
    let mut square = p(2, 1.0);
    square.y = 1.0;
    assert!(!intersects(&circle, &wheel, &square, &t(vec![])));
    square.x = 0.9;
    square.y = 0.9;
    assert!(intersects(&circle, &wheel, &square, &t(vec![])));
    assert!(!intersects(&circle, &wheel, &p(2, 2.0), &wheel));
    assert!(!intersects(&p(1, 0.0), &t(vec![]), &p(2, 0.5), &t(vec![])));
}

#[test]
fn invalid_polygons_and_zero_size_do_not_form_solid_regions() {
    for vertices in [
        vec![(0.0, 0.0); 3],
        vec![(0.0, 0.0), (1.0, 0.0), (f64::NAN, 1.0)],
        vec![(0.0, 0.0), (2.0, 0.0), (0.5, 0.5), (2.0, 2.0), (0.0, 2.0)],
        vec![(0.0, 0.0), (2.0, 2.0), (0.0, 2.0), (2.0, 0.0)],
    ] {
        assert!(!valid_polygon(&vertices));
    }
    let mut kind = t(vec![]);
    kind.width = 0;
    kind.height = 0;
    assert!(!intersects(&p(1, 0.0), &kind, &p(2, 0.0), &kind));
    assert!(!contains_point(&p(1, 0.0), &kind, Vec2d::default()));
}

#[test]
fn grid_step_rounds_half_up_per_axis_and_stays_symmetric() {
    // 实测取整规则：最近一格，恰好半格向远离零方向进位。
    for (value, expected) in [
        (0.04, 0.0),
        (0.24, 0.0),
        (0.25, EDITOR_GRID_STEP),
        (0.26, EDITOR_GRID_STEP),
        (-0.24, 0.0),
        (-0.25, -EDITOR_GRID_STEP),
        (-0.26, -EDITOR_GRID_STEP),
        (0.74, EDITOR_GRID_STEP),
        (0.75, 1.0),
        (1.26, 1.5),
        (-1.26, -1.5),
    ] {
        let actual = quantize_coordinate(value);
        assert!(
            (actual - expected).abs() < 1e-9,
            "{value} 量化到 {actual}，期望 {expected}"
        );
    }
    // x、y 独立量化：一轴不足半格不动，不会带走另一轴。
    let (x, y) = quantize_point((0.24, 0.26));
    assert!(x.abs() < 1e-9);
    assert!((y - EDITOR_GRID_STEP).abs() < 1e-9);
    assert_eq!(quantize_coordinate(f64::INFINITY), f64::INFINITY);
    assert!(quantize_coordinate(f64::NAN).is_nan());
}
