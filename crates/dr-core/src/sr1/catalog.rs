//! SR1 PC 原生部件目录读取与静态归一。
//!
//! 本模块独立于编辑器的严格目录 DTO。长度、角度、Shape 舍入及缺省值按
//! PC 加载器还原；原始元素保留未消费字段，不代表本层实现了物理模拟。

use super::scalars::{parse_f64, read_bool, read_f64, read_i32};
use super::xml::{self, Element};
use super::{Error, Result};

/// ObjectCreator 的实际工厂分支，不等同于编辑器的展示分类。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartClass {
    Generic,
    Engine,
    Detacher,
    DockConnector,
    DockPort,
    Lander,
    Parachute,
    Pod,
    Nosecone,
    Rcs,
    Solar,
    Tank,
    Wheel,
}

impl PartClass {
    pub fn from_type(value: &str) -> Self {
        match value {
            "engine" => Self::Engine,
            "detacher" => Self::Detacher,
            "dockconnector" => Self::DockConnector,
            "dockport" => Self::DockPort,
            "lander" => Self::Lander,
            "parachute" => Self::Parachute,
            "pod" => Self::Pod,
            "nosecone" => Self::Nosecone,
            "rcs" => Self::Rcs,
            "solar" => Self::Solar,
            "tank" => Self::Tank,
            "wheel" => Self::Wheel,
            _ => Self::Generic,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Catalog {
    /// 按资源读取次序保存；直接修改后由调用方维持 ID、名称唯一性。
    pub parts: Vec<PartType>,
    pub raw: Element,
}

impl Catalog {
    pub fn from_xml(data: &str) -> Result<Self> {
        Self::from_element(&xml::parse(data)?)
    }

    pub fn from_element(element: &Element) -> Result<Self> {
        element.require_name("PartTypes")?;
        let mut parts: Vec<PartType> = Vec::new();
        // PC 首个 PartType 之后使用无名称过滤的 NextSiblingElement。
        for source in siblings_from_first(element, "PartType") {
            let part = PartType::read(source)?;
            if let Some(previous) = parts
                .iter()
                .find(|previous| previous.id == part.id || previous.name == part.name)
            {
                return Err(Error::InvalidData(format!(
                    "部件目录存在重复 ID 或名称：{} / {} 与 {} / {}",
                    part.id, part.name, previous.id, previous.name
                )));
            }
            parts.push(part);
        }
        Ok(Self {
            parts,
            raw: element.clone(),
        })
    }

    /// 按 ID 精确查询；解析阶段已拒绝重复 ID，不使用末项覆盖。
    pub fn get(&self, id: &str) -> Option<&PartType> {
        self.parts.iter().find(|part| part.id == id)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PartType {
    pub id: String,
    pub name: String,
    pub description: String,
    pub sprite: String,
    /// 原始 type 字符串；未知值和 fuselage/strut 均进入 Generic 工厂。
    pub kind: String,
    pub class: PartClass,
    pub category: String,
    pub mass: f64,
    /// 资源 width/height 读成 f64 后乘 0.5，非原始网格整数。
    pub width: f64,
    pub height: f64,
    /// 与 height 相同尺度；属性解析成功时才再乘 0.5。
    pub cover_height: f64,
    pub drag: f64,
    pub friction: f64,
    pub buoyancy: f64,
    pub can_explode: bool,
    pub hidden: bool,
    pub max_occurrences: i32,
    pub disable_editor_rotation: bool,
    pub ignore_editor_intersections: bool,
    pub sandbox_only: bool,
    /// 原生 PartType+0x138 的分级激活标记，仅还原静态读取结果。
    pub stageable: bool,
    pub damage: DamageSpec,
    /// Tank 类总有容量；Engine 类无 Tank 子元素时仍有零容量。
    pub tank: Option<TankSpec>,
    pub engine: Option<EngineSpec>,
    pub rcs: Option<RcsSpec>,
    /// Solar 子元素缺失时不猜测原生未初始化的 chargeRate。
    pub solar: Option<SolarSpec>,
    pub lander: Option<LanderSpec>,
    pub attach_points: Vec<AttachPoint>,
    pub shapes: Vec<Shape>,
    pub raw: Element,
}

impl PartType {
    pub fn from_element(element: &Element) -> Result<Self> {
        element.require_name("PartType")?;
        Self::read(element)
    }

    pub fn fuel_capacity(&self) -> Option<f64> {
        self.tank.as_ref().map(|tank| tank.fuel)
    }

    pub fn has_attach_point(&self, id: i32) -> bool {
        self.attach_points.iter().any(|point| point.id == id)
    }

    fn read(element: &Element) -> Result<Self> {
        let id = string_attr(element, "id");
        let kind = string_attr(element, "type");
        let class = PartClass::from_type(&kind);
        let mut category = string_attr(element, "category");
        if category.is_empty() {
            category = match class {
                PartClass::Engine => "Engines".into(),
                PartClass::Tank => "Fuel Tanks".into(),
                _ => String::new(),
            };
        }
        let engine = if class == PartClass::Engine {
            let source = required_child(element, "Engine")?;
            Some(EngineSpec {
                power: number(source, "power"),
                consumption: number(source, "consumption"),
                size: number(source, "size"),
                turn_radians: number(source, "turn") * DEGREES_TO_RADIANS,
                fuel_type: read_i32(source.attr("fuelType"), 0),
                throttle_exponential: read_bool(source.attr("throttleExponential"), false),
            })
        } else {
            None
        };
        let tank = match class {
            PartClass::Tank => {
                let source = required_child(element, "Tank")?;
                Some(TankSpec {
                    fuel: number(source, "fuel"),
                    dry_mass: number(source, "dryMass"),
                    fuel_type: read_i32(source.attr("fuelType"), 0),
                })
            }
            PartClass::Engine => {
                let source = element.child("Tank");
                Some(TankSpec {
                    fuel: source.map_or(0.0, |source| number(source, "fuel")),
                    dry_mass: source.map_or(0.0, |source| number(source, "dryMass")),
                    // PC 明确继承 Engine@fuelType，不读此 Tank@fuelType。
                    fuel_type: engine.as_ref().expect("Engine 类已读取 Engine").fuel_type,
                })
            }
            _ => None,
        };
        let rcs = if class == PartClass::Rcs {
            let source = required_child(element, "Rcs")?;
            Some(RcsSpec {
                power: number(source, "power"),
                consumption: number(source, "consumption"),
                size: number(source, "size"),
            })
        } else {
            None
        };
        let solar = if class == PartClass::Solar {
            element.child("Solar").map(|source| SolarSpec {
                charge_rate: number(source, "chargeRate"),
            })
        } else {
            None
        };
        let lander = if class == PartClass::Lander {
            let source = required_child(element, "Lander")?;
            Some(LanderSpec {
                min_length: number(source, "minLength"),
                max_length: number(source, "maxLength"),
                width: number(source, "width"),
                max_angle: number(source, "maxAngle"),
                angle_speed: number(source, "angleSpeed"),
                length_speed: number(source, "lengthSpeed"),
            })
        } else {
            None
        };
        let width = number(element, "width") * 0.5;
        let height = number(element, "height") * 0.5;
        let cover_height = element
            .attr("coverHeight")
            .and_then(parse_f64)
            .map_or(height, |height| height * 0.5);
        let mut attach_points = Vec::new();
        for (index, source) in
            siblings_from_first(required_child(element, "AttachPoints")?, "AttachPoint")
                .iter()
                .enumerate()
        {
            let id = i32::try_from(index + 1)
                .map_err(|_| Error::InvalidData("部件连接点数量超过 i32 范围".into()))?;
            attach_points.push(AttachPoint::read(source, id, width, height));
        }
        pair_mirror_points(&mut attach_points);
        let shapes = read_shapes(element, width, height);
        let stageable = matches!(
            class,
            PartClass::Engine
                | PartClass::Parachute
                | PartClass::Lander
                | PartClass::Detacher
                | PartClass::Solar
        ) || (class == PartClass::Wheel && id != "wheel-1");
        Ok(Self {
            id,
            name: string_attr(element, "name"),
            description: string_attr(element, "description"),
            sprite: string_attr(element, "sprite"),
            kind,
            class,
            category,
            mass: number(element, "mass"),
            width,
            height,
            cover_height,
            drag: read_f64(element.attr("drag"), 1.0),
            friction: read_f64(element.attr("friction"), 0.75),
            buoyancy: read_f64(element.attr("buoyancy"), 1.0),
            can_explode: read_bool(element.attr("canExplode"), true),
            hidden: read_bool(element.attr("hidden"), false),
            max_occurrences: read_i32(element.attr("maxOccurrences"), 0),
            disable_editor_rotation: read_bool(element.attr("disableEditorRotation"), false),
            ignore_editor_intersections: read_bool(
                element.attr("ignoreEditorIntersections"),
                false,
            ),
            sandbox_only: read_bool(element.attr("sandboxOnly"), false),
            stageable,
            damage: DamageSpec::read(element.child("Damage")),
            tank,
            engine,
            rcs,
            solar,
            lander,
            attach_points,
            shapes,
            raw: element.clone(),
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TankSpec {
    pub fuel: f64,
    pub dry_mass: f64,
    pub fuel_type: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EngineSpec {
    pub power: f64,
    pub consumption: f64,
    pub size: f64,
    pub turn_radians: f64,
    pub fuel_type: i32,
    pub throttle_exponential: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RcsSpec {
    pub power: f64,
    pub consumption: f64,
    pub size: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SolarSpec {
    pub charge_rate: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LanderSpec {
    pub min_length: f64,
    pub max_length: f64,
    pub width: f64,
    /// 目录加载阶段原样读取，不在此处假设已变成弧度。
    pub max_angle: f64,
    pub angle_speed: f64,
    pub length_speed: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DamageSpec {
    pub disconnect: f64,
    pub explode: f64,
    pub explosion_power: f64,
    pub explosion_size: f64,
}

impl DamageSpec {
    fn read(source: Option<&Element>) -> Self {
        if let Some(source) = source {
            Self {
                disconnect: number(source, "disconnect"),
                explode: number(source, "explode"),
                explosion_power: number(source, "explosionPower"),
                explosion_size: number(source, "explosionSize"),
            }
        } else {
            Self {
                disconnect: 3000.0,
                explode: 3000.0,
                explosion_power: 1.0,
                explosion_size: 5.0,
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Shape {
    /// PC 先按 f64 读坐标并乘 0.5，再转成 f32。
    pub vertices: Vec<(f32, f32)>,
    pub sensor: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AttachPoint {
    /// 从 1 自动编号，资源上的同名属性不参与读取。
    pub id: i32,
    pub location: String,
    pub x: f64,
    pub y: f64,
    /// 显式 width/height 原样读取；命名位置按部件尺寸派生。
    pub width: f64,
    pub height: f64,
    pub max_parts: i32,
    pub order: i32,
    pub group: i32,
    pub fuel_line: bool,
    pub flip_x: bool,
    pub flip_y: bool,
    pub break_angle_radians: f64,
    pub break_force: f64,
    pub swap_on_mirror: bool,
    pub swap_id: i32,
    /// 原生在 Bottom/BottomCenter 设置的方向标记。
    pub bottom: bool,
    pub raw: Element,
}

impl AttachPoint {
    fn read(source: &Element, id: i32, width: f64, height: f64) -> Self {
        let location = string_attr(source, "location");
        let (x, y, point_width, point_height, max_parts) = match location.as_str() {
            "TopCenter" => (0.0, height * 0.5, 0.0, 0.0, 1),
            "BottomCenter" => (0.0, height * -0.5, 0.0, 0.0, 1),
            "Top" => (0.0, height * 0.5, width - 1.0, 0.0, 0),
            "Bottom" => (0.0, height * -0.5, width - 1.0, 0.0, 0),
            "LeftCenter" => (width * -0.5, 0.0, 0.0, 0.0, 1),
            "RightCenter" => (width * 0.5, 0.0, 0.0, 0.0, 1),
            "LeftSide" => (width * -0.5, 0.0, 0.0, height - 1.0, 0),
            "RightSide" => (width * 0.5, 0.0, 0.0, height - 1.0, 0),
            _ => (
                number(source, "x") * 0.5,
                number(source, "y") * 0.5,
                number(source, "width"),
                number(source, "height"),
                read_i32(source.attr("maxParts"), 0),
            ),
        };
        let mut break_angle_radians = number(source, "breakAngle") * DEGREES_TO_RADIANS;
        if break_angle_radians == 0.0 {
            break_angle_radians = 15.0 * DEGREES_TO_RADIANS;
        }
        let mut break_force = number(source, "breakForce");
        if break_force == 0.0 {
            break_force = 125.0;
        }
        let swap_id = read_i32(source.attr("swapId"), 0);
        let swap_on_mirror = read_bool(
            source.attr("swapOnMirror"),
            matches!(
                location.as_str(),
                "LeftCenter" | "RightCenter" | "LeftSide" | "RightSide"
            ),
        ) || swap_id > 0;
        Self {
            id,
            bottom: matches!(location.as_str(), "Bottom" | "BottomCenter"),
            location,
            x,
            y,
            width: point_width,
            height: point_height,
            max_parts,
            order: read_i32(source.attr("order"), 0),
            group: read_i32(source.attr("group"), 0),
            fuel_line: read_bool(source.attr("fuelLine"), false),
            flip_x: read_bool(source.attr("flipX"), false),
            flip_y: read_bool(source.attr("flipY"), false),
            break_angle_radians,
            break_force,
            swap_on_mirror,
            swap_id,
            raw: source.clone(),
        }
    }
}

const DEGREES_TO_RADIANS: f64 = 0.017453292519943295;

fn string_attr(source: &Element, name: &str) -> String {
    source.attr(name).unwrap_or_default().to_owned()
}

fn number(source: &Element, name: &str) -> f64 {
    read_f64(source.attr(name), 0.0)
}

fn required_child<'a>(source: &'a Element, name: &str) -> Result<&'a Element> {
    source.child(name).ok_or_else(|| {
        Error::InvalidData(format!(
            "部件目录 {} 缺少原生加载所需的 {name} 子元素",
            source.attr("id").unwrap_or_default()
        ))
    })
}

fn siblings_from_first<'a>(parent: &'a Element, name: &str) -> &'a [Element] {
    let start = parent.children.iter().position(|child| child.name == name);
    start.map_or(&[], |start| &parent.children[start..])
}

fn read_shapes(source: &Element, width: f64, height: f64) -> Vec<Shape> {
    let mut shapes: Vec<_> = source
        .children_named("Shape")
        .map(|shape| Shape {
            sensor: read_bool(shape.attr("sensor"), false),
            vertices: siblings_from_first(shape, "Vertex")
                .iter()
                .map(|vertex| {
                    (
                        (number(vertex, "x") * 0.5) as f32,
                        (number(vertex, "y") * 0.5) as f32,
                    )
                })
                .collect(),
        })
        .collect();
    if shapes.is_empty() {
        // 与 PC cvtpd2ps 后的 mulss/subss 次序保持一致；仅 x 收缩 0.025。
        let x = (width as f32) * 0.5_f32 - 0.025_f32;
        let y = (height as f32) * 0.5_f32;
        shapes.push(Shape {
            vertices: vec![(-x, -y), (x, -y), (x, y), (-x, y)],
            sensor: false,
        });
    }
    shapes
}

fn pair_mirror_points(points: &mut [AttachPoint]) {
    for index in 0..points.len() {
        if points[index].swap_on_mirror && points[index].swap_id == 0 {
            let counterpart = match points[index].location.as_str() {
                "LeftCenter" => Some("RightCenter"),
                "RightCenter" => Some("LeftCenter"),
                "LeftSide" => Some("RightSide"),
                "RightSide" => Some("LeftSide"),
                _ => None,
            };
            if let Some(other) = counterpart
                .and_then(|location| points.iter().position(|point| point.location == location))
            {
                points[index].swap_id = points[other].id;
                points[other].swap_id = points[index].id;
            }
        }
        if i64::from(points[index].swap_id) > points.len() as i64 {
            points[index].swap_on_mirror = false;
            points[index].swap_id = 0;
        }
    }
}

#[cfg(test)]
mod tests;
