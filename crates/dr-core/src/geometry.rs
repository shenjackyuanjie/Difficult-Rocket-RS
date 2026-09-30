use crate::model::{AttachPoint, Part, PartType};

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
    let mut local = Vec2d {
        x: attach.x / 2.0,
        y: attach.y / 2.0,
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
    let mut best = None;
    for (source_index, source_attach) in source_type.attach_points.iter().enumerate() {
        let source_pos = part_world_attach(source, source_attach);
        for (target_index, target_attach) in target_type.attach_points.iter().enumerate() {
            let target_pos = part_world_attach(target, target_attach);
            let distance = source_pos.distance(target_pos);
            if distance <= threshold
                && best
                    .map(|b: SnapCandidate| b.distance > distance)
                    .unwrap_or(true)
            {
                best = Some(SnapCandidate {
                    source_index,
                    target_index,
                    position: Vec2d {
                        x: source.x + target_pos.x - source_pos.x,
                        y: source.y + target_pos.y - source_pos.y,
                    },
                    distance,
                    dock: source_attach.dock || target_attach.dock,
                });
            }
        }
    }
    best
}

pub fn intersects(a: &Part, at: &PartType, b: &Part, bt: &PartType) -> bool {
    if at.ignore_editor_intersections || bt.ignore_editor_intersections {
        return false;
    }
    let (aw, ah) = at.half_extents();
    let (bw, bh) = bt.half_extents();
    let (asin, acos) = a.angle.sin_cos();
    let (bsin, bcos) = b.angle.sin_cos();
    let axes = [(acos, asin), (-asin, acos), (bcos, bsin), (-bsin, bcos)];
    axes.iter().all(|&(x, y)| {
        let distance = ((a.x - b.x) * x + (a.y - b.y) * y).abs();
        let ar = aw * (acos * x + asin * y).abs() + ah * (-asin * x + acos * y).abs();
        let br = bw * (bcos * x + bsin * y).abs() + bh * (-bsin * x + bcos * y).abs();
        distance < ar + br - 1e-9
    })
}

/// 逆旋转鼠标坐标，确保命中区域与部件渲染尺寸一致。
pub fn contains_point(part: &Part, kind: &PartType, point: Vec2d) -> bool {
    let dx = point.x - part.x;
    let dy = point.y - part.y;
    let (sin, cos) = part.angle.sin_cos();
    let (width, height) = kind.half_extents();
    (dx * cos + dy * sin).abs() <= width && (-dx * sin + dy * cos).abs() <= height
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
        }
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
}
