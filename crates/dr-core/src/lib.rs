//! Difficult Rocket 的可复用数据、XML 和编辑几何核心。

pub mod edit;
pub mod geometry;
pub mod io;
pub mod model;

pub use edit::{CommandError, EditorCommand, EditorHistory, EditorState};
pub use geometry::{SnapCandidate, Vec2d};
pub use io::{CoreError, load_catalog, load_ship, save_ship};
pub use model::{Connection, Part, PartCatalog, PartType, Ship, ShipGroup};
