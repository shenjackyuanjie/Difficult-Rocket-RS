use crate::model::{AttachPoint, Part, PartKind, PartType};

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Vec2d {
    pub x: f64,
    pub y: f64,
}

impl Vec2d {
    pub fn distance(self, other: Self) -> f64 {
        ((self.x - other.x).powi(2) + (self.y - other.y).powi(2)).sqrt()
    }
    pub fn rotate(self, turns: i32) -> Self {
        match turns.rem_euclid(4) {
            1 => Self {
                x: -self.y,
                y: self.x,
            },
            2 => Self {
                x: -self.x,
                y: -self.y,
            },
            3 => Self {
                x: self.y,
                y: -self.x,
            },
            _ => self,
        }
    }
}

pub const SR1_TO_PIXELS: f64 = 60.0;

pub fn part_world_attach(part: &Part, attach: &AttachPoint) -> Vec2d {
    local_to_world(part, (attach.x, attach.y))
}

/// PartList 局部坐标到 Ship 坐标，与贴图使用同一组镜像和旋转。
fn local_to_world(part: &Part, point: (f64, f64)) -> Vec2d {
    let mut local = Vec2d {
        x: point.0 / 2.0,
        y: point.1 / 2.0,
    };
    if part.flip_x {
        local.x = -local.x;
    }
    if part.flip_y {
        local.y = -local.y;
    }
    let (sin, cos) = part.angle.sin_cos();
    let local = Vec2d {
        x: local.x * cos - local.y * sin,
        y: local.x * sin + local.y * cos,
    };
    Vec2d {
        x: part.x + local.x,
        y: part.y + local.y,
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SnapCandidate {
    pub source_index: usize,
    pub target_index: usize,
    pub position: Vec2d,
    pub distance: f64,
    pub dock: bool,
}

pub fn find_snap(
    source: &Part,
    source_type: &PartType,
    target: &Part,
    target_type: &PartType,
    threshold: f64,
) -> Option<SnapCandidate> {
    crate::connections::candidates(source, source_type, target, target_type, threshold)
        .into_iter()
        .next()
}

pub fn intersects(a: &Part, at: &PartType, b: &Part, bt: &PartType) -> bool {
    if at.ignore_editor_intersections || bt.ignore_editor_intersections {
        return false;
    }
    if (a.x - b.x).hypot(a.y - b.y) > bounding_radius(at) + bounding_radius(bt) {
        return false;
    }
    let a_shapes = world_shapes(a, at);
    let b_shapes = world_shapes(b, bt);
    a_shapes
        .iter()
        .any(|a| b_shapes.iter().any(|b| shapes_intersect(a, b)))
}

/// 一次预览中的静止部件集合；缓存目录查询和保守半径，供多个落点重复检查。
pub struct CollisionSet<'a>(Vec<(&'a Part, &'a PartType, f64)>);

impl<'a> CollisionSet<'a> {
    pub fn new(parts: impl Iterator<Item = &'a Part>, catalog: &'a crate::PartCatalog) -> Self {
        Self(
            parts
                .filter_map(|part| {
                    let kind = catalog.get(&part.part_type)?;
                    (!kind.ignore_editor_intersections).then(|| (part, kind, bounding_radius(kind)))
                })
                .collect(),
        )
    }

    pub fn intersects(&self, part: &Part, kind: &PartType) -> bool {
        if kind.ignore_editor_intersections {
            return false;
        }
        let radius = bounding_radius(kind);
        self.0.iter().any(|(other, other_kind, other_radius)| {
            let distance_squared = (part.x - other.x).powi(2) + (part.y - other.y).powi(2);
            distance_squared <= (radius + other_radius).powi(2)
                && intersects(part, kind, other, other_kind)
        })
    }
}

const EPSILON: f64 = 1e-9;

/// 框选依据实际实体轮廓；允许重叠的部件仍然可以被选择。
pub fn intersects_rect(part: &Part, kind: &PartType, a: Vec2d, b: Vec2d) -> bool {
    let low = Vec2d {
        x: a.x.min(b.x),
        y: a.y.min(b.y),
    };
    let high = Vec2d {
        x: a.x.max(b.x),
        y: a.y.max(b.y),
    };
    if high.x - low.x <= EPSILON || high.y - low.y <= EPSILON {
        return false;
    }
    let rect = WorldShape::Polygon(vec![
        low,
        Vec2d {
            x: high.x,
            y: low.y,
        },
        high,
        Vec2d {
            x: low.x,
            y: high.y,
        },
    ]);
    world_shapes(part, kind)
        .iter()
        .any(|shape| shapes_intersect(shape, &rect))
}

fn bounding_radius(kind: &PartType) -> f64 {
    if kind.shapes.is_empty() {
        let (w, h) = kind.half_extents();
        w.hypot(h)
    } else {
        kind.shapes
            .iter()
            .filter(|shape| !shape.sensor)
            .flat_map(|shape| &shape.vertices)
            .map(|(x, y)| x.hypot(*y) / 2.0)
            .fold(0.0, f64::max)
    }
}

fn cross(a: (f64, f64), b: (f64, f64), c: (f64, f64)) -> f64 {
    (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0)
}

/// 每条边的其余顶点必须位于同侧；拒绝退化、自交和凹多边形。
pub(crate) fn valid_polygon(vertices: &[(f64, f64)]) -> bool {
    if vertices.len() < 3
        || vertices
            .iter()
            .any(|(x, y)| !x.is_finite() || !y.is_finite())
    {
        return false;
    }
    let mut orientation = 0.0_f64;
    for (i, &a) in vertices.iter().enumerate() {
        let b = vertices[(i + 1) % vertices.len()];
        if (a.0 - b.0).hypot(a.1 - b.1) <= EPSILON {
            return false;
        }
        for &c in vertices {
            let side = cross(a, b, c);
            if side.abs() <= EPSILON {
                continue;
            }
            if orientation == 0.0 {
                orientation = side.signum();
            }
            if side * orientation < 0.0 {
                return false;
            }
        }
    }
    orientation != 0.0
}

#[derive(Debug)]
enum WorldShape {
    Polygon(Vec<Vec2d>),
    Circle(Vec2d, f64),
}

fn world_shapes(part: &Part, kind: &PartType) -> Vec<WorldShape> {
    if !kind.shapes.is_empty() {
        return kind
            .shapes
            .iter()
            .filter(|shape| !shape.sensor)
            .map(|shape| {
                WorldShape::Polygon(
                    shape
                        .vertices
                        .iter()
                        .map(|&point| local_to_world(part, point))
                        .collect(),
                )
            })
            .collect();
    }
    if kind.kind == PartKind::Wheel {
        return vec![WorldShape::Circle(
            Vec2d {
                x: part.x,
                y: part.y,
            },
            kind.width.min(kind.height) as f64 / 4.0,
        )];
    }
    let w = kind.width as f64 / 2.0;
    let h = kind.height as f64 / 2.0;
    vec![WorldShape::Polygon(
        [(-w, -h), (w, -h), (w, h), (-w, h)]
            .into_iter()
            .map(|point| local_to_world(part, point))
            .collect(),
    )]
}

fn project(shape: &WorldShape, axis: Vec2d) -> (f64, f64) {
    match shape {
        WorldShape::Polygon(vertices) => vertices
            .iter()
            .map(|p| p.x * axis.x + p.y * axis.y)
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(min, max), value| {
                (min.min(value), max.max(value))
            }),
        WorldShape::Circle(center, radius) => {
            let middle = center.x * axis.x + center.y * axis.y;
            (middle - radius, middle + radius)
        }
    }
}

fn shapes_intersect(a: &WorldShape, b: &WorldShape) -> bool {
    if let (WorldShape::Circle(a, ar), WorldShape::Circle(b, br)) = (a, b) {
        return a.distance(*b) < ar + br - EPSILON;
    }
    let mut axes = vec![];
    for (shape, other) in [(a, b), (b, a)] {
        if let WorldShape::Polygon(vertices) = shape {
            if vertices.len() < 3 {
                return false;
            }
            for (i, point) in vertices.iter().enumerate() {
                let next = vertices[(i + 1) % vertices.len()];
                axes.push(Vec2d {
                    x: point.y - next.y,
                    y: next.x - point.x,
                });
            }
            if let WorldShape::Circle(center, _) = other {
                let nearest = vertices
                    .iter()
                    .min_by(|a, b| a.distance(*center).total_cmp(&b.distance(*center)))
                    .unwrap();
                axes.push(Vec2d {
                    x: nearest.x - center.x,
                    y: nearest.y - center.y,
                });
            }
        }
    }
    let axes: Vec<_> = axes
        .into_iter()
        .filter_map(|axis| {
            let length = axis.x.hypot(axis.y);
            (length > EPSILON).then(|| Vec2d {
                x: axis.x / length,
                y: axis.y / length,
            })
        })
        .collect();
    !axes.is_empty()
        && axes.into_iter().all(|axis| {
            let (amin, amax) = project(a, axis);
            let (bmin, bmax) = project(b, axis);
            amax > bmin + EPSILON && bmax > amin + EPSILON
        })
}

/// 组合形状按并集命中，传感器不属于可选的实体轮廓。
pub fn contains_point(part: &Part, kind: &PartType, point: Vec2d) -> bool {
    if (point.x - part.x).hypot(point.y - part.y) > bounding_radius(kind) + EPSILON {
        return false;
    }
    world_shapes(part, kind).iter().any(|shape| match shape {
        WorldShape::Circle(center, radius) => center.distance(point) <= radius + EPSILON,
        WorldShape::Polygon(vertices) => {
            if vertices.len() < 3 {
                return false;
            }
            let mut positive = false;
            let mut negative = false;
            for (i, a) in vertices.iter().enumerate() {
                let b = vertices[(i + 1) % vertices.len()];
                let side = cross((a.x, a.y), (b.x, b.y), (point.x, point.y));
                positive |= side > EPSILON;
                negative |= side < -EPSILON;
                if positive && negative {
                    return false;
                }
            }
            positive || negative
        }
    })
}

#[cfg(test)]
mod tests {
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
    fn quarter_turn() {
        assert_eq!(
            Vec2d { x: 1.0, y: 2.0 }.rotate(1),
            Vec2d { x: -2.0, y: 1.0 }
        );
    }
    #[test]
    fn snap() {
        let a = t(vec![AttachPoint {
            location: String::new(),
            flip_y: false,
            x: 1.0,
            y: 0.0,
            dock: false,
            fuel_line: false,
            flip_x: false,
            group: None,
            order: None,
            break_angle: None,
            break_force: None,
        }]);
        let b = t(vec![AttachPoint {
            location: String::new(),
            flip_y: false,
            x: -1.0,
            y: 0.0,
            dock: false,
            fuel_line: false,
            flip_x: false,
            group: None,
            order: None,
            break_angle: None,
            break_force: None,
        }]);
        assert!(find_snap(&p(1, 0.0), &a, &p(2, 1.01), &b, 0.1).is_some());
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
}
