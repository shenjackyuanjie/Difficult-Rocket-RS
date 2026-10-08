//! Difficult Rocket 的可复用数据、XML 和编辑几何核心。

pub mod bounds;
pub use bounds::{Bounds, image_bounds, image_corners, part_bounds, ship_bounds};
pub use model::ShipScope;

pub mod topology;
pub use topology::{ConnectionRef, LinkForest, Topology, TopologyEdge, UnresolvedConnection};
pub mod connections;
pub mod edit;
pub mod geometry;
pub mod io;
pub mod model;

pub use edit::{
    CommandError, ConnectionRole, DuplicateRepair, EditorCommand, EditorHistory, EditorState,
    LinkKind, ReferenceSite, SelectionPose, SelectionTransform, ShipFragment,
};
pub use geometry::{SnapCandidate, Vec2d, find_snap, intersects, part_world_attach};
pub use io::{
    CoreError, catalog_from_xml, catalog_to_xml, load_catalog, load_ship, save_catalog, save_ship,
    ship_from_xml, ship_to_xml,
};
pub use model::{
    Activation, AttachPoint, Connection, DamageSpec, EngineSpec, FuelKind, LanderSpec, Part,
    PartCatalog, PartKey, PartKind, PartType, PodState, PolygonShape, RcsSpec, Ship, ShipGroup,
    SolarSpec, StageStep, StagingState, TankSpec,
};
