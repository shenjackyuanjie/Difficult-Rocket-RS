use crate::model::*;
use quick_xml::{de::from_str, se::to_string};
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::{fs, path::Path};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("船体 XML 格式错误: {0}")]
    InvalidDocument(String),
    #[error("无法保存 {path}: {source}")]
    Write {
        path: String,
        source: std::io::Error,
    },
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

mod catalog;
pub use catalog::{catalog_from_xml, catalog_to_xml, load_catalog, save_catalog};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawShip {
    #[serde(rename = "@name", default)]
    name: String,
    #[serde(rename = "@description", default)]
    description: String,
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
#[serde(deny_unknown_fields)]
struct RawParts {
    #[serde(rename = "Part", default)]
    parts: Vec<RawPart>,
}
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConnections {
    #[serde(rename = "$value", default)]
    connections: Vec<RawConnection>,
}
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawDisconnected {
    #[serde(rename = "DisconnectedPart", default)]
    groups: Vec<RawGroup>,
}
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawGroup {
    #[serde(rename = "Parts", default)]
    parts: RawParts,
    #[serde(rename = "Connections", default)]
    connections: RawConnections,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
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
    #[serde(rename = "@chuteX")]
    chute_x: Option<f64>,
    #[serde(rename = "@chuteY")]
    chute_y: Option<f64>,
    #[serde(rename = "@chuteAngle")]
    chute_angle: Option<f64>,
    #[serde(rename = "@chuteHeight")]
    chute_height: Option<f64>,
    #[serde(rename = "@inflation")]
    inflation: Option<f64>,
    #[serde(rename = "@inflate")]
    inflate: Option<i8>,
    #[serde(rename = "@deployed")]
    deployed: Option<i8>,
    #[serde(rename = "@rope")]
    rope: Option<i8>,
    #[serde(rename = "@lower")]
    lower: Option<i8>,
    #[serde(rename = "@raise")]
    raise: Option<i8>,
    #[serde(rename = "@length")]
    length: Option<f64>,
    #[serde(rename = "@legAngle")]
    leg_angle: Option<f64>,
    #[serde(rename = "Tank")]
    tank: Option<RawFuel>,
    #[serde(rename = "Engine")]
    engine: Option<RawFuel>,
    #[serde(rename = "Pod")]
    pod: Option<RawPod>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFuel {
    #[serde(rename = "@fuel", default)]
    fuel: f64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPod {
    #[serde(rename = "@throttle", default)]
    throttle: f64,
    #[serde(rename = "@name", default)]
    name: String,
    #[serde(rename = "Staging")]
    staging: Option<RawStaging>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawStaging {
    #[serde(rename = "@currentStage", default)]
    current_stage: i32,
    #[serde(rename = "Step", default)]
    steps: Vec<RawStageStep>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawStageStep {
    #[serde(rename = "Activate", default)]
    activations: Vec<RawActivation>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawActivation {
    #[serde(rename = "@Id")]
    id: i64,
    #[serde(rename = "@moved", default)]
    moved: i8,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
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
    let (fuel, fuel_kind) = match (raw.tank, raw.engine) {
        (Some(fuel), _) => (Some(fuel.fuel), Some(FuelKind::Tank)),
        (None, Some(fuel)) => (Some(fuel.fuel), Some(FuelKind::Engine)),
        (None, None) => (None, None),
    };
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
        fuel,
        fuel_kind,
        extension: raw.extension,
        lander: LanderState {
            lower: raw.lower,
            raise: raw.raise,
            length: raw.length,
            leg_angle: raw.leg_angle,
        },
        parachute: ParachuteState {
            x: raw.chute_x,
            y: raw.chute_y,
            angle: raw.chute_angle,
            height: raw.chute_height,
            inflation: raw.inflation,
            inflate: raw.inflate,
            deployed: raw.deployed,
            rope: raw.rope,
        },
        pod: raw.pod.map(|pod| PodState {
            throttle: pod.throttle,
            name: pod.name,
            staging: pod.staging.map(|staging| StagingState {
                current_stage: staging.current_stage,
                steps: staging
                    .steps
                    .into_iter()
                    .map(|step| StageStep {
                        activations: step
                            .activations
                            .into_iter()
                            .map(|activation| Activation {
                                id: activation.id,
                                moved: activation.moved != 0,
                            })
                            .collect(),
                    })
                    .collect(),
            }),
        }),
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
    ship_from_xml(&source)
}

/// 按 dr_rs 的 SR1 字段与默认值读取船体。
pub fn ship_from_xml(source: &str) -> Result<Ship, CoreError> {
    validate_ship_root(source)?;
    let raw: RawShip = from_str(source)?;
    for part in raw.parts.parts.iter().chain(
        raw.disconnected
            .groups
            .iter()
            .flat_map(|group| &group.parts.parts),
    ) {
        if part.tank.is_some() && part.engine.is_some() {
            return Err(CoreError::InvalidDocument(format!(
                "部件 {} 同时包含 Tank 和 Engine，无法无损解释燃料状态",
                part.id
            )));
        }
    }
    Ok(Ship {
        name: raw.name,
        description: raw.description,
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

// quick-xml 的 Serde 入口会忽略根元素名称及部分尾随内容，需要单独核对。
fn validate_ship_root(source: &str) -> Result<(), CoreError> {
    use quick_xml::{Reader, events::Event};
    let mut reader = Reader::from_str(source);
    let mut depth = 0_usize;
    let mut seen_root = false;
    loop {
        let event = reader
            .read_event()
            .map_err(|error| CoreError::InvalidDocument(error.to_string()))?;
        let empty = matches!(event, Event::Empty(_));
        match event {
            Event::Start(ref element) | Event::Empty(ref element) => {
                if depth == 0 {
                    if seen_root || element.name().as_ref() != b"Ship" {
                        return Err(CoreError::InvalidDocument(
                            "必须且只能有一个 Ship 根元素".into(),
                        ));
                    }
                    seen_root = true;
                }
                if !empty {
                    depth += 1;
                }
            }
            Event::End(_) => {
                depth = depth
                    .checked_sub(1)
                    .ok_or_else(|| CoreError::InvalidDocument("多余的结束标签".into()))?;
            }
            Event::Text(text)
                if depth == 0 && !text.as_ref().iter().all(u8::is_ascii_whitespace) =>
            {
                return Err(CoreError::InvalidDocument("根元素外存在文本".into()));
            }
            Event::CData(_) | Event::GeneralRef(_) if depth == 0 => {
                return Err(CoreError::InvalidDocument("根元素外存在内容".into()));
            }
            Event::Eof => break,
            _ => {}
        }
    }
    if !seen_root || depth != 0 {
        return Err(CoreError::InvalidDocument(
            "缺失或未闭合的 Ship 根元素".into(),
        ));
    }
    Ok(())
}

#[derive(Debug, Serialize)]
#[serde(rename = "Ship")]
struct OutShip<'a> {
    #[serde(rename = "@name", skip_serializing_if = "str::is_empty")]
    name: &'a str,
    #[serde(rename = "@description", skip_serializing_if = "str::is_empty")]
    description: &'a str,
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
    disconnected: OutDisconnected<'a>,
}
#[derive(Debug, Serialize)]
struct OutDisconnected<'a> {
    #[serde(rename = "DisconnectedPart")]
    groups: Vec<OutGroup<'a>>,
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
    #[serde(rename = "@chuteX", skip_serializing_if = "Option::is_none")]
    chute_x: Option<f64>,
    #[serde(rename = "@chuteY", skip_serializing_if = "Option::is_none")]
    chute_y: Option<f64>,
    #[serde(rename = "@chuteAngle", skip_serializing_if = "Option::is_none")]
    chute_angle: Option<f64>,
    #[serde(rename = "@chuteHeight", skip_serializing_if = "Option::is_none")]
    chute_height: Option<f64>,
    #[serde(rename = "@inflation", skip_serializing_if = "Option::is_none")]
    inflation: Option<f64>,
    #[serde(rename = "@inflate", skip_serializing_if = "Option::is_none")]
    inflate: Option<i8>,
    #[serde(rename = "@deployed", skip_serializing_if = "Option::is_none")]
    deployed: Option<i8>,
    #[serde(rename = "@rope", skip_serializing_if = "Option::is_none")]
    rope: Option<i8>,
    #[serde(rename = "@lower", skip_serializing_if = "Option::is_none")]
    lower: Option<i8>,
    #[serde(rename = "@raise", skip_serializing_if = "Option::is_none")]
    raise: Option<i8>,
    #[serde(rename = "@length", skip_serializing_if = "Option::is_none")]
    length: Option<f64>,
    #[serde(rename = "@legAngle", skip_serializing_if = "Option::is_none")]
    leg_angle: Option<f64>,
    #[serde(rename = "Tank", skip_serializing_if = "Option::is_none")]
    tank: Option<OutFuel>,
    #[serde(rename = "Engine", skip_serializing_if = "Option::is_none")]
    engine: Option<OutFuel>,
    #[serde(rename = "Pod", skip_serializing_if = "Option::is_none")]
    pod: Option<OutPod>,
}
#[derive(Debug, Serialize)]
struct OutFuel {
    #[serde(rename = "@fuel")]
    fuel: f64,
}

#[derive(Debug, Serialize)]
struct OutPod {
    #[serde(rename = "@throttle")]
    throttle: f64,
    #[serde(rename = "@name")]
    name: String,
    #[serde(rename = "Staging", skip_serializing_if = "Option::is_none")]
    staging: Option<OutStaging>,
}

#[derive(Debug, Serialize)]
struct OutStaging {
    #[serde(rename = "@currentStage")]
    current_stage: i32,
    #[serde(rename = "Step", default)]
    steps: Vec<OutStageStep>,
}

#[derive(Debug, Serialize)]
struct OutStageStep {
    #[serde(rename = "Activate", default)]
    activations: Vec<OutActivation>,
}

#[derive(Debug, Serialize)]
struct OutActivation {
    #[serde(rename = "@Id")]
    id: i64,
    #[serde(rename = "@moved")]
    moved: i8,
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
    let tank = (part.fuel_kind == Some(FuelKind::Tank))
        .then(|| part.fuel.map(|fuel| OutFuel { fuel }))
        .flatten();
    let engine = (part.fuel_kind == Some(FuelKind::Engine))
        .then(|| part.fuel.map(|fuel| OutFuel { fuel }))
        .flatten();
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
        lower: part.lander.lower,
        raise: part.lander.raise,
        length: part.lander.length,
        leg_angle: part.lander.leg_angle,

        chute_x: part.parachute.x,
        chute_y: part.parachute.y,
        chute_angle: part.parachute.angle,
        chute_height: part.parachute.height,
        inflation: part.parachute.inflation,
        inflate: part.parachute.inflate,
        deployed: part.parachute.deployed,
        rope: part.parachute.rope,

        tank,
        engine,
        pod: part.pod.as_ref().map(|pod| OutPod {
            throttle: pod.throttle,
            name: pod.name.clone(),
            staging: pod.staging.as_ref().map(|staging| OutStaging {
                current_stage: staging.current_stage,
                steps: staging
                    .steps
                    .iter()
                    .map(|step| OutStageStep {
                        activations: step
                            .activations
                            .iter()
                            .map(|activation| OutActivation {
                                id: activation.id,
                                moved: activation.moved as i8,
                            })
                            .collect(),
                    })
                    .collect(),
            }),
        }),
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

pub fn ship_to_xml(ship: &Ship) -> Result<String, CoreError> {
    let data = OutShip {
        name: &ship.name,
        description: &ship.description,
        version: ship.version,
        lifted_off: ship.lifted_off as i8,
        touching_ground: ship.touching_ground as i8,
        parts: OutParts {
            parts: ship.parts.iter().map(out_part).collect(),
        },
        connections: OutConnections {
            connections: ship.connections.iter().map(out_connection).collect(),
        },
        disconnected: OutDisconnected {
            groups: ship
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
        },
    };
    Ok(to_string(&data)?)
}

pub fn save_ship(path: impl AsRef<Path>, ship: &Ship) -> Result<(), CoreError> {
    let xml = ship_to_xml(ship)?;
    let path = path.as_ref();
    atomic_write(path, xml.as_bytes()).map_err(|source| CoreError::Write {
        path: path.display().to_string(),
        source,
    })?;
    Ok(())
}

/// 先在目标目录写完整临时文件，再替换目标；写入或替换失败不截断原文件。
fn atomic_write(path: &Path, contents: &[u8]) -> std::io::Result<()> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    if let Ok(metadata) = fs::metadata(path) {
        if metadata.permissions().readonly() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "目标文件为只读",
            ));
        }
        temporary
            .as_file()
            .set_permissions(metadata.permissions())?;
    }
    temporary.write_all(contents)?;
    temporary.as_file().sync_all()?;
    temporary.persist(path).map_err(|error| error.error)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_wrong_root_trailing_document_and_unmodelled_fields() {
        for input in [
            "<PartTypes/>",
            "<Ship/><Ship/>",
            "<Ship/>garbage",
            "<Ship>",
            "<Ship futureField=\"1\"/>",
            "<Ship><Unknown/></Ship>",
            "<Ship><Parts><Part id=\"1\" futureField=\"1\"/></Parts></Ship>",
            "<Ship><Parts><Part id=\"1\"><Tank fuel=\"2\"/><Engine fuel=\"3\"/></Part></Parts></Ship>",
        ] {
            assert!(ship_from_xml(input).is_err(), "错误输入被静默接受：{input}");
        }
        assert!(ship_from_xml("<?xml version=\"1.0\"?><!-- 注释 --><Ship/><!-- 结尾 -->").is_ok());
    }

    #[test]
    fn preserves_all_lander_attributes() {
        let ship = ship_from_xml(r#"<Ship><Parts><Part id="1" partType="lander-1" lower="0" raise="1" length="2.26" legAngle="-10000000"/></Parts></Ship>"#).unwrap();
        assert_eq!(
            ship.parts[0].lander,
            LanderState {
                lower: Some(0),
                raise: Some(1),
                length: Some(2.26),
                leg_angle: Some(-10000000.0)
            }
        );
        let xml = ship_to_xml(&ship).unwrap();
        assert!(xml.contains("legAngle=\"-10000000\""));
        assert_eq!(ship_from_xml(&xml).unwrap(), ship);
    }

    #[test]
    fn atomic_save_replaces_complete_file_and_leaves_no_temporary_files() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("船体.xml");
        fs::write(&path, "旧内容").unwrap();
        let ship = Ship::default();
        save_ship(&path, &ship).unwrap();
        assert_eq!(load_ship(&path).unwrap(), ship);
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[test]
    fn failed_replacement_preserves_existing_target_and_cleans_up() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("不能替换的目录.xml");
        fs::create_dir(&path).unwrap();
        fs::write(path.join("原内容"), b"unchanged").unwrap();
        assert!(matches!(
            save_ship(&path, &Ship::default()),
            Err(CoreError::Write { .. })
        ));
        assert_eq!(fs::read(path.join("原内容")).unwrap(), b"unchanged");
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[test]
    fn read_only_save_preserves_original_bytes() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("只读.xml");
        fs::write(&path, "不能丢失的原文件").unwrap();
        let original = fs::metadata(&path).unwrap().permissions();
        let mut read_only = original.clone();
        read_only.set_readonly(true);
        fs::set_permissions(&path, read_only).unwrap();
        let result = save_ship(&path, &Ship::default());
        fs::set_permissions(&path, original).unwrap();
        assert!(matches!(result, Err(CoreError::Write { .. })));
        assert_eq!(fs::read_to_string(&path).unwrap(), "不能丢失的原文件");
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }
    #[test]
    fn preserves_disconnected_groups_and_parachutes() {
        let source = r#"<Ship><Parts><Part id="1" partType="pod-1"><Pod name="甲 &amp; 乙" throttle="0.5"><Staging currentStage="1"><Step/><Step><Activate Id="2" moved="1"/></Step></Staging></Pod></Part></Parts>
        <Connections/><DisconnectedParts>
        <DisconnectedPart><Parts><Part id="2" partType="parachute-1" chuteX="1" chuteY="2" chuteAngle="0.25" chuteHeight="5" inflation="0.1" inflate="1" rope="0" deployed="1" extension="0.5"/><Part id="3" partType="engine-1"><Engine fuel="2"/></Part></Parts><Connections><DockConnection dockPart="3" parentPart="2" childPart="3"/></Connections></DisconnectedPart>
        <DisconnectedPart><Parts><Part id="4" partType="tank"><Tank fuel="5"/></Part></Parts><Connections/></DisconnectedPart>
        </DisconnectedParts></Ship>"#;
        let ship = ship_from_xml(source).unwrap();
        assert_eq!(ship.version, 1);
        assert!(ship.touching_ground);
        assert_eq!(ship.disconnected.len(), 2);
        let chute = &ship.part(2).unwrap().parachute;
        assert_eq!(chute.x, Some(1.0));
        assert_eq!(chute.inflation, Some(0.1));
        assert_eq!(chute.rope, Some(0));
        assert_eq!(chute.deployed, Some(1));
        let saved = ship_to_xml(&ship).unwrap();
        assert!(saved.starts_with("<Ship "));
        assert!(saved.contains("<DisconnectedParts><DisconnectedPart>"));
        assert_eq!(ship, ship_from_xml(&saved).unwrap());
        assert_eq!(Ship::default(), ship_from_xml("<Ship/>").unwrap());
    }

    #[test]
    fn parses_minimal_ship() {
        let path = std::env::temp_dir().join("dr-core-test.xml");
        fs::write(&path, r#"<Ship version="1" liftedOff="0" touchingGround="1"><Parts><Part partType="pod-1" id="1" x="0" y="0" angle="0" angleV="0" editorAngle="0"/></Parts><Connections/></Ship>"#).unwrap();
        let ship = load_ship(&path).unwrap();
        assert_eq!(ship.parts[0].part_type, "pod-1");
        let _ = fs::remove_file(path);
    }

    #[test]
    fn parses_catalog_locations_and_specs() {
        let path = std::env::temp_dir().join("dr-core-part-list.xml");
        fs::write(
            &path,
            r#"<PartTypes>
                <PartType id="tank" type="tank" width="4" height="8" mass="2">
                    <Tank fuel="3000" dryMass="0.8" fuelType="1" />
                    <AttachPoints>
                        <AttachPoint location="TopCenter" fuelLine="true" group="2" />
                        <AttachPoint x="1" y="2" breakForce="5" />
                    </AttachPoints>
                </PartType>
            </PartTypes>"#,
        )
        .unwrap();
        let catalog = load_catalog(&path).unwrap();
        let part = catalog.get("tank").unwrap();
        assert_eq!(part.attach_points[0].y, 4.0);
        assert!(part.attach_points[0].fuel_line);
        assert_eq!(part.attach_points[0].group, Some(2));
        assert_eq!(part.tank.as_ref().unwrap().fuel, 3000.0);
        assert_eq!(part.attach_points[1].x, 1.0);
        assert_eq!(part.attach_points[1].break_force, Some(5.0));
        let _ = fs::remove_file(path);
    }

    #[test]
    fn catalog_preserves_compound_shapes_and_sensor_attribute() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("shapes.xml");
        fs::write(
            &path,
            r#"<PartTypes><PartType id="compound" width="4" height="4">
            <Shape><Vertex x="-2" y="-2"/><Vertex x="2" y="-2"/><Vertex y="2"/></Shape>
            <Shape sensor="true"><Vertex/><Vertex x="1"/><Vertex y="1"/></Shape>
        </PartType></PartTypes>"#,
        )
        .unwrap();
        let catalog = load_catalog(&path).unwrap();
        let kind = catalog.get("compound").unwrap();
        assert_eq!(kind.shapes.len(), 2);
        assert_eq!(
            kind.shapes[0].vertices,
            vec![(-2.0, -2.0), (2.0, -2.0), (0.0, 2.0)]
        );
        assert!(!kind.shapes[0].sensor);
        assert!(kind.shapes[1].sensor);
        fs::write(&path, r#"<PartTypes><PartType id="broken"><Shape><Vertex/><Vertex x="1"/></Shape></PartType></PartTypes>"#).unwrap();
        assert!(
            load_catalog(&path)
                .unwrap_err()
                .to_string()
                .contains("Shape")
        );
    }

    #[test]
    fn preserves_pod_and_engine_kind_on_roundtrip() {
        let input = std::env::temp_dir().join("dr-core-pod.xml");
        let output = std::env::temp_dir().join("dr-core-pod-out.xml");
        fs::write(
            &input,
            r#"<Ship version="1" liftedOff="0" touchingGround="1"><Parts>
                <Part partType="pod-1" id="1" x="0" y="0" angle="0" angleV="0" editorAngle="0">
                    <Pod throttle="0.5" name="test"><Staging currentStage="2"><Step><Activate Id="7" moved="1"/></Step></Staging></Pod>
                </Part>
                <Part partType="engine-1" id="2"><Engine fuel="4"/></Part>
            </Parts><Connections/></Ship>"#,
        )
        .unwrap();
        let ship = load_ship(&input).unwrap();
        assert_eq!(
            ship.parts[0]
                .pod
                .as_ref()
                .unwrap()
                .staging
                .as_ref()
                .unwrap()
                .current_stage,
            2
        );
        assert_eq!(ship.parts[1].fuel_kind, Some(FuelKind::Engine));
        save_ship(&output, &ship).unwrap();
        let saved = fs::read_to_string(&output).unwrap();
        assert!(saved.contains("<Engine fuel=\"4\""));
        assert!(saved.contains("<Pod throttle=\"0.5\""));
        let _ = fs::remove_file(input);
        let _ = fs::remove_file(output);
    }
}
