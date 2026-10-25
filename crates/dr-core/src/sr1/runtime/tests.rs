
use super::*;

fn catalog() -> Catalog {
    Catalog::from_xml(r#"<PartTypes><PartType id="pod-1" name="Pod" type="pod"><AttachPoints/></PartType></PartTypes>"#).unwrap()
}

fn system() -> SolarSystem {
    SolarSystem::from_xml(r#"<SolarSystem name="Smolar System"><Planet name="Sun" radius="69634200"><Terrain/>
          <Children><Planet name="Smearth" radius="637100"><Orbit v="-3.013170"/><Terrain/>
            <Children><Planet name="Smoon" radius="173710"><Orbit v="-2.138368"/><Terrain/></Planet></Children>
          </Planet></Children></Planet></SolarSystem>"#).unwrap()
}

fn node(planet: &str, attrs: &str) -> String {
    format!(
        r#"<ShipNode planet="{planet}" {attrs}><Ship version="1"><Parts><Part partType="pod-1" id="1"/></Parts><Connections/></Ship></ShipNode>"#
    )
}

fn runtime(nodes: &str, attrs: &str) -> Runtime {
    Runtime::from_xml(
        &format!(
            r#"<Runtime solarSystem="SmolarSystem.xml" {attrs}><Nodes>{nodes}</Nodes></Runtime>"#
        ),
        &catalog(),
    )
    .unwrap()
}

#[test]
fn runtime_defaults_and_selection_requests_remain_separate() {
    let base = Runtime::from_xml("<Runtime><Nodes/></Runtime>", &catalog()).unwrap();
    assert_eq!(base.time, 0.0);
    assert!(!base.first_stage_activated);
    assert_eq!(base.ship_id, -1);
    assert_eq!(base.pod_id, -1);
    assert_eq!(base.solar_system_or("default.xml"), "default.xml");
    let invalid = runtime(
        "",
        "time='bad' firstStageActivated='TRUE' shipId='bad' podId='bad'",
    );
    assert_eq!(invalid.time, 0.0);
    assert!(!invalid.first_stage_activated);
    assert_eq!(invalid.ship_id, -1);
    assert_eq!(invalid.pod_id, -1);
}

#[test]
fn shipnode_restores_only_proven_fields_and_keeps_source_radius_inert() {
    let runtime = runtime(
        &node(
            "Smearth",
            "planetRadius='999999999' id='7tail' x='16777217.25m' y='bad' vx='-2.5tail' vy='bad'",
        ),
        "time='12.5s' firstStageActivated='2'",
    );
    let ship = runtime.ship_nodes().next().unwrap();
    assert_eq!(ship.id, 7);
    assert_eq!(ship.x, 16777217.25);
    assert_eq!(ship.y, 0.0);
    assert_eq!(ship.vx, -2.5);
    assert_eq!(ship.vy, 0.0);
    assert_eq!(ship.source_planet_radius.as_deref(), Some("999999999"));
    assert_eq!(runtime.time, 12.5);
    assert!(runtime.first_stage_activated);
    let system = system();
    let world = runtime.bind_solar_system(&system).unwrap();
    assert_eq!(world.ships[0].planet.radius, 637100.0);
    let saved = ship.to_element(world.ships[0].planet).unwrap();
    assert_eq!(saved.attr("planetRadius"), Some("637100.000000"));
}

#[test]
fn missing_shipnode_numeric_fields_default_to_zero() {
    let runtime = runtime(&node("Smearth", ""), "");
    let ship = runtime.ship_nodes().next().unwrap();
    assert_eq!(ship.id, 0);
    assert_eq!((ship.x, ship.y, ship.vx, ship.vy), (0.0, 0.0, 0.0, 0.0));
    assert!(ship.source_planet_radius.is_none());
    let RuntimeNode::Ship(boxed) = &runtime.nodes[0] else {
        panic!("预期装箱后的 ShipNode");
    };
    assert!(std::ptr::eq(ship, boxed.as_ref()));
}

#[test]
fn true_anomaly_missing_or_failed_does_not_overwrite_initial_or_previous_value() {
    let runtime = runtime(
        r#"<PlanetNode name="Smearth" trueAnomaly="1.25tail"/>
          <PlanetNode name="Smearth" trueAnomaly="bad"/><PlanetNode name="Smearth"/>
          <PlanetNode name="Smoon" trueAnomaly="bad"/><PlanetNode name="unknown" trueAnomaly="123"/>"#,
        "",
    );
    let system = system();
    let world = runtime.bind_solar_system(&system).unwrap();
    assert_eq!(world.planets[0].true_anomaly, None);
    assert_eq!(world.planets[1].true_anomaly, Some(1.25));
    assert_eq!(world.planets[2].true_anomaly, Some(-2.138368));
    assert_eq!(
        system.planet("Smearth").unwrap().orbit.unwrap().v,
        -3.013170
    );
}

#[test]
fn save_uses_actual_optional_pod_not_the_load_request() {
    let runtime = runtime(&node("Smearth", "id='7'"), "shipId='7' podId='999'");
    let system = system();
    let saved = runtime
        .to_element(
            &system,
            SaveSelection {
                ship_id: 7,
                pod_id: None,
            },
        )
        .unwrap();
    assert_eq!(saved.attr("shipId"), Some("7"));
    assert_eq!(saved.attr("podId"), None);
    let saved = runtime
        .to_element(
            &system,
            SaveSelection {
                ship_id: 7,
                pod_id: Some(1),
            },
        )
        .unwrap();
    assert_eq!(saved.attr("podId"), Some("1"));
}

#[test]
fn save_rebuilds_resource_tree_order_and_defaults_for_unsaved_planets() {
    let runtime = runtime(
        &format!("{}{}", node("Sun", "id='3'"), node("Smearth", "id='4'")),
        "",
    );
    let saved = runtime
        .to_element(
            &system(),
            SaveSelection {
                ship_id: 3,
                pod_id: Some(1),
            },
        )
        .unwrap();
    let children = &saved.child("Nodes").unwrap().children;
    assert_eq!(
        children.iter().map(|n| n.name.as_str()).collect::<Vec<_>>(),
        [
            "PlanetNode",
            "PlanetNode",
            "PlanetNode",
            "ShipNode",
            "ShipNode"
        ]
    );
    assert_eq!(children[0].attr("name"), Some("Sun"));
    assert_eq!(children[0].attr("trueAnomaly"), None);
    assert_eq!(children[1].attr("trueAnomaly"), Some("-3.013170"));
    assert_eq!(children[3].attr("id"), Some("4"));
    assert_eq!(children[4].attr("id"), Some("3"));
}

#[test]
fn unknown_ship_planet_is_an_error_not_a_fake_zero_radius() {
    let runtime = runtime(&node("missing", "planetRadius='637100'"), "");
    assert!(matches!(
        runtime.bind_solar_system(&system()),
        Err(Error::InvalidData(_))
    ));
    assert!(matches!(
        runtime.to_element(
            &system(),
            SaveSelection {
                ship_id: 0,
                pod_id: None
            }
        ),
        Err(Error::InvalidData(_))
    ));
}

#[test]
fn wrong_roots_and_missing_required_nodes_are_explicit_errors() {
    assert!(matches!(
        Runtime::from_xml("<Ship/>", &catalog()),
        Err(Error::WrongRoot { .. })
    ));
    assert!(matches!(
        Runtime::from_xml("<Runtime/>", &catalog()),
        Err(Error::InvalidData(_))
    ));
    assert!(matches!(
        Runtime::from_xml("<Runtime><Nodes><ShipNode/></Nodes></Runtime>", &catalog()),
        Err(Error::InvalidData(_))
    ));
}

#[test]
fn unrecognized_runtime_node_is_ignored() {
    let runtime = runtime("<Other/><PlanetNode name='Sun'/>", "");
    assert_eq!(runtime.nodes.len(), 1);
}

#[test]
fn challenge_kind_is_explicit_and_uppercase_fuel_is_not_lowercase_fuel() {
    let root = xml::parse(
        r#"<Runtime MoonTouchDown="true" FuelUsed="16777217.25" fuelUsed="3.75" maxSpeed="42.5"/>"#,
    )
    .unwrap();
    assert_eq!(
        ChallengeState::from_element(&root, ChallengeKind::None),
        ChallengeState::None
    );
    assert_eq!(
        ChallengeState::from_element(&root, ChallengeKind::MoonTouchDown),
        ChallengeState::MoonTouchDown {
            moon_touch_down: true
        }
    );
    assert_eq!(
        ChallengeState::from_element(&root, ChallengeKind::MoonTouchDownFuelUsed),
        ChallengeState::MoonTouchDownFuelUsed {
            moon_touch_down: true,
            fuel_used: 16777217.25
        }
    );
    assert_eq!(
        ChallengeState::from_element(&root, ChallengeKind::FuelUsed),
        ChallengeState::FuelUsed {
            fuel_used: Some(3.75)
        }
    );
    assert_eq!(
        ChallengeState::from_element(&root, ChallengeKind::MaxSpeed),
        ChallengeState::MaxSpeed { max_speed: 42.5 }
    );
}

#[test]
fn all_challenge_default_reads_use_zero_and_false() {
    let root = xml::parse(r#"<Runtime MoonTouchDown="bad" FuelUsed="bad" fuelUsed="bad" maxSpeed="bad" angleTraveled="bad" lastAngle="bad" lastAngleDelta="bad" recordedAngle="bad" goodJob="bad"/>"#).unwrap();
    assert_eq!(
        ChallengeState::from_element(&root, ChallengeKind::MoonTouchDownFuelUsed),
        ChallengeState::MoonTouchDownFuelUsed {
            moon_touch_down: false,
            fuel_used: 0.0
        }
    );
    assert_eq!(
        ChallengeState::from_element(&root, ChallengeKind::FuelUsed),
        ChallengeState::FuelUsed {
            fuel_used: Some(0.0)
        }
    );
    assert_eq!(
        ChallengeState::from_element(&root, ChallengeKind::MaxSpeed),
        ChallengeState::MaxSpeed { max_speed: 0.0 }
    );
    assert_eq!(
        ChallengeState::from_element(&root, ChallengeKind::LowFlyer),
        ChallengeState::LowFlyer(LowFlyerState::default())
    );
}

#[test]
fn low_flyer_restores_and_writes_all_five_fields() {
    let root = xml::parse(r#"<Runtime angleTraveled="1.25tail" lastAngle="-3.5" lastAngleDelta="16777217.25" recordedAngle="2" goodJob="true"/>"#).unwrap();
    let state = ChallengeState::from_element(&root, ChallengeKind::LowFlyer);
    assert_eq!(
        state,
        ChallengeState::LowFlyer(LowFlyerState {
            angle_traveled: 1.25,
            last_angle: -3.5,
            last_angle_delta: 16777217.25,
            recorded_angle: true,
            good_job: true
        })
    );
    let mut saved = Element::new("Runtime");
    state.write_attributes(&mut saved);
    assert_eq!(saved.attr("angleTraveled"), Some("1.250000"));
    assert_eq!(saved.attr("lastAngle"), Some("-3.500000"));
    assert_eq!(saved.attr("lastAngleDelta"), Some("16777217.250000"));
    assert_eq!(saved.attr("recordedAngle"), Some("1"));
    assert_eq!(saved.attr("goodJob"), Some("1"));
}

#[test]
fn fuel_status_absence_omits_lowercase_attribute() {
    let mut root = Element::new("Runtime");
    ChallengeState::FuelUsed { fuel_used: None }.write_attributes(&mut root);
    assert!(root.attr("fuelUsed").is_none());
    ChallengeState::MoonTouchDownFuelUsed {
        moon_touch_down: false,
        fuel_used: 2.5,
    }
    .write_attributes(&mut root);
    assert_eq!(root.attr("MoonTouchDown"), Some("0"));
    assert_eq!(root.attr("FuelUsed"), Some("2.500000"));
    assert!(root.attr("fuelUsed").is_none());
}

#[test]
fn runtime_challenge_entry_and_semantic_save_roundtrip() {
    let source = format!(
        r#"<Runtime solarSystem="SmolarSystem.xml" time="12.5" MoonTouchDown="1" FuelUsed="10.25"><Nodes>{}</Nodes></Runtime>"#,
        node("Smearth", "id='2'")
    );
    let catalog = catalog();
    let first =
        Runtime::from_xml_with_kind(&source, &catalog, ChallengeKind::MoonTouchDownFuelUsed)
            .unwrap();
    let saved = first
        .to_xml(
            &system(),
            SaveSelection {
                ship_id: 2,
                pod_id: Some(1),
            },
        )
        .unwrap();
    let second =
        Runtime::from_xml_with_kind(&saved, &catalog, ChallengeKind::MoonTouchDownFuelUsed)
            .unwrap();
    assert_eq!(first.challenge, second.challenge);
    assert_eq!(second.time, 12.5);
    assert_eq!(second.pod_id, 1);
    assert_eq!(
        second.ship_nodes().next().unwrap().ship.tree.parts().len(),
        1
    );
}

#[test]
fn ship_save_eligibility_matches_all_flag_combinations_and_counts() {
    for bits in 0..8 {
        let flags = ShipSaveFlags {
            node_100: bits & 1 != 0,
            root_1cb: bits & 2 != 0,
            node_184: bits & 4 != 0,
        };
        for count in [0, 1, 2] {
            assert_eq!(
                flags.eligible(count),
                bits == 5 && count == 2,
                "bits={bits}, count={count}"
            );
        }
    }
}

#[test]
fn unresolved_default_solar_resource_name_cannot_be_saved() {
    let mut runtime = Runtime::from_xml("<Runtime><Nodes/></Runtime>", &catalog()).unwrap();
    let selection = SaveSelection {
        ship_id: 0,
        pod_id: None,
    };
    assert!(matches!(
        runtime.to_element(&system(), selection),
        Err(Error::InvalidData(_))
    ));
    runtime.solar_system = runtime.solar_system_or("SmolarSystem.xml").to_owned();
    assert!(runtime.to_element(&system(), selection).is_ok());
}
