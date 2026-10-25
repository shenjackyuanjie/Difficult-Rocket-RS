//! 不依赖 Bevy 的物理范围与贴图范围查询。物理范围用 Ship 单位，贴图范围用像素。
use crate::geometry::{SR1_TO_PIXELS, Vec2d, WorldShape, world_shapes};
use crate::model::ShipScope;
use crate::{Part, PartCatalog, PartType, Ship};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bounds {
    pub min: Vec2d,
    pub max: Vec2d,
}

impl Bounds {
    pub fn from_points(points: impl IntoIterator<Item = Vec2d>) -> Option<Self> {
        let mut result: Option<Self> = None;
        for p in points {
            if !p.x.is_finite() || !p.y.is_finite() {
                return None;
            }
            match &mut result {
                Some(bounds) => {
                    bounds.min.x = bounds.min.x.min(p.x);
                    bounds.min.y = bounds.min.y.min(p.y);
                    bounds.max.x = bounds.max.x.max(p.x);
                    bounds.max.y = bounds.max.y.max(p.y);
                }
                None => result = Some(Self { min: p, max: p }),
            }
        }
        result
    }
    pub fn union(self, other: Self) -> Self {
        Self {
            min: Vec2d {
                x: self.min.x.min(other.min.x),
                y: self.min.y.min(other.min.y),
            },
            max: Vec2d {
                x: self.max.x.max(other.max.x),
                y: self.max.y.max(other.max.y),
            },
        }
    }
}

pub fn part_bounds(part: &Part, kind: &PartType) -> Option<Bounds> {
    Bounds::from_points(
        world_shapes(part, kind)
            .into_iter()
            .flat_map(|shape| match shape {
                WorldShape::Polygon(vertices) => vertices,
                WorldShape::Circle(p, r) => vec![
                    Vec2d {
                        x: p.x - r,
                        y: p.y - r,
                    },
                    Vec2d {
                        x: p.x + r,
                        y: p.y + r,
                    },
                ],
            }),
    )
}

pub fn ship_bounds(ship: &Ship, catalog: &PartCatalog, scope: ShipScope) -> Option<Bounds> {
    ship.parts_in(scope)
        .filter_map(|part| {
            catalog
                .get(&part.part_type)
                .and_then(|kind| part_bounds(part, kind))
        })
        .reduce(Bounds::union)
}

/// 原始 PNG 像素尺寸；保持奇数像素的 floor 锚点及镜像偏移，输出世界像素四角。
pub fn image_corners(part: &Part, size: (f64, f64)) -> [Vec2d; 4] {
    let offset_x = ((size.0 * 0.5).floor() - size.0 * 0.5) * if part.flip_x { -1.0 } else { 1.0 };
    let offset_y = ((size.1 * 0.5).floor() - size.1 * 0.5) * if part.flip_y { -1.0 } else { 1.0 };
    let (sin, cos) = part.angle.sin_cos();
    [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)].map(|(u, v)| {
        let x = (u - 0.5) * size.0 - offset_x;
        let y = (v - 0.5) * size.1 - offset_y;
        Vec2d {
            x: part.x * SR1_TO_PIXELS + x * cos - y * sin,
            y: part.y * SR1_TO_PIXELS + x * sin + y * cos,
        }
    })
}

/// 只聚合可见贴图；资源尺寸由调用方提供，核心不加载窗口或 GPU 资源。
pub fn image_bounds(
    ship: &Ship,
    catalog: &PartCatalog,
    scope: ShipScope,
    mut size: impl FnMut(&str) -> Option<(f64, f64)>,
) -> Option<Bounds> {
    ship.parts_in(scope)
        .filter_map(|part| {
            let kind = catalog.get(&part.part_type);
            let dimensions = kind.and_then(|kind| size(&kind.sprite)).unwrap_or_else(|| {
                kind.map_or((30.0, 30.0), |kind| {
                    (kind.width as f64 * 30.0, kind.height as f64 * 30.0)
                })
            });
            Bounds::from_points(image_corners(part, dimensions))
        })
        .reduce(Bounds::union)
}

#[cfg(test)]
mod tests;
