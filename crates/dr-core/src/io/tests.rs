use crate::io::*;

#[test]
fn rejects_wrong_root_trailing_document_and_unmodelled_fields() {
    for input in [
        "<PartTypes/>",
        "<Ship/><Ship/>",
        "<Ship/>garbage",
        "<Ship>",
        "<Ship futureField=\"1\"/>",
        "<Ship><Unknown/></Ship>",
        "<Ship><Parts><Part id=\"1\" futureField=\"1\"/></Parts></Ship>",
        "<Ship><Parts><Part id=\"1\"><Tank fuel=\"2\"/><Engine fuel=\"3\"/></Part></Parts></Ship>",
        "<Ship/>&#65;",
        "<Ship/>&amp;",
    ] {
        assert!(ship_from_xml(input).is_err(), "错误输入被静默接受：{input}");
    }
    assert!(ship_from_xml("<?xml version=\"1.0\"?><!-- 注释 --><Ship/><!-- 结尾 -->").is_ok());
    // 根外的 ASCII 空白仍然允许，非 ASCII 空白仍按内容拒绝。
    assert!(ship_from_xml("\n  <Ship/>\t\r\n").is_ok());
    assert!(ship_from_xml("<Ship/>\u{3000}").is_err());
}

#[test]
fn resolves_character_references_inside_root() {
    let ship = ship_from_xml(
            r#"<Ship name="A&#65;&#x42;" description="p&#45;1"><Parts><Part id="1" partType="pod&#45;1"/></Parts></Ship>"#,
        )
        .unwrap();
    assert_eq!(ship.name, "AAB");
    assert_eq!(ship.description, "p-1");
    assert_eq!(ship.parts[0].part_type, "pod-1");
}

#[test]
fn preserves_attribute_whitespace_through_roundtrip() {
    let ship = Ship {
        name: "甲\r\n\t乙".into(),
        description: "回车\r换行\n制表\t".into(),
        ..Ship::default()
    };
    let xml = ship_to_xml(&ship).unwrap();
    // 属性值里的回车、换行、制表符要写成字符引用，否则读回时会被属性值空白归一化压成空格。
    for reference in ["&#13;", "&#10;", "&#9;"] {
        assert!(xml.contains(reference), "缺少 {reference}：{xml}");
    }
    assert_eq!(ship_from_xml(&xml).unwrap(), ship);
}

#[test]
fn rejects_deeply_nested_document_without_stack_overflow() {
    // 船体模型仍拒绝未知字段；另用可递归模型明确验证上游默认深度限制，避免只测到未知字段错误。
    #[derive(serde::Deserialize)]
    struct Nested {
        #[serde(rename = "Nested", default)]
        _nested: Option<Box<Nested>>,
    }
    assert!(quick_xml::de::from_str::<Nested>("<Ship><Nested/></Ship>").is_ok());
    let mut source = String::from("<Ship>");
    source.push_str(&"<Nested>".repeat(300));
    source.push_str(&"</Nested>".repeat(300));
    source.push_str("</Ship>");
    assert!(ship_from_xml(&source).is_err());
    assert!(matches!(
        quick_xml::de::from_str::<Nested>(&source),
        Err(quick_xml::DeError::TooDeeplyNested(128))
    ));
}

#[test]
fn preserves_all_lander_attributes() {
    let ship = ship_from_xml(r#"<Ship><Parts><Part id="1" partType="lander-1" lower="0" raise="1" length="2.26" legAngle="-10000000"/></Parts></Ship>"#).unwrap();
    assert_eq!(
        ship.parts[0].lander,
        LanderState {
            lower: Some(0),
            raise: Some(1),
            length: Some(2.26),
            leg_angle: Some(-10000000.0)
        }
    );
    let xml = ship_to_xml(&ship).unwrap();
    assert!(xml.contains("legAngle=\"-10000000\""));
    assert_eq!(ship_from_xml(&xml).unwrap(), ship);
}

#[test]
fn atomic_save_replaces_complete_file_and_leaves_no_temporary_files() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("船体.xml");
    fs::write(&path, "旧内容").unwrap();
    let ship = Ship::default();
    save_ship(&path, &ship).unwrap();
    assert_eq!(load_ship(&path).unwrap(), ship);
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
}

#[test]
fn failed_replacement_preserves_existing_target_and_cleans_up() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("不能替换的目录.xml");
    fs::create_dir(&path).unwrap();
    fs::write(path.join("原内容"), b"unchanged").unwrap();
    assert!(matches!(
        save_ship(&path, &Ship::default()),
        Err(CoreError::Write { .. })
    ));
    assert_eq!(fs::read(path.join("原内容")).unwrap(), b"unchanged");
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
}

#[test]
fn read_only_save_preserves_original_bytes() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("只读.xml");
    fs::write(&path, "不能丢失的原文件").unwrap();
    let original = fs::metadata(&path).unwrap().permissions();
    let mut read_only = original.clone();
    read_only.set_readonly(true);
    fs::set_permissions(&path, read_only).unwrap();
    let result = save_ship(&path, &Ship::default());
    fs::set_permissions(&path, original).unwrap();
    assert!(matches!(result, Err(CoreError::Write { .. })));
    assert_eq!(fs::read_to_string(&path).unwrap(), "不能丢失的原文件");
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
}
#[test]
fn preserves_disconnected_groups_and_parachutes() {
    let source = r#"<Ship><Parts><Part id="1" partType="pod-1"><Pod name="甲 &amp; 乙" throttle="0.5"><Staging currentStage="1"><Step/><Step><Activate Id="2" moved="1"/></Step></Staging></Pod></Part></Parts>
        <Connections/><DisconnectedParts>
        <DisconnectedPart><Parts><Part id="2" partType="parachute-1" chuteX="1" chuteY="2" chuteAngle="0.25" chuteHeight="5" inflation="0.1" inflate="1" rope="0" deployed="1" extension="0.5"/><Part id="3" partType="engine-1"><Engine fuel="2"/></Part></Parts><Connections><DockConnection dockPart="3" parentPart="2" childPart="3"/></Connections></DisconnectedPart>
        <DisconnectedPart><Parts><Part id="4" partType="tank"><Tank fuel="5"/></Part></Parts><Connections/></DisconnectedPart>
        </DisconnectedParts></Ship>"#;
    let ship = ship_from_xml(source).unwrap();
    assert_eq!(ship.version, 1);
    assert!(ship.touching_ground);
    assert_eq!(ship.disconnected.len(), 2);
    let chute = &ship.part(2).unwrap().parachute;
    assert_eq!(chute.x, Some(1.0));
    assert_eq!(chute.inflation, Some(0.1));
    assert_eq!(chute.rope, Some(0));
    assert_eq!(chute.deployed, Some(1));
    let saved = ship_to_xml(&ship).unwrap();
    assert!(saved.starts_with("<Ship "));
    assert!(saved.contains("<DisconnectedParts><DisconnectedPart>"));
    assert_eq!(ship, ship_from_xml(&saved).unwrap());
    assert_eq!(Ship::default(), ship_from_xml("<Ship/>").unwrap());
}

#[test]
fn parses_catalog_locations_and_specs() {
    let path = std::env::temp_dir().join("dr-core-part-list.xml");
    fs::write(
        &path,
        r#"<PartTypes>
                <PartType id="tank" type="tank" width="4" height="8" mass="2">
                    <Tank fuel="3000" dryMass="0.8" fuelType="1" />
                    <AttachPoints>
                        <AttachPoint location="TopCenter" fuelLine="true" group="2" />
                        <AttachPoint x="1" y="2" breakForce="5" />
                    </AttachPoints>
                </PartType>
            </PartTypes>"#,
    )
    .unwrap();
    let catalog = load_catalog(&path).unwrap();
    let part = catalog.get("tank").unwrap();
    assert_eq!(part.attach_points[0].y, 4.0);
    assert!(part.attach_points[0].fuel_line);
    assert_eq!(part.attach_points[0].group, Some(2));
    assert_eq!(part.tank.as_ref().unwrap().fuel, 3000.0);
    assert_eq!(part.attach_points[1].x, 1.0);
    assert_eq!(part.attach_points[1].break_force, Some(5.0));
    let _ = fs::remove_file(path);
}

#[test]
fn catalog_preserves_compound_shapes_and_sensor_attribute() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("shapes.xml");
    fs::write(
        &path,
        r#"<PartTypes><PartType id="compound" width="4" height="4">
            <Shape><Vertex x="-2" y="-2"/><Vertex x="2" y="-2"/><Vertex y="2"/></Shape>
            <Shape sensor="true"><Vertex/><Vertex x="1"/><Vertex y="1"/></Shape>
        </PartType></PartTypes>"#,
    )
    .unwrap();
    let catalog = load_catalog(&path).unwrap();
    let kind = catalog.get("compound").unwrap();
    assert_eq!(kind.shapes.len(), 2);
    assert_eq!(
        kind.shapes[0].vertices,
        vec![(-2.0, -2.0), (2.0, -2.0), (0.0, 2.0)]
    );
    assert!(!kind.shapes[0].sensor);
    assert!(kind.shapes[1].sensor);
    fs::write(&path, r#"<PartTypes><PartType id="broken"><Shape><Vertex/><Vertex x="1"/></Shape></PartType></PartTypes>"#).unwrap();
    assert!(
        load_catalog(&path)
            .unwrap_err()
            .to_string()
            .contains("Shape")
    );
}

#[test]
fn preserves_pod_and_engine_kind_on_roundtrip() {
    let input = std::env::temp_dir().join("dr-core-pod.xml");
    let output = std::env::temp_dir().join("dr-core-pod-out.xml");
    fs::write(
            &input,
            r#"<Ship version="1" liftedOff="0" touchingGround="1"><Parts>
                <Part partType="pod-1" id="1" x="0" y="0" angle="0" angleV="0" editorAngle="0">
                    <Pod throttle="0.5" name="test"><Staging currentStage="2"><Step><Activate Id="7" moved="1"/></Step></Staging></Pod>
                </Part>
                <Part partType="engine-1" id="2"><Engine fuel="4"/></Part>
            </Parts><Connections/></Ship>"#,
        )
        .unwrap();
    let ship = load_ship(&input).unwrap();
    assert_eq!(
        ship.parts[0]
            .pod
            .as_ref()
            .unwrap()
            .staging
            .as_ref()
            .unwrap()
            .current_stage,
        2
    );
    assert_eq!(ship.parts[1].fuel_kind, Some(FuelKind::Engine));
    save_ship(&output, &ship).unwrap();
    let saved = fs::read_to_string(&output).unwrap();
    assert!(saved.contains("<Engine fuel=\"4\""));
    assert!(saved.contains("<Pod throttle=\"0.5\""));
    let _ = fs::remove_file(input);
    let _ = fs::remove_file(output);
}
