use super::CoreError;
use crate::model::*;
use quick_xml::de::from_str;
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename = "PartTypes")]
struct RawPartList {
    #[serde(rename = "@name", default, skip_serializing_if = "String::is_empty")]
    name: String,
    #[serde(rename = "PartType", default)]
    parts: Vec<RawPartType>,
}

#[derive(Debug, Deserialize, Serialize)]
struct RawPartType {
    #[serde(rename = "@id")]
    id: String,
    #[serde(rename = "@name", default)]
    name: String,
    #[serde(rename = "@description", default)]
    description: String,
    #[serde(rename = "@sprite", default)]
    sprite: String,
    #[serde(rename = "@type", default)]
    kind: String,
    #[serde(rename = "@mass", default)]
    mass: f64,
    #[serde(rename = "@width", default)]
    width: u32,
    #[serde(rename = "@height", default)]
    height: u32,
    #[serde(rename = "@category", default)]
    category: String,
    #[serde(rename = "@hidden", default)]
    hidden: bool,
    #[serde(rename = "@ignoreEditorIntersections", default)]
    ignore_editor_intersections: bool,
    #[serde(rename = "@disableEditorRotation", default)]
    disable_editor_rotation: bool,
    #[serde(rename = "@maxOccurrences")]
    #[serde(skip_serializing_if = "Option::is_none")]
    max_occurrences: Option<u32>,
    #[serde(rename = "@friction")]
    #[serde(skip_serializing_if = "Option::is_none")]
    friction: Option<f64>,
    #[serde(rename = "@canExplode")]
    #[serde(skip_serializing_if = "Option::is_none")]
    can_explode: Option<bool>,
    #[serde(rename = "@coverHeight")]
    #[serde(skip_serializing_if = "Option::is_none")]
    cover_height: Option<u32>,
    #[serde(rename = "@sandboxOnly")]
    #[serde(skip_serializing_if = "Option::is_none")]
    sandbox_only: Option<bool>,
    #[serde(rename = "@drag")]
    #[serde(skip_serializing_if = "Option::is_none")]
    drag: Option<f64>,
    #[serde(rename = "@buoyancy")]
    #[serde(skip_serializing_if = "Option::is_none")]
    buoyancy: Option<f64>,
    #[serde(rename = "Damage")]
    #[serde(skip_serializing_if = "Option::is_none")]
    damage: Option<RawDamageSpec>,
    #[serde(rename = "Rcs")]
    #[serde(skip_serializing_if = "Option::is_none")]
    rcs: Option<RawRcsSpec>,
    #[serde(rename = "Solar")]
    #[serde(skip_serializing_if = "Option::is_none")]
    solar: Option<RawSolarSpec>,
    #[serde(rename = "Lander")]
    #[serde(skip_serializing_if = "Option::is_none")]
    lander: Option<RawLanderSpec>,
    #[serde(rename = "Tank")]
    #[serde(skip_serializing_if = "Option::is_none")]
    tank: Option<RawTankSpec>,
    #[serde(rename = "Engine")]
    #[serde(skip_serializing_if = "Option::is_none")]
    engine: Option<RawEngineSpec>,
    #[serde(rename = "AttachPoints")]
    #[serde(skip_serializing_if = "Option::is_none")]
    attach_points: Option<RawAttachPoints>,
    #[serde(rename = "Shape", default)]
    shapes: Vec<RawShape>,
}

#[derive(Debug, Deserialize, Serialize)]
struct RawShape {
    #[serde(rename = "Vertex", default)]
    vertices: Vec<RawVertex>,
    #[serde(rename = "@sensor", default)]
    sensor: bool,
}

#[derive(Debug, Deserialize, Serialize)]
struct RawVertex {
    #[serde(rename = "@x", default)]
    x: f64,
    #[serde(rename = "@y", default)]
    y: f64,
}

#[derive(Debug, Deserialize, Serialize)]
struct RawAttachPoints {
    #[serde(rename = "AttachPoint", default)]
    points: Vec<RawAttachPoint>,
}

#[derive(Debug, Deserialize, Serialize)]
struct RawAttachPoint {
    #[serde(rename = "@x", default, skip_serializing_if = "Option::is_none")]
    x: Option<f64>,
    #[serde(rename = "@y", default, skip_serializing_if = "Option::is_none")]
    y: Option<f64>,
    #[serde(rename = "@dock", default)]
    dock: bool,
    #[serde(rename = "@location", default)]
    location: String,
    #[serde(rename = "@fuelLine", default)]
    fuel_line: bool,
    #[serde(rename = "@flipX", default)]
    flip_x: bool,
    #[serde(rename = "@flipY", default)]
    flip_y: bool,
    #[serde(rename = "@group")]
    #[serde(skip_serializing_if = "Option::is_none")]
    group: Option<i32>,
    #[serde(rename = "@order")]
    #[serde(skip_serializing_if = "Option::is_none")]
    order: Option<i32>,
    #[serde(rename = "@breakAngle")]
    #[serde(skip_serializing_if = "Option::is_none")]
    break_angle: Option<f64>,
    #[serde(rename = "@breakForce")]
    #[serde(skip_serializing_if = "Option::is_none")]
    break_force: Option<f64>,
}

#[derive(Debug, Deserialize, Serialize)]
struct RawTankSpec {
    #[serde(rename = "@fuel", default)]
    fuel: f64,
    #[serde(rename = "@dryMass")]
    #[serde(skip_serializing_if = "Option::is_none")]
    dry_mass: Option<f64>,
    #[serde(rename = "@fuelType")]
    #[serde(skip_serializing_if = "Option::is_none")]
    fuel_type: Option<i32>,
}

#[derive(Debug, Deserialize, Serialize)]
struct RawEngineSpec {
    #[serde(rename = "@power")]
    #[serde(skip_serializing_if = "Option::is_none")]
    power: Option<f64>,
    #[serde(rename = "@consumption")]
    #[serde(skip_serializing_if = "Option::is_none")]
    consumption: Option<f64>,
    #[serde(rename = "@size")]
    #[serde(skip_serializing_if = "Option::is_none")]
    size: Option<f64>,
    #[serde(rename = "@turn")]
    #[serde(skip_serializing_if = "Option::is_none")]
    turn: Option<f64>,
    #[serde(rename = "@fuelType")]
    #[serde(skip_serializing_if = "Option::is_none")]
    fuel_type: Option<i32>,
    #[serde(rename = "@throttleExponential", default)]
    throttle_exponential: bool,
}

#[derive(Debug, Deserialize, Serialize)]
struct RawDamageSpec {
    #[serde(rename = "@disconnect")]
    disconnect: f64,
    #[serde(rename = "@explode")]
    explode: f64,
    #[serde(rename = "@explosionPower")]
    #[serde(skip_serializing_if = "Option::is_none")]
    explosion_power: Option<f64>,
    #[serde(rename = "@explosionSize")]
    #[serde(skip_serializing_if = "Option::is_none")]
    explosion_size: Option<f64>,
}

#[derive(Debug, Deserialize, Serialize)]
struct RawRcsSpec {
    #[serde(rename = "@power")]
    power: f64,
    #[serde(rename = "@consumption")]
    consumption: f64,
    #[serde(rename = "@size")]
    size: f64,
}

#[derive(Debug, Deserialize, Serialize)]
struct RawSolarSpec {
    #[serde(rename = "@chargeRate")]
    charge_rate: f64,
}

#[derive(Debug, Deserialize, Serialize)]
struct RawLanderSpec {
    #[serde(rename = "@maxAngle")]
    max_angle: f64,
    #[serde(rename = "@minLength")]
    min_length: f64,
    #[serde(rename = "@maxLength")]
    max_length: f64,
    #[serde(rename = "@angleSpeed")]
    #[serde(skip_serializing_if = "Option::is_none")]
    angle_speed: Option<f64>,
    #[serde(rename = "@lengthSpeed")]
    #[serde(skip_serializing_if = "Option::is_none")]
    length_speed: Option<f64>,
    #[serde(rename = "@width")]
    width: f64,
}

/// 将 PartList.xml 的 location 名称换算成部件中心坐标。
fn attach_location(location: &str, width: u32, height: u32) -> (f64, f64) {
    let half_width = width as f64 / 2.0;
    let half_height = height as f64 / 2.0;
    match location {
        "Top" | "TopSide" | "TopCenter" => (0.0, half_height),
        "Bottom" | "BottomSide" | "BottomCenter" => (0.0, -half_height),
        "Left" | "LeftSide" | "LeftCenter" => (-half_width, 0.0),
        "Right" | "RightSide" | "RightCenter" => (half_width, 0.0),
        _ => (0.0, 0.0),
    }
}

pub fn load_catalog(path: impl AsRef<Path>) -> Result<PartCatalog, CoreError> {
    let path_ref = path.as_ref();
    if !path_ref.exists() {
        return Err(CoreError::Missing(path_ref.display().to_string()));
    }
    let source = fs::read_to_string(path_ref).map_err(|source| CoreError::Read {
        path: path_ref.display().to_string(),
        source,
    })?;
    let mut catalog = catalog_from_xml(&source)?;
    if catalog.name.is_empty() {
        catalog.name = path_ref
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("catalog")
            .into();
    }
    Ok(catalog)
}

/// 从内存读取部件目录，保留静态规格；不依赖编辑器或文件系统。
pub fn catalog_from_xml(source: &str) -> Result<PartCatalog, CoreError> {
    let raw: RawPartList = from_str(source)?;
    let parts = raw
        .parts
        .into_iter()
        .map(|p| {
            let width = p.width;
            let height = p.height;
            for attach in p.attach_points.iter().flat_map(|points| &points.points) {
                if !matches!(
                    attach.location.as_str(),
                    "" | "Top"
                        | "Bottom"
                        | "Left"
                        | "Right"
                        | "TopCenter"
                        | "BottomCenter"
                        | "LeftCenter"
                        | "RightCenter"
                        | "TopSide"
                        | "BottomSide"
                        | "LeftSide"
                        | "RightSide"
                ) || !attach.x.is_none_or(f64::is_finite)
                    || !attach.y.is_none_or(f64::is_finite)
                {
                    return Err(CoreError::InvalidDocument(format!(
                        "部件 {} 的连接点位置无效",
                        p.id
                    )));
                }
            }
            let shapes = p
                .shapes
                .into_iter()
                .map(|shape| {
                    let mut vertices: Vec<_> =
                        shape.vertices.into_iter().map(|v| (v.x, v.y)).collect();
                    if vertices.len() > 3 && vertices.first() == vertices.last() {
                        vertices.pop();
                    }
                    if !crate::geometry::valid_polygon(&vertices) {
                        return Err(CoreError::InvalidDocument(format!(
                            "部件 {} 的 Shape 不是有效凸多边形",
                            p.id
                        )));
                    }
                    Ok(PolygonShape {
                        vertices,
                        sensor: shape.sensor,
                    })
                })
                .collect::<Result<Vec<_>, CoreError>>()?;
            Ok(PartType {
                id: p.id,
                name: p.name,
                description: p.description,
                sprite: p.sprite,
                kind: PartKind::from_xml(&p.kind),
                mass: p.mass,
                width,
                height,
                category: p.category,
                hidden: p.hidden,
                ignore_editor_intersections: p.ignore_editor_intersections,
                disable_editor_rotation: p.disable_editor_rotation,
                max_occurrences: p.max_occurrences,
                friction: p.friction,
                can_explode: p.can_explode,
                cover_height: p.cover_height,
                sandbox_only: p.sandbox_only,
                drag: p.drag,
                buoyancy: p.buoyancy,
                damage: p.damage.map(|value| DamageSpec {
                    disconnect: value.disconnect,
                    explode: value.explode,
                    explosion_power: value.explosion_power,
                    explosion_size: value.explosion_size,
                }),
                rcs: p.rcs.map(|value| RcsSpec {
                    power: value.power,
                    consumption: value.consumption,
                    size: value.size,
                }),
                solar: p.solar.map(|value| SolarSpec {
                    charge_rate: value.charge_rate,
                }),
                lander: p.lander.map(|value| LanderSpec {
                    max_angle: value.max_angle,
                    min_length: value.min_length,
                    max_length: value.max_length,
                    angle_speed: value.angle_speed,
                    length_speed: value.length_speed,
                    width: value.width,
                }),
                tank: p.tank.map(|tank| TankSpec {
                    fuel: tank.fuel,
                    dry_mass: tank.dry_mass,
                    fuel_type: tank.fuel_type,
                }),
                engine: p.engine.map(|engine| EngineSpec {
                    power: engine.power,
                    consumption: engine.consumption,
                    size: engine.size,
                    turn: engine.turn,
                    fuel_type: engine.fuel_type,
                    throttle_exponential: engine.throttle_exponential,
                }),
                attach_points: p
                    .attach_points
                    .map(|a| {
                        a.points
                            .into_iter()
                            .map(|p| AttachPoint {
                                x: if p.location.is_empty() {
                                    p.x.unwrap_or(0.0)
                                } else {
                                    attach_location(&p.location, width, height).0
                                },
                                y: if p.location.is_empty() {
                                    p.y.unwrap_or(0.0)
                                } else {
                                    attach_location(&p.location, width, height).1
                                },
                                dock: p.dock,
                                location: p.location,
                                fuel_line: p.fuel_line,
                                flip_x: p.flip_x,
                                flip_y: p.flip_y,
                                group: p.group,
                                order: p.order,
                                break_angle: p.break_angle,
                                break_force: p.break_force,
                            })
                            .collect()
                    })
                    .unwrap_or_default(),
                shapes,
            })
        })
        .collect::<Result<Vec<_>, CoreError>>()?;
    Ok(PartCatalog::new(raw.name, parts))
}

/// 输出 PartTypes XML。包含目录名称扩展属性；未声明的静态字段不填入虚构缺省值。
pub fn catalog_to_xml(catalog: &PartCatalog) -> Result<String, CoreError> {
    let raw = RawPartList {
        name: catalog.name.clone(),
        parts: catalog
            .types
            .iter()
            .map(|p| RawPartType {
                id: p.id.clone(),
                name: p.name.clone(),
                description: p.description.clone(),
                sprite: p.sprite.clone(),
                category: p.category.clone(),
                kind: p.kind.as_xml().into(),
                mass: p.mass,
                width: p.width,
                height: p.height,
                hidden: p.hidden,
                ignore_editor_intersections: p.ignore_editor_intersections,
                disable_editor_rotation: p.disable_editor_rotation,
                max_occurrences: p.max_occurrences,
                friction: p.friction,
                can_explode: p.can_explode,
                cover_height: p.cover_height,
                sandbox_only: p.sandbox_only,
                drag: p.drag,
                buoyancy: p.buoyancy,
                tank: p.tank.as_ref().map(|v| RawTankSpec {
                    fuel: v.fuel,
                    dry_mass: v.dry_mass,
                    fuel_type: v.fuel_type,
                }),
                engine: p.engine.as_ref().map(|v| RawEngineSpec {
                    power: v.power,
                    consumption: v.consumption,
                    size: v.size,
                    turn: v.turn,
                    fuel_type: v.fuel_type,
                    throttle_exponential: v.throttle_exponential,
                }),
                damage: p.damage.as_ref().map(|v| RawDamageSpec {
                    disconnect: v.disconnect,
                    explode: v.explode,
                    explosion_power: v.explosion_power,
                    explosion_size: v.explosion_size,
                }),
                rcs: p.rcs.as_ref().map(|v| RawRcsSpec {
                    power: v.power,
                    consumption: v.consumption,
                    size: v.size,
                }),
                solar: p.solar.as_ref().map(|v| RawSolarSpec {
                    charge_rate: v.charge_rate,
                }),
                lander: p.lander.as_ref().map(|v| RawLanderSpec {
                    max_angle: v.max_angle,
                    min_length: v.min_length,
                    max_length: v.max_length,
                    angle_speed: v.angle_speed,
                    length_speed: v.length_speed,
                    width: v.width,
                }),
                shapes: p
                    .shapes
                    .iter()
                    .map(|shape| RawShape {
                        sensor: shape.sensor,
                        vertices: shape
                            .vertices
                            .iter()
                            .map(|&(x, y)| RawVertex { x, y })
                            .collect(),
                    })
                    .collect(),
                attach_points: (!p.attach_points.is_empty()).then(|| RawAttachPoints {
                    points: p
                        .attach_points
                        .iter()
                        .map(|a| RawAttachPoint {
                            location: a.location.clone(),
                            x: a.location.is_empty().then_some(a.x),
                            y: a.location.is_empty().then_some(a.y),
                            dock: a.dock,
                            fuel_line: a.fuel_line,
                            flip_x: a.flip_x,
                            flip_y: a.flip_y,
                            group: a.group,
                            order: a.order,
                            break_angle: a.break_angle,
                            break_force: a.break_force,
                        })
                        .collect(),
                }),
            })
            .collect(),
    };
    let xml = quick_xml::se::to_string(&raw)?;
    // 校验我们自己的规范化规则：不能把已编辑的坐标/轮廓静默改成另一种模型。
    if catalog_from_xml(&xml)?.types != catalog.types {
        return Err(CoreError::InvalidDocument(
            "目录包含不可无损输出的类型、连接点或轮廓".into(),
        ));
    }
    Ok(xml)
}

pub fn save_catalog(path: impl AsRef<Path>, catalog: &PartCatalog) -> Result<(), CoreError> {
    let xml = catalog_to_xml(catalog)?;
    let path = path.as_ref();
    super::atomic_write(path, xml.as_bytes()).map_err(|source| CoreError::Write {
        path: path.display().to_string(),
        source,
    })
}

#[cfg(test)]
mod tests;
