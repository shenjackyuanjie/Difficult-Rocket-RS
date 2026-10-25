use crate::model::{AttachPoint, Part, PartKind, PartType};

mod overlap;
pub use overlap::{MAX_EDITOR_OVERLAP, editor_overlap_blocked, overlap_ratio};

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Vec2d {
    pub x: f64,
    pub y: f64,
}

impl Vec2d {
    pub fn distance(self, other: Self) -> f64 {
        ((self.x - other.x).powi(2) + (self.y - other.y).powi(2)).sqrt()
    }
}

pub const SR1_TO_PIXELS: f64 = 60.0;

/// 编辑器网格步长（SR1 逻辑格）：拖动位移与放置坐标按此步长量化。
/// 取自 SimpleRockets PC 1.6.11 的真实拖拽实测（四档缩放下位移均为 0.5，
/// 0.1 与 0.25 从未出现；证据见 target/grid-probe/README.md）。
/// 实测值变化时只改这一处，其余逻辑全部引用该常量。
pub const EDITOR_GRID_STEP: f64 = 0.5;

/// 把单个坐标四舍五入到最近的网格点；正负对称，恰好半格时向远离零的方向进位。
/// 先换算成格数再四舍五入，避免 0.35 这类十进制值在除法后落到半格下方；
/// 再吸收步长的十万分之一量级的浮点噪声（远小于最高缩放下的一个屏幕像素），
/// 让恰好半格的拖动稳定进位，而不是随相机换算的末位误差逐帧翻转。
pub fn quantize_coordinate(value: f64) -> f64 {
    let per_unit = 1.0 / EDITOR_GRID_STEP;
    let steps = value * per_unit;
    (steps + steps.signum() * 1e-5).round() / per_unit
}

/// 逐轴量化逻辑坐标：x、y 互不影响，可分别落在不同网格线上。
pub fn quantize_point(point: (f64, f64)) -> (f64, f64) {
    (quantize_coordinate(point.0), quantize_coordinate(point.1))
}

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

/// 一次预览中的静止部件集合；按 X 排序，仅对保守半径内的实例做精确碰撞。
/// Shape 可以超出贴图，轮廓半径仍由实际形状决定，不按部件宽高裁剪。
pub struct CollisionSet<'a> {
    parts: Vec<(&'a Part, &'a PartType, f64)>,
    max_radius: f64,
}

impl<'a> CollisionSet<'a> {
    pub fn new(parts: impl Iterator<Item = &'a Part>, catalog: &'a crate::PartCatalog) -> Self {
        let mut parts: Vec<_> = parts
            .filter_map(|part| {
                let kind = catalog.get(&part.part_type)?;
                (!kind.ignore_editor_intersections).then(|| (part, kind, bounding_radius(kind)))
            })
            .collect();
        parts.sort_by(|a, b| a.0.x.total_cmp(&b.0.x));
        let max_radius = parts
            .iter()
            .map(|(_, _, radius)| *radius)
            .fold(0.0, f64::max);
        Self { parts, max_radius }
    }

    pub fn intersects(&self, part: &Part, kind: &PartType) -> bool {
        self.any_nearby(part, kind, intersects)
    }

    /// 编辑连接允许轻微相交，但必须严格小于较小实体面积的 5%。
    pub fn editor_overlap_blocked(&self, part: &Part, kind: &PartType) -> bool {
        self.any_nearby(part, kind, editor_overlap_blocked)
    }

    fn any_nearby(
        &self,
        part: &Part,
        kind: &PartType,
        check: impl Fn(&Part, &PartType, &Part, &PartType) -> bool,
    ) -> bool {
        if kind.ignore_editor_intersections {
            return false;
        }
        let radius = bounding_radius(kind);
        let reach = radius + self.max_radius;
        let start = self
            .parts
            .partition_point(|(other, _, _)| other.x < part.x - reach);
        let end = self
            .parts
            .partition_point(|(other, _, _)| other.x <= part.x + reach);
        self.parts[start..end]
            .iter()
            .any(|(other, other_kind, other_radius)| {
                let distance_squared = (part.x - other.x).powi(2) + (part.y - other.y).powi(2);
                distance_squared <= (radius + other_radius).powi(2)
                    && check(part, kind, other, other_kind)
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
pub enum WorldShape {
    Polygon(Vec<Vec2d>),
    Circle(Vec2d, f64),
}

/// 用于命中、碰撞和调试显示的同一组世界坐标实体轮廓。
pub fn world_shapes(part: &Part, kind: &PartType) -> Vec<WorldShape> {
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
mod tests;
