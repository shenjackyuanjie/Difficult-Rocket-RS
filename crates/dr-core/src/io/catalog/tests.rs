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
