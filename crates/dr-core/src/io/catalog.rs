use super::CoreError;
use crate::model::*;
use quick_xml::de::from_str;
use serde::Deserialize;
use std::{fs, path::Path};

#[derive(Debug, Deserialize)]
struct RawPartList {
    #[serde(rename = "PartType", default)]
    parts: Vec<RawPartType>,
}

#[derive(Debug, Deserialize)]
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
    max_occurrences: Option<u32>,
    #[serde(rename = "@friction")]
    friction: Option<f64>,
    #[serde(rename = "@canExplode")]
    can_explode: Option<bool>,
    #[serde(rename = "@coverHeight")]
    cover_height: Option<u32>,
    #[serde(rename = "@sandboxOnly")]
    sandbox_only: Option<bool>,
    #[serde(rename = "@drag")]
    drag: Option<f64>,
    #[serde(rename = "@buoyancy")]
    buoyancy: Option<f64>,
    #[serde(rename = "Damage")]
    damage: Option<RawDamageSpec>,
    #[serde(rename = "Rcs")]
    rcs: Option<RawRcsSpec>,
    #[serde(rename = "Solar")]
    solar: Option<RawSolarSpec>,
    #[serde(rename = "Lander")]
    lander: Option<RawLanderSpec>,
    #[serde(rename = "Tank")]
    tank: Option<RawTankSpec>,
    #[serde(rename = "Engine")]
    engine: Option<RawEngineSpec>,
    #[serde(rename = "AttachPoints")]
    attach_points: Option<RawAttachPoints>,
    #[serde(rename = "Shape", default)]
    shapes: Vec<RawShape>,
}

#[derive(Debug, Deserialize)]
struct RawShape {
    #[serde(rename = "Vertex", default)]
    vertices: Vec<RawVertex>,
    #[serde(rename = "@sensor", default)]
    sensor: bool,
}

#[derive(Debug, Deserialize)]
struct RawVertex {
    #[serde(rename = "@x", default)]
    x: f64,
    #[serde(rename = "@y", default)]
    y: f64,
}

#[derive(Debug, Deserialize)]
struct RawAttachPoints {
    #[serde(rename = "AttachPoint", default)]
    points: Vec<RawAttachPoint>,
}

#[derive(Debug, Deserialize)]
struct RawAttachPoint {
    #[serde(rename = "@x", default)]
    x: f64,
    #[serde(rename = "@y", default)]
    y: f64,
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
    group: Option<i32>,
    #[serde(rename = "@order")]
    order: Option<i32>,
    #[serde(rename = "@breakAngle")]
    break_angle: Option<f64>,
    #[serde(rename = "@breakForce")]
    break_force: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct RawTankSpec {
    #[serde(rename = "@fuel", default)]
    fuel: f64,
    #[serde(rename = "@dryMass")]
    dry_mass: Option<f64>,
    #[serde(rename = "@fuelType")]
    fuel_type: Option<i32>,
}

#[derive(Debug, Deserialize)]
struct RawEngineSpec {
    #[serde(rename = "@power")]
    power: Option<f64>,
    #[serde(rename = "@consumption")]
    consumption: Option<f64>,
    #[serde(rename = "@size")]
    size: Option<f64>,
    #[serde(rename = "@turn")]
    turn: Option<f64>,
    #[serde(rename = "@fuelType")]
    fuel_type: Option<i32>,
    #[serde(rename = "@throttleExponential", default)]
    throttle_exponential: bool,
}

#[derive(Debug, Deserialize)]
struct RawDamageSpec {
    #[serde(rename = "@disconnect")]
    disconnect: f64,
    #[serde(rename = "@explode")]
    explode: f64,
    #[serde(rename = "@explosionPower")]
    explosion_power: Option<f64>,
    #[serde(rename = "@explosionSize")]
    explosion_size: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct RawRcsSpec {
    #[serde(rename = "@power")]
    power: f64,
    #[serde(rename = "@consumption")]
    consumption: f64,
    #[serde(rename = "@size")]
    size: f64,
}

#[derive(Debug, Deserialize)]
struct RawSolarSpec {
    #[serde(rename = "@chargeRate")]
    charge_rate: f64,
}

#[derive(Debug, Deserialize)]
struct RawLanderSpec {
    #[serde(rename = "@maxAngle")]
    max_angle: f64,
    #[serde(rename = "@minLength")]
    min_length: f64,
    #[serde(rename = "@maxLength")]
    max_length: f64,
    #[serde(rename = "@angleSpeed")]
    angle_speed: Option<f64>,
    #[serde(rename = "@lengthSpeed")]
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
    catalog.name = path_ref
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("catalog")
        .into();
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
                ) || !attach.x.is_finite()
                    || !attach.y.is_finite()
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
                                    p.x
                                } else {
                                    attach_location(&p.location, width, height).0
                                },
                                y: if p.location.is_empty() {
                                    p.y
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
    Ok(PartCatalog::new("catalog", parts))
}

#[cfg(test)]
mod tests;
