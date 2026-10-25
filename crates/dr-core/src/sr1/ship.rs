//! SR1 PC 的 Ship / PartTree / ShipEditor 读取语义。
//!
//! 本模块保留各树实例边界，不调用严格编辑器模型，也不将分离组并入主树。
//! 拓扑在加载时解析为部件数组索引，重复 XML ID 不会使内部引用失去实例身份。

use super::catalog::Catalog;
use super::parts::Part;
use super::scalars::{read_bool, read_i32, write_bool};
use super::staging::Staging;
use super::xml::{self, Element};
use super::{Error, Result};

/// 连接引用的是 [`PartTree::parts`] 中的实例索引，而不是 XML 部件 ID。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Connection {
    pub parent: usize,
    pub child: usize,
    pub kind: ConnectionKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionKind {
    Normal {
        parent_attach_point: i32,
        child_attach_point: i32,
    },
    Dock {
        /// 对接部件在同一棵树的部件数组中的实例索引。
        dock_part: usize,
    },
}

/// 一次 PartTree::Load 的结果，包含且仅包含一个 XML 树组。
///
/// `parts` 保留输入次序和孤立项，`traversal()` 表示 PC 实际使用的根前序遍历。
/// 写出只使用后者；孤立项不会被自动变为编辑器分离组。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PartTree {
    parts: Vec<Part>,
    connections: Vec<Connection>,
    root_index: Option<usize>,
    part_order: Vec<usize>,
    connection_order: Vec<usize>,
}

impl PartTree {
    /// 读取承载 Parts / Connections 的元素；独立调用按 version=1 解释。
    pub fn from_element(element: &Element, catalog: &Catalog) -> Result<Self> {
        Self::from_element_with_version(element, catalog, 1)
    }

    /// PC 的版本号来自外层 Ship；仅 version == 0 启用旧根分级迁移。
    pub fn from_element_with_version(
        element: &Element,
        catalog: &Catalog,
        version: i32,
    ) -> Result<Self> {
        let parts_element = element.child("Parts").ok_or_else(|| {
            Error::InvalidData(format!(
                "{} 缺少 Parts；PC 此处会解引用空节点",
                element.name
            ))
        })?;
        let connections_element = element.child("Connections").ok_or_else(|| {
            Error::InvalidData(format!(
                "{} 缺少 Connections；PC 此处会解引用空节点",
                element.name
            ))
        })?;

        // PC 对 Parts/Connections 使用 FirstChildElement()，不按子元素名称过滤。
        let mut parts = Vec::with_capacity(parts_element.children.len());
        for child in &parts_element.children {
            let mut source = child.clone();
            source.name = "Part".into();
            parts.push(Part::from_element(&source, catalog)?);
        }
        let mut connections = Vec::new();
        for child in &connections_element.children {
            if let Some(connection) = load_connection(child, &parts, catalog)? {
                connections.push(connection);
            }
        }

        let root_index = resolve_root(parts.len(), &connections)?;
        // 每次载入只建立一次，主树及各 Pod 子树共用原连接数组的出边下标。
        let outgoing_edges = index_outgoing_edges(parts.len(), &connections);
        let (part_order, connection_order) = match root_index {
            Some(root) => walk(root, &connections, &outgoing_edges)?,
            None => (Vec::new(), Vec::new()),
        };
        // 仅根部件自己的 Pod 接收旧根分级；已有自身 Staging 时不覆盖。
        if version == 0
            && let (Some(root), Some(legacy)) = (root_index, element.child("Staging"))
            && let Some(pod) = parts[root].pod_mut()
            && pod.staging.is_none()
        {
            pod.staging = Some(Staging::from_element_with_current_stage(
                legacy,
                read_i32(element.attr("currentStage"), 0),
            )?);
        }

        // Pod 的 deferred load 从 Pod 所属 Part 开始遍历，而不是从整船根开始。
        for index in 0..parts.len() {
            if parts[index].pod().is_some() {
                let (subtree, _) = walk(index, &connections, &outgoing_edges)?;
                let ids = subtree.into_iter().map(|i| parts[i].id).collect::<Vec<_>>();
                if let Some(pod) = parts[index].pod_mut()
                    && let Some(staging) = &mut pod.staging
                {
                    staging.retain_part_ids(&ids);
                }
            }
        }
        Ok(Self {
            parts,
            connections,
            root_index,
            part_order,
            connection_order,
        })
    }

    pub fn parts(&self) -> &[Part] {
        &self.parts
    }

    /// 允许修改部件运行状态；拓扑及分级 ID 引用仍属于原加载快照。
    pub fn parts_mut(&mut self) -> &mut [Part] {
        &mut self.parts
    }

    pub fn connections(&self) -> &[Connection] {
        &self.connections
    }

    pub fn root_index(&self) -> Option<usize> {
        self.root_index
    }

    pub fn root(&self) -> Option<&Part> {
        self.root_index.map(|index| &self.parts[index])
    }

    /// 与 PC PartTree::GetAllParts 相同的根前序次序；多路径可重复访问同一实例。
    pub fn traversal(&self) -> impl Iterator<Item = &Part> {
        self.part_order.iter().map(|&index| &self.parts[index])
    }

    /// 未被实际根触达的输入项，仅供诊断；PC 保存不会输出它们。
    pub fn unreachable_parts(&self) -> impl Iterator<Item = &Part> {
        self.parts
            .iter()
            .enumerate()
            .filter_map(|(index, part)| (!self.part_order.contains(&index)).then_some(part))
    }

    /// 将本树写成指定容器，不改变树组边界。
    pub fn to_element(&self, name: &str) -> Element {
        let mut element = Element::new(name);
        self.write_into(&mut element);
        element
    }

    fn write_into(&self, element: &mut Element) {
        let mut parts = Element::new("Parts");
        parts
            .children
            .extend(self.traversal().map(Part::to_element));
        let mut connections = Element::new("Connections");
        for &index in &self.connection_order {
            let connection = &self.connections[index];
            let mut child = match connection.kind {
                ConnectionKind::Normal {
                    parent_attach_point,
                    child_attach_point,
                } => {
                    let mut child = Element::new("Connection");
                    child.set_attr("parentAttachPoint", parent_attach_point.to_string());
                    child.set_attr("childAttachPoint", child_attach_point.to_string());
                    child
                }
                ConnectionKind::Dock { dock_part } => {
                    let mut child = Element::new("DockConnection");
                    child.set_attr("dockPart", self.parts[dock_part].id.to_string());
                    child
                }
            };
            child.set_attr("parentPart", self.parts[connection.parent].id.to_string());
            child.set_attr("childPart", self.parts[connection.child].id.to_string());
            connections.children.push(child);
        }
        element.children.push(parts);
        element.children.push(connections);
    }
}

/// 本体船体。`disconnected` 由显式的编辑器入口读写。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Ship {
    /// 原始加载版本；保存始终写 1。
    pub version: i32,
    pub lifted_off: bool,
    pub touching_ground: bool,
    pub tree: PartTree,
    pub disconnected: Vec<PartTree>,
    selected_pod: Option<usize>,
}

impl Ship {
    /// 与 PC Ship::Load 对应：只加载主树，不解析编辑器分离组。
    pub fn from_element(element: &Element, catalog: &Catalog) -> Result<Self> {
        if element.name != "Ship" {
            return Err(Error::WrongRoot {
                expected: "Ship",
                found: element.name.clone(),
            });
        }
        let version = read_i32(element.attr("version"), 0);
        let tree = PartTree::from_element_with_version(element, catalog, version)?;
        let selected_pod = tree
            .part_order
            .iter()
            .copied()
            .find(|&index| tree.parts[index].pod().is_some());
        Ok(Self {
            version,
            lifted_off: read_bool(element.attr("liftedOff"), false),
            touching_ground: read_bool(element.attr("touchingGround"), false),
            tree,
            disconnected: Vec::new(),
            selected_pod,
        })
    }

    pub fn from_xml(source: &str, catalog: &Catalog) -> Result<Self> {
        Self::from_element(&xml::parse(source)?, catalog)
    }

    /// ShipEditor 在主树加载之后按同一 Ship 版本号逐组加载。
    pub fn from_editor_element(element: &Element, catalog: &Catalog) -> Result<Self> {
        let mut ship = Self::from_element(element, catalog)?;
        if let Some(groups) = element.child("DisconnectedParts") {
            for group in groups.children_named("DisconnectedPart") {
                ship.disconnected.push(PartTree::from_element_with_version(
                    group,
                    catalog,
                    ship.version,
                )?);
            }
        }
        Ok(ship)
    }

    pub fn from_editor_xml(source: &str, catalog: &Catalog) -> Result<Self> {
        Self::from_editor_element(&xml::parse(source)?, catalog)
    }

    /// 构造时选根 Pod；否则按主树前序选第一个 Pod。不扫描分离组。
    pub fn selected_pod_id(&self) -> Option<i32> {
        self.selected_pod
            .and_then(|index| self.tree.parts.get(index))
            .filter(|part| part.pod().is_some())
            .map(|part| part.id)
    }

    /// 显式按主树遍历次序选择匹配 ID 的 Pod。未找到时保留当前选择。
    pub fn select_pod(&mut self, id: i32) -> bool {
        if let Some(index) = self.tree.part_order.iter().copied().find(|&index| {
            let part = &self.tree.parts[index];
            part.id == id && part.pod().is_some()
        }) {
            self.selected_pod = Some(index);
            true
        } else {
            false
        }
    }

    /// 与 PC Ship::Save 对应：固定 version=1，只写本体主树。
    pub fn to_element(&self) -> Element {
        let mut element = Element::new("Ship");
        element.set_attr("version", "1");
        element.set_attr("liftedOff", write_bool(self.lifted_off));
        element.set_attr("touchingGround", write_bool(self.touching_ground));
        self.tree.write_into(&mut element);
        element
    }

    pub fn to_xml(&self) -> Result<String> {
        xml::to_string(&self.to_element())
    }

    /// 编辑器将 DisconnectedParts 插入主树 Parts 之前，连空容器也保留。
    pub fn to_editor_element(&self) -> Element {
        let mut element = self.to_element();
        let mut groups = Element::new("DisconnectedParts");
        for tree in &self.disconnected {
            groups.children.push(tree.to_element("DisconnectedPart"));
        }
        element.children.insert(0, groups);
        element
    }

    pub fn to_editor_xml(&self) -> Result<String> {
        xml::to_string(&self.to_editor_element())
    }
}

fn load_connection(
    element: &Element,
    parts: &[Part],
    catalog: &Catalog,
) -> Result<Option<Connection>> {
    let parent_id = read_i32(element.attr("parentPart"), 0);
    let child_id = read_i32(element.attr("childPart"), 0);
    let mut parent = None;
    let mut child = None;
    // PC 的一次正向扫描不提前停止，parent 分支优先于 child 分支。
    for (index, part) in parts.iter().enumerate() {
        if part.id == parent_id {
            parent = Some(index);
        } else if part.id == child_id {
            child = Some(index);
        }
    }
    let parent = parent.ok_or_else(|| {
        Error::InvalidData(format!(
            "{} 引用不存在的 parentPart={parent_id}",
            element.name
        ))
    })?;
    let child = child.ok_or_else(|| {
        Error::InvalidData(format!(
            "{} 引用不存在或与父 ID 相同的 childPart={child_id}",
            element.name
        ))
    })?;
    let kind = if element.name == "Connection" {
        let parent_attach_point = read_i32(element.attr("parentAttachPoint"), 0);
        let child_attach_point = read_i32(element.attr("childAttachPoint"), 0);
        let parent_valid = catalog
            .get(&parts[parent].part_type)
            .is_some_and(|ty| ty.has_attach_point(parent_attach_point));
        let child_valid = catalog
            .get(&parts[child].part_type)
            .is_some_and(|ty| ty.has_attach_point(child_attach_point));
        if !parent_valid || !child_valid {
            return Ok(None);
        }
        ConnectionKind::Normal {
            parent_attach_point,
            child_attach_point,
        }
    } else {
        // PC 只有名称 == Connection 才走普通分支，所有其他名称都走对接分支。
        let dock_id = read_i32(element.attr("dockPart"), 0);
        let dock_part = parts
            .iter()
            .position(|part| part.id == dock_id)
            .ok_or_else(|| {
                Error::InvalidData(format!("{} 引用不存在的 dockPart={dock_id}", element.name))
            })?;
        ConnectionKind::Dock { dock_part }
    };
    Ok(Some(Connection {
        parent,
        child,
        kind,
    }))
}

fn resolve_root(part_count: usize, connections: &[Connection]) -> Result<Option<usize>> {
    if part_count == 0 {
        return Ok(None);
    }
    let mut parents = vec![None; part_count];
    for connection in connections {
        parents[connection.child] = Some(connection.parent);
    }
    let mut current = 0;
    let mut seen = vec![false; part_count];
    loop {
        if seen[current] {
            return Err(Error::InvalidData(format!(
                "PartTree 从首部件沿父链在实例索引 {current} 遇到环"
            )));
        }
        seen[current] = true;
        match parents[current] {
            Some(parent) => current = parent,
            None => return Ok(Some(current)),
        }
    }
}

/// 按原连接次序记录每个 parent 的出边，不复制连接或合并重复边。
fn index_outgoing_edges(part_count: usize, connections: &[Connection]) -> Vec<Vec<usize>> {
    let mut outgoing_edges = vec![Vec::new(); part_count];
    for (index, connection) in connections.iter().enumerate() {
        outgoing_edges[connection.parent].push(index);
    }
    outgoing_edges
}

fn walk(
    root: usize,
    connections: &[Connection],
    outgoing_edges: &[Vec<usize>],
) -> Result<(Vec<usize>, Vec<usize>)> {
    enum Visit {
        Part(usize),
        Leave(usize),
        Edge(usize),
    }
    let mut stack = vec![Visit::Part(root)];
    let mut active = vec![false; outgoing_edges.len()];
    let mut parts = Vec::new();
    let mut edges = Vec::new();
    while let Some(visit) = stack.pop() {
        match visit {
            Visit::Part(index) => {
                if active[index] {
                    return Err(Error::InvalidData(format!(
                        "PartTree 子树遍历在实例索引 {index} 遇到环"
                    )));
                }
                active[index] = true;
                parts.push(index);
                stack.push(Visit::Leave(index));
                for &edge in outgoing_edges[index].iter().rev() {
                    stack.push(Visit::Edge(edge));
                }
            }
            Visit::Leave(index) => active[index] = false,
            Visit::Edge(edge) => {
                edges.push(edge);
                stack.push(Visit::Part(connections[edge].child));
            }
        }
    }
    Ok((parts, edges))
}

#[cfg(test)]
mod tests;
