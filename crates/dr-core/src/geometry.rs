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
        x: attach.x,
        y: attach.y,
    };
    if part.flip_x {
        local.x = -local.x;
    }
    if part.flip_y {
        local.y = -local.y;
    }
    let local = local.rotate(part.editor_angle);
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
    (a.x - b.x).abs() < aw + bw && (a.y - b.y).abs() < ah + bh
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
        assert!(find_snap(&p(1, 0.0), &a, &p(2, 2.01), &b, 0.1).is_some());
    }
    #[test]
    fn overlap_respects_flag() {
        let mut a = t(vec![]);
        let b = t(vec![]);
        assert!(intersects(&p(1, 0.0), &a, &p(2, 0.1), &b));
        a.ignore_editor_intersections = true;
        assert!(!intersects(&p(1, 0.0), &a, &p(2, 0.1), &b));
    }
}
