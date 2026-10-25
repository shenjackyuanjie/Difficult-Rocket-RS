use crate::sr1::catalog::*;

// 从 PC PartList 逐项摘录，保持各项内部字段和顺序，不访问游戏目录。
const PC_RESOURCE_FRAGMENT: &str = r#"<PartTypes xmlns="http://jundroo.com/simplerockets/partlist.xsd">
	<PartType id="pod-1" name="Command Pod Mk1" description="This is your ship's brain. Be careful with it." sprite="Pod.png" type="pod" mass="1.0" width="4" height="3" hidden="true">
		<Damage disconnect="1500" explode="1500" explosionPower="5" explosionSize="10" />
		<Shape>
			<!-- Must be counter clockwise -->
			<Vertex x="-2.0" y="-1.5" />
			<Vertex x="2.0" y="-1.5" />
			<Vertex x="1.3" y="1.5" />
			<Vertex x="-1.3" y="1.5" />
		</Shape>
		<AttachPoints>
			<AttachPoint location="TopCenter" />
			<AttachPoint location="BottomCenter" />
		</AttachPoints>
	</PartType>
	<PartType id="fueltank-4" name="Puffy T750" description="Monopropellant for RCS thrusters." sprite="Puffy750.png" type="tank" mass="1.85" width="4" height="2" category="Satellite">
		<Tank fuel="750.0" dryMass="0.35" fuelType="1" />
		<Damage disconnect="2500" explode="2500" explosionPower="5" explosionSize="10" />
		<AttachPoints>
			<AttachPoint location="TopCenter" />
			<AttachPoint location="BottomCenter" />
			<AttachPoint location="LeftSide" />
			<AttachPoint location="RightSide" />
		</AttachPoints>
	</PartType>
	<PartType id="engine-0" name="Tiny 21" description="Really only useful for landing on Smoon." sprite="EngineTiny.png" type="engine" mass="0.5" width="4" height="2" buoyancy="0.5">
		<Engine power="0.25" consumption="4.00" size="0.50" turn="20.0" throttleExponential="true" />
		<Shape>
			<Vertex x="-0.9" y="-1.0" />
			<Vertex x="0.9" y="-1.0" />
			<Vertex x="2.0" y="1.0" />
			<Vertex x="-2.0" y="1.0" />
		</Shape>
		<AttachPoints>
			<AttachPoint location="TopCenter" fuelLine="true" order="1" />
			<AttachPoint location="BottomCenter" order="2" />
		</AttachPoints>
	</PartType>
	<PartType id="engine-4" name="Blasto SRB 500" description="Great for blasting off, but they can't be throttle or turn." sprite="SolidRocketBooster.png" type="engine" mass="22.0" width="4" height="26" buoyancy="0.5" coverHeight="2" sandboxOnly="true">
		<Engine power="6.0" consumption="175" size="1.0" turn="0.0" fuelType="3" />
		<Tank fuel="9000.0" dryMass="3.15" />
		<AttachPoints>
			<AttachPoint location="TopCenter" order="1" />
			<AttachPoint location="BottomCenter" order="2" />
			<AttachPoint location="LeftSide" />
			<AttachPoint location="RightSide" />
		</AttachPoints>
	</PartType>
	<PartType id="rcs-1" name="RCS Thrusters" description="Reaction control system for precision maneuvering." sprite="RcsBlock.png" type="rcs" mass="0.25" width="1" height="3" category="Satellite" disableEditorRotation="true" buoyancy="0.5">
		<Rcs power="1.0" consumption="0.1" size="1.00" />
		<AttachPoints>
			<AttachPoint location="RightCenter" flipX="true" group="1" />
			<AttachPoint location="LeftCenter" group="1" />
		</AttachPoints>
	</PartType>
	<PartType id="solar-1" name="Solar Panel" description="Expanding solar panel." sprite="SolarPanelBase.png" type="solar" mass="1.0" width="1" height="4" category="Satellite" sandboxOnly="true">
		<Solar chargeRate="2.0" />
		<Damage disconnect="250" explode="3000" explosionPower="1" explosionSize="5" />
		<AttachPoints>
			<AttachPoint location="RightCenter" flipX="true" group="1" />
			<AttachPoint location="LeftCenter" group="1" />
		</AttachPoints>
	</PartType>
	<PartType id="battery-0" name="Batteries" description="Batteries can be recharged by solar panels." sprite="Battery.png" type="tank" mass="1.25" width="4" height="1" category="Satellite" sandboxOnly="true">
		<Tank fuel="250.0" dryMass="1.24" fuelType="2" />
		<Damage disconnect="2500" explode="2500" explosionPower="5" explosionSize="10" />
		<AttachPoints>
			<AttachPoint location="TopCenter" />
			<AttachPoint location="BottomCenter" />
			<AttachPoint location="LeftSide" />
			<AttachPoint location="RightSide" />
		</AttachPoints>
	</PartType>
	<PartType id="port-1" name="Docking Port" description="Accepts a docking plug for docking in space." sprite="DockingPort.png" type="dockport" mass="0.75" width="2" height="4" friction="0.1" category="Satellite" buoyancy="0.5">
		<Shape>
			<Vertex x="-0.2" y="-2.0" />
			<Vertex x="1.0" y="-2.0" />
			<Vertex x="1.0" y="2.0" />
			<Vertex x="-0.2" y="2.0" />
		</Shape>
		<Shape>
			<Vertex x="-1.0" y="-2.0" />
			<Vertex x="0.0" y="-2.0" />
			<Vertex x="0.0" y="-0.1" />
			<Vertex x="-1.0" y="-1.10" />
		</Shape>
		<Shape>
			<Vertex x="-1.0" y="1.10" />
			<Vertex x="0.0" y="0.1" />
			<Vertex x="0.0" y="2.0" />
			<Vertex x="-1.0" y="2.0" />
		</Shape>
		<Shape sensor="true">
			<Vertex x="-0.33" y="-1.0" />
			<Vertex x="-0.2" y="-1.0" />
			<Vertex x="-0.2" y="1.0" />
			<Vertex x="-0.33" y="1.0" />
		</Shape>
		<AttachPoints>
			<AttachPoint location="RightCenter" group="1" />
			<AttachPoint location="LeftCenter" flipX="true" group="1" />
		</AttachPoints>
	</PartType>
	<PartType id="lander-1" name="Lander" description="Activate these babies to make landing a little easier." sprite="LanderLegPreview.png" type="lander" mass="0.5" width="1" height="5" ignoreEditorIntersections="true" buoyancy="0.5">
		<Lander maxAngle="140" minLength="2.26" maxLength="4.15" angleSpeed="25" lengthSpeed="0.5" width="0.5" />
		<AttachPoints>
			<AttachPoint location="LeftCenter" group="1" />
			<AttachPoint location="RightCenter" group="1" />
		</AttachPoints>
	</PartType>
</PartTypes>"#;

fn part(data: &str) -> PartType {
    PartType::from_element(&xml::parse(data).unwrap()).unwrap()
}

#[test]
fn real_resource_fragment_loads_special_classes_and_capacities() {
    let catalog = Catalog::from_xml(PC_RESOURCE_FRAGMENT).unwrap();
    assert_eq!(catalog.parts.len(), 9);
    assert_eq!(catalog.parts[0].id, "pod-1");
    assert_eq!(catalog.parts[8].id, "lander-1");
    let pod = catalog.get("pod-1").unwrap();
    assert_eq!(pod.class, PartClass::Pod);
    assert!(pod.hidden);
    assert_eq!(pod.width, 2.0);
    assert_eq!(pod.shapes[0].vertices[0], (-1.0, -0.75));
    assert_eq!(pod.damage.disconnect, 1500.0);
    let tank = catalog.get("fueltank-4").unwrap();
    assert_eq!(tank.fuel_capacity(), Some(750.0));
    assert_eq!(tank.tank.as_ref().unwrap().fuel_type, 1);
    assert_eq!(tank.category, "Satellite");
    let engine = catalog.get("engine-0").unwrap();
    assert_eq!(engine.class, PartClass::Engine);
    assert_eq!(engine.category, "Engines");
    assert_eq!(engine.fuel_capacity(), Some(0.0));
    assert_eq!(
        engine.engine.as_ref().unwrap().turn_radians,
        20.0 * DEGREES_TO_RADIANS
    );
    assert!(engine.engine.as_ref().unwrap().throttle_exponential);
    let srb = catalog.get("engine-4").unwrap();
    assert_eq!(srb.fuel_capacity(), Some(9000.0));
    assert_eq!(srb.tank.as_ref().unwrap().fuel_type, 3);
    assert_eq!(srb.cover_height, 1.0);
    assert_eq!(
        catalog
            .get("battery-0")
            .unwrap()
            .tank
            .as_ref()
            .unwrap()
            .fuel_type,
        2
    );
    assert_eq!(
        catalog
            .get("rcs-1")
            .unwrap()
            .rcs
            .as_ref()
            .unwrap()
            .consumption,
        0.1
    );
    assert_eq!(
        catalog
            .get("solar-1")
            .unwrap()
            .solar
            .as_ref()
            .unwrap()
            .charge_rate,
        2.0
    );
    let port = catalog.get("port-1").unwrap();
    assert_eq!(port.class, PartClass::DockPort);
    assert_eq!(port.shapes.len(), 4);
    assert!(port.shapes[3].sensor);
    let lander = catalog.get("lander-1").unwrap().lander.as_ref().unwrap();
    assert_eq!(lander.min_length, 2.26);
    assert_eq!(lander.max_angle, 140.0);
    assert_eq!(lander.angle_speed, 25.0);
}

#[test]
fn native_general_defaults_and_f32_rectangle_are_not_editor_defaults() {
    let value = part(r#"<PartType width="4" height="6"><AttachPoints/></PartType>"#);
    assert_eq!(value.class, PartClass::Generic);
    assert_eq!(value.mass, 0.0);
    assert_eq!(
        (value.width, value.height, value.cover_height),
        (2.0, 3.0, 3.0)
    );
    assert_eq!(
        (value.drag, value.friction, value.buoyancy),
        (1.0, 0.75, 1.0)
    );
    assert!(value.can_explode);
    assert!(!value.hidden);
    assert_eq!(value.max_occurrences, 0);
    assert!(!value.stageable);
    assert_eq!(
        value.damage,
        DamageSpec {
            disconnect: 3000.0,
            explode: 3000.0,
            explosion_power: 1.0,
            explosion_size: 5.0,
        }
    );
    let x = 1.0_f32 - 0.025_f32;
    assert_eq!(
        value.shapes[0].vertices,
        vec![(-x, -1.5), (x, -1.5), (x, 1.5), (-x, 1.5)]
    );
    assert_eq!(value.fuel_capacity(), None);
}

#[test]
fn present_empty_damage_is_zero_not_missing_damage_defaults() {
    let value = part("<PartType><Damage/><AttachPoints/></PartType>");
    assert_eq!(
        value.damage,
        DamageSpec {
            disconnect: 0.0,
            explode: 0.0,
            explosion_power: 0.0,
            explosion_size: 0.0,
        }
    );
}

#[test]
fn scalar_prefixes_negative_integers_and_failed_defaults_are_preserved() {
    let value = part(
        r#"<PartType mass="1.234567890123tail" width="4.5units" height="bad" drag="bad" friction=".125x" canExplode="bad" hidden="-2extra" maxOccurrences="-3parts"><AttachPoints/></PartType>"#,
    );
    assert_eq!(value.mass, 1.234567890123);
    assert_eq!(value.width, 2.25);
    assert_eq!(value.height, 0.0);
    assert_eq!(value.drag, 1.0);
    assert_eq!(value.friction, 0.125);
    assert!(value.can_explode);
    assert!(value.hidden);
    assert_eq!(value.max_occurrences, -3);
}

#[test]
fn cover_height_only_scales_successfully_parsed_overrides() {
    for (attribute, expected) in [("", 3.0), ("bad", 3.0), ("10tail", 5.0), ("0", 0.0)] {
        let source =
            format!(r#"<PartType height="6" coverHeight="{attribute}"><AttachPoints/></PartType>"#);
        assert_eq!(part(&source).cover_height, expected);
    }
}

#[test]
fn named_attach_locations_use_native_scaled_dimensions() {
    let value = part(
        r#"<PartType width="8" height="12"><AttachPoints>
            <AttachPoint location="TopCenter" x="999"/>
            <AttachPoint location="BottomCenter"/>
            <AttachPoint location="Top"/>
            <AttachPoint location="Bottom"/>
            <AttachPoint location="LeftCenter"/>
            <AttachPoint location="RightCenter"/>
            <AttachPoint location="LeftSide"/>
            <AttachPoint location="RightSide"/>
        </AttachPoints></PartType>"#,
    );
    let expected = [
        (0.0, 3.0, 0.0, 0.0, 1),
        (0.0, -3.0, 0.0, 0.0, 1),
        (0.0, 3.0, 3.0, 0.0, 0),
        (0.0, -3.0, 3.0, 0.0, 0),
        (-2.0, 0.0, 0.0, 0.0, 1),
        (2.0, 0.0, 0.0, 0.0, 1),
        (-2.0, 0.0, 0.0, 5.0, 0),
        (2.0, 0.0, 0.0, 5.0, 0),
    ];
    for (index, (point, expected)) in value.attach_points.iter().zip(expected).enumerate() {
        assert_eq!(point.id, index as i32 + 1);
        assert_eq!(
            (point.x, point.y, point.width, point.height, point.max_parts),
            expected
        );
    }
    assert!(value.attach_points[1].bottom);
    assert!(value.attach_points[3].bottom);
    assert_eq!(value.attach_points[4].swap_id, 6);
    assert_eq!(value.attach_points[5].swap_id, 5);
    assert_eq!(value.attach_points[6].swap_id, 8);
    assert!(value.has_attach_point(8));
    assert!(!value.has_attach_point(0));
}

#[test]
fn explicit_attach_points_scale_coordinates_but_not_ranges() {
    let value = part(
        r#"<PartType><AttachPoints><AttachPoint id="99" location="custom" x="1.5" y="-2" width="5" height="6" maxParts="3" order="7" group="2" fuelLine="1" flipX="true" flipY="true" breakAngle="90" breakForce="150"/></AttachPoints></PartType>"#,
    );
    let point = &value.attach_points[0];
    assert_eq!(point.id, 1);
    assert_eq!(
        (point.x, point.y, point.width, point.height),
        (0.75, -1.0, 5.0, 6.0)
    );
    assert_eq!((point.max_parts, point.order, point.group), (3, 7, 2));
    assert!(point.fuel_line && point.flip_x && point.flip_y);
    assert_eq!(point.break_angle_radians, 90.0 * DEGREES_TO_RADIANS);
    assert_eq!(point.break_force, 150.0);
    assert_eq!(point.raw.attr("id"), Some("99"));
}

#[test]
fn zero_and_invalid_break_limits_use_native_nonzero_defaults() {
    let value = part(
        r#"<PartType><AttachPoints><AttachPoint breakAngle="-0" breakForce="0"/><AttachPoint breakAngle="bad" breakForce="bad"/></AttachPoints></PartType>"#,
    );
    for point in &value.attach_points {
        assert_eq!(point.break_angle_radians, 15.0 * DEGREES_TO_RADIANS);
        assert_eq!(point.break_force, 125.0);
    }
}

#[test]
fn mirror_pairing_preserves_native_direction_and_invalid_id_behavior() {
    let value = part(
        r#"<PartType><AttachPoints><AttachPoint location="LeftCenter" swapOnMirror="false"/><AttachPoint location="RightCenter"/><AttachPoint swapId="99"/></AttachPoints></PartType>"#,
    );
    assert!(!value.attach_points[0].swap_on_mirror);
    assert_eq!(value.attach_points[0].swap_id, 2);
    assert_eq!(value.attach_points[1].swap_id, 1);
    assert!(!value.attach_points[2].swap_on_mirror);
    assert_eq!(value.attach_points[2].swap_id, 0);
    let unmatched = part(
        r#"<PartType><AttachPoints><AttachPoint location="LeftSide"/></AttachPoints></PartType>"#,
    );
    assert!(unmatched.attach_points[0].swap_on_mirror);
    assert_eq!(unmatched.attach_points[0].swap_id, 0);
}

#[test]
fn explicit_shape_coordinates_round_after_f64_scaling_and_empty_shape_stays_empty() {
    let value = part(
        r#"<PartType width="4"><Shape sensor="1"><Vertex x="2.000000123" y="-1.23456789"/><Vertex x="bad"/></Shape><Shape/><AttachPoints/></PartType>"#,
    );
    assert_eq!(value.shapes.len(), 2);
    assert!(value.shapes[0].sensor);
    assert_eq!(
        value.shapes[0].vertices[0],
        (
            (2.000000123_f64 * 0.5) as f32,
            (-1.23456789_f64 * 0.5) as f32
        )
    );
    assert_eq!(value.shapes[0].vertices[1], (0.0, 0.0));
    assert!(value.shapes[1].vertices.is_empty());
}

#[test]
fn engine_tank_inherits_engine_fuel_type_and_nonengine_static_children_are_ignored() {
    let value = part(
        r#"<PartType type="engine"><Engine fuelType="3"/><Tank fuel="9000.123456789" dryMass="3.15" fuelType="9"/><AttachPoints/></PartType>"#,
    );
    assert_eq!(value.fuel_capacity(), Some(9000.123456789));
    assert_eq!(value.tank.as_ref().unwrap().fuel_type, 3);
    let generic = part(
        r#"<PartType type="fuselage"><Tank fuel="7"/><Engine power="4"/><AttachPoints/></PartType>"#,
    );
    assert_eq!(generic.class, PartClass::Generic);
    assert!(generic.tank.is_none() && generic.engine.is_none());
    assert!(generic.raw.child("Tank").is_some());
}

#[test]
fn factory_is_exact_and_wheel_activation_has_legacy_id_exception() {
    assert_eq!(PartClass::from_type("Tank"), PartClass::Generic);
    assert_eq!(PartClass::from_type("strut"), PartClass::Generic);
    assert_eq!(PartClass::from_type("nosecone"), PartClass::Nosecone);
    assert_eq!(
        PartClass::from_type("dockconnector"),
        PartClass::DockConnector
    );
    for (id, stageable) in [("wheel-1", false), ("wheel-2", true)] {
        let source = format!(r#"<PartType id="{id}" type="wheel"><AttachPoints/></PartType>"#);
        assert_eq!(part(&source).stageable, stageable);
    }
}

#[test]
fn missing_required_native_children_return_explicit_errors_but_solar_stays_optional() {
    assert!(PartType::from_element(&xml::parse("<PartType/>").unwrap()).is_err());
    for kind in ["engine", "tank", "rcs", "lander"] {
        let source = format!(r#"<PartType type="{kind}"><AttachPoints/></PartType>"#);
        assert!(matches!(
            PartType::from_element(&xml::parse(&source).unwrap()),
            Err(Error::InvalidData(_))
        ));
    }
    let solar = part(r#"<PartType type="solar"><AttachPoints/></PartType>"#);
    assert!(solar.solar.is_none());
    assert!(solar.stageable);
    let solar = part(r#"<PartType type="solar"><Solar/><AttachPoints/></PartType>"#);
    assert_eq!(solar.solar.unwrap().charge_rate, 0.0);
}

#[test]
fn duplicate_ids_or_display_names_are_rejected_without_last_wins() {
    for second in [r#"id="a" name="B""#, r#"id="b" name="A""#] {
        let source = format!(
            r#"<PartTypes><PartType id="a" name="A"><AttachPoints/></PartType><PartType {second}><AttachPoints/></PartType></PartTypes>"#
        );
        assert!(matches!(
            Catalog::from_xml(&source),
            Err(Error::InvalidData(_))
        ));
    }
    assert!(Catalog::from_xml("<PartTypes/>").unwrap().parts.is_empty());
    assert!(matches!(
        Catalog::from_xml("<PartList/>"),
        Err(Error::WrongRoot { .. })
    ));
}

#[test]
fn sibling_walks_and_unconsumed_fields_are_preserved() {
    let source = r#"<PartTypes future="keep"><Ignored/><PartType id="a" name="A" future="part"><Shape><Ignored/><Vertex x="2"/><Other x="4"/></Shape><AttachPoints><Ignored/><AttachPoint/><Other/></AttachPoints><Future value="7"/></PartType><Other id="b" name="B"><AttachPoints/></Other></PartTypes>"#;
    let catalog = Catalog::from_xml(source).unwrap();
    assert_eq!(catalog.parts.len(), 2);
    let value = catalog.get("a").unwrap();
    assert_eq!(value.shapes[0].vertices, vec![(1.0, 0.0), (2.0, 0.0)]);
    assert_eq!(value.attach_points.len(), 2);
    assert_eq!(value.raw.attr("future"), Some("part"));
    assert_eq!(value.raw.child("Future").unwrap().attr("value"), Some("7"));
    assert_eq!(catalog.raw.attr("future"), Some("keep"));
    assert_eq!(catalog.get("b").unwrap().raw.name, "Other");
}
