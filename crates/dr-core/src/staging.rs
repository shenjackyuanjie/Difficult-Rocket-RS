//! 指定驾驶舱的基础自动分级；纯计算，不改变船体、手动计划或编辑历史。

use crate::model::PartId;
use crate::{
    Activation, Connection, PartCatalog, PartKey, PartKind, Ship, StageStep, StagingState,
};
use std::cmp::Reverse;
use std::collections::{BTreeMap, HashSet, VecDeque};
use thiserror::Error;

/// 基础分级目前只接受驾驶舱所在的、引用唯一的普通连接树。
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum StagingError {
    #[error("驾驶舱位置已失效：{0}")]
    PodNotFound(PartKey),
    #[error("所选部件需要是具有 Pod 状态的驾驶舱：{0}")]
    NotPod(PartKey),
    #[error("组 {group} 的部件 #{id} 有多个实例，需先明确引用归属")]
    AmbiguousPart { group: usize, id: PartId },
    #[error("组 {group} 的连接引用了不存在的部件 #{id}")]
    MissingPart { group: usize, id: PartId },
    #[error("{part} 的类型 {part_type} 不在部件目录中")]
    MissingPartType { part: PartKey, part_type: String },
    #[error("组 {group} 的连接 {connection} 是对接连接，需手动分级")]
    UnsupportedDock { group: usize, connection: usize },
    #[error("组 {group} 的连接 {connection} 构成环或重复边，需手动分级")]
    Cycle { group: usize, connection: usize },
    #[error("驾驶舱 {pod} 的连接分量中还有驾驶舱 {other}，需手动分级")]
    MultiplePods { pod: PartKey, other: PartKey },
}

/// 从指定 Pod 根化的无向连接树生成基础分级。
///
/// 仅检查同组可达分量，其他组及不连通部件不参与生成或验证。分量内必须
/// 只有一个驾驶舱、部件 ID 引用唯一且仅有 Normal 连接；自环和重复边也拒绝。
/// 指定部件必须在目录中为 Pod 且具有 Pod 状态，以便通过现有 SetStaging 提交。
///
/// 每条连接边增加一级深度，只纳入 Engine、Detacher、Parachute。同深度同类别
/// 归入一张卡，卡片按深度降序、Engine → Detacher → Parachute 排列，卡内按 ID
/// 升序。异类别的次序是本项目的稳定约定，不代表完全复刻原版的遍历次序。
///
/// 结果按 steps[0] 开始的执行顺序保存，current_stage 为 0，所有 moved 为 false，
/// 没有空步骤；没有候选部件时返回空计划。此函数不合并或保护旧的手动分级，
/// 调用方应在用户确认生成/重置后通过一个 SetStaging 事务替换旧计划。
pub fn generate_basic_staging(
    ship: &Ship,
    catalog: &PartCatalog,
    pod: PartKey,
) -> Result<StagingState, StagingError> {
    let (parts, connections) = ship
        .group(pod.group)
        .ok_or(StagingError::PodNotFound(pod))?;
    if ship.part_at(pod).is_none() {
        return Err(StagingError::PodNotFound(pod));
    }

    let mut visited = HashSet::from([pod.id]);
    let mut queue = VecDeque::from([(pod.id, 0usize, None)]);
    let mut stages = BTreeMap::<(Reverse<usize>, u8), Vec<Activation>>::new();
    while let Some((id, depth, incoming)) = queue.pop_front() {
        let mut matches = parts.iter().filter(|part| part.id == id);
        let part = matches.next().ok_or(StagingError::MissingPart {
            group: pod.group,
            id,
        })?;
        if matches.next().is_some() {
            return Err(StagingError::AmbiguousPart {
                group: pod.group,
                id,
            });
        }
        let key = PartKey::new(pod.group, id, 0);
        let kind = catalog
            .get(&part.part_type)
            .ok_or_else(|| StagingError::MissingPartType {
                part: key,
                part_type: part.part_type.clone(),
            })?
            .kind;
        if id == pod.id {
            if kind != PartKind::Pod || part.pod.is_none() {
                return Err(StagingError::NotPod(pod));
            }
        } else if kind == PartKind::Pod || part.pod.is_some() {
            return Err(StagingError::MultiplePods { pod, other: key });
        }

        let rank = match kind {
            PartKind::Engine => Some(0),
            PartKind::Detacher => Some(1),
            PartKind::Parachute => Some(2),
            _ => None,
        };
        if let Some(rank) = rank {
            stages
                .entry((Reverse(depth), rank))
                .or_default()
                .push(Activation { id, moved: false });
        }

        for (index, connection) in connections.iter().enumerate() {
            if Some(index) == incoming || !connection.touches(id) {
                continue;
            }
            let next = match connection {
                Connection::Normal { parent, child, .. } => {
                    if *parent == id {
                        *child
                    } else {
                        *parent
                    }
                }
                Connection::Dock { .. } => {
                    return Err(StagingError::UnsupportedDock {
                        group: pod.group,
                        connection: index,
                    });
                }
            };
            if !visited.insert(next) {
                return Err(StagingError::Cycle {
                    group: pod.group,
                    connection: index,
                });
            }
            queue.push_back((next, depth + 1, Some(index)));
        }
    }

    Ok(StagingState {
        current_stage: 0,
        steps: stages
            .into_values()
            .map(|mut activations| {
                activations.sort_unstable_by_key(|activation| activation.id);
                StageStep { activations }
            })
            .collect(),
    })
}

#[cfg(test)]
mod tests;
