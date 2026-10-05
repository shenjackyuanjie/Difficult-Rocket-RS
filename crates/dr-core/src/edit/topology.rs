use super::{CommandError, LinkKind, scoped};
use crate::topology::{ConnectionRef, Topology};
use crate::{PartCatalog, PartKey, Ship};

pub fn unlink(ship: &mut Ship, reference: &ConnectionRef) -> Result<(), CommandError> {
    let Some((_, connections)) = ship.group_mut(reference.group) else {
        return Err(CommandError::InvalidConnection("连接所属组已失效".into()));
    };
    if connections.get(reference.index) != Some(&reference.expected) {
        return Err(CommandError::InvalidConnection(
            "连接已变化，请重新选择边".into(),
        ));
    }
    connections.remove(reference.index);
    Ok(())
}

pub fn reparent(
    ship: &mut Ship,
    catalog: Option<&PartCatalog>,
    parent: PartKey,
    child: PartKey,
    kind: &LinkKind,
) -> Result<(), CommandError> {
    let graph = Topology::from_ship(ship);
    let p = graph
        .nodes
        .iter()
        .position(|key| *key == parent)
        .ok_or(CommandError::MissingInstance(parent))?;
    let c = graph
        .nodes
        .iter()
        .position(|key| *key == child)
        .ok_or(CommandError::MissingInstance(child))?;
    if graph.reachable(c, true).contains(&p) {
        return Err(CommandError::InvalidConnection(
            "树编辑不能把节点接到自身或后代；需要环时请使用图编辑".into(),
        ));
    }
    // 不因树投影而删除图的其他边。多父关系必须在图视图中显式处理。
    let incoming: Vec<_> = graph
        .edges
        .iter()
        .filter(|edge| {
            edge.child == c || (edge.dock == Some(c) && edge.parent != c && edge.child != c)
        })
        .collect();
    if incoming.len() > 1 {
        return Err(CommandError::InvalidConnection(
            "节点有多条入边，请在图视图选择要断开的父边".into(),
        ));
    }
    if graph.unresolved.iter().any(|edge| {
        edge.reference.group == child.group && edge.reference.expected.touches(child.id)
    }) {
        return Err(CommandError::InvalidConnection(
            "节点存在无法解析的连接，需先修复或显式断开".into(),
        ));
    }
    if let Some(edge) = incoming.first() {
        if edge.dock == Some(c) && edge.parent != c && edge.child != c {
            return Err(CommandError::InvalidConnection(
                "该历史对接边包含独立第三引用，请在图视图显式编辑整条边".into(),
            ));
        }
        unlink(ship, &edge.reference)?;
    }
    scoped::connect(ship, catalog, parent, child, kind)
}
