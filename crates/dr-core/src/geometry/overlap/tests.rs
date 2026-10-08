use super::*;
use crate::catalog_from_xml;

fn catalog() -> crate::PartCatalog {
    catalog_from_xml(r#"<PartTypes>
      <PartType id="square" width="2" height="2"/>
      <PartType id="large" width="4" height="4"/>
      <PartType id="wheel" type="wheel" width="4" height="4"/>
      <PartType id="compound" width="4" height="2"><Shapes>
        <Shape><Vertex x="-2" y="-1"/><Vertex x="1" y="-1"/><Vertex x="1" y="1"/><Vertex x="-2" y="1"/></Shape>
        <Shape><Vertex x="-1" y="-1"/><Vertex x="2" y="-1"/><Vertex x="2" y="1"/><Vertex x="-1" y="1"/></Shape>
      </Shapes></PartType>
    </PartTypes>"#).unwrap()
}

#[test]
fn five_percent_is_strict_and_uses_the_smaller_solid() {
    let catalog = catalog();
    let kind = catalog.get("square").unwrap();
    let a = kind.instantiate(1, (0.0, 0.0));
    for (x, expected, blocked) in [
        (1.0, 0.0, false),
        (0.951, 0.049, false),
        (0.95, 0.05, true),
        (0.949, 0.051, true),
        (0.0, 1.0, true),
    ] {
        let b = kind.instantiate(2, (x, 0.0));
        assert!((overlap_ratio(&a, kind, &b, kind) - expected).abs() < 1e-8);
        assert_eq!(editor_overlap_blocked(&a, kind, &b, kind), blocked);
    }
    let large = catalog.get("large").unwrap();
    assert_eq!(
        overlap_ratio(&a, kind, &large.instantiate(2, (0.0, 0.0)), large),
        1.0
    );
}

#[test]
fn rotation_mirroring_translation_and_shape_union_preserve_area() {
    let catalog = catalog();
    let square = catalog.get("square").unwrap();
    let compound = catalog.get("compound").unwrap();
    for angle in [0.0, 0.37, 1.19] {
        for mirror in [false, true] {
            let mut a = compound.instantiate(1, (10000.0, -12345.0));
            a.angle = angle;
            a.flip_x = mirror;
            a.flip_y = mirror;
            let mut b =
                square.instantiate(2, (a.x + 1.451 * angle.cos(), a.y + 1.451 * angle.sin()));
            b.angle = angle;
            // 两个 Shape 的交集不能重复计入：整个实体面积为 2，而不是 3。
            assert!((overlap_ratio(&a, compound, &b, square) - 0.049).abs() < 1e-7);
            assert!(!editor_overlap_blocked(&a, compound, &b, square));
        }
    }
}

#[test]
fn circles_use_analytic_lenses_and_polygon_intersections() {
    let catalog = catalog();
    let wheel = catalog.get("wheel").unwrap();
    let large = catalog.get("large").unwrap();
    let a = wheel.instantiate(1, (0.0, 0.0));
    let b = wheel.instantiate(2, (1.0, 0.0));
    let expected = (2.0 * std::f64::consts::PI / 3.0 - 3.0_f64.sqrt() / 2.0) / std::f64::consts::PI;
    assert!((overlap_ratio(&a, wheel, &b, wheel) - expected).abs() < 1e-12);
    let b = large.instantiate(2, (1.0, 0.0));
    assert!((overlap_ratio(&a, wheel, &b, large) - 0.5).abs() < 1e-12);
    assert!((overlap_ratio(&b, large, &a, wheel) - 0.5).abs() < 1e-12);
    assert_eq!(
        overlap_ratio(&a, wheel, &wheel.instantiate(2, (2.0, 0.0)), wheel),
        0.0
    );
    let mut low = 1.0;
    let mut high = 2.0;
    for _ in 0..60 {
        let d = (low + high) / 2.0;
        if circle_intersection(d, 1.0, 1.0) / std::f64::consts::PI > 0.05 {
            low = d;
        } else {
            high = d;
        }
    }
    assert!(editor_overlap_blocked(
        &a,
        wheel,
        &wheel.instantiate(2, (high, 0.0)),
        wheel
    ));
    assert!(!editor_overlap_blocked(
        &a,
        wheel,
        &wheel.instantiate(2, (high + 1e-5, 0.0)),
        wheel
    ));
}

#[test]
fn sensors_holes_and_intersection_exemptions_do_not_block() {
    let catalog = catalog();
    let mut kind = catalog.get("square").unwrap().clone();
    kind.shapes = vec![
        crate::PolygonShape {
            vertices: vec![(-4.0, -1.0), (-2.0, -1.0), (-2.0, 1.0), (-4.0, 1.0)],
            sensor: false,
        },
        crate::PolygonShape {
            vertices: vec![(2.0, -1.0), (4.0, -1.0), (4.0, 1.0), (2.0, 1.0)],
            sensor: false,
        },
        crate::PolygonShape {
            vertices: vec![(-2.0, -1.0), (2.0, -1.0), (2.0, 1.0), (-2.0, 1.0)],
            sensor: true,
        },
    ];
    let square = catalog.get("square").unwrap();
    let a = kind.instantiate(1, (0.0, 0.0));
    let b = square.instantiate(2, (0.0, 0.0));
    assert_eq!(overlap_ratio(&a, &kind, &b, square), 0.0);
    kind.ignore_editor_intersections = true;
    assert!(!editor_overlap_blocked(
        &a,
        &kind,
        &square.instantiate(2, (1.5, 0.0)),
        square
    ));
}
