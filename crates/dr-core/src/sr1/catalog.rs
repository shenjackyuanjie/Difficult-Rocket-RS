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
mod tests {
    use super::*;

    // 从 PC PartList 逐项摘录，保持各项内部字段和顺序，不访问游戏目录。
    const PC_RESOURCE_FRAGMENT: &str = r#"<PartTypes xmlns="http://jundroo.com/simplerockets/partlist.xsd">
	<PartType id="pod-1" name="Command Pod Mk1" description="This is your ship's brain. Be careful with it." sprite="Pod.png" type="pod" mass="1.0" width="4" height="3" hidden="true">
		<Damage disconnect="1500" explode="1500" explosionPower="5" explosionSize="10" />
		<Shape>
			<!-- Must be counter clockwise -->
			<Vertex x="-2.0" y="-1.5" />
			<Vertex x="2.0" y="-1.5" />
			<Vertex x="1.3" y="1.5" />
			<Vertex x="-1.3" y="1.5" />
		</Shape>
		<AttachPoints>
			<AttachPoint location="TopCenter" />
			<AttachPoint location="BottomCenter" />
		</AttachPoints>
	</PartType>
	<PartType id="fueltank-4" name="Puffy T750" description="Monopropellant for RCS thrusters." sprite="Puffy750.png" type="tank" mass="1.85" width="4" height="2" category="Satellite">
		<Tank fuel="750.0" dryMass="0.35" fuelType="1" />
		<Damage disconnect="2500" explode="2500" explosionPower="5" explosionSize="10" />
		<AttachPoints>
			<AttachPoint location="TopCenter" />
			<AttachPoint location="BottomCenter" />
			<AttachPoint location="LeftSide" />
			<AttachPoint location="RightSide" />
		</AttachPoints>
	</PartType>
	<PartType id="engine-0" name="Tiny 21" description="Really only useful for landing on Smoon." sprite="EngineTiny.png" type="engine" mass="0.5" width="4" height="2" buoyancy="0.5">
		<Engine power="0.25" consumption="4.00" size="0.50" turn="20.0" throttleExponential="true" />
		<Shape>
			<Vertex x="-0.9" y="-1.0" />
			<Vertex x="0.9" y="-1.0" />
			<Vertex x="2.0" y="1.0" />
			<Vertex x="-2.0" y="1.0" />
		</Shape>
		<AttachPoints>
			<AttachPoint location="TopCenter" fuelLine="true" order="1" />
			<AttachPoint location="BottomCenter" order="2" />
		</AttachPoints>
	</PartType>
	<PartType id="engine-4" name="Blasto SRB 500" description="Great for blasting off, but they can't be throttle or turn." sprite="SolidRocketBooster.png" type="engine" mass="22.0" width="4" height="26" buoyancy="0.5" coverHeight="2" sandboxOnly="true">
		<Engine power="6.0" consumption="175" size="1.0" turn="0.0" fuelType="3" />
		<Tank fuel="9000.0" dryMass="3.15" />
		<AttachPoints>
			<AttachPoint location="TopCenter" order="1" />
			<AttachPoint location="BottomCenter" order="2" />
			<AttachPoint location="LeftSide" />
			<AttachPoint location="RightSide" />
		</AttachPoints>
	</PartType>
	<PartType id="rcs-1" name="RCS Thrusters" description="Reaction control system for precision maneuvering." sprite="RcsBlock.png" type="rcs" mass="0.25" width="1" height="3" category="Satellite" disableEditorRotation="true" buoyancy="0.5">
		<Rcs power="1.0" consumption="0.1" size="1.00" />
		<AttachPoints>
			<AttachPoint location="RightCenter" flipX="true" group="1" />
			<AttachPoint location="LeftCenter" group="1" />
		</AttachPoints>
	</PartType>
	<PartType id="solar-1" name="Solar Panel" description="Expanding solar panel." sprite="SolarPanelBase.png" type="solar" mass="1.0" width="1" height="4" category="Satellite" sandboxOnly="true">
		<Solar chargeRate="2.0" />
		<Damage disconnect="250" explode="3000" explosionPower="1" explosionSize="5" />
		<AttachPoints>
			<AttachPoint location="RightCenter" flipX="true" group="1" />
			<AttachPoint location="LeftCenter" group="1" />
		</AttachPoints>
	</PartType>
	<PartType id="battery-0" name="Batteries" description="Batteries can be recharged by solar panels." sprite="Battery.png" type="tank" mass="1.25" width="4" height="1" category="Satellite" sandboxOnly="true">
		<Tank fuel="250.0" dryMass="1.24" fuelType="2" />
		<Damage disconnect="2500" explode="2500" explosionPower="5" explosionSize="10" />
		<AttachPoints>
			<AttachPoint location="TopCenter" />
			<AttachPoint location="BottomCenter" />
			<AttachPoint location="LeftSide" />
			<AttachPoint location="RightSide" />
		</AttachPoints>
	</PartType>
	<PartType id="port-1" name="Docking Port" description="Accepts a docking plug for docking in space." sprite="DockingPort.png" type="dockport" mass="0.75" width="2" height="4" friction="0.1" category="Satellite" buoyancy="0.5">
		<Shape>
			<Vertex x="-0.2" y="-2.0" />
			<Vertex x="1.0" y="-2.0" />
			<Vertex x="1.0" y="2.0" />
			<Vertex x="-0.2" y="2.0" />
		</Shape>
		<Shape>
			<Vertex x="-1.0" y="-2.0" />
			<Vertex x="0.0" y="-2.0" />
			<Vertex x="0.0" y="-0.1" />
			<Vertex x="-1.0" y="-1.10" />
		</Shape>
		<Shape>
			<Vertex x="-1.0" y="1.10" />
			<Vertex x="0.0" y="0.1" />
			<Vertex x="0.0" y="2.0" />
			<Vertex x="-1.0" y="2.0" />
		</Shape>
		<Shape sensor="true">
			<Vertex x="-0.33" y="-1.0" />
			<Vertex x="-0.2" y="-1.0" />
			<Vertex x="-0.2" y="1.0" />
			<Vertex x="-0.33" y="1.0" />
		</Shape>
		<AttachPoints>
			<AttachPoint location="RightCenter" group="1" />
			<AttachPoint location="LeftCenter" flipX="true" group="1" />
		</AttachPoints>
	</PartType>
	<PartType id="lander-1" name="Lander" description="Activate these babies to make landing a little easier." sprite="LanderLegPreview.png" type="lander" mass="0.5" width="1" height="5" ignoreEditorIntersections="true" buoyancy="0.5">
		<Lander maxAngle="140" minLength="2.26" maxLength="4.15" angleSpeed="25" lengthSpeed="0.5" width="0.5" />
		<AttachPoints>
			<AttachPoint location="LeftCenter" group="1" />
			<AttachPoint location="RightCenter" group="1" />
		</AttachPoints>
	</PartType>
</PartTypes>"#;

    fn part(data: &str) -> PartType {
        PartType::from_element(&xml::parse(data).unwrap()).unwrap()
    }

    #[test]
    fn real_resource_fragment_loads_special_classes_and_capacities() {
        let catalog = Catalog::from_xml(PC_RESOURCE_FRAGMENT).unwrap();
        assert_eq!(catalog.parts.len(), 9);
        assert_eq!(catalog.parts[0].id, "pod-1");
        assert_eq!(catalog.parts[8].id, "lander-1");
        let pod = catalog.get("pod-1").unwrap();
        assert_eq!(pod.class, PartClass::Pod);
        assert!(pod.hidden);
        assert_eq!(pod.width, 2.0);
        assert_eq!(pod.shapes[0].vertices[0], (-1.0, -0.75));
        assert_eq!(pod.damage.disconnect, 1500.0);
        let tank = catalog.get("fueltank-4").unwrap();
        assert_eq!(tank.fuel_capacity(), Some(750.0));
        assert_eq!(tank.tank.as_ref().unwrap().fuel_type, 1);
        assert_eq!(tank.category, "Satellite");
        let engine = catalog.get("engine-0").unwrap();
        assert_eq!(engine.class, PartClass::Engine);
        assert_eq!(engine.category, "Engines");
        assert_eq!(engine.fuel_capacity(), Some(0.0));
        assert_eq!(
            engine.engine.as_ref().unwrap().turn_radians,
            20.0 * DEGREES_TO_RADIANS
        );
        assert!(engine.engine.as_ref().unwrap().throttle_exponential);
        let srb = catalog.get("engine-4").unwrap();
        assert_eq!(srb.fuel_capacity(), Some(9000.0));
        assert_eq!(srb.tank.as_ref().unwrap().fuel_type, 3);
        assert_eq!(srb.cover_height, 1.0);
        assert_eq!(
            catalog
                .get("battery-0")
                .unwrap()
                .tank
                .as_ref()
                .unwrap()
                .fuel_type,
            2
        );
        assert_eq!(
            catalog
                .get("rcs-1")
                .unwrap()
                .rcs
                .as_ref()
                .unwrap()
                .consumption,
            0.1
        );
        assert_eq!(
            catalog
                .get("solar-1")
                .unwrap()
                .solar
                .as_ref()
                .unwrap()
                .charge_rate,
            2.0
        );
        let port = catalog.get("port-1").unwrap();
        assert_eq!(port.class, PartClass::DockPort);
        assert_eq!(port.shapes.len(), 4);
        assert!(port.shapes[3].sensor);
        let lander = catalog.get("lander-1").unwrap().lander.as_ref().unwrap();
        assert_eq!(lander.min_length, 2.26);
        assert_eq!(lander.max_angle, 140.0);
        assert_eq!(lander.angle_speed, 25.0);
    }

    #[test]
    fn native_general_defaults_and_f32_rectangle_are_not_editor_defaults() {
        let value = part(r#"<PartType width="4" height="6"><AttachPoints/></PartType>"#);
        assert_eq!(value.class, PartClass::Generic);
        assert_eq!(value.mass, 0.0);
        assert_eq!(
            (value.width, value.height, value.cover_height),
            (2.0, 3.0, 3.0)
        );
        assert_eq!(
            (value.drag, value.friction, value.buoyancy),
            (1.0, 0.75, 1.0)
        );
        assert!(value.can_explode);
        assert!(!value.hidden);
        assert_eq!(value.max_occurrences, 0);
        assert!(!value.stageable);
        assert_eq!(
            value.damage,
            DamageSpec {
                disconnect: 3000.0,
                explode: 3000.0,
                explosion_power: 1.0,
                explosion_size: 5.0,
            }
        );
        let x = 1.0_f32 - 0.025_f32;
        assert_eq!(
            value.shapes[0].vertices,
            vec![(-x, -1.5), (x, -1.5), (x, 1.5), (-x, 1.5)]
        );
        assert_eq!(value.fuel_capacity(), None);
    }

    #[test]
    fn present_empty_damage_is_zero_not_missing_damage_defaults() {
        let value = part("<PartType><Damage/><AttachPoints/></PartType>");
        assert_eq!(
            value.damage,
            DamageSpec {
                disconnect: 0.0,
                explode: 0.0,
                explosion_power: 0.0,
                explosion_size: 0.0,
            }
        );
    }

    #[test]
    fn scalar_prefixes_negative_integers_and_failed_defaults_are_preserved() {
        let value = part(
            r#"<PartType mass="1.234567890123tail" width="4.5units" height="bad" drag="bad" friction=".125x" canExplode="bad" hidden="-2extra" maxOccurrences="-3parts"><AttachPoints/></PartType>"#,
        );
        assert_eq!(value.mass, 1.234567890123);
        assert_eq!(value.width, 2.25);
        assert_eq!(value.height, 0.0);
        assert_eq!(value.drag, 1.0);
        assert_eq!(value.friction, 0.125);
        assert!(value.can_explode);
        assert!(value.hidden);
        assert_eq!(value.max_occurrences, -3);
    }

    #[test]
    fn cover_height_only_scales_successfully_parsed_overrides() {
        for (attribute, expected) in [("", 3.0), ("bad", 3.0), ("10tail", 5.0), ("0", 0.0)] {
            let source = format!(
                r#"<PartType height="6" coverHeight="{attribute}"><AttachPoints/></PartType>"#
            );
            assert_eq!(part(&source).cover_height, expected);
        }
    }

    #[test]
    fn named_attach_locations_use_native_scaled_dimensions() {
        let value = part(
            r#"<PartType width="8" height="12"><AttachPoints>
            <AttachPoint location="TopCenter" x="999"/>
            <AttachPoint location="BottomCenter"/>
            <AttachPoint location="Top"/>
            <AttachPoint location="Bottom"/>
            <AttachPoint location="LeftCenter"/>
            <AttachPoint location="RightCenter"/>
            <AttachPoint location="LeftSide"/>
            <AttachPoint location="RightSide"/>
        </AttachPoints></PartType>"#,
        );
        let expected = [
            (0.0, 3.0, 0.0, 0.0, 1),
            (0.0, -3.0, 0.0, 0.0, 1),
            (0.0, 3.0, 3.0, 0.0, 0),
            (0.0, -3.0, 3.0, 0.0, 0),
            (-2.0, 0.0, 0.0, 0.0, 1),
            (2.0, 0.0, 0.0, 0.0, 1),
            (-2.0, 0.0, 0.0, 5.0, 0),
            (2.0, 0.0, 0.0, 5.0, 0),
        ];
        for (index, (point, expected)) in value.attach_points.iter().zip(expected).enumerate() {
            assert_eq!(point.id, index as i32 + 1);
            assert_eq!(
                (point.x, point.y, point.width, point.height, point.max_parts),
                expected
            );
        }
        assert!(value.attach_points[1].bottom);
        assert!(value.attach_points[3].bottom);
        assert_eq!(value.attach_points[4].swap_id, 6);
        assert_eq!(value.attach_points[5].swap_id, 5);
        assert_eq!(value.attach_points[6].swap_id, 8);
        assert!(value.has_attach_point(8));
        assert!(!value.has_attach_point(0));
    }

    #[test]
    fn explicit_attach_points_scale_coordinates_but_not_ranges() {
        let value = part(
            r#"<PartType><AttachPoints><AttachPoint id="99" location="custom" x="1.5" y="-2" width="5" height="6" maxParts="3" order="7" group="2" fuelLine="1" flipX="true" flipY="true" breakAngle="90" breakForce="150"/></AttachPoints></PartType>"#,
        );
        let point = &value.attach_points[0];
        assert_eq!(point.id, 1);
        assert_eq!(
            (point.x, point.y, point.width, point.height),
            (0.75, -1.0, 5.0, 6.0)
        );
        assert_eq!((point.max_parts, point.order, point.group), (3, 7, 2));
        assert!(point.fuel_line && point.flip_x && point.flip_y);
        assert_eq!(point.break_angle_radians, 90.0 * DEGREES_TO_RADIANS);
        assert_eq!(point.break_force, 150.0);
        assert_eq!(point.raw.attr("id"), Some("99"));
    }

    #[test]
    fn zero_and_invalid_break_limits_use_native_nonzero_defaults() {
        let value = part(
            r#"<PartType><AttachPoints><AttachPoint breakAngle="-0" breakForce="0"/><AttachPoint breakAngle="bad" breakForce="bad"/></AttachPoints></PartType>"#,
        );
        for point in &value.attach_points {
            assert_eq!(point.break_angle_radians, 15.0 * DEGREES_TO_RADIANS);
            assert_eq!(point.break_force, 125.0);
        }
    }

    #[test]
    fn mirror_pairing_preserves_native_direction_and_invalid_id_behavior() {
        let value = part(
            r#"<PartType><AttachPoints><AttachPoint location="LeftCenter" swapOnMirror="false"/><AttachPoint location="RightCenter"/><AttachPoint swapId="99"/></AttachPoints></PartType>"#,
        );
        assert!(!value.attach_points[0].swap_on_mirror);
        assert_eq!(value.attach_points[0].swap_id, 2);
        assert_eq!(value.attach_points[1].swap_id, 1);
        assert!(!value.attach_points[2].swap_on_mirror);
        assert_eq!(value.attach_points[2].swap_id, 0);
        let unmatched = part(
            r#"<PartType><AttachPoints><AttachPoint location="LeftSide"/></AttachPoints></PartType>"#,
        );
        assert!(unmatched.attach_points[0].swap_on_mirror);
        assert_eq!(unmatched.attach_points[0].swap_id, 0);
    }

    #[test]
    fn explicit_shape_coordinates_round_after_f64_scaling_and_empty_shape_stays_empty() {
        let value = part(
            r#"<PartType width="4"><Shape sensor="1"><Vertex x="2.000000123" y="-1.23456789"/><Vertex x="bad"/></Shape><Shape/><AttachPoints/></PartType>"#,
        );
        assert_eq!(value.shapes.len(), 2);
        assert!(value.shapes[0].sensor);
        assert_eq!(
            value.shapes[0].vertices[0],
            (
                (2.000000123_f64 * 0.5) as f32,
                (-1.23456789_f64 * 0.5) as f32
            )
        );
        assert_eq!(value.shapes[0].vertices[1], (0.0, 0.0));
        assert!(value.shapes[1].vertices.is_empty());
    }

    #[test]
    fn engine_tank_inherits_engine_fuel_type_and_nonengine_static_children_are_ignored() {
        let value = part(
            r#"<PartType type="engine"><Engine fuelType="3"/><Tank fuel="9000.123456789" dryMass="3.15" fuelType="9"/><AttachPoints/></PartType>"#,
        );
        assert_eq!(value.fuel_capacity(), Some(9000.123456789));
        assert_eq!(value.tank.as_ref().unwrap().fuel_type, 3);
        let generic = part(
            r#"<PartType type="fuselage"><Tank fuel="7"/><Engine power="4"/><AttachPoints/></PartType>"#,
        );
        assert_eq!(generic.class, PartClass::Generic);
        assert!(generic.tank.is_none() && generic.engine.is_none());
        assert!(generic.raw.child("Tank").is_some());
    }

    #[test]
    fn factory_is_exact_and_wheel_activation_has_legacy_id_exception() {
        assert_eq!(PartClass::from_type("Tank"), PartClass::Generic);
        assert_eq!(PartClass::from_type("strut"), PartClass::Generic);
        assert_eq!(PartClass::from_type("nosecone"), PartClass::Nosecone);
        assert_eq!(
            PartClass::from_type("dockconnector"),
            PartClass::DockConnector
        );
        for (id, stageable) in [("wheel-1", false), ("wheel-2", true)] {
            let source = format!(r#"<PartType id="{id}" type="wheel"><AttachPoints/></PartType>"#);
            assert_eq!(part(&source).stageable, stageable);
        }
    }

    #[test]
    fn missing_required_native_children_return_explicit_errors_but_solar_stays_optional() {
        assert!(PartType::from_element(&xml::parse("<PartType/>").unwrap()).is_err());
        for kind in ["engine", "tank", "rcs", "lander"] {
            let source = format!(r#"<PartType type="{kind}"><AttachPoints/></PartType>"#);
            assert!(matches!(
                PartType::from_element(&xml::parse(&source).unwrap()),
                Err(Error::InvalidData(_))
            ));
        }
        let solar = part(r#"<PartType type="solar"><AttachPoints/></PartType>"#);
        assert!(solar.solar.is_none());
        assert!(solar.stageable);
        let solar = part(r#"<PartType type="solar"><Solar/><AttachPoints/></PartType>"#);
        assert_eq!(solar.solar.unwrap().charge_rate, 0.0);
    }

    #[test]
    fn duplicate_ids_or_display_names_are_rejected_without_last_wins() {
        for second in [r#"id="a" name="B""#, r#"id="b" name="A""#] {
            let source = format!(
                r#"<PartTypes><PartType id="a" name="A"><AttachPoints/></PartType><PartType {second}><AttachPoints/></PartType></PartTypes>"#
            );
            assert!(matches!(
                Catalog::from_xml(&source),
                Err(Error::InvalidData(_))
            ));
        }
        assert!(Catalog::from_xml("<PartTypes/>").unwrap().parts.is_empty());
        assert!(matches!(
            Catalog::from_xml("<PartList/>"),
            Err(Error::WrongRoot { .. })
        ));
    }

    #[test]
    fn sibling_walks_and_unconsumed_fields_are_preserved() {
        let source = r#"<PartTypes future="keep"><Ignored/><PartType id="a" name="A" future="part"><Shape><Ignored/><Vertex x="2"/><Other x="4"/></Shape><AttachPoints><Ignored/><AttachPoint/><Other/></AttachPoints><Future value="7"/></PartType><Other id="b" name="B"><AttachPoints/></Other></PartTypes>"#;
        let catalog = Catalog::from_xml(source).unwrap();
        assert_eq!(catalog.parts.len(), 2);
        let value = catalog.get("a").unwrap();
        assert_eq!(value.shapes[0].vertices, vec![(1.0, 0.0), (2.0, 0.0)]);
        assert_eq!(value.attach_points.len(), 2);
        assert_eq!(value.raw.attr("future"), Some("part"));
        assert_eq!(value.raw.child("Future").unwrap().attr("value"), Some("7"));
        assert_eq!(catalog.raw.attr("future"), Some("keep"));
        assert_eq!(catalog.get("b").unwrap().raw.name, "Other");
    }
}
