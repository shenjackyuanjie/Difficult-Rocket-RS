use crate::model::*;
use quick_xml::{de::from_str, se::to_string};
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("无法读取 {path}: {source}")]
    Read {
        path: String,
        source: std::io::Error,
    },
    #[error("XML 解析失败: {0}")]
    Xml(#[from] quick_xml::DeError),
    #[error("XML 序列化失败: {0}")]
    Serialize(#[from] quick_xml::SeError),
    #[error("路径不存在: {0}")]
    Missing(String),
}

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
    #[serde(rename = "AttachPoints")]
    attach_points: Option<RawAttachPoints>,
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
    let raw: RawPartList = from_str(&source)?;
    let parts = raw
        .parts
        .into_iter()
        .map(|p| PartType {
            id: p.id,
            name: p.name,
            description: p.description,
            sprite: p.sprite,
            kind: PartKind::from_xml(&p.kind),
            mass: p.mass,
            width: p.width,
            height: p.height,
            category: p.category,
            hidden: p.hidden,
            ignore_editor_intersections: p.ignore_editor_intersections,
            disable_editor_rotation: p.disable_editor_rotation,
            max_occurrences: p.max_occurrences,
            attach_points: p
                .attach_points
                .map(|a| {
                    a.points
                        .into_iter()
                        .map(|p| AttachPoint {
                            x: p.x,
                            y: p.y,
                            dock: p.dock,
                        })
                        .collect()
                })
                .unwrap_or_default(),
        })
        .collect();
    Ok(PartCatalog::new(
        path_ref
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("catalog"),
        parts,
    ))
}

#[derive(Debug, Deserialize)]
struct RawShip {
    #[serde(rename = "@version", default = "default_version")]
    version: i32,
    #[serde(rename = "@liftedOff", default)]
    lifted_off: i8,
    #[serde(rename = "@touchingGround", default = "default_touching_ground")]
    touching_ground: i8,
    #[serde(rename = "Parts", default)]
    parts: RawParts,
    #[serde(rename = "Connections", default)]
    connections: RawConnections,
    #[serde(rename = "DisconnectedParts", default)]
    disconnected: RawDisconnected,
}
fn default_version() -> i32 {
    1
}
fn default_touching_ground() -> i8 {
    1
}

#[derive(Debug, Default, Deserialize)]
struct RawParts {
    #[serde(rename = "Part", default)]
    parts: Vec<RawPart>,
}
#[derive(Debug, Default, Deserialize)]
struct RawConnections {
    #[serde(rename = "$value", default)]
    connections: Vec<RawConnection>,
}
#[derive(Debug, Default, Deserialize)]
struct RawDisconnected {
    #[serde(rename = "DisconnectedPart", default)]
    groups: Vec<RawGroup>,
}
#[derive(Debug, Default, Deserialize)]
struct RawGroup {
    #[serde(rename = "Parts", default)]
    parts: RawParts,
    #[serde(rename = "Connections", default)]
    connections: RawConnections,
}

#[derive(Debug, Deserialize)]
struct RawPart {
    #[serde(rename = "@partType", default)]
    part_type: String,
    #[serde(rename = "@id")]
    id: i64,
    #[serde(rename = "@x", default)]
    x: f64,
    #[serde(rename = "@y", default)]
    y: f64,
    #[serde(rename = "@angle", default)]
    angle: f64,
    #[serde(rename = "@editorAngle", default)]
    editor_angle: i32,
    #[serde(rename = "@angleV", default)]
    angle_v: f64,
    #[serde(rename = "@flippedX", default)]
    flip_x: i8,
    #[serde(rename = "@flippedY", default)]
    flip_y: i8,
    #[serde(rename = "@activated", default)]
    active: i8,
    #[serde(rename = "@exploded", default)]
    exploded: i8,
    #[serde(rename = "@extension")]
    extension: Option<f64>,
    #[serde(rename = "Tank")]
    tank: Option<RawFuel>,
    #[serde(rename = "Engine")]
    engine: Option<RawFuel>,
}
#[derive(Debug, Deserialize)]
struct RawFuel {
    #[serde(rename = "@fuel", default)]
    fuel: f64,
}

#[derive(Debug, Deserialize)]
enum RawConnection {
    #[serde(rename = "Connection")]
    Normal {
        #[serde(rename = "@parentAttachPoint")]
        parent_attach: i32,
        #[serde(rename = "@childAttachPoint")]
        child_attach: i32,
        #[serde(rename = "@parentPart")]
        parent: i64,
        #[serde(rename = "@childPart")]
        child: i64,
    },
    #[serde(rename = "DockConnection")]
    Dock {
        #[serde(rename = "@dockPart")]
        dock: i64,
        #[serde(rename = "@parentPart")]
        parent: i64,
        #[serde(rename = "@childPart")]
        child: i64,
    },
}

fn convert_part(raw: RawPart) -> Part {
    Part {
        id: raw.id,
        part_type: raw.part_type,
        x: raw.x,
        y: raw.y,
        angle: raw.angle,
        editor_angle: raw.editor_angle,
        angle_v: raw.angle_v,
        flip_x: raw.flip_x != 0,
        flip_y: raw.flip_y != 0,
        active: raw.active != 0,
        exploded: raw.exploded != 0,
        fuel: raw.tank.or(raw.engine).map(|f| f.fuel),
        extension: raw.extension,
    }
}
fn convert_connection(raw: RawConnection) -> Connection {
    match raw {
        RawConnection::Normal {
            parent_attach,
            child_attach,
            parent,
            child,
        } => Connection::Normal {
            parent_attach,
            child_attach,
            parent,
            child,
        },
        RawConnection::Dock {
            dock,
            parent,
            child,
        } => Connection::Dock {
            dock,
            parent,
            child,
        },
    }
}

pub fn load_ship(path: impl AsRef<Path>) -> Result<Ship, CoreError> {
    let path_ref = path.as_ref();
    if !path_ref.exists() {
        return Err(CoreError::Missing(path_ref.display().to_string()));
    }
    let source = fs::read_to_string(path_ref).map_err(|source| CoreError::Read {
        path: path_ref.display().to_string(),
        source,
    })?;
    let raw: RawShip = from_str(&source)?;
    Ok(Ship {
        version: raw.version,
        lifted_off: raw.lifted_off != 0,
        touching_ground: raw.touching_ground != 0,
        parts: raw.parts.parts.into_iter().map(convert_part).collect(),
        connections: raw
            .connections
            .connections
            .into_iter()
            .map(convert_connection)
            .collect(),
        disconnected: raw
            .disconnected
            .groups
            .into_iter()
            .map(|g| ShipGroup {
                parts: g.parts.parts.into_iter().map(convert_part).collect(),
                connections: g
                    .connections
                    .connections
                    .into_iter()
                    .map(convert_connection)
                    .collect(),
            })
            .collect(),
    })
}

#[derive(Debug, Serialize)]
struct OutShip<'a> {
    #[serde(rename = "@version")]
    version: i32,
    #[serde(rename = "@liftedOff")]
    lifted_off: i8,
    #[serde(rename = "@touchingGround")]
    touching_ground: i8,
    #[serde(rename = "Parts")]
    parts: OutParts<'a>,
    #[serde(rename = "Connections")]
    connections: OutConnections,
    #[serde(rename = "DisconnectedParts")]
    disconnected: Vec<OutGroup<'a>>,
}
#[derive(Debug, Serialize)]
struct OutParts<'a> {
    #[serde(rename = "Part")]
    parts: Vec<OutPart<'a>>,
}
#[derive(Debug, Serialize)]
struct OutConnections {
    #[serde(rename = "$value")]
    connections: Vec<OutConnection>,
}
#[derive(Debug, Serialize)]
struct OutGroup<'a> {
    #[serde(rename = "Parts")]
    parts: OutParts<'a>,
    #[serde(rename = "Connections")]
    connections: OutConnections,
}
#[derive(Debug, Serialize)]
struct OutPart<'a> {
    #[serde(rename = "@partType")]
    part_type: &'a str,
    #[serde(rename = "@id")]
    id: i64,
    #[serde(rename = "@x")]
    x: f64,
    #[serde(rename = "@y")]
    y: f64,
    #[serde(rename = "@angle")]
    angle: f64,
    #[serde(rename = "@editorAngle")]
    editor_angle: i32,
    #[serde(rename = "@angleV")]
    angle_v: f64,
    #[serde(rename = "@flippedX")]
    flip_x: i8,
    #[serde(rename = "@flippedY")]
    flip_y: i8,
    #[serde(rename = "@activated")]
    active: i8,
    #[serde(rename = "@exploded")]
    exploded: i8,
    #[serde(rename = "@extension", skip_serializing_if = "Option::is_none")]
    extension: Option<f64>,
    #[serde(rename = "Tank", skip_serializing_if = "Option::is_none")]
    tank: Option<OutFuel>,
    #[serde(rename = "Engine", skip_serializing_if = "Option::is_none")]
    engine: Option<OutFuel>,
}
#[derive(Debug, Serialize)]
struct OutFuel {
    #[serde(rename = "@fuel")]
    fuel: f64,
}
#[derive(Debug, Serialize)]
enum OutConnection {
    #[serde(rename = "Connection")]
    Normal {
        #[serde(rename = "@parentAttachPoint")]
        parent_attach: i32,
        #[serde(rename = "@childAttachPoint")]
        child_attach: i32,
        #[serde(rename = "@parentPart")]
        parent: i64,
        #[serde(rename = "@childPart")]
        child: i64,
    },
    #[serde(rename = "DockConnection")]
    Dock {
        #[serde(rename = "@dockPart")]
        dock: i64,
        #[serde(rename = "@parentPart")]
        parent: i64,
        #[serde(rename = "@childPart")]
        child: i64,
    },
}

fn out_part(part: &Part) -> OutPart<'_> {
    OutPart {
        part_type: &part.part_type,
        id: part.id,
        x: part.x,
        y: part.y,
        angle: part.angle,
        editor_angle: part.editor_angle,
        angle_v: part.angle_v,
        flip_x: part.flip_x as i8,
        flip_y: part.flip_y as i8,
        active: part.active as i8,
        exploded: part.exploded as i8,
        extension: part.extension,
        tank: part.fuel.map(|fuel| OutFuel { fuel }),
        engine: None,
    }
}
fn out_connection(connection: &Connection) -> OutConnection {
    match connection {
        Connection::Normal {
            parent_attach,
            child_attach,
            parent,
            child,
        } => OutConnection::Normal {
            parent_attach: *parent_attach,
            child_attach: *child_attach,
            parent: *parent,
            child: *child,
        },
        Connection::Dock {
            dock,
            parent,
            child,
        } => OutConnection::Dock {
            dock: *dock,
            parent: *parent,
            child: *child,
        },
    }
}

pub fn save_ship(path: impl AsRef<Path>, ship: &Ship) -> Result<(), CoreError> {
    let data = OutShip {
        version: ship.version,
        lifted_off: ship.lifted_off as i8,
        touching_ground: ship.touching_ground as i8,
        parts: OutParts {
            parts: ship.parts.iter().map(out_part).collect(),
        },
        connections: OutConnections {
            connections: ship.connections.iter().map(out_connection).collect(),
        },
        disconnected: ship
            .disconnected
            .iter()
            .map(|g| OutGroup {
                parts: OutParts {
                    parts: g.parts.iter().map(out_part).collect(),
                },
                connections: OutConnections {
                    connections: g.connections.iter().map(out_connection).collect(),
                },
            })
            .collect(),
    };
    let xml = to_string(&data)?;
    fs::write(path.as_ref(), xml).map_err(|source| CoreError::Read {
        path: path.as_ref().display().to_string(),
        source,
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_minimal_ship() {
        let path = std::env::temp_dir().join("dr-core-test.xml");
        fs::write(&path, r#"<Ship version="1" liftedOff="0" touchingGround="1"><Parts><Part partType="pod-1" id="1" x="0" y="0" angle="0" angleV="0" editorAngle="0"/></Parts><Connections/></Ship>"#).unwrap();
        let ship = load_ship(&path).unwrap();
        assert_eq!(ship.parts[0].part_type, "pod-1");
        let _ = fs::remove_file(path);
    }
}
