use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub type PartId = i64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
    Unknown,
}

impl Default for PartKind {
    fn default() -> Self {
        Self::Unknown
    }
}

impl PartKind {
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttachPoint {
    pub x: f64,
    pub y: f64,
    pub dock: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    pub attach_points: Vec<AttachPoint>,
}

impl PartType {
    pub fn half_extents(&self) -> (f64, f64) {
        (self.width as f64 / 2.0, self.height as f64 / 2.0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    pub extension: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    pub fn touches(&self, id: PartId) -> bool {
        match self {
            Self::Normal { parent, child, .. } | Self::Dock { parent, child, .. } => {
                *parent == id || *child == id
            }
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ShipGroup {
    pub parts: Vec<Part>,
    pub connections: Vec<Connection>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Ship {
    pub version: i32,
    pub lifted_off: bool,
    pub touching_ground: bool,
    pub parts: Vec<Part>,
    pub connections: Vec<Connection>,
    pub disconnected: Vec<ShipGroup>,
}

impl Ship {
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
            self.connections.retain(|c| !c.touches(id));
            for g in &mut self.disconnected {
                g.connections.retain(|c| !c.touches(id));
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

#[derive(Debug, Clone, Default)]
pub struct PartCatalog {
    pub name: String,
    pub types: Vec<PartType>,
    index: HashMap<String, usize>,
}

impl PartCatalog {
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
