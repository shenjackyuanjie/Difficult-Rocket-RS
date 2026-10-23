//! 本体语义管线的跨模块回归，合成资源随仓库携带，不依赖本机游戏目录。

use dr_core::sr1::{
    catalog::Catalog,
    parts::{Part, PartState},
    runtime::{Runtime, SaveSelection},
    ship::Ship,
    solar::SolarSystem,
    xml,
};

const CATALOG: &str = include_str!("fixtures/sr1/catalog.xml");
const SOLAR: &str = include_str!("fixtures/sr1/solar.xml");

fn catalog() -> Catalog {
    Catalog::from_xml(CATALOG).unwrap()
}

#[test]
fn native_and_strict_entry_points_remain_explicitly_different() {
    let source = "<Ship><Parts><Part partType='pod-1' id='1' x='2.5tail' y='0' angle='0' angleV='0'/></Parts><Connections/></Ship>";
    let native = Ship::from_editor_xml(source, &catalog()).unwrap();
    assert_eq!(native.version, 0);
    assert!(!native.touching_ground);
    assert_eq!(native.tree.parts()[0].x, 2.5);
    assert!(dr_core::io::ship_from_xml(source).is_err());
    let strict_source = source.replace("2.5tail", "2.5");
    let strict = dr_core::io::ship_from_xml(&strict_source).unwrap();
    assert!(strict.touching_ground);
}

#[test]
fn legacy_staging_migrates_then_roundtrips_through_native_editor_save() {
    let source = r#"<Ship currentStage="2"><Parts>
        <Part partType="pod-1" id="1"/><Part partType="fueltank-1" id="2"><Tank fuel="100"/></Part>
        </Parts><Connections><Connection parentPart="1" childPart="2" parentAttachPoint="2" childAttachPoint="1"/></Connections>
        <Staging><Step/><Step><Activate Id="2" moved="0"/><Activate Id="999"/></Step></Staging></Ship>"#;
    let catalog = catalog();
    let ship = Ship::from_editor_xml(source, &catalog).unwrap();
    let pod = ship.tree.parts().iter().find_map(Part::pod).unwrap();
    let staging = pod.staging.as_ref().unwrap();
    assert_eq!(staging.current_stage, 2);
    let output = ship.to_editor_xml().unwrap();
    let document = xml::parse(&output).unwrap();
    assert_eq!(document.attr("version"), Some("1"));
    assert_eq!(document.children[0].name, "DisconnectedParts");
    assert!(document.child("Staging").is_none());
    let root = document.child("Parts").unwrap().child("Part").unwrap();
    let saved_staging = root.child("Pod").unwrap().child("Staging").unwrap();
    assert_eq!(saved_staging.attr("currentStage"), Some("2"));
    assert_eq!(saved_staging.children[0].name, "Step");
    assert!(saved_staging.children[0].children.is_empty());
    assert_eq!(saved_staging.children[1].children.len(), 1);
    assert_eq!(
        saved_staging.children[1].children[0].attr("moved"),
        Some("1")
    );
    let reparsed = Ship::from_editor_xml(&output, &catalog).unwrap();
    assert_eq!(reparsed.tree.parts().len(), 2);
    let tank = reparsed
        .tree
        .parts()
        .iter()
        .find(|part| part.id == 2)
        .unwrap();
    assert!(matches!(&tank.state, PartState::Tank(state) if state.fuel == 10.0));
}

#[test]
fn native_special_part_states_are_not_dropped_by_generic_schema() {
    let catalog = catalog();
    let lander = Part::from_element(
        &xml::parse(
            "<Part partType='lander-1' lower='true' raise='0' length='3.125' legAngle='45'/>",
        )
        .unwrap(),
        &catalog,
    )
    .unwrap();
    assert!(
        matches!(&lander.state, PartState::Lander(state) if state.lower && state.length == 3.125 && state.leg_angle == 45.0)
    );
    let output = lander.to_element();
    assert_eq!(output.attr("lower"), Some("1"));
    assert_eq!(output.attr("length"), Some("3.125000"));
    assert_eq!(output.attr("legAngle"), Some("45.000000"));
    let solar = Part::from_element(
        &xml::parse("<Part partType='solar-1' extension='0.5'/>").unwrap(),
        &catalog,
    )
    .unwrap();
    assert_eq!(solar.to_element().attr("extension"), Some("0.500000"));
}

#[test]
fn per_class_virtual_load_does_not_uniformly_apply_base_flags() {
    let catalog = catalog();
    let input = |id| {
        xml::parse(&format!(
            "<Part partType='{id}' activated='1' exploded='1' flippedX='1'/>"
        ))
        .unwrap()
    };
    let pod = Part::from_element(&input("pod-1"), &catalog).unwrap();
    let generic = Part::from_element(&input("strut-1"), &catalog).unwrap();
    assert!(!pod.active && !pod.exploded && !pod.flipped_x);
    assert!(generic.active && generic.exploded && generic.flipped_x);
    assert!(pod.to_element().attr("flippedX").is_none());
    assert_eq!(generic.to_element().attr("flippedX"), Some("1"));
}

#[test]
fn runtime_binds_real_planet_context_and_omits_absent_selected_pod() {
    let catalog = catalog();
    let system = SolarSystem::from_xml(SOLAR).unwrap();
    let source = r#"<Runtime solarSystem="Fixture.xml"><Nodes>
        <PlanetNode name="Earth" trueAnomaly="1.25"/><PlanetNode name="Earth" trueAnomaly="not-a-number"/>
        <UnknownNode/><ShipNode id="7" planet="Earth" planetRadius="999999" x="3.5tail"><Ship><Parts>
        <Part partType="pod-1" id="1"/><Part partType="fueltank-1" id="2"/>
        </Parts><Connections><Connection parentPart="1" childPart="2" parentAttachPoint="2" childAttachPoint="1"/></Connections></Ship></ShipNode>
        </Nodes></Runtime>"#;
    let runtime = Runtime::from_xml(source, &catalog).unwrap();
    assert_eq!(runtime.ship_id, -1);
    assert_eq!(runtime.pod_id, -1);
    assert_eq!(runtime.nodes.len(), 3);
    let binding = runtime.bind_solar_system(&system).unwrap();
    assert_eq!(binding.ships.len(), 1);
    let earth = binding
        .planets
        .iter()
        .find(|state| state.planet.name == "Earth")
        .unwrap();
    assert_eq!(earth.true_anomaly, Some(1.25));
    let output = runtime
        .to_element(
            &system,
            SaveSelection {
                ship_id: 7,
                pod_id: None,
            },
        )
        .unwrap();
    assert!(output.attr("podId").is_none());
    let ship = output.child("Nodes").unwrap().child("ShipNode").unwrap();
    assert_eq!(ship.attr("planetRadius"), Some("100.000000"));
    assert_eq!(ship.attr("x"), Some("3.500000"));
    assert!(
        ship.child("Ship")
            .unwrap()
            .child("DisconnectedParts")
            .is_none()
    );
    assert!(Runtime::from_element(&output, &catalog).is_ok());
}

#[test]
fn saving_runtime_requires_a_real_matching_planet_instead_of_input_radius() {
    let catalog = catalog();
    let system = SolarSystem::from_xml(SOLAR).unwrap();
    let source = "<Runtime solarSystem='Fixture.xml'><Nodes><ShipNode planet='Missing' planetRadius='100'><Ship><Parts/><Connections/></Ship></ShipNode></Nodes></Runtime>";
    let runtime = Runtime::from_xml(source, &catalog).unwrap();
    assert!(runtime.bind_solar_system(&system).is_err());
    assert!(
        runtime
            .to_element(
                &system,
                SaveSelection {
                    ship_id: -1,
                    pod_id: None
                }
            )
            .is_err()
    );
}
