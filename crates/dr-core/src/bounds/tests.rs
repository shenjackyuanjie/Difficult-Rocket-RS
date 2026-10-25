
use super::*;
use crate::{ShipGroup, catalog_from_xml};
#[test]
fn main_and_all_bounds_use_real_rotated_shapes_without_including_origin() {
    let catalog =
        catalog_from_xml(r#"<PartTypes><PartType id="x" width="4" height="2"/></PartTypes>"#)
            .unwrap();
    let kind = catalog.get("x").unwrap();
    let mut part = kind.instantiate(1, (20.0, 30.0));
    part.angle = std::f64::consts::FRAC_PI_2;
    let ship = Ship {
        parts: vec![part],
        disconnected: vec![ShipGroup {
            parts: vec![kind.instantiate(2, (-10.0, 0.0))],
            connections: vec![],
        }],
        ..Ship::default()
    };
    let main = ship_bounds(&ship, &catalog, ShipScope::Main).unwrap();
    assert!((main.min.x - 19.5).abs() < 1e-10);
    assert!((main.min.y - 29.0).abs() < 1e-10);
    assert!(ship_bounds(&ship, &catalog, ShipScope::All).unwrap().min.x < 0.0);
    assert!(ship_bounds(&Ship::default(), &catalog, ShipScope::All).is_none());
}
#[test]
fn image_bounds_keep_odd_pixel_anchor_and_do_not_use_physical_size() {
    let catalog = catalog_from_xml(
        r#"<PartTypes><PartType id="x" width="1" height="1" sprite="x.png"/></PartTypes>"#,
    )
    .unwrap();
    let kind = catalog.get("x").unwrap();
    let mut ship = Ship {
        parts: vec![kind.instantiate(1, (0.0, 0.0))],
        ..Ship::default()
    };
    let normal = image_bounds(&ship, &catalog, ShipScope::Main, |_| Some((129.0, 30.0))).unwrap();
    assert_eq!((normal.min.x, normal.max.x), (-64.0, 65.0));
    ship.parts[0].flip_x = true;
    let flipped = image_bounds(&ship, &catalog, ShipScope::All, |_| Some((129.0, 30.0))).unwrap();
    assert_eq!((flipped.min.x, flipped.max.x), (-65.0, 64.0));
    assert!(part_bounds(&ship.parts[0], kind).unwrap().max.x < 1.0);
}
