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
        let (part_order, connection_order) = match root_index {
            Some(root) => walk(root, parts.len(), &connections)?,
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
                let (subtree, _) = walk(index, parts.len(), &connections)?;
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

fn walk(
    root: usize,
    part_count: usize,
    connections: &[Connection],
) -> Result<(Vec<usize>, Vec<usize>)> {
    enum Visit {
        Part(usize),
        Leave(usize),
        Edge(usize),
    }
    let mut stack = vec![Visit::Part(root)];
    let mut active = vec![false; part_count];
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
                for (edge, _) in connections
                    .iter()
                    .enumerate()
                    .rev()
                    .filter(|(_, connection)| connection.parent == index)
                {
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
mod tests {
    use super::*;

    fn catalog() -> Catalog {
        Catalog::from_xml(r#"<PartTypes><PartType id="pod" name="Pod" type="pod"><AttachPoints><AttachPoint/><AttachPoint/></AttachPoints></PartType><PartType id="tank" name="Tank" type="tank"><Tank fuel="100"/><AttachPoints><AttachPoint/><AttachPoint/></AttachPoints></PartType><PartType id="dock" name="Dock" type="dockconnector"><AttachPoints><AttachPoint/><AttachPoint/></AttachPoints></PartType></PartTypes>"#).unwrap()
    }

    fn load(body: &str) -> Ship {
        Ship::from_xml(body, &catalog()).unwrap()
    }

    #[test]
    fn native_ship_defaults_and_save_version_differ_from_strict_editor_contract() {
        let ship = load(
            r#"<Ship name="unused" currentStage="9"><Parts><Part partType="pod" id="1"/></Parts><Connections/></Ship>"#,
        );
        assert_eq!(ship.version, 0);
        assert!(!ship.lifted_off);
        assert!(!ship.touching_ground);
        let saved = ship.to_element();
        assert_eq!(saved.attr("version"), Some("1"));
        assert_eq!(saved.attr("liftedOff"), Some("0"));
        assert_eq!(saved.attr("touchingGround"), Some("0"));
        assert_eq!(saved.attr("name"), None);
        assert_eq!(saved.attr("currentStage"), None);
    }

    #[test]
    fn legacy_staging_migrates_only_for_zero_version_and_does_not_overwrite_pod() {
        for version in ["", "version=\"0\"", "version=\"1\"", "version=\"-1\""] {
            let xml = format!(
                r#"<Ship {version} currentStage="4"><Parts><Part partType="pod" id="1"/></Parts><Connections/><Staging><Step><Activate Id="1"/><Activate Id="404"/></Step></Staging></Ship>"#
            );
            let ship = load(&xml);
            let staging = &ship.tree.root().unwrap().pod().unwrap().staging;
            if ship.version == 0 {
                let staging = staging.as_ref().unwrap();
                assert_eq!(staging.current_stage, 4);
                assert_eq!(staging.steps[0].parts.len(), 1);
            } else {
                assert!(staging.is_none());
            }
        }
        let ship = load(
            r#"<Ship currentStage="4"><Parts><Part partType="pod" id="1"><Pod><Staging currentStage="8"><Step/></Staging></Pod></Part></Parts><Connections/><Staging><Step><Activate Id="1"/></Step></Staging></Ship>"#,
        );
        let staging = ship
            .tree
            .root()
            .unwrap()
            .pod()
            .unwrap()
            .staging
            .as_ref()
            .unwrap();
        assert_eq!(staging.current_stage, 8);
        assert!(staging.steps[0].parts.is_empty());
    }

    #[test]
    fn legacy_staging_uses_parent_chain_root_not_first_xml_part_or_first_pod() {
        let ship = load(
            r#"<Ship><Parts><Part partType="pod" id="2"/><Part partType="tank" id="1"/></Parts><Connections><Connection parentPart="1" childPart="2" parentAttachPoint="1" childAttachPoint="1"/></Connections><Staging><Step><Activate Id="2"/></Step></Staging></Ship>"#,
        );
        assert_eq!(ship.tree.root().unwrap().id, 1);
        assert_eq!(ship.selected_pod_id(), Some(2));
        assert!(ship.tree.parts()[0].pod().unwrap().staging.is_none());
        assert_eq!(
            ship.tree.traversal().map(|p| p.id).collect::<Vec<_>>(),
            [1, 2]
        );
    }

    #[test]
    fn pod_staging_filters_against_its_own_subtree() {
        let ship = load(
            r#"<Ship version="1"><Parts><Part partType="pod" id="1"/><Part partType="pod" id="2"><Pod><Staging currentStage="12"><Step><Activate Id="1"/><Activate Id="2"/><Activate Id="3"/></Step><Step><Activate Id="99"/></Step></Staging></Pod></Part><Part partType="tank" id="3"/></Parts><Connections><Connection parentPart="1" childPart="2" parentAttachPoint="1" childAttachPoint="1"/><Connection parentPart="2" childPart="3" parentAttachPoint="2" childAttachPoint="1"/></Connections></Ship>"#,
        );
        let staging = ship.tree.parts()[1]
            .pod()
            .unwrap()
            .staging
            .as_ref()
            .unwrap();
        assert_eq!(staging.current_stage, 12);
        assert_eq!(
            staging.steps[0]
                .parts
                .iter()
                .map(|p| p.part_id)
                .collect::<Vec<_>>(),
            [2, 3]
        );
        assert!(staging.steps[1].parts.is_empty());
        assert_eq!(ship.selected_pod_id(), Some(1));
    }

    #[test]
    fn invalid_attach_points_are_skipped_and_do_not_merge_isolated_parts() {
        let ship = load(
            r#"<Ship version="1"><Parts><Part partType="pod" id="1"/><Part partType="tank" id="2"/></Parts><Connections><Connection parentPart="1" childPart="2" parentAttachPoint="0" childAttachPoint="1"/></Connections></Ship>"#,
        );
        assert!(ship.tree.connections().is_empty());
        assert_eq!(ship.tree.parts().len(), 2);
        assert_eq!(
            ship.tree
                .unreachable_parts()
                .map(|p| p.id)
                .collect::<Vec<_>>(),
            [2]
        );
        assert_eq!(ship.to_element().child("Parts").unwrap().children.len(), 1);
    }

    #[test]
    fn duplicate_part_ids_resolve_normal_endpoints_to_last_instances() {
        let ship = load(
            r#"<Ship version="1"><Parts><Part partType="pod" id="1"/><Part partType="tank" id="2"/><Part partType="pod" id="1"/><Part partType="tank" id="2"/></Parts><Connections><Connection parentPart="1" childPart="2" parentAttachPoint="1" childAttachPoint="2"/></Connections></Ship>"#,
        );
        let connection = &ship.tree.connections()[0];
        assert_eq!((connection.parent, connection.child), (2, 3));
        assert_eq!(ship.tree.root_index(), Some(0));
        assert_eq!(ship.tree.traversal().count(), 1);
    }

    #[test]
    fn unknown_connection_names_use_dock_branch_and_dock_id_uses_first_match() {
        let ship = load(
            r#"<Ship version="1"><Parts><Part partType="pod" id="1"/><Part partType="tank" id="2"/><Part partType="dock" id="3"/><Part partType="dock" id="3"/></Parts><Connections><Other parentPart="1" childPart="2" dockPart="3"/></Connections></Ship>"#,
        );
        assert_eq!(
            ship.tree.connections()[0].kind,
            ConnectionKind::Dock { dock_part: 2 }
        );
        let output = ship.to_element();
        let connection = &output.child("Connections").unwrap().children[0];
        assert_eq!(connection.name, "DockConnection");
        assert_eq!(connection.attr("dockPart"), Some("3"));
        assert_eq!(connection.attr("parentPart"), Some("1"));
        assert_eq!(connection.attr("childPart"), Some("2"));
    }

    #[test]
    fn dangling_references_and_cycles_return_explicit_errors() {
        let catalog = catalog();
        for source in [
            r#"<Ship><Parts/><Connections><Connection parentPart="1" childPart="2"/></Connections></Ship>"#,
            r#"<Ship><Parts><Part partType="pod" id="1"/><Part partType="tank" id="2"/></Parts><Connections><DockConnection parentPart="1" childPart="2" dockPart="404"/></Connections></Ship>"#,
            r#"<Ship><Parts><Part partType="pod" id="1"/><Part partType="tank" id="2"/></Parts><Connections><Connection parentPart="1" childPart="2" parentAttachPoint="1" childAttachPoint="1"/><Connection parentPart="2" childPart="1" parentAttachPoint="2" childAttachPoint="2"/></Connections></Ship>"#,
            r#"<Ship/>"#,
        ] {
            assert!(matches!(
                Ship::from_xml(source, &catalog),
                Err(Error::InvalidData(_))
            ));
        }
    }

    #[test]
    fn editor_groups_keep_order_local_ids_and_prepend_container() {
        let xml = r#"<Ship><Parts><Part partType="pod" id="1"/></Parts><Connections/><DisconnectedParts><Ignored/><DisconnectedPart><Parts><Part partType="tank" id="1"/></Parts><Connections/></DisconnectedPart><Ignored/><DisconnectedPart><Parts><Part partType="pod" id="7"/></Parts><Connections/></DisconnectedPart></DisconnectedParts></Ship>"#;
        let catalog = catalog();
        assert!(
            Ship::from_xml(xml, &catalog)
                .unwrap()
                .disconnected
                .is_empty()
        );
        let ship = Ship::from_editor_xml(xml, &catalog).unwrap();
        assert_eq!(ship.disconnected.len(), 2);
        assert_eq!(ship.disconnected[0].root().unwrap().id, 1);
        assert_eq!(ship.disconnected[1].root().unwrap().id, 7);
        assert_eq!(ship.selected_pod_id(), Some(1));
        let output = ship.to_editor_element();
        assert_eq!(
            output
                .children
                .iter()
                .map(|e| e.name.as_str())
                .collect::<Vec<_>>(),
            ["DisconnectedParts", "Parts", "Connections"]
        );
        let groups = &output.children[0].children;
        assert_eq!(groups[0].name, "DisconnectedPart");
        assert_eq!(
            groups[0].child("Parts").unwrap().children[0].attr("id"),
            Some("1")
        );
        assert_eq!(
            groups[1].child("Parts").unwrap().children[0].attr("id"),
            Some("7")
        );
        assert!(ship.to_element().child("DisconnectedParts").is_none());
    }

    #[test]
    fn native_tree_output_uses_depth_first_part_and_connection_order() {
        let ship = load(
            r#"<Ship version="1"><Parts><Part partType="tank" id="4"/><Part partType="tank" id="2"/><Part partType="pod" id="1"/><Part partType="tank" id="3"/></Parts><Connections><Connection parentPart="2" childPart="4" parentAttachPoint="1" childAttachPoint="1"/><Connection parentPart="1" childPart="2" parentAttachPoint="1" childAttachPoint="1"/><Connection parentPart="1" childPart="3" parentAttachPoint="2" childAttachPoint="1"/></Connections></Ship>"#,
        );
        assert_eq!(ship.tree.root_index(), Some(2));
        assert_eq!(
            ship.tree
                .traversal()
                .map(|part| part.id)
                .collect::<Vec<_>>(),
            [1, 2, 4, 3]
        );
        let output = ship.to_element();
        assert_eq!(
            output
                .child("Parts")
                .unwrap()
                .children
                .iter()
                .map(|part| part.attr("id").unwrap())
                .collect::<Vec<_>>(),
            ["1", "2", "4", "3"]
        );
        assert_eq!(
            output
                .child("Connections")
                .unwrap()
                .children
                .iter()
                .map(|connection| (
                    connection.attr("parentPart").unwrap(),
                    connection.attr("childPart").unwrap()
                ))
                .collect::<Vec<_>>(),
            [("1", "2"), ("2", "4"), ("1", "3")]
        );
    }

    #[test]
    fn repeated_paths_are_not_silently_deduplicated_and_last_parent_selects_root() {
        let ship = load(
            r#"<Ship version="1"><Parts><Part partType="tank" id="3"/><Part partType="pod" id="1"/><Part partType="tank" id="2"/></Parts><Connections><Connection parentPart="1" childPart="3" parentAttachPoint="1" childAttachPoint="1"/><Connection parentPart="1" childPart="2" parentAttachPoint="2" childAttachPoint="1"/><Connection parentPart="2" childPart="3" parentAttachPoint="2" childAttachPoint="2"/></Connections></Ship>"#,
        );
        assert_eq!(ship.tree.root().unwrap().id, 1);
        assert_eq!(
            ship.tree
                .traversal()
                .map(|part| part.id)
                .collect::<Vec<_>>(),
            [1, 3, 2, 3]
        );
        assert_eq!(ship.to_element().child("Parts").unwrap().children.len(), 4);
    }

    #[test]
    fn numeric_defaults_and_native_sibling_walk_are_applied_to_tree_items() {
        let ship = load(
            r#"<Ship version="bad" liftedOff="true" touchingGround="2tail"><Parts><Other partType="pod"/><Other partType="tank" id="-2tail"/></Parts><Connections><Connection parentPart="bad" childPart="-2" parentAttachPoint="1tail" childAttachPoint="2"/></Connections></Ship>"#,
        );
        assert_eq!(ship.version, 0);
        assert!(ship.lifted_off);
        assert!(ship.touching_ground);
        assert_eq!(
            ship.tree
                .traversal()
                .map(|part| part.id)
                .collect::<Vec<_>>(),
            [0, -2]
        );
        assert_eq!(
            ship.tree.connections()[0].kind,
            ConnectionKind::Normal {
                parent_attach_point: 1,
                child_attach_point: 2
            }
        );
    }

    #[test]
    fn editor_disconnected_groups_use_ship_version_and_reject_cross_group_references() {
        let source = r#"<Ship currentStage="3"><Parts><Part partType="pod" id="1"/></Parts><Connections/><DisconnectedParts><DisconnectedPart currentStage="7"><Parts><Part partType="pod" id="1"/></Parts><Connections/><Staging><Step><Activate Id="1"/></Step></Staging></DisconnectedPart></DisconnectedParts></Ship>"#;
        let mut root = xml::parse(source).unwrap();
        let catalog = catalog();
        let ship = Ship::from_editor_element(&root, &catalog).unwrap();
        assert_eq!(
            ship.disconnected[0]
                .root()
                .unwrap()
                .pod()
                .unwrap()
                .staging
                .as_ref()
                .unwrap()
                .current_stage,
            7
        );
        root.set_attr("version", "1");
        let ship = Ship::from_editor_element(&root, &catalog).unwrap();
        assert!(
            ship.disconnected[0]
                .root()
                .unwrap()
                .pod()
                .unwrap()
                .staging
                .is_none()
        );
        let source = r#"<Ship version="1"><Parts><Part partType="pod" id="1"/></Parts><Connections/><DisconnectedParts><DisconnectedPart><Parts><Part partType="tank" id="2"/></Parts><Connections><Connection parentPart="1" childPart="2" parentAttachPoint="1" childAttachPoint="1"/></Connections></DisconnectedPart></DisconnectedParts></Ship>"#;
        assert!(matches!(
            Ship::from_editor_xml(source, &catalog),
            Err(Error::InvalidData(_))
        ));
    }

    #[test]
    fn default_ship_resource_migrates_and_saves_pod_staging_canonically() {
        // 自 PC DefaultShip.xml 缩小空白；目录保留相应类型、容量及连接点次序。
        let catalog = Catalog::from_xml(r#"<PartTypes><PartType id="pod-1" name="Command Pod Mk1" type="pod"><AttachPoints><AttachPoint location="TopCenter"/><AttachPoint location="BottomCenter"/></AttachPoints></PartType><PartType id="fueltank-2" name="Sloshy T3000" type="tank"><Tank fuel="3000"/><AttachPoints><AttachPoint location="TopCenter"/><AttachPoint location="BottomCenter"/><AttachPoint location="LeftSide"/><AttachPoint location="RightSide"/></AttachPoints></PartType><PartType id="engine-2" name="Blasto 170" type="engine"><Engine power="2.0" consumption="50" size="0.8" turn="3.0"/><AttachPoints><AttachPoint location="TopCenter" fuelLine="true" order="1"/><AttachPoint location="BottomCenter" order="2"/></AttachPoints></PartType></PartTypes>"#).unwrap();
        let source = r#"<Ship currentStage="0" throttle="0.000000" liftedOff="0"><Parts><Part partType="pod-1" id="1" x="0.000000" y="0.750000" angle="0.000000" angleV="0.000000" activated="0" exploded="0"/><Part partType="fueltank-2" id="9" x="0.000000" y="-2.000000" angle="0.000000" angleV="0.000000"><Tank fuel="3000.000000"/></Part><Part partType="engine-2" id="10" x="0.000000" y="-5.500000" angle="0.000000" angleV="0.000000" activated="0" exploded="0"/></Parts><Connections><Connection parentPart="1" parentAttachPoint="2" childPart="9" childAttachPoint="1"/><Connection parentPart="9" parentAttachPoint="2" childPart="10" childAttachPoint="1"/></Connections><Staging><Step><Activate Id="10"/></Step></Staging></Ship>"#;
        let ship = Ship::from_xml(source, &catalog).unwrap();
        assert_eq!(
            ship.tree
                .traversal()
                .map(|part| part.id)
                .collect::<Vec<_>>(),
            [1, 9, 10]
        );
        assert_eq!(ship.selected_pod_id(), Some(1));
        let output = ship.to_element();
        assert_eq!(output.attr("version"), Some("1"));
        assert!(output.child("Staging").is_none());
        let staging = output.child("Parts").unwrap().children[0]
            .child("Pod")
            .unwrap()
            .child("Staging")
            .unwrap();
        assert_eq!(staging.children[0].children[0].attr("Id"), Some("10"));
        assert_eq!(staging.children[0].children[0].attr("moved"), Some("1"));
        let reloaded = Ship::from_element(&output, &catalog).unwrap();
        assert_eq!(reloaded.to_element(), output);
    }

    #[test]
    fn empty_tree_and_wrong_root_are_explicit_non_crashing_boundaries() {
        let ship = load("<Ship><Parts/><Connections/></Ship>");
        assert!(ship.tree.root().is_none());
        assert!(ship.selected_pod_id().is_none());
        assert!(ship.tree.traversal().next().is_none());
        assert!(matches!(
            Ship::from_xml("<Runtime/>", &catalog()),
            Err(Error::WrongRoot {
                expected: "Ship",
                ..
            })
        ));
    }

    #[test]
    fn editor_save_keeps_empty_disconnected_container_and_selected_pod_is_explicit() {
        let mut ship = load(
            r#"<Ship version="1"><Parts><Part partType="pod" id="1"/></Parts><Connections/></Ship>"#,
        );
        assert_eq!(
            ship.to_editor_element().children[0].name,
            "DisconnectedParts"
        );
        assert!(ship.select_pod(1));
        assert!(!ship.select_pod(404));
        assert_eq!(ship.selected_pod_id(), Some(1));
    }
}
