use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub type PartId = i64;

/// 当前文档快照内的实例位置；结构编辑后应重新解析，不能作为跨撤销的永久 ID。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PartKey {
    pub group: usize,
    pub id: PartId,
    pub occurrence: usize,
}

impl PartKey {
    pub const fn new(group: usize, id: PartId, occurrence: usize) -> Self {
        Self {
            group,
            id,
            occurrence,
        }
    }
}

impl std::fmt::Display for PartKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "组 {} / #{} / 实例 {}",
            self.group,
            self.id,
            self.occurrence + 1
        )
    }
}

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
    /// 保留 location，区分固定中心点与允许沿边移动的连接面。
    pub location: String,
    pub x: f64,
    pub y: f64,
    pub dock: bool,
    pub fuel_line: bool,
    pub flip_x: bool,
    pub flip_y: bool,
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
    /// 静态物理字段保留缺省与显式值的区别，不猜测运行时物理参数。
    pub friction: Option<f64>,
    pub can_explode: Option<bool>,
    pub cover_height: Option<u32>,
    pub sandbox_only: Option<bool>,
    pub drag: Option<f64>,
    pub buoyancy: Option<f64>,
    pub damage: Option<DamageSpec>,
    pub rcs: Option<RcsSpec>,
    pub solar: Option<SolarSpec>,
    pub lander: Option<LanderSpec>,
    pub tank: Option<TankSpec>,
    pub engine: Option<EngineSpec>,
    pub attach_points: Vec<AttachPoint>,
    /// PartList 局部坐标中的凸多边形；多个 Shape 共同组成部件。
    pub shapes: Vec<PolygonShape>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PolygonShape {
    pub vertices: Vec<(f64, f64)>,
    pub sensor: bool,
}

impl PartType {
    /// 按目录规格初始化编辑器新部件，燃料种类和驾驶舱状态保持 SR1 语义。
    pub fn instantiate(&self, id: PartId, position: (f64, f64)) -> Part {
        let (fuel, fuel_kind) = if let Some(tank) = &self.tank {
            (Some(tank.fuel), Some(FuelKind::Tank))
        } else if self.engine.is_some() {
            (Some(0.0), Some(FuelKind::Engine))
        } else {
            (None, None)
        };
        Part {
            id,
            part_type: self.id.clone(),
            x: position.0,
            y: position.1,
            angle: 0.0,
            editor_angle: 0,
            angle_v: 0.0,
            flip_x: false,
            flip_y: false,
            active: false,
            exploded: false,
            fuel,
            fuel_kind,
            extension: None,
            parachute: Default::default(),
            lander: Default::default(),
            pod: (self.kind == PartKind::Pod).then(|| PodState {
                staging: Some(StagingState::default()),
                ..Default::default()
            }),
        }
    }

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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DamageSpec {
    pub disconnect: f64,
    pub explode: f64,
    pub explosion_power: Option<f64>,
    pub explosion_size: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RcsSpec {
    pub power: f64,
    pub consumption: f64,
    pub size: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SolarSpec {
    pub charge_rate: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LanderSpec {
    pub max_angle: f64,
    pub min_length: f64,
    pub max_length: f64,
    pub angle_speed: Option<f64>,
    pub length_speed: Option<f64>,
    pub width: f64,
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
    pub lander: LanderState,
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

/// 着陆架运行状态，来自原版船体的部件属性。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LanderState {
    pub lower: Option<i8>,
    pub raise: Option<i8>,
    pub length: Option<f64>,
    pub leg_angle: Option<f64>,
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

/// main 只统计主组；all 包括主组及每一个断开组。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShipScope {
    Main,
    All,
}

/// SR1 船体文档及其已断开的部件组。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Ship {
    /// 船体级元数据，不与驾驶舱名称混用；非空时以 XML 根属性持久化。
    pub name: String,
    pub description: String,
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
            name: String::new(),
            description: String::new(),
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
    /// 文档自身与所拥有堆分配的容量估算，不包含分配器元数据。
    pub fn retained_bytes(&self) -> usize {
        let group_bytes = |parts: &Vec<Part>, connections: &Vec<Connection>| {
            parts.capacity() * std::mem::size_of::<Part>()
                + connections.capacity() * std::mem::size_of::<Connection>()
                + parts
                    .iter()
                    .map(|part| {
                        part.part_type.capacity()
                            + part.pod.as_ref().map_or(0, |pod| {
                                pod.name.capacity()
                                    + pod.staging.as_ref().map_or(0, |staging| {
                                        staging.steps.capacity() * std::mem::size_of::<StageStep>()
                                            + staging
                                                .steps
                                                .iter()
                                                .map(|step| {
                                                    step.activations.capacity()
                                                        * std::mem::size_of::<Activation>()
                                                })
                                                .sum::<usize>()
                                    })
                            })
                    })
                    .sum::<usize>()
        };
        std::mem::size_of::<Self>()
            + self.name.capacity()
            + self.description.capacity()
            + self.disconnected.capacity() * std::mem::size_of::<ShipGroup>()
            + group_bytes(&self.parts, &self.connections)
            + self
                .disconnected
                .iter()
                .map(|group| group_bytes(&group.parts, &group.connections))
                .sum::<usize>()
    }

    pub fn group(&self, group: usize) -> Option<(&[Part], &[Connection])> {
        if group == 0 {
            Some((&self.parts, &self.connections))
        } else {
            self.disconnected
                .get(group - 1)
                .map(|g| (g.parts.as_slice(), g.connections.as_slice()))
        }
    }

    pub(crate) fn group_mut(
        &mut self,
        group: usize,
    ) -> Option<(&mut Vec<Part>, &mut Vec<Connection>)> {
        if group == 0 {
            Some((&mut self.parts, &mut self.connections))
        } else {
            self.disconnected
                .get_mut(group - 1)
                .map(|g| (&mut g.parts, &mut g.connections))
        }
    }

    pub fn keyed_parts(&self) -> impl Iterator<Item = (PartKey, &Part)> {
        self.groups().flat_map(|(group, parts, _)| {
            let mut occurrences = HashMap::<PartId, usize>::new();
            parts.iter().map(move |part| {
                let occurrence = occurrences.entry(part.id).or_default();
                let key = PartKey::new(group, part.id, *occurrence);
                *occurrence += 1;
                (key, part)
            })
        })
    }

    pub fn part_at(&self, key: PartKey) -> Option<&Part> {
        self.group(key.group)?
            .0
            .iter()
            .filter(|p| p.id == key.id)
            .nth(key.occurrence)
    }

    pub fn part_at_mut(&mut self, key: PartKey) -> Option<&mut Part> {
        self.group_mut(key.group)?
            .0
            .iter_mut()
            .filter(|p| p.id == key.id)
            .nth(key.occurrence)
    }

    /// XML 连接和分级的组内引用；歧义引用不能随意选中某一个实例。
    pub fn group_part(&self, group: usize, id: PartId) -> Option<&Part> {
        let mut parts = self.group(group)?.0.iter().filter(|part| part.id == id);
        let part = parts.next()?;
        parts.next().is_none().then_some(part)
    }

    /// 无作用域的旧 API 只接受全局唯一的 ID，避免悄悄选中第一个重复实例。
    pub fn unique_key(&self, id: PartId) -> Option<PartKey> {
        let mut matches = self.groups().flat_map(|(group, parts, _)| {
            parts
                .iter()
                .filter(move |part| part.id == id)
                .map(move |_| PartKey::new(group, id, 0))
        });
        let key = matches.next()?;
        matches.next().is_none().then_some(key)
    }

    /// 主船体为 0，断开组依文档顺序从 1 开始；不同组可能复用 SR1 部件 ID。
    pub fn groups(&self) -> impl Iterator<Item = (usize, &[Part], &[Connection])> {
        std::iter::once((0, self.parts.as_slice(), self.connections.as_slice())).chain(
            self.disconnected
                .iter()
                .enumerate()
                .map(|(i, group)| (i + 1, group.parts.as_slice(), group.connections.as_slice())),
        )
    }

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
        if let Some(key) = self.unique_key(id) {
            let _ = crate::edit::scoped::disconnect(self, key);
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
        self.part_at(self.unique_key(id)?)
    }
    pub fn part_mut(&mut self, id: PartId) -> Option<&mut Part> {
        let key = self.unique_key(id)?;
        self.part_at_mut(key)
    }
    pub fn remove_part(&mut self, id: PartId) -> Option<Part> {
        let key = self.unique_key(id)?;
        let part = self.part_at(key)?.clone();
        crate::edit::scoped::delete(self, key).ok()?;
        Some(part)
    }
    pub fn count_type(&self, type_id: &str) -> usize {
        self.parts
            .iter()
            .chain(self.disconnected.iter().flat_map(|g| g.parts.iter()))
            .filter(|p| p.part_type == type_id)
            .count()
    }
    /// 旧接口保留 all 口径，避免已有调用方的统计结果被悄悄改变。
    pub fn total_mass(&self, catalog: &PartCatalog) -> f64 {
        self.mass(catalog, ShipScope::All)
    }
    pub fn parts_in(&self, scope: ShipScope) -> impl Iterator<Item = &Part> {
        self.parts.iter().chain(
            self.disconnected
                .iter()
                .filter(move |_| scope == ShipScope::All)
                .flat_map(|group| &group.parts),
        )
    }
    pub fn mass(&self, catalog: &PartCatalog, scope: ShipScope) -> f64 {
        self.parts_in(scope)
            .filter_map(|part| catalog.get(&part.part_type).map(|kind| kind.mass))
            .sum()
    }
}

/// 可直接编辑的部件目录。查询始终读取当前列表；重复 ID 按原版取首项。
#[derive(Debug, Clone, Default)]
pub struct PartCatalog {
    pub name: String,
    pub types: Vec<PartType>,
}

impl PartCatalog {
    /// 保留输入顺序，不维护会因公开列表修改而过期的旁路索引。
    pub fn new(name: impl Into<String>, types: Vec<PartType>) -> Self {
        Self {
            name: name.into(),
            types,
        }
    }
    pub fn get(&self, id: &str) -> Option<&PartType> {
        self.types.iter().find(|part| part.id == id)
    }
    pub fn visible(&self) -> impl Iterator<Item = &PartType> {
        self.types.iter().filter(|p| !p.hidden)
    }
}

#[cfg(test)]
#[path = "model_tests.rs"]
mod tests;
