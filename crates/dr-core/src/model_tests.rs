use super::*;

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
