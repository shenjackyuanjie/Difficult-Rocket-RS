use crate::model::{Connection, Part, PartId, Ship};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CommandError {
    #[error("部件不存在: {0}")]
    MissingPart(PartId),
    #[error("部件已存在: {0}")]
    DuplicatePart(PartId),
    #[error("操作不适用")]
    Invalid,
}

#[derive(Debug, Clone)]
pub enum EditorCommand {
    Place(Part),
    Delete(PartId),
    Move {
        id: PartId,
        from: (f64, f64),
        to: (f64, f64),
    },
    Rotate(PartId),
    FlipX(PartId),
    FlipY(PartId),
    Connect(Connection),
}

#[derive(Debug, Clone)]
struct Entry {
    before: Ship,
    after: Ship,
}

#[derive(Debug, Default)]
pub struct EditorHistory {
    undo: Vec<Entry>,
    redo: Vec<Entry>,
    limit: usize,
}

impl EditorHistory {
    pub fn with_limit(limit: usize) -> Self {
        Self {
            limit: limit.max(1),
            ..Self::default()
        }
    }
    pub fn execute(&mut self, ship: &mut Ship, command: EditorCommand) -> Result<(), CommandError> {
        let before = ship.clone();
        command.apply(ship)?;
        if self.limit == 0 {
            self.limit = 256;
        }
        self.undo.push(Entry {
            before,
            after: ship.clone(),
        });
        if self.undo.len() > self.limit {
            self.undo.remove(0);
        }
        self.redo.clear();
        Ok(())
    }
    pub fn undo(&mut self, ship: &mut Ship) -> bool {
        if let Some(entry) = self.undo.pop() {
            *ship = entry.before.clone();
            self.redo.push(entry);
            true
        } else {
            false
        }
    }
    pub fn redo(&mut self, ship: &mut Ship) -> bool {
        if let Some(entry) = self.redo.pop() {
            *ship = entry.after.clone();
            self.undo.push(entry);
            true
        } else {
            false
        }
    }
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
}

impl EditorCommand {
    pub fn apply(&self, ship: &mut Ship) -> Result<(), CommandError> {
        match self {
            Self::Place(part) => {
                if ship.part(part.id).is_some() {
                    return Err(CommandError::DuplicatePart(part.id));
                }
                ship.parts.push(part.clone());
            }
            Self::Delete(id) => {
                ship.remove_part(*id)
                    .ok_or(CommandError::MissingPart(*id))?;
            }
            Self::Move { id, to, .. } => {
                let part = ship.part_mut(*id).ok_or(CommandError::MissingPart(*id))?;
                part.x = to.0;
                part.y = to.1;
            }
            Self::Rotate(id) => {
                let part = ship.part_mut(*id).ok_or(CommandError::MissingPart(*id))?;
                part.editor_angle = (part.editor_angle + 1).rem_euclid(4);
                part.angle = (part.editor_angle as f64) * std::f64::consts::FRAC_PI_2;
            }
            Self::FlipX(id) => {
                let part = ship.part_mut(*id).ok_or(CommandError::MissingPart(*id))?;
                part.flip_x = !part.flip_x;
            }
            Self::FlipY(id) => {
                let part = ship.part_mut(*id).ok_or(CommandError::MissingPart(*id))?;
                part.flip_y = !part.flip_y;
            }
            Self::Connect(connection) => {
                if connection.touches(0) {
                    return Err(CommandError::Invalid);
                }
                ship.connections.push(connection.clone());
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct EditorState {
    pub ship: Ship,
    pub selected: Option<PartId>,
    pub dirty: bool,
}

impl EditorState {
    pub fn new(ship: Ship) -> Self {
        Self {
            ship,
            selected: None,
            dirty: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn part(id: i64) -> Part {
        Part {
            id,
            part_type: "pod-1".into(),
            x: 0.0,
            y: 0.0,
            angle: 0.0,
            editor_angle: 0,
            angle_v: 0.0,
            flip_x: false,
            flip_y: false,
            active: false,
            exploded: false,
            fuel: None,
            extension: None,
        }
    }
    #[test]
    fn undo_redo_restores_ship() {
        let mut ship = Ship::default();
        let mut history = EditorHistory::with_limit(8);
        history
            .execute(&mut ship, EditorCommand::Place(part(1)))
            .unwrap();
        assert!(history.undo(&mut ship));
        assert!(ship.parts.is_empty());
        assert!(history.redo(&mut ship));
        assert_eq!(ship.parts.len(), 1);
    }
}
