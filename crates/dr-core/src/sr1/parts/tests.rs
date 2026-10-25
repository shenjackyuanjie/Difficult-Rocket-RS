use crate::sr1::parts::*;

fn catalog() -> Catalog {
    // PC PartList/DefaultShip 的字段缩小样本，不依赖本机游戏路径。
    Catalog::from_xml(r#"<PartTypes>
            <PartType id="pod-1" name="Pod" type="pod"><AttachPoints/></PartType>
            <PartType id="fueltank-2" name="Tank" type="tank"><Tank fuel="3000" dryMass="0.85"/><AttachPoints/></PartType>
            <PartType id="engine-1" name="Engine" type="engine"><Engine/><Tank fuel="12.125"/><AttachPoints/></PartType>
            <PartType id="solid-1" name="Solid" type="engine"><Engine/><AttachPoints/></PartType>
            <PartType id="lander-1" name="Lander" type="lander"><Lander minLength="2.5"/><AttachPoints/></PartType>
            <PartType id="parachute-1" name="Parachute" type="parachute"><AttachPoints/></PartType>
            <PartType id="solar-1" name="Solar" type="solar"><Solar/><AttachPoints/></PartType>
            <PartType id="strut-1" name="Strut" type="strut"><AttachPoints/></PartType>
        </PartTypes>"#).unwrap()
}

fn load(source: &str) -> Part {
    Part::from_xml(source, &catalog()).unwrap()
}

fn fuel(part: &Part) -> &FuelState {
    match &part.state {
        PartState::Tank(fuel) | PartState::Engine(fuel) => fuel,
        _ => panic!("非燃料状态"),
    }
}

#[test]
fn part_common_fields_keep_native_widths_prefixes_and_independent_angles() {
    let part = load(
        r#"<Part partType="strut-1" id="-9tail" x="16777217.25x" y="-2.5" angle="0.125" angleV="-0.25" editorAngle="3tail" flippedX="1" flippedY="true" activated="-2" exploded="bad"/>"#,
    );
    assert_eq!(part.id, -9);
    assert_eq!(part.x, 16777217.25);
    assert_eq!(part.y, -2.5);
    assert_eq!(part.angle, 0.125);
    assert_eq!(part.angle_v, -0.25);
    assert_eq!(part.editor_angle, 3);
    assert!(part.flipped_x && part.flipped_y && part.active);
    assert!(!part.exploded);
    let saved = part.to_element();
    assert_eq!(saved.attr("angle"), Some("0.125000"));
    assert_eq!(saved.attr("editorAngle"), Some("3"));
    assert_eq!(saved.attr("activated"), Some("1"));
}

#[test]
fn part_missing_or_bad_common_values_use_loader_zero() {
    let part =
        load(r#"<Part partType="strut-1" id="bad" x="bad" angleV="bad" editorAngle="bad"/>"#);
    assert_eq!((part.id, part.editor_angle), (0, 0));
    assert_eq!(
        (part.x, part.y, part.angle, part.angle_v),
        (0.0, 0.0, 0.0, 0.0)
    );
    assert!(!part.active && !part.exploded && !part.flipped_x && !part.flipped_y);
}

#[test]
fn fuel_missing_child_keeps_capacity_but_present_child_missing_value_loads_zero() {
    for (id, child, capacity) in [
        ("fueltank-2", "Tank", 3000.0),
        ("engine-1", "Engine", 12.125),
    ] {
        assert_eq!(
            fuel(&load(&format!("<Part partType=\"{id}\"/>"))).fuel,
            capacity
        );
        for value in ["", " fuel=\"bad\""] {
            let part = load(&format!("<Part partType=\"{id}\"><{child}{value}/></Part>"));
            assert_eq!(fuel(&part).fuel, 0.0);
            assert_eq!(fuel(&part).capacity, capacity);
        }
    }
}

#[test]
fn fuel_uses_f64_and_clips_only_the_matching_class_child() {
    for (input, expected) in [
        ("-1", 0.0),
        ("3001", 3000.0),
        ("2999.9999999", 2999.9999999),
    ] {
        let part = load(&format!(
            "<Part partType=\"fueltank-2\"><Tank fuel=\"{input}\"/></Part>"
        ));
        assert_eq!(fuel(&part).fuel, expected);
    }
    let part = load(
        r#"<Part partType="fueltank-2"><Engine fuel="1"/><Tank fuel="7.125"/><Tank fuel="9"/></Part>"#,
    );
    assert_eq!(fuel(&part).fuel, 7.125);
    let part = load(r#"<Part partType="solid-1"><Engine fuel="100"/></Part>"#);
    assert_eq!(fuel(&part).fuel, 0.0);
    assert_eq!(clamp_loaded_fuel(1.0, -1.0), 0.0);
    assert_eq!(clamp_loaded_fuel(f64::NAN, 10.0), 10.0);
    assert_eq!(clamp_loaded_fuel(1.0, f64::NAN), 0.0);
}

#[test]
fn pod_and_tank_skip_base_fields_while_engine_loads_them() {
    for id in ["pod-1", "fueltank-2", "engine-1"] {
        let part = load(&format!(
            "<Part partType=\"{id}\" activated=\"1\" exploded=\"1\" flippedX=\"1\" flippedY=\"1\"/>"
        ));
        let reads_base = id == "engine-1";
        assert_eq!(part.active, reads_base);
        assert_eq!(part.exploded, reads_base);
        assert_eq!(part.flipped_x, reads_base);
        assert_eq!(part.flipped_y, reads_base);
        assert_eq!(part.to_element().attr("activated").is_some(), reads_base);
    }
}

#[test]
fn throttle_uses_promoted_float_thresholds_and_strict_inequalities() {
    let low = THROTTLE_ZERO_THRESHOLD;
    let high = THROTTLE_FULL_THRESHOLD;
    assert_eq!(normalize_throttle(low), low);
    assert_eq!(normalize_throttle(high), high);
    assert_eq!(normalize_throttle(f64::from_bits(low.to_bits() - 1)), 0.0);
    assert_eq!(normalize_throttle(f64::from_bits(high.to_bits() + 1)), 1.0);
    assert_eq!(normalize_throttle(0.99), 0.99);
    assert_eq!(normalize_throttle(-1.0), 0.0);
    assert_eq!(normalize_throttle(10.0), 1.0);
    assert!(normalize_throttle(f64::NAN).is_nan());
    let part = load(r#"<Part partType="pod-1"><Pod throttle="0.990000005" name="名字"/></Part>"#);
    assert_eq!(part.pod().unwrap().throttle, 0.990000005);
}

#[test]
fn pod_staging_presence_supports_migration_and_save_always_writes_a_table() {
    let mut part = load(r#"<Part partType="pod-1"/>"#);
    assert!(part.pod().unwrap().staging.is_none());
    let saved = part.to_element();
    let pod = saved.child("Pod").unwrap();
    assert_eq!(pod.attr("throttle"), Some("0.000000"));
    assert_eq!(
        pod.child("Staging").unwrap().attr("currentStage"),
        Some("0")
    );
    let staged = load(
        r#"<Part partType="pod-1"><Pod><Staging currentStage="7"><Step><Activate Id="-1" moved="0"/></Step></Staging></Pod></Part>"#,
    );
    let staging = staged.pod().unwrap().staging.as_ref().unwrap();
    assert_eq!(staging.current_stage, 7);
    assert_eq!(staging.steps[0].parts[0].part_id, -1);
    assert!(staging.steps[0].parts[0].moved);
    part.pod_mut().unwrap().name = "测试".into();
    assert_eq!(
        part.to_element().child("Pod").unwrap().attr("name"),
        Some("测试")
    );
}

#[test]
fn parachute_load_overrides_constructor_and_preserves_mixed_float_widths() {
    assert_eq!(ParachuteState::default().inflation, 0.1_f32);
    let empty = load(r#"<Part partType="parachute-1"/>"#);
    let PartState::Parachute(empty) = empty.state else {
        panic!("非降落伞")
    };
    assert_eq!(empty.inflation, 0.0);
    let part = load(
        r#"<Part partType="parachute-1" chuteX="16777217" chuteY="-1.25" chuteAngle="16777217" chuteHeight="-3" inflation="1.25" inflate="true" deployed="1" rope="-1"/>"#,
    );
    let PartState::Parachute(chute) = &part.state else {
        panic!("非降落伞")
    };
    assert_eq!(chute.chute_x, 16777217.0);
    assert_eq!(chute.chute_angle, 16777216.0_f32);
    assert_eq!(chute.chute_height, -3.0);
    assert_eq!(chute.inflation, 1.25);
    assert!(chute.inflate && chute.deployed && chute.rope);
    let saved = part.to_element();
    assert_eq!(saved.attr("chuteX"), Some("16777217.000000"));
    assert_eq!(saved.attr("chuteAngle"), Some("16777216.000000"));
}

#[test]
fn lander_load_and_save_all_four_fields_without_clamping_or_mirror_rewrite() {
    assert_eq!(LanderState::new(2.5).length, 2.5);
    let empty = load(r#"<Part partType="lander-1"/>"#);
    let PartState::Lander(empty) = empty.state else {
        panic!("非Lander")
    };
    assert_eq!(empty.length, 0.0);
    let part = load(
        r#"<Part partType="lander-1" lower="1" raise="true" length="-2.125" legAngle="16777217" flippedX="true"/>"#,
    );
    let PartState::Lander(lander) = &part.state else {
        panic!("非Lander")
    };
    assert_eq!(lander.length, -2.125);
    assert_eq!(lander.leg_angle, 16777216.0_f32);
    let saved = part.to_element();
    assert_eq!(saved.attr("lower"), Some("1"));
    assert_eq!(saved.attr("raise"), Some("1"));
    assert_eq!(saved.attr("length"), Some("-2.125000"));
    assert_eq!(saved.attr("legAngle"), Some("16777216.000000"));
    assert_eq!(saved.attr("flippedX"), Some("1"));
}

#[test]
fn solar_extension_is_a_narrowed_double_and_is_not_clamped() {
    let part = load(r#"<Part partType="solar-1" extension="16777217"/>"#);
    let PartState::Solar(solar) = &part.state else {
        panic!("非Solar")
    };
    assert_eq!(solar.extension, 16777216.0_f32);
    assert_eq!(part.to_element().attr("extension"), Some("16777216.000000"));
    let part = load(r#"<Part partType="solar-1" extension="-2.5"/>"#);
    assert_eq!(part.to_element().attr("extension"), Some("-2.500000"));
}

#[test]
fn source_retains_unused_data_but_native_save_does_not_copy_it() {
    let part =
        load(r#"<Part partType="strut-1" unused="42"><Tank fuel="100"/><Mod key="保留"/></Part>"#);
    assert_eq!(part.source_element().attr("unused"), Some("42"));
    assert!(part.source_element().child("Mod").is_some());
    assert!(part.to_element().attr("unused").is_none());
    assert!(part.to_element().children.is_empty());
}

#[test]
fn direct_float_scan_differs_from_double_scan_then_narrowing_at_overflow_midpoint() {
    // 同版 CRT 的 %f 金样：直接扫描是 MAX，先 %lf 后窄化则为无穷大。
    let value = "3.4028235677973366e38";
    let chute = load(&format!(
        "<Part partType=\"parachute-1\" chuteAngle=\"{value}\"/>"
    ));
    let PartState::Parachute(chute) = chute.state else {
        panic!("非降落伞")
    };
    assert_eq!(chute.chute_angle.to_bits(), f32::MAX.to_bits());
    let solar = load(&format!(
        "<Part partType=\"solar-1\" extension=\"{value}\"/>"
    ));
    let PartState::Solar(solar) = solar.state else {
        panic!("非Solar")
    };
    assert_eq!(solar.extension, f32::INFINITY);
}

#[test]
fn factory_reports_wrong_root_and_unresolved_type() {
    assert!(matches!(
        Part::from_xml("<Ship/>", &catalog()),
        Err(Error::WrongRoot { .. })
    ));
    assert!(matches!(
        Part::from_xml("<Part partType=\"missing\"/>", &catalog()),
        Err(Error::InvalidData(_))
    ));
}
