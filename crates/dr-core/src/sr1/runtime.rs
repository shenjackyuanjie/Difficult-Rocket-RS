//! SR1 PC Runtime、世界节点与挑战数据的独立读取层。
//!
//! 读取结果保留选择请求与原 XML 半径旁证；星体绑定、保存快照选择均显式提供上下文。
//! 本模块不推进物理、不从任意路径加载资源，也不把 planetRadius 当成恢复来源。

use super::catalog::Catalog;
use super::scalars::{parse_f64, read_bool, read_f64, read_i32, write_bool, write_f64};
use super::ship::Ship;
use super::solar::{Planet, SolarSystem};
use super::xml::{self, Element};
use super::{Error, Result};

#[derive(Debug, Clone, PartialEq)]
pub struct Runtime {
    pub time: f64,
    pub first_stage_activated: bool,
    /// 空字符串表示需要调用方提供默认星系资源名；不会自行读取磁盘。
    pub solar_system: String,
    /// 载入时的选择请求，缺失或失败为 -1；不是已经绑定的当前选择。
    pub ship_id: i32,
    pub pod_id: i32,
    pub nodes: Vec<RuntimeNode>,
    pub challenge: ChallengeState,
}

impl Runtime {
    pub fn from_xml(source: &str, catalog: &Catalog) -> Result<Self> {
        Self::from_element(&xml::parse(source)?, catalog)
    }

    pub fn from_element(element: &Element, catalog: &Catalog) -> Result<Self> {
        Self::from_element_with_kind(element, catalog, ChallengeKind::None)
    }

    pub fn from_xml_with_kind(
        source: &str,
        catalog: &Catalog,
        kind: ChallengeKind,
    ) -> Result<Self> {
        Self::from_element_with_kind(&xml::parse(source)?, catalog, kind)
    }

    /// 挑战类型来自调用方选定的关卡 Runtime 类，而不是猜测 XML 属性。
    pub fn from_element_with_kind(
        element: &Element,
        catalog: &Catalog,
        kind: ChallengeKind,
    ) -> Result<Self> {
        element.require_name("Runtime")?;
        let children = element
            .child("Nodes")
            .ok_or_else(|| Error::InvalidData("Runtime 缺少 Nodes".into()))?;
        let mut nodes = Vec::new();
        for child in &children.children {
            match child.name.as_str() {
                "PlanetNode" => nodes.push(RuntimeNode::Planet(PlanetNode::from_element(child)?)),
                "ShipNode" => nodes.push(RuntimeNode::Ship(Box::new(ShipNode::from_element(
                    child, catalog,
                )?))),
                _ => {}
            }
        }
        Ok(Self {
            time: read_f64(element.attr("time"), 0.0),
            first_stage_activated: read_bool(element.attr("firstStageActivated"), false),
            solar_system: element.attr("solarSystem").unwrap_or_default().into(),
            ship_id: read_i32(element.attr("shipId"), -1),
            pod_id: read_i32(element.attr("podId"), -1),
            nodes,
            challenge: ChallengeState::from_element(element, kind),
        })
    }

    /// 与 PC 一样，资源名缺失或为空时使用调用方的默认值。
    pub fn solar_system_or<'a>(&'a self, default: &'a str) -> &'a str {
        if self.solar_system.is_empty() {
            default
        } else {
            &self.solar_system
        }
    }

    pub fn ship_nodes(&self) -> impl Iterator<Item = &ShipNode> {
        self.nodes.iter().filter_map(|node| match node {
            RuntimeNode::Ship(ship) => Some(ship.as_ref()),
            _ => None,
        })
    }

    /// 建立资源→运行态绑定，只重建已证实的名称引用和轨道初值覆盖。
    /// 未命中的 PlanetNode 与 PC 一样忽略；未知的 ShipNode.planet 明确报错。
    pub fn bind_solar_system<'a>(&'a self, system: &'a SolarSystem) -> Result<WorldBindings<'a>> {
        let mut planets: Vec<PlanetState<'a>> = system
            .planets()
            .into_iter()
            .map(|planet| PlanetState {
                planet,
                true_anomaly: planet.orbit.as_ref().map(|orbit| orbit.v),
            })
            .collect();
        let mut ships = Vec::new();
        for node in &self.nodes {
            match node {
                RuntimeNode::Planet(saved) => {
                    if let (Some(value), Some(state)) = (
                        saved.true_anomaly,
                        planets
                            .iter_mut()
                            .find(|state| state.planet.name == saved.name),
                    ) {
                        state.true_anomaly = Some(value);
                    }
                }
                RuntimeNode::Ship(ship) => {
                    let planet = system.planet(&ship.planet).ok_or_else(|| {
                        Error::InvalidData(format!(
                            "ShipNode {} 引用未知 Planet {:?}",
                            ship.id, ship.planet
                        ))
                    })?;
                    ships.push(ShipBinding {
                        ship: ship.as_ref(),
                        planet,
                    });
                }
            }
        }
        Ok(WorldBindings { planets, ships })
    }

    /// 写出当前数据快照。半径来自实际星系，podId 仅在实际选择提供 Some 时写出。
    /// selection 不从 shipId/podId 载入请求推断；调用方负责传入当前有效选择。
    /// challenge 数值应由调用方更新为当前快照，不在此推测遥测计算公式。
    /// nodes 是调用方筛选后的待保存快照，不模拟原生对象内存 flag / 存活状态过滤。
    pub fn to_element(&self, system: &SolarSystem, selection: SaveSelection) -> Result<Element> {
        if self.solar_system.is_empty() {
            return Err(Error::InvalidData(
                "保存前需提供已解析的 solarSystem 资源名".into(),
            ));
        }
        let world = self.bind_solar_system(system)?;
        let mut root = Element::new("Runtime");
        root.set_attr("time", write_f64(self.time));
        root.set_attr(
            "firstStageActivated",
            write_bool(self.first_stage_activated),
        );
        root.set_attr("solarSystem", &self.solar_system);
        root.set_attr("shipId", selection.ship_id.to_string());
        if let Some(pod_id) = selection.pod_id {
            root.set_attr("podId", pod_id.to_string());
        }
        let mut nodes = Element::new("Nodes");
        append_world_nodes(&system.root, &world, &mut nodes)?;
        root.children.push(nodes);
        self.challenge.write_attributes(&mut root);
        Ok(root)
    }

    pub fn to_xml(&self, system: &SolarSystem, selection: SaveSelection) -> Result<String> {
        xml::to_string(&self.to_element(system, selection)?)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum RuntimeNode {
    Planet(PlanetNode),
    /// 大载荷使用间接存储，避免每个 Planet 节点都承担完整 ShipNode 的体积。
    Ship(Box<ShipNode>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlanetNode {
    pub name: String,
    /// 缺失及解析失败均为 None：不覆盖资源初值或更早的有效补丁。
    pub true_anomaly: Option<f64>,
}

impl PlanetNode {
    pub fn from_element(element: &Element) -> Result<Self> {
        element.require_name("PlanetNode")?;
        Ok(Self {
            name: element.attr("name").unwrap_or_default().into(),
            true_anomaly: element.attr("trueAnomaly").and_then(parse_f64),
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ShipNode {
    pub id: i32,
    pub planet: String,
    /// 仅用于追溯输入；本体不读取该属性，绑定和保存均不使用它。
    pub source_planet_radius: Option<String>,
    pub x: f64,
    pub y: f64,
    pub vx: f64,
    pub vy: f64,
    pub ship: Ship,
}

impl ShipNode {
    pub fn from_element(element: &Element, catalog: &Catalog) -> Result<Self> {
        element.require_name("ShipNode")?;
        let ship = element
            .child("Ship")
            .ok_or_else(|| Error::InvalidData("ShipNode 缺少 Ship".into()))?;
        Ok(Self {
            id: read_i32(element.attr("id"), 0),
            planet: element.attr("planet").unwrap_or_default().into(),
            source_planet_radius: element.attr("planetRadius").map(str::to_owned),
            x: read_f64(element.attr("x"), 0.0),
            y: read_f64(element.attr("y"), 0.0),
            vx: read_f64(element.attr("vx"), 0.0),
            vy: read_f64(element.attr("vy"), 0.0),
            ship: Ship::from_element(ship, catalog)?,
        })
    }

    pub fn to_element(&self, planet: &Planet) -> Result<Element> {
        if planet.name != self.planet {
            return Err(Error::InvalidData(format!(
                "ShipNode {} 的 Planet 上下文不匹配",
                self.id
            )));
        }
        let mut node = Element::new("ShipNode");
        node.set_attr("id", self.id.to_string());
        node.set_attr("planet", &planet.name);
        node.set_attr("planetRadius", write_f64(planet.radius));
        node.set_attr("x", write_f64(self.x));
        node.set_attr("y", write_f64(self.y));
        node.set_attr("vx", write_f64(self.vx));
        node.set_attr("vy", write_f64(self.vy));
        node.children.push(self.ship.to_element());
        Ok(node)
    }
}

/// 保存接口需要的真实当前选择；无 Pod 的船只应明确提供 None。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SaveSelection {
    pub ship_id: i32,
    pub pod_id: Option<i32>,
}

/// 原生 Save 的三个内存标志；名称保留实际对象偏移，不推断业务含义。
/// 这些值不来自 Runtime XML，必须由调用方明确提供。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShipSaveFlags {
    pub node_100: bool,
    pub root_1cb: bool,
    pub node_184: bool,
}

impl ShipSaveFlags {
    /// PC VA 0x450BE5..0x450C3C：三个标志按原分支判断，根前序项数必须 > 1。
    pub fn eligible(self, root_traversal_count: usize) -> bool {
        self.node_100 && !self.root_1cb && self.node_184 && root_traversal_count > 1
    }
}

pub struct WorldBindings<'a> {
    pub planets: Vec<PlanetState<'a>>,
    pub ships: Vec<ShipBinding<'a>>,
}

pub struct PlanetState<'a> {
    pub planet: &'a Planet,
    /// 尚未推进的资源初值，加上 Runtime 成功读取的补丁。
    pub true_anomaly: Option<f64>,
}

pub struct ShipBinding<'a> {
    pub ship: &'a ShipNode,
    pub planet: &'a Planet,
}

fn append_world_nodes(
    planet: &Planet,
    world: &WorldBindings<'_>,
    nodes: &mut Element,
) -> Result<()> {
    let mut node = Element::new("PlanetNode");
    node.set_attr("name", &planet.name);
    let state = world
        .planets
        .iter()
        .find(|state| std::ptr::eq(state.planet, planet))
        .ok_or_else(|| Error::InvalidData("缺少 Planet 保存上下文".into()))?;
    if let Some(value) = state.true_anomaly {
        node.set_attr("trueAnomaly", write_f64(value));
    }
    nodes.children.push(node);
    // PC 树先有资源子星体，载入 ShipNode 后追加到对应星体末尾。
    for child in &planet.children {
        append_world_nodes(child, world, nodes)?;
    }
    for binding in &world.ships {
        if std::ptr::eq(binding.planet, planet) {
            nodes.children.push(binding.ship.to_element(planet)?);
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ChallengeKind {
    #[default]
    None,
    MoonTouchDown,
    MoonTouchDownFuelUsed,
    MaxSpeed,
    FuelUsed,
    LowFlyer,
}

/// 只包含各挑战类实证会读写的数据，不按出现的属性自动合并不同类。
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum ChallengeState {
    #[default]
    None,
    MoonTouchDown {
        moon_touch_down: bool,
    },
    MoonTouchDownFuelUsed {
        moon_touch_down: bool,
        fuel_used: f64,
    },
    MaxSpeed {
        max_speed: f64,
    },
    /// Some 表示保存时有燃料遥测对象；None 对应原生 Save 的空指针省略分支。
    /// 载入总会读出一个 f64 初值（缺失/失败为 0），故读取结果为 Some。
    FuelUsed {
        fuel_used: Option<f64>,
    },
    LowFlyer(LowFlyerState),
}

impl ChallengeState {
    pub fn from_element(root: &Element, kind: ChallengeKind) -> Self {
        match kind {
            ChallengeKind::None => Self::None,
            ChallengeKind::MoonTouchDown => Self::MoonTouchDown {
                moon_touch_down: read_bool(root.attr("MoonTouchDown"), false),
            },
            ChallengeKind::MoonTouchDownFuelUsed => Self::MoonTouchDownFuelUsed {
                moon_touch_down: read_bool(root.attr("MoonTouchDown"), false),
                fuel_used: read_f64(root.attr("FuelUsed"), 0.0),
            },
            ChallengeKind::MaxSpeed => Self::MaxSpeed {
                max_speed: read_f64(root.attr("maxSpeed"), 0.0),
            },
            ChallengeKind::FuelUsed => Self::FuelUsed {
                fuel_used: Some(read_f64(root.attr("fuelUsed"), 0.0)),
            },
            ChallengeKind::LowFlyer => Self::LowFlyer(LowFlyerState {
                angle_traveled: read_f64(root.attr("angleTraveled"), 0.0),
                last_angle: read_f64(root.attr("lastAngle"), 0.0),
                last_angle_delta: read_f64(root.attr("lastAngleDelta"), 0.0),
                recorded_angle: read_bool(root.attr("recordedAngle"), false),
                good_job: read_bool(root.attr("goodJob"), false),
            }),
        }
    }

    /// 数值来自当前调用方快照；不将 maxSpeed 或 fuelUsed 误写成位置/燃料差值推算。
    pub fn write_attributes(&self, root: &mut Element) {
        match *self {
            Self::None => {}
            Self::MoonTouchDown { moon_touch_down } => {
                root.set_attr("MoonTouchDown", write_bool(moon_touch_down));
            }
            Self::MoonTouchDownFuelUsed {
                moon_touch_down,
                fuel_used,
            } => {
                root.set_attr("MoonTouchDown", write_bool(moon_touch_down));
                root.set_attr("FuelUsed", write_f64(fuel_used));
            }
            Self::MaxSpeed { max_speed } => {
                root.set_attr("maxSpeed", write_f64(max_speed));
            }
            Self::FuelUsed { fuel_used } => {
                if let Some(value) = fuel_used {
                    root.set_attr("fuelUsed", write_f64(value));
                }
            }
            Self::LowFlyer(state) => {
                root.set_attr("angleTraveled", write_f64(state.angle_traveled));
                root.set_attr("lastAngle", write_f64(state.last_angle));
                root.set_attr("lastAngleDelta", write_f64(state.last_angle_delta));
                root.set_attr("recordedAngle", write_bool(state.recorded_angle));
                root.set_attr("goodJob", write_bool(state.good_job));
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LowFlyerState {
    pub angle_traveled: f64,
    pub last_angle: f64,
    pub last_angle_delta: f64,
    pub recorded_angle: bool,
    pub good_job: bool,
}

#[cfg(test)]
mod tests;
