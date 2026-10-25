use crate::model::*;

fn kind(id: &str, mass: f64) -> PartType {
    PartType {
        id: id.into(),
        name: "x".into(),
        description: String::new(),
        sprite: String::new(),
        kind: PartKind::Strut,
        mass,
        width: 1,
        height: 1,
        category: String::new(),
        hidden: false,
        ignore_editor_intersections: false,
        disable_editor_rotation: false,
        max_occurrences: None,
        friction: None,
        can_explode: None,
        cover_height: None,
        sandbox_only: None,
        drag: None,
        buoyancy: None,
        damage: None,
        rcs: None,
        solar: None,
        lander: None,
        tank: None,
        engine: None,
        attach_points: vec![],
        shapes: vec![],
    }
}
#[test]
fn catalog_lookup_tracks_insert_remove_reorder_and_rename() {
    let mut catalog = PartCatalog::new("test", vec![kind("a", 1.0), kind("b", 2.0)]);
    catalog.types.insert(0, kind("c", 3.0));
    assert_eq!(catalog.get("a").unwrap().mass, 1.0);
    assert_eq!(catalog.get("c").unwrap().mass, 3.0);
    catalog.types.swap(0, 2);
    assert_eq!(catalog.get("b").unwrap().mass, 2.0);
    catalog.types[0].id = "renamed".into();
    assert!(catalog.get("b").is_none());
    assert_eq!(catalog.get("renamed").unwrap().mass, 2.0);
    catalog.types.remove(1);
    assert!(catalog.get("a").is_none());
    assert_eq!(catalog.get("c").unwrap().mass, 3.0);
}

#[test]
fn duplicate_catalog_ids_follow_current_first_occurrence() {
    let mut catalog = PartCatalog::new("test", vec![kind("same", 10.0), kind("same", 20.0)]);
    assert_eq!(catalog.get("same").unwrap().mass, 10.0);
    catalog.types.reverse();
    assert_eq!(catalog.get("same").unwrap().mass, 20.0);
    catalog.types.remove(0);
    assert_eq!(catalog.get("same").unwrap().mass, 10.0);
}

#[test]
fn mass_main_excludes_disconnected_while_all_keeps_existing_contract() {
    let catalog = PartCatalog::new("mass", vec![kind("main", 10.0), kind("detached", 20.0)]);
    let ship = Ship {
        parts: vec![catalog.get("main").unwrap().instantiate(1, (0.0, 0.0))],
        disconnected: vec![ShipGroup {
            parts: vec![catalog.get("detached").unwrap().instantiate(2, (0.0, 0.0))],
            connections: vec![],
        }],
        ..Ship::default()
    };
    assert_eq!(ship.mass(&catalog, ShipScope::Main), 10.0);
    assert_eq!(ship.mass(&catalog, ShipScope::All), 30.0);
    assert_eq!(ship.total_mass(&catalog), 30.0);
}

#[test]
fn next_part_id_avoids_connections_and_staging_of_every_group() {
    let catalog = PartCatalog::new("ids", vec![kind("a", 1.0)]);
    let make = |id| catalog.get("a").unwrap().instantiate(id, (0.0, 0.0));
    let mut ship = Ship {
        parts: vec![make(1), make(2)],
        connections: vec![Connection::Normal {
            parent: 2,
            child: 3,
            parent_attach: 1,
            child_attach: 1,
        }],
        disconnected: vec![ShipGroup {
            parts: vec![make(4)],
            connections: vec![Connection::Dock {
                dock: 5,
                parent: 4,
                child: 1,
            }],
        }],
        ..Ship::default()
    };
    // 部件、普通连接、对接连接和断开组内的引用一起占用编号。
    assert_eq!(ship.next_part_id(), 6);
    let mut pod = make(1);
    pod.pod = Some(PodState {
        staging: Some(StagingState {
            current_stage: 0,
            steps: vec![StageStep {
                activations: vec![Activation {
                    id: 7,
                    moved: false,
                }],
            }],
        }),
        ..Default::default()
    });
    ship.parts.push(pod);
    assert_eq!(ship.next_part_id(), 8);
    // 没有额外引用时仍沿用最大部件号加一。
    ship.connections.clear();
    ship.disconnected.clear();
    ship.parts.truncate(2);
    assert_eq!(ship.next_part_id(), 3);
}

#[test]
fn metadata_is_atomic_persistent_and_included_in_history_budget() {
    use crate::{EditorCommand, EditorHistory, ship_from_xml, ship_to_xml};
    let mut ship = Ship::default();
    let before = ship.clone();
    let bytes = ship.retained_bytes();
    let mut history = EditorHistory::default();
    history
        .execute(
            &mut ship,
            EditorCommand::SetMetadata {
                name: "船体 & <主组>".into(),
                description: "不是驾驶舱的名称".into(),
            },
        )
        .unwrap();
    assert!(ship.retained_bytes() > bytes);
    assert_eq!(ship_from_xml(&ship_to_xml(&ship).unwrap()).unwrap(), ship);
    assert!(history.undo(&mut ship));
    assert_eq!(ship, before);
    assert!(history.redo(&mut ship));
    assert_eq!(ship.name, "船体 & <主组>");
    let old = ship_to_xml(&Ship::default()).unwrap();
    assert!(!old.contains("description="));
    assert_eq!(ship_from_xml("<Ship/>").unwrap(), Ship::default());
}
