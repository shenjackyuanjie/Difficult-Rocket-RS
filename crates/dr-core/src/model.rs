use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub type PartId = i64;

/// SR1 部件在物理和编辑器中的功能分类。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum PartKind {
    Pod,
    Detacher,
    Wheel,
    Fuselage,
    Strut,
    Tank,
    Engine,
    Parachute,
    Nosecone,
    Rcs,
    Solar,
    DockConnector,
    DockPort,
    Lander,
    #[default]
    Unknown,
}

impl PartKind {
    /// 将 PartList.xml 中的字符串转换为类型枚举。
    pub fn from_xml(value: &str) -> Self {
        match value {
            "pod" => Self::Pod,
            "detacher" => Self::Detacher,
            "wheel" => Self::Wheel,
            "fuselage" => Self::Fuselage,
            "strut" => Self::Strut,
            "tank" => Self::Tank,
            "engine" => Self::Engine,
            "parachute" => Self::Parachute,
            "nosecone" => Self::Nosecone,
            "rcs" => Self::Rcs,
            "solar" => Self::Solar,
            "dockconnector" => Self::DockConnector,
            "dockport" => Self::DockPort,
            "lander" => Self::Lander,
            _ => Self::Unknown,
        }
    }
    pub fn as_xml(self) -> &'static str {
        match self {
            Self::Pod => "pod",
            Self::Detacher => "detacher",
            Self::Wheel => "wheel",
            Self::Fuselage => "fuselage",
            Self::Strut => "strut",
            Self::Tank => "tank",
            Self::Engine => "engine",
            Self::Parachute => "parachute",
            Self::Nosecone => "nosecone",
            Self::Rcs => "rcs",
            Self::Solar => "solar",
            Self::DockConnector => "dockconnector",
            Self::DockPort => "dockport",
            Self::Lander => "lander",
            Self::Unknown => "unknown",
        }
    }
}

/// 部件上的一个连接点。
///
/// 坐标以部件中心为原点，单位与 SR1 的网格单位一致。`fuel_line`、
/// `group` 和断裂参数是编辑器及未来物理模拟所需的元数据。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AttachPoint {
    pub x: f64,
    pub y: f64,
    pub dock: bool,
    pub fuel_line: bool,
    pub flip_x: bool,
    pub group: Option<i32>,
    pub order: Option<i32>,
    pub break_angle: Option<f64>,
    pub break_force: Option<f64>,
}

/// PartList.xml 中描述的一种可放置部件。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PartType {
    pub id: String,
    pub name: String,
    pub description: String,
    pub sprite: String,
    pub kind: PartKind,
    pub mass: f64,
    pub width: u32,
    pub height: u32,
    pub category: String,
    pub hidden: bool,
    pub ignore_editor_intersections: bool,
    pub disable_editor_rotation: bool,
    pub max_occurrences: Option<u32>,
    pub tank: Option<TankSpec>,
    pub engine: Option<EngineSpec>,
    pub attach_points: Vec<AttachPoint>,
}

impl PartType {
    pub fn half_extents(&self) -> (f64, f64) {
        // PartList 尺寸每单位 30 像素，Ship 位置每单位 60 像素。
        (self.width as f64 / 4.0, self.height as f64 / 4.0)
    }
}

/// 燃料箱的静态规格。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TankSpec {
    pub fuel: f64,
    pub dry_mass: Option<f64>,
    pub fuel_type: Option<i32>,
}

/// 发动机的静态规格。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EngineSpec {
    pub power: Option<f64>,
    pub consumption: Option<f64>,
    pub size: Option<f64>,
    pub turn: Option<f64>,
    pub fuel_type: Option<i32>,
    pub throttle_exponential: bool,
}

/// 部件实例中的燃料来源，用于区分 SR1 的 Tank 和 Engine 子节点。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FuelKind {
    Tank,
    Engine,
}

/// Pod 的运行状态和名称。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PodState {
    pub throttle: f64,
    pub name: String,
    pub staging: Option<StagingState>,
}

/// 船体的分级控制数据。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct StagingState {
    pub current_stage: i32,
    pub steps: Vec<StageStep>,
}

/// 一个分级步骤及其激活动作。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct StageStep {
    pub activations: Vec<Activation>,
}

/// 分级步骤中对某个部件的激活动作。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Activation {
    pub id: PartId,
    pub moved: bool,
}

/// 船体中的一个部件实例。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Part {
    pub id: PartId,
    pub part_type: String,
    pub x: f64,
    pub y: f64,
    pub angle: f64,
    pub editor_angle: i32,
    pub angle_v: f64,
    pub flip_x: bool,
    pub flip_y: bool,
    pub active: bool,
    pub exploded: bool,
    pub fuel: Option<f64>,
    pub fuel_kind: Option<FuelKind>,
    pub extension: Option<f64>,
    pub parachute: ParachuteState,
    pub pod: Option<PodState>,
}

/// SR1 降落伞运行状态；缺省值与显式的零值分别保留。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ParachuteState {
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub angle: Option<f64>,
    pub height: Option<f64>,
    pub inflation: Option<f64>,
    pub inflate: Option<i8>,
    pub deployed: Option<i8>,
    pub rope: Option<i8>,
}

/// 两个部件之间的连接关系。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Connection {
    Normal {
        parent_attach: i32,
        child_attach: i32,
        parent: PartId,
        child: PartId,
    },
    Dock {
        dock: PartId,
        parent: PartId,
        child: PartId,
    },
}

impl Connection {
    pub fn equivalent(&self, other: &Self) -> bool {
        match (self, other) {
            (
                Self::Normal {
                    parent: p,
                    child: c,
                    parent_attach: pa,
                    child_attach: ca,
                },
                Self::Normal {
                    parent: q,
                    child: d,
                    parent_attach: qa,
                    child_attach: da,
                },
            ) => {
                (p == q && c == d && pa == qa && ca == da)
                    || (p == d && c == q && pa == da && ca == qa)
            }
            (
                Self::Dock {
                    dock: a,
                    parent: p,
                    child: c,
                },
                Self::Dock {
                    dock: b,
                    parent: q,
                    child: d,
                },
            ) => a == b && ((p == q && c == d) || (p == d && c == q)),
            _ => false,
        }
    }

    /// 判断连接是否引用给定部件。
    pub fn touches(&self, id: PartId) -> bool {
        match self {
            Self::Normal { parent, child, .. } => *parent == id || *child == id,
            Self::Dock {
                dock,
                parent,
                child,
            } => *dock == id || *parent == id || *child == id,
        }
    }
}

/// 一个与主船体断开的部件组。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ShipGroup {
    pub parts: Vec<Part>,
    pub connections: Vec<Connection>,
}

/// SR1 船体文档及其已断开的部件组。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Ship {
    pub version: i32,
    pub lifted_off: bool,
    pub touching_ground: bool,
    pub parts: Vec<Part>,
    pub connections: Vec<Connection>,
    pub disconnected: Vec<ShipGroup>,
}

impl Default for Ship {
    fn default() -> Self {
        Self {
            version: 1,
            lifted_off: false,
            touching_ground: true,
            parts: Vec::new(),
            connections: Vec::new(),
            disconnected: Vec::new(),
        }
    }
}

impl Ship {
    pub fn all_parts(&self) -> impl DoubleEndedIterator<Item = &Part> {
        self.parts
            .iter()
            .chain(self.disconnected.iter().flat_map(|g| g.parts.iter()))
    }
    pub fn all_connections(&self) -> impl Iterator<Item = &Connection> {
        self.connections
            .iter()
            .chain(self.disconnected.iter().flat_map(|g| g.connections.iter()))
    }
    pub fn disconnect_part(&mut self, id: PartId) {
        self.connections.retain(|c| !c.touches(id));
        for group in &mut self.disconnected {
            group.connections.retain(|c| !c.touches(id));
        }
    }
    /// 跨组连接时合并被连接的组，保留同组连接所在的 XML 容器。
    pub(crate) fn add_connection(&mut self, connection: Connection) {
        let joins_main = self.parts.iter().any(|p| connection.touches(p.id));
        let mut indices: Vec<_> = self
            .disconnected
            .iter()
            .enumerate()
            .filter(|(_, g)| g.parts.iter().any(|p| connection.touches(p.id)))
            .map(|(i, _)| i)
            .collect();
        if joins_main || indices.is_empty() {
            for index in indices.into_iter().rev() {
                let group = self.disconnected.remove(index);
                self.parts.extend(group.parts);
                self.connections.extend(group.connections);
            }
            self.connections.push(connection);
        } else {
            let target = indices.remove(0);
            for index in indices.into_iter().rev() {
                let group = self.disconnected.remove(index);
                self.disconnected[target].parts.extend(group.parts);
                self.disconnected[target]
                    .connections
                    .extend(group.connections);
            }
            self.disconnected[target].connections.push(connection);
        }
    }

    /// 返回所有部件中可用的下一个正整数 ID。
    pub fn next_part_id(&self) -> PartId {
        self.parts
            .iter()
            .chain(self.disconnected.iter().flat_map(|g| g.parts.iter()))
            .map(|p| p.id)
            .max()
            .unwrap_or(0)
            + 1
    }
    pub fn part(&self, id: PartId) -> Option<&Part> {
        self.parts.iter().find(|p| p.id == id).or_else(|| {
            self.disconnected
                .iter()
                .flat_map(|g| g.parts.iter())
                .find(|p| p.id == id)
        })
    }
    pub fn part_mut(&mut self, id: PartId) -> Option<&mut Part> {
        if let Some(p) = self.parts.iter_mut().find(|p| p.id == id) {
            return Some(p);
        }
        self.disconnected
            .iter_mut()
            .flat_map(|g| g.parts.iter_mut())
            .find(|p| p.id == id)
    }
    pub fn remove_part(&mut self, id: PartId) -> Option<Part> {
        let removed = self
            .parts
            .iter()
            .position(|p| p.id == id)
            .map(|i| self.parts.remove(i));
        let removed = removed.or_else(|| {
            self.disconnected.iter_mut().find_map(|g| {
                g.parts
                    .iter()
                    .position(|p| p.id == id)
                    .map(|i| g.parts.remove(i))
            })
        });
        if removed.is_some() {
            self.disconnect_part(id);
            for part in self.parts.iter_mut().chain(
                self.disconnected
                    .iter_mut()
                    .flat_map(|g| g.parts.iter_mut()),
            ) {
                if let Some(staging) = part.pod.as_mut().and_then(|pod| pod.staging.as_mut()) {
                    for step in &mut staging.steps {
                        step.activations.retain(|a| a.id != id);
                    }
                }
            }
            self.disconnected.retain(|g| !g.parts.is_empty());
        }
        removed
    }
    pub fn count_type(&self, type_id: &str) -> usize {
        self.parts
            .iter()
            .chain(self.disconnected.iter().flat_map(|g| g.parts.iter()))
            .filter(|p| p.part_type == type_id)
            .count()
    }
    pub fn total_mass(&self, catalog: &PartCatalog) -> f64 {
        self.parts
            .iter()
            .chain(self.disconnected.iter().flat_map(|g| g.parts.iter()))
            .filter_map(|p| catalog.get(&p.part_type).map(|t| t.mass))
            .sum()
    }
}

/// 可复用的部件目录及其快速索引。
#[derive(Debug, Clone, Default)]
pub struct PartCatalog {
    pub name: String,
    pub types: Vec<PartType>,
    index: HashMap<String, usize>,
}

impl PartCatalog {
    /// 构建目录，同时建立部件 ID 到数组索引的映射。
    pub fn new(name: impl Into<String>, types: Vec<PartType>) -> Self {
        let index = types
            .iter()
            .enumerate()
            .map(|(i, p)| (p.id.clone(), i))
            .collect();
        Self {
            name: name.into(),
            types,
            index,
        }
    }
    pub fn get(&self, id: &str) -> Option<&PartType> {
        self.index.get(id).and_then(|i| self.types.get(*i))
    }
    pub fn visible(&self) -> impl Iterator<Item = &PartType> {
        self.types.iter().filter(|p| !p.hidden)
    }
}
