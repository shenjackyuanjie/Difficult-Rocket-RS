use crate::sr1::solar::*;

// 按真实 SmolarSystem.xml 缩小：保留 Sun/Smearth/Smoon 层级、轨道和海洋山脉数据。
const SMOLAR: &str = r#"<SolarSystem name="Smolar System" description="small">
      <Planet name="Sun" gravity="274.0" radius="69634200.0"><Terrain maxHeight="0"/>
        <Children><Planet name="Smearth" gravity="9.798000" radius="637100.0" mapColor="103,157,255">
          <Orbit w="1.993303" e="0.016711" prograde="1" a="12559826100.0" v="-3.013170"/>
          <Terrain maxHeight="3250.0" minHeight="-1000.0" noise="2.0" waterDensity="75">
            <Ranges><Range startAngle="20" endAngle="89" minHeight="-2000" maxHeight="-1000"/>
              <Range startAngle="91" endAngle="93" minHeight="4000" maxHeight="8000"/></Ranges>
          </Terrain><Atmosphere height="70000" surfacePressure="1.0" graphicType="1"/>
          <Children><Planet name="Smoon" gravity="1.622000" radius="173710.0">
            <Orbit w="1.993303" e="0.054900" prograde="1" a="12500000.0" v="-2.138368"/>
            <Terrain maxHeight="500.0" noise="3.5"/></Planet></Children>
        </Planet></Children>
      </Planet></SolarSystem>"#;

#[test]
fn smolar_resource_subset_preserves_tree_and_orbits() {
    let system = SolarSystem::from_xml(SMOLAR).unwrap();
    assert_eq!(
        system
            .planets()
            .iter()
            .map(|p| p.name.as_str())
            .collect::<Vec<_>>(),
        ["Sun", "Smearth", "Smoon"]
    );
    let earth = system.planet("Smearth").unwrap();
    assert_eq!(earth.radius, 637100.0);
    assert_eq!(earth.orbit.unwrap().v, -3.013170);
    assert!(earth.launch_enabled);
    assert!(earth.terrain.has_water);
    assert_eq!(earth.terrain.ranges.len(), 2);
    assert_eq!(earth.atmosphere.unwrap().surface_pressure, 1.0);
    assert_eq!(
        earth.mass(),
        637100.0 * 637100.0 * 9.798 / GRAVITATIONAL_CONSTANT
    );
}

#[test]
fn missing_or_invalid_planet_values_keep_pc_defaults() {
    let system = SolarSystem::from_xml(r#"<SolarSystem><Planet gravity="bad" radius="bad" launchEnabled="bad"><Orbit/><Atmosphere/><Terrain/></Planet></SolarSystem>"#).unwrap();
    assert_eq!(system.name, "");
    assert_eq!(system.root.gravity, 0.0);
    assert_eq!(system.root.radius, 0.0);
    assert!(system.root.launch_enabled);
    assert_eq!(system.root.orbit, Some(Orbit::default()));
    assert_eq!(system.root.atmosphere, Some(Atmosphere::default()));
    assert_eq!(system.root.map_color, Color::BLACK);
}

#[test]
fn missing_terrain_is_an_error_and_only_first_root_is_loaded() {
    assert!(matches!(
        SolarSystem::from_xml("<SolarSystem><Planet/></SolarSystem>"),
        Err(Error::InvalidData(_))
    ));
    let system = SolarSystem::from_xml("<SolarSystem><Planet name='first'><Terrain/></Planet><Planet name='ignored'/></SolarSystem>").unwrap();
    assert_eq!(system.root.name, "first");
}

#[test]
fn terrain_casts_only_pc_float_fields_and_sorts_angle_endpoints() {
    let system = SolarSystem::from_xml(r#"<SolarSystem><Planet><Terrain noise="16777217" maxHeight="-16777217" minHeight="-16777217" waterDensity="0.1"><Ranges><Range startAngle="110" endAngle="80" minHeight="16777217" maxHeight="16777219"/></Ranges><Layers><Layer depth="16777217" waveLength="42" waveHeight="-2"/></Layers></Terrain></Planet></SolarSystem>"#).unwrap();
    let terrain = &system.root.terrain;
    assert_eq!(terrain.noise, 16777216.0_f32);
    assert_eq!(terrain.max_height, -16777216.0_f32);
    assert_eq!(terrain.water_level, -16777217.0);
    assert_eq!(terrain.base_height, terrain.max_height);
    assert!(!terrain.has_water);
    assert_eq!(terrain.ranges[0].start_angle, 80.0);
    assert_eq!(terrain.ranges[0].end_angle, 110.0);
    assert_eq!(terrain.ranges[0].min_height, 16777216.0_f32);
    assert_eq!(terrain.layers[0].depth, 16777217.0);
}

#[test]
fn terrain_positive_heights_and_water_threshold_follow_native_branches() {
    let system = SolarSystem::from_xml("<SolarSystem><Planet><Terrain maxHeight='10' minHeight='20' waterDensity='0.100000001'/></Planet></SolarSystem>").unwrap();
    assert_eq!(system.root.terrain.base_height, 0.0);
    assert_eq!(system.root.terrain.water_level, 0.0);
    assert!(system.root.terrain.has_water);
}

#[test]
fn color_skips_empty_items_truncates_components_and_requires_exact_alpha_count() {
    assert_eq!(Color::from_attribute(None), Color::BLACK);
    assert_eq!(Color::from_attribute(Some("1,2")), Color::BLACK);
    assert_eq!(
        Color::from_attribute(Some(",257,,-1,3tail,4,")),
        Color {
            r: 1,
            g: 255,
            b: 3,
            a: 4
        }
    );
    assert_eq!(Color::from_attribute(Some("1,2,invalid,4,5")).a, 255);
    assert_eq!(Color::from_attribute(Some("1,2,invalid,4,5")).b, 0);
}

#[test]
fn duplicate_names_keep_first_preorder_match_and_children_filter_by_name() {
    let system = SolarSystem::from_xml("<SolarSystem><Planet name='same'><Terrain/><Children><Other/><Planet name='same'><Terrain/></Planet></Children></Planet></SolarSystem>").unwrap();
    assert_eq!(system.planets().len(), 2);
    assert!(std::ptr::eq(system.planet("same").unwrap(), &system.root));
}

#[test]
fn ranges_and_layers_read_all_elements_after_first_named_entry() {
    let system = SolarSystem::from_xml("<SolarSystem><Planet><Terrain><Ranges><Other startAngle='99'/><Range startAngle='4' endAngle='1'/><Other startAngle='9' endAngle='6'/></Ranges><Layers><Other/><Layer depth='1'/><Other depth='2'/></Layers></Terrain></Planet></SolarSystem>").unwrap();
    assert_eq!(system.root.terrain.ranges.len(), 2);
    assert_eq!(system.root.terrain.ranges[1].start_angle, 6.0);
    assert_eq!(system.root.terrain.layers.len(), 2);
    assert_eq!(system.root.terrain.layers[1].depth, 2.0);
}
