use super::*;

#[test]
fn catalog_keeps_physics_limits_damage_and_all_static_specs() {
    let xml = r#"<PartTypes><PartType id="test" type="lander" width="2" height="3"
      friction="0.1" canExplode="false" coverHeight="2" sandboxOnly="true" drag="-1" buoyancy="0.5">
      <Damage disconnect="1500" explode="1700" explosionPower="5" explosionSize="10"/>
      <Rcs power="1" consumption="0.1" size="1"/><Solar chargeRate="2"/>
      <Lander maxAngle="140" minLength="2.26" maxLength="4.15" angleSpeed="25" lengthSpeed="0.5" width="0.5"/>
    </PartType></PartTypes>"#;
    let before = catalog_from_xml(xml).unwrap();
    let p = before.get("test").unwrap();
    assert_eq!(
        (
            p.friction,
            p.can_explode,
            p.cover_height,
            p.sandbox_only,
            p.drag,
            p.buoyancy
        ),
        (
            Some(0.1),
            Some(false),
            Some(2),
            Some(true),
            Some(-1.0),
            Some(0.5)
        )
    );
    assert_eq!(p.damage.as_ref().unwrap().disconnect, 1500.0);
    assert_eq!(p.rcs.as_ref().unwrap().power, 1.0);
    assert_eq!(p.solar.as_ref().unwrap().charge_rate, 2.0);
    assert_eq!(p.lander.as_ref().unwrap().max_angle, 140.0);
    let changed = xml
        .replace("power=\"1\"", "power=\"8\"")
        .replace("chargeRate=\"2\"", "chargeRate=\"9\"")
        .replace("maxAngle=\"140\"", "maxAngle=\"150\"")
        .replace("disconnect=\"1500\"", "disconnect=\"1900\"");
    let after = catalog_from_xml(&changed).unwrap();
    let p = after.get("test").unwrap();
    assert_eq!(p.rcs.as_ref().unwrap().power, 8.0);
    assert_eq!(p.solar.as_ref().unwrap().charge_rate, 9.0);
    assert_eq!(p.lander.as_ref().unwrap().max_angle, 150.0);
    assert_eq!(p.damage.as_ref().unwrap().disconnect, 1900.0);
    assert_ne!(before.types, after.types, "静态规格变更不可再被静默忽略");
}

#[test]
fn omitted_physics_is_not_confused_with_explicit_zero_or_false() {
    let omitted = catalog_from_xml(r#"<PartTypes><PartType id="a"/></PartTypes>"#).unwrap();
    let explicit = catalog_from_xml(
        r#"<PartTypes><PartType id="a" friction="0" canExplode="false"/></PartTypes>"#,
    )
    .unwrap();
    assert!(omitted.get("a").unwrap().friction.is_none());
    assert_eq!(explicit.get("a").unwrap().friction, Some(0.0));
    assert_ne!(omitted.types, explicit.types);
}

#[test]
fn exported_catalog_keeps_order_specs_shapes_attachment_locations_and_name() {
    let source = r#"<PartTypes name="自定义 &amp; 目录"><PartType id="a" type="rcs" width="4" height="2" friction="0.1" canExplode="false">
    <Damage disconnect="10" explode="20"/><Rcs power="1" consumption="0.1" size="2"/>
    <Shape sensor="true"><Vertex x="0" y="0"/><Vertex x="1" y="0"/><Vertex x="0" y="1"/></Shape>
    <AttachPoints><AttachPoint location="TopSide" group="2" flipX="true" breakForce="5"/><AttachPoint x="1.5" y="-0.3"/></AttachPoints>
    </PartType><PartType id="a" mass="20"/><PartType id="solar"><Solar chargeRate="2"/></PartType></PartTypes>"#;
    let mut catalog = catalog_from_xml(source).unwrap();
    catalog.types[0].rcs.as_mut().unwrap().power = 7.0;
    catalog.types[2].solar.as_mut().unwrap().charge_rate = 9.0;
    let xml = catalog_to_xml(&catalog).unwrap();
    assert!(xml.starts_with("<PartTypes "));
    assert!(xml.contains("power=\"7\""));
    assert!(xml.contains("chargeRate=\"9\""));
    assert!(!xml.contains("<Lander"));
    let restored = catalog_from_xml(&xml).unwrap();
    assert_eq!(restored.name, catalog.name);
    assert_eq!(restored.types, catalog.types);
    assert_eq!(restored.get("a").unwrap().rcs.as_ref().unwrap().power, 7.0);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("saved.xml");
    save_catalog(&path, &catalog).unwrap();
    assert_eq!(load_catalog(path).unwrap().name, catalog.name);
}

#[test]
fn invalid_edited_outline_does_not_replace_existing_catalog() {
    let mut catalog =
        catalog_from_xml(r#"<PartTypes><PartType id="a" width="2" height="2"/></PartTypes>"#)
            .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("catalog.xml");
    save_catalog(&path, &catalog).unwrap();
    let before = fs::read(&path).unwrap();
    catalog.types[0].shapes.push(PolygonShape {
        vertices: vec![(0.0, 0.0), (1.0, 0.0)],
        sensor: false,
    });
    assert!(save_catalog(&path, &catalog).is_err());
    assert_eq!(fs::read(&path).unwrap(), before);
}
