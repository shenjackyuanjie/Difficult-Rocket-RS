mod repair;
pub mod selection;
mod topology;
pub use selection::{SelectionPose, SelectionTransform, ShipFragment};
pub(crate) mod scoped;
pub use repair::{ConnectionRole, DuplicateRepair, ReferenceSite};

use crate::model::{Connection, Part, PartCatalog, PartId, PartKey, Ship, StagingState};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CommandError {
    #[error("部件不存在: {0}")]
    MissingPart(PartId),
    #[error("部件 ID 有多个实例，请指定所属组与实例: {0}")]
    AmbiguousPart(PartId),
    #[error("部件位置已失效: {0}")]
    MissingInstance(PartKey),
    #[error("同组重复 ID 的连接或分级引用无法判定归属: {0}")]
    AmbiguousReference(PartKey),
    #[error("部件已存在: {0}")]
    DuplicatePart(PartId),
    #[error("操作不适用")]
    Invalid,
    #[error("无法连接: {0}")]
    InvalidConnection(String),
    #[error("无法修复重复编号: {0}")]
    InvalidRepair(String),
    #[error("无法编辑所选部件: {0}")]
    InvalidSelection(String),
}

#[derive(Debug, Clone)]
pub enum EditorCommand {
    /// 自由编辑：关闭几何/占用判定，变换保留外部连接；引用和目录约束仍校验。
    FreeEdit(Box<EditorCommand>),
    RemoveConnection(crate::topology::ConnectionRef),
    /// 树编辑只替换唯一父边，拒绝引入环或静默抹去多父关系。
    Reparent {
        parent: PartKey,
        child: PartKey,
        kind: LinkKind,
    },
    SetMetadata {
        name: String,
        description: String,
    },
    DeleteSelection(Vec<PartKey>),
    TransformSelection {
        parts: Vec<PartKey>,
        transform: SelectionTransform,
    },
    /// 现有部件拖拽：允许重叠，保留内部边并断开外部边。
    DragSelection {
        parts: Vec<PartKey>,
        pivot: (f64, f64),
        turns: u8,
        delta: (f64, f64),
    },
    /// 任意角度及轴向镜像的拖拽，姿态和位移原子提交。
    DragPoseSelection {
        parts: Vec<PartKey>,
        pose: SelectionPose,
        delta: (f64, f64),
    },
    Paste {
        fragment: Box<ShipFragment>,
        offset: (f64, f64),
    },
    RepairDuplicates(Box<DuplicateRepair>),
    /// 对当前快照中一个明确实例执行属性或变换命令。
    Scoped {
        part: PartKey,
        command: Box<EditorCommand>,
    },
    /// XML 中的 ID 是组内引用；连接两组时只重编号被合并组的冲突 ID。
    ConnectParts {
        parent: PartKey,
        child: PartKey,
        kind: LinkKind,
    },
    Place(Box<Part>),
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
    Disconnect(PartId),
    SetActive(PartId, bool),
    SetFuel(PartId, f64),
    RenamePod(PartId, String),
    SetThrottle(PartId, f64),
    SetStaging(PartId, Option<StagingState>),
    Batch(Vec<EditorCommand>),
}

#[derive(Debug, Clone)]
pub enum LinkKind {
    Normal {
        parent_attach: i32,
        child_attach: i32,
    },
    Dock {
        connector: PartKey,
    },
}

#[derive(Debug, Clone)]
struct Entry {
    snapshot: Ship,
    // 为两端较大的一份保留预算，撤销/重做交换时无需丢弃其他记录。
    reserved_bytes: usize,
}

#[derive(Debug)]
pub struct EditorHistory {
    undo: Vec<Entry>,
    redo: Vec<Entry>,
    limit: usize,
    byte_limit: usize,
}

impl Default for EditorHistory {
    fn default() -> Self {
        Self {
            undo: vec![],
            redo: vec![],
            limit: 256,
            byte_limit: 64 * 1024 * 1024,
        }
    }
}

impl EditorHistory {
    pub fn with_limit(limit: usize) -> Self {
        Self {
            limit: limit.max(1),
            ..Self::default()
        }
    }
    /// 按步骤和快照容量限制历史；即使一份快照超过预算，也保留最近一步。
    pub fn with_limits(steps: usize, bytes: usize) -> Self {
        Self {
            limit: steps.max(1),
            byte_limit: bytes.max(1),
            ..Self::default()
        }
    }
    pub fn undo_len(&self) -> usize {
        self.undo.len()
    }
    pub fn redo_len(&self) -> usize {
        self.redo.len()
    }
    pub fn reserved_bytes(&self) -> usize {
        self.undo
            .iter()
            .chain(&self.redo)
            .map(|entry| entry.reserved_bytes)
            .sum()
    }
    pub fn execute(&mut self, ship: &mut Ship, command: EditorCommand) -> Result<(), CommandError> {
        self.execute_inner(ship, None, command)
    }
    pub fn execute_with_catalog(
        &mut self,
        ship: &mut Ship,
        catalog: &PartCatalog,
        command: EditorCommand,
    ) -> Result<(), CommandError> {
        self.execute_inner(ship, Some(catalog), command)
    }
    fn execute_inner(
        &mut self,
        ship: &mut Ship,
        catalog: Option<&PartCatalog>,
        command: EditorCommand,
    ) -> Result<(), CommandError> {
        let after = command.preview_checked(ship, catalog)?;
        if *ship == after {
            return Ok(());
        }
        let reserved_bytes = ship.retained_bytes().max(after.retained_bytes());
        let before = std::mem::replace(ship, after);
        self.redo.clear();
        self.undo.push(Entry {
            snapshot: before,
            reserved_bytes,
        });
        let mut bytes = self.reserved_bytes();
        while self.undo.len() > 1 && (self.undo.len() > self.limit || bytes > self.byte_limit) {
            bytes -= self.undo[0].reserved_bytes;
            self.undo.remove(0);
        }
        Ok(())
    }
    pub fn undo(&mut self, ship: &mut Ship) -> bool {
        if let Some(mut entry) = self.undo.pop() {
            std::mem::swap(ship, &mut entry.snapshot);
            self.redo.push(entry);
            true
        } else {
            false
        }
    }
    pub fn redo(&mut self, ship: &mut Ship) -> bool {
        if let Some(mut entry) = self.redo.pop() {
            std::mem::swap(ship, &mut entry.snapshot);
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
    pub fn at(self, part: PartKey) -> Self {
        Self::Scoped {
            part,
            command: Box::new(self),
        }
    }

    /// 原子执行：任何子命令失败时保留原始船体。
    pub fn apply(&self, ship: &mut Ship) -> Result<(), CommandError> {
        self.apply_checked(ship, None)
    }

    fn apply_checked(
        &self,
        ship: &mut Ship,
        catalog: Option<&PartCatalog>,
    ) -> Result<(), CommandError> {
        *ship = self.preview_checked(ship, catalog)?;
        Ok(())
    }

    pub fn preview_with_catalog(
        &self,
        ship: &Ship,
        catalog: &PartCatalog,
    ) -> Result<Ship, CommandError> {
        self.preview_checked(ship, Some(catalog))
    }

    fn preview_checked(
        &self,
        ship: &Ship,
        catalog: Option<&PartCatalog>,
    ) -> Result<Ship, CommandError> {
        let mut after = ship.clone();
        self.apply_inner(&mut after, catalog, None, false)?;
        // 批量命令完成后再压缩被编辑清空的组，保留输入本身已有的空组。
        let mut group = 0;
        after.disconnected.retain(|current| {
            let original = ship.disconnected.get(group);
            group += 1;
            !current.parts.is_empty()
                || !current.connections.is_empty()
                || original.is_some_and(|original| {
                    original.parts.is_empty() && original.connections.is_empty()
                })
        });
        Ok(after)
    }

    fn apply_inner(
        &self,
        ship: &mut Ship,
        catalog: Option<&PartCatalog>,
        scope: Option<PartKey>,
        free: bool,
    ) -> Result<(), CommandError> {
        match self {
            Self::FreeEdit(command) => command.apply_inner(ship, catalog, scope, true)?,
            Self::RemoveConnection(reference) => topology::unlink(ship, reference)?,
            Self::Reparent {
                parent,
                child,
                kind,
            } => topology::reparent(ship, catalog, *parent, *child, kind, free)?,
            Self::SetMetadata { name, description } => {
                ship.name = name.clone();
                ship.description = description.clone();
            }
            Self::DeleteSelection(parts) => selection::delete(ship, parts)?,
            Self::TransformSelection { parts, transform } => {
                selection::transform(ship, catalog, parts, *transform, free)?
            }
            Self::DragSelection {
                parts,
                pivot,
                turns,
                delta,
            } => selection::drag(ship, catalog, parts, *pivot, *turns, *delta, free)?,
            Self::DragPoseSelection { parts, pose, delta } => {
                selection::drag_pose(ship, catalog, parts, *pose, *delta, free)?
            }
            Self::Paste { fragment, offset } => {
                selection::paste(ship, catalog, fragment, *offset, free)?
            }
            Self::RepairDuplicates(repair) => repair.apply_inner(ship)?,
            Self::Scoped { part, command } => {
                if scope.is_some() || ship.part_at(*part).is_none() {
                    return Err(CommandError::MissingInstance(*part));
                }
                command.apply_inner(ship, catalog, Some(*part), free)?;
            }
            Self::ConnectParts {
                parent,
                child,
                kind,
            } => {
                scoped::connect(ship, catalog, *parent, *child, kind, free)?;
            }
            Self::Batch(commands) => {
                for command in commands {
                    command.apply_inner(ship, catalog, scope, free)?;
                }
            }
            Self::SetActive(id, active) => {
                let key = scoped::resolve(ship, *id, scope)?;
                ship.part_at_mut(key).unwrap().active = *active;
            }
            Self::SetFuel(id, fuel) => {
                let key = scoped::resolve(ship, *id, scope)?;
                let part = ship.part_at_mut(key).unwrap();
                if !fuel.is_finite() || *fuel < 0.0 || part.fuel_kind.is_none() {
                    return Err(CommandError::Invalid);
                }
                part.fuel = Some(*fuel);
            }
            Self::RenamePod(id, name) => {
                let key = scoped::resolve(ship, *id, scope)?;
                let part = ship.part_at_mut(key).unwrap();
                part.pod.as_mut().ok_or(CommandError::Invalid)?.name = name.clone();
            }
            Self::SetThrottle(id, throttle) => {
                if !throttle.is_finite() || !(0.0..=1.0).contains(throttle) {
                    return Err(CommandError::Invalid);
                }
                let key = scoped::resolve(ship, *id, scope)?;
                let part = ship.part_at_mut(key).unwrap();
                part.pod.as_mut().ok_or(CommandError::Invalid)?.throttle = *throttle;
            }
            Self::SetStaging(id, staging) => {
                let key = scoped::resolve(ship, *id, scope)?;
                if let Some(staging) = staging {
                    if staging.current_stage < 0 {
                        return Err(CommandError::Invalid);
                    }
                    for step in &staging.steps {
                        let mut ids = std::collections::HashSet::new();
                        for activation in &step.activations {
                            scoped::resolve_in_group(ship, key.group, activation.id)?;
                            if !ids.insert(activation.id) {
                                return Err(CommandError::Invalid);
                            }
                        }
                    }
                }
                let key = scoped::resolve(ship, *id, scope)?;
                let part = ship.part_at_mut(key).unwrap();
                part.pod.as_mut().ok_or(CommandError::Invalid)?.staging = staging.clone();
            }
            Self::Disconnect(id) => {
                let key = scoped::resolve(ship, *id, scope)?;
                scoped::disconnect(ship, key)?;
            }
            Self::Place(part) => {
                if part.id <= 0 || !part.x.is_finite() || !part.y.is_finite() {
                    return Err(CommandError::Invalid);
                }
                if ship.all_parts().any(|p| p.id == part.id) {
                    return Err(CommandError::DuplicatePart(part.id));
                }
                ship.parts.push((**part).clone());
            }
            Self::Delete(id) => {
                let key = scoped::resolve(ship, *id, scope)?;
                scoped::delete(ship, key)?;
            }
            Self::Move { id, to, .. } => {
                if !to.0.is_finite() || !to.1.is_finite() {
                    return Err(CommandError::Invalid);
                }
                let key = scoped::resolve(ship, *id, scope)?;
                let part = ship.part_at_mut(key).unwrap();
                part.x = to.0;
                part.y = to.1;
            }
            Self::Rotate(id) => {
                let key = scoped::resolve(ship, *id, scope)?;
                let part = ship.part_at_mut(key).unwrap();
                part.editor_angle = (part.editor_angle.rem_euclid(4) + 1).rem_euclid(4);
                part.angle =
                    (part.angle + std::f64::consts::FRAC_PI_2).rem_euclid(std::f64::consts::TAU);
            }
            Self::FlipX(id) => {
                let key = scoped::resolve(ship, *id, scope)?;
                let part = ship.part_at_mut(key).unwrap();
                part.flip_x = !part.flip_x;
            }
            Self::FlipY(id) => {
                let key = scoped::resolve(ship, *id, scope)?;
                let part = ship.part_at_mut(key).unwrap();
                part.flip_y = !part.flip_y;
            }
            Self::Connect(connection) => {
                let (parent, child, kind) = match *connection {
                    Connection::Normal {
                        parent,
                        child,
                        parent_attach,
                        child_attach,
                    } => (
                        parent,
                        child,
                        LinkKind::Normal {
                            parent_attach,
                            child_attach,
                        },
                    ),
                    Connection::Dock {
                        parent,
                        child,
                        dock,
                    } => (
                        parent,
                        child,
                        LinkKind::Dock {
                            connector: scoped::resolve(ship, dock, None)?,
                        },
                    ),
                };
                let parent = scoped::resolve(ship, parent, None)?;
                let child = scoped::resolve(ship, child, None)?;
                scoped::connect(ship, catalog, parent, child, &kind, free)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod repair_tests;
#[cfg(test)]
mod scoped_tests;
#[cfg(test)]
mod selection_tests;

#[cfg(test)]
mod id_boundary_tests;

#[cfg(test)]
mod tests;
