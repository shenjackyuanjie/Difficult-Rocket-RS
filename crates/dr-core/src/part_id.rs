//! 严格编辑模型的编号预留与分配；不适用于 SR1 本体的 i32 编号语义。

use crate::model::{Connection, Part, PartId};
use std::collections::BTreeSet;

/// 历史悬空引用也占用编号，避免新部件无意顶替原来的引用目标。
pub fn group_ids<'a>(
    parts: &'a [Part],
    connections: &'a [Connection],
) -> impl Iterator<Item = PartId> + 'a {
    let linked = connections.iter().flat_map(|connection| {
        let (parent, child, dock) = match connection {
            Connection::Normal { parent, child, .. } => (*parent, *child, None),
            Connection::Dock {
                dock,
                parent,
                child,
            } => (*parent, *child, Some(*dock)),
        };
        [Some(parent), Some(child), dock].into_iter().flatten()
    });
    let staged = parts.iter().flat_map(|part| {
        part.pod
            .iter()
            .flat_map(|pod| pod.staging.iter())
            .flat_map(|staging| staging.steps.iter())
            .flat_map(|step| step.activations.iter().map(|activation| activation.id))
    });
    parts.iter().map(|part| part.id).chain(linked).chain(staged)
}

/// 通常从最大已占用号加一分配；触及上界后从 1 查找空正数。
pub struct PartIdAllocator {
    reserved: BTreeSet<PartId>,
    next: PartId,
}

impl PartIdAllocator {
    pub fn new(ids: impl IntoIterator<Item = PartId>) -> Self {
        let reserved: BTreeSet<_> = ids.into_iter().collect();
        let next = reserved
            .last()
            .copied()
            .unwrap_or(0)
            .max(0)
            .checked_add(1)
            .unwrap_or(1);
        Self { reserved, next }
    }

    /// 返回的编号立即预留，同一事务内连续分配也不会重用。
    pub fn allocate(&mut self) -> Option<PartId> {
        let start = self.next;
        loop {
            let candidate = self.next;
            self.next = candidate.checked_add(1).unwrap_or(1);
            if self.reserved.insert(candidate) {
                return Some(candidate);
            }
            if self.next == start {
                return None;
            }
        }
    }
}

#[cfg(test)]
mod tests;
