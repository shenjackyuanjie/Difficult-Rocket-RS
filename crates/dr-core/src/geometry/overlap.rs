//! 用真实轮廓计算相交面积；多个 Shape 先取并集，圆形保留解析曲线。
use super::{WorldShape, intersects, world_shapes};
use crate::{Part, PartType, Vec2d};
use geo::{Area, BooleanOps, LineString, MultiPolygon, Polygon};

pub const MAX_EDITOR_OVERLAP: f64 = 0.05;

enum Solid {
    Polygons(MultiPolygon<f64>),
    Circle(Vec2d, f64),
}

impl Solid {
    fn from_part(part: &Part, kind: &PartType) -> Self {
        let mut polygons = MultiPolygon::new(vec![]);
        for shape in world_shapes(part, kind) {
            match shape {
                WorldShape::Circle(center, radius) => return Self::Circle(center, radius),
                WorldShape::Polygon(vertices) => {
                    let polygon = Polygon::new(
                        LineString::from(
                            vertices.into_iter().map(|p| (p.x, p.y)).collect::<Vec<_>>(),
                        ),
                        vec![],
                    );
                    polygons = polygons.union(&polygon);
                }
            }
        }
        Self::Polygons(polygons)
    }

    fn area(&self) -> f64 {
        match self {
            Self::Polygons(polygons) => polygons.unsigned_area(),
            Self::Circle(_, radius) => std::f64::consts::PI * radius * radius,
        }
    }

    fn intersection(&self, other: &Self) -> f64 {
        match (self, other) {
            (Self::Polygons(a), Self::Polygons(b)) => a.intersection(b).unsigned_area(),
            (Self::Circle(a, ar), Self::Circle(b, br)) => {
                circle_intersection(a.distance(*b), *ar, *br)
            }
            (Self::Circle(center, radius), Self::Polygons(polygons))
            | (Self::Polygons(polygons), Self::Circle(center, radius)) => polygons
                .0
                .iter()
                .map(|polygon| {
                    circle_ring_area(polygon.exterior(), *center, *radius).abs()
                        - polygon
                            .interiors()
                            .iter()
                            .map(|ring| circle_ring_area(ring, *center, *radius).abs())
                            .sum::<f64>()
                })
                .sum(),
        }
    }
}

/// 相交面积 / 较小零件实体面积；传感器不计入，镜像及任意角度与渲染一致。
pub fn overlap_ratio(a: &Part, at: &PartType, b: &Part, bt: &PartType) -> f64 {
    if !intersects(a, at, b, bt) {
        return 0.0;
    }
    // 使用第一部件的局部坐标系，避免大坐标消减及共同旋转带来的裁剪量化误差。
    let mut local_a = a.clone();
    let mut local_b = b.clone();
    local_a.x = 0.0;
    local_a.y = 0.0;
    local_a.angle = 0.0;
    let (sin, cos) = a.angle.sin_cos();
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    local_b.x = dx * cos + dy * sin;
    local_b.y = dy * cos - dx * sin;
    local_b.angle -= a.angle;
    let a = Solid::from_part(&local_a, at);
    let b = Solid::from_part(&local_b, bt);
    let area = a.area().min(b.area());
    if area <= 0.0 {
        return 0.0;
    }
    (a.intersection(&b) / area).clamp(0.0, 1.0)
}

pub fn editor_overlap_blocked(a: &Part, at: &PartType, b: &Part, bt: &PartType) -> bool {
    // geo 裁剪会映射到整数网格；容忍约 1e-8 的面积比量化误差，
    // 保守拒绝等于 5% 的边界，不把旋转后的裁剪误差当作可连接空间。
    overlap_ratio(a, at, b, bt) >= MAX_EDITOR_OVERLAP - 1e-8
}

fn circle_intersection(distance: f64, a: f64, b: f64) -> f64 {
    if distance >= a + b {
        return 0.0;
    }
    if distance <= (a - b).abs() {
        return std::f64::consts::PI * a.min(b).powi(2);
    }
    let angle = |r: f64, other: f64| {
        ((distance * distance + r * r - other * other) / (2.0 * distance * r))
            .clamp(-1.0, 1.0)
            .acos()
    };
    let lens = ((-distance + a + b) * (distance + a - b) * (distance - a + b) * (distance + a + b))
        .max(0.0)
        .sqrt()
        / 2.0;
    a * a * angle(a, b) + b * b * angle(b, a) - lens
}

fn circle_ring_area(ring: &LineString<f64>, center: Vec2d, radius: f64) -> f64 {
    ring.0
        .windows(2)
        .map(|pair| {
            let a = Vec2d {
                x: pair[0].x - center.x,
                y: pair[0].y - center.y,
            };
            let b = Vec2d {
                x: pair[1].x - center.x,
                y: pair[1].y - center.y,
            };
            let d = Vec2d {
                x: b.x - a.x,
                y: b.y - a.y,
            };
            let length = d.x * d.x + d.y * d.y;
            let mut cuts = vec![0.0, 1.0];
            if length > 0.0 {
                let projection = a.x * d.x + a.y * d.y;
                let discriminant =
                    projection * projection - length * (a.x * a.x + a.y * a.y - radius * radius);
                if discriminant > 0.0 {
                    for t in [
                        (-projection - discriminant.sqrt()) / length,
                        (-projection + discriminant.sqrt()) / length,
                    ] {
                        if t > 0.0 && t < 1.0 {
                            cuts.push(t);
                        }
                    }
                }
            }
            cuts.sort_by(f64::total_cmp);
            let point = |t: f64| Vec2d {
                x: a.x + d.x * t,
                y: a.y + d.y * t,
            };
            cuts.windows(2)
                .map(|cut| {
                    let p = point(cut[0]);
                    let q = point(cut[1]);
                    let middle = point((cut[0] + cut[1]) / 2.0);
                    let cross = p.x * q.y - p.y * q.x;
                    if middle.x * middle.x + middle.y * middle.y <= radius * radius {
                        cross / 2.0
                    } else {
                        radius * radius * cross.atan2(p.x * q.x + p.y * q.y) / 2.0
                    }
                })
                .sum::<f64>()
        })
        .sum()
}

#[cfg(test)]
mod tests;
