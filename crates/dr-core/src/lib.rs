//! Difficult Rocket 的可复用数据、XML 和编辑几何核心。

pub mod connections;
pub mod edit;
pub mod geometry;
pub mod io;
pub mod model;

pub use edit::{CommandError, EditorCommand, EditorHistory, EditorState, LinkKind};
pub use geometry::{SnapCandidate, Vec2d, find_snap, intersects, part_world_attach};
pub use io::{CoreError, load_catalog, load_ship, save_ship, ship_from_xml, ship_to_xml};
pub use model::{
    Activation, AttachPoint, Connection, EngineSpec, FuelKind, Part, PartCatalog, PartKey,
    PartKind, PartType, PodState, PolygonShape, Ship, ShipGroup, StageStep, StagingState, TankSpec,
};
