
use super::*;

fn catalog() -> Catalog {
    Catalog::from_xml(r#"<PartTypes><PartType id="pod" name="Pod" type="pod"><AttachPoints><AttachPoint/><AttachPoint/></AttachPoints></PartType><PartType id="tank" name="Tank" type="tank"><Tank fuel="100"/><AttachPoints><AttachPoint/><AttachPoint/></AttachPoints></PartType><PartType id="dock" name="Dock" type="dockconnector"><AttachPoints><AttachPoint/><AttachPoint/></AttachPoints></PartType></PartTypes>"#).unwrap()
}

fn load(body: &str) -> Ship {
    Ship::from_xml(body, &catalog()).unwrap()
}

// 保留优化前的全边扫描算法作为独立参照，不调用出边索引构造器。
fn reference_walk(
    root: usize,
    part_count: usize,
    connections: &[Connection],
) -> Result<(Vec<usize>, Vec<usize>)> {
    enum Visit {
        Part(usize),
        Leave(usize),
        Edge(usize),
    }
    let mut stack = vec![Visit::Part(root)];
    let mut active = vec![false; part_count];
    let mut parts = Vec::new();
    let mut edges = Vec::new();
    while let Some(visit) = stack.pop() {
        match visit {
            Visit::Part(index) => {
                if active[index] {
                    return Err(Error::InvalidData(format!(
                        "PartTree 子树遍历在实例索引 {index} 遇到环"
                    )));
                }
                active[index] = true;
                parts.push(index);
                stack.push(Visit::Leave(index));
                for (edge, _) in connections
                    .iter()
                    .enumerate()
                    .rev()
                    .filter(|(_, connection)| connection.parent == index)
                {
                    stack.push(Visit::Edge(edge));
                }
            }
            Visit::Leave(index) => active[index] = false,
            Visit::Edge(edge) => {
                edges.push(edge);
                stack.push(Visit::Part(connections[edge].child));
            }
        }
    }
    Ok((parts, edges))
}

fn shared_subtree_source() -> Element {
    xml::parse(r#"<Ship currentStage="7">
            <Parts>
                <Part partType="tank" id="4"/>
                <Part partType="pod" id="2"><Pod><Staging currentStage="2">
                    <Step><Activate Id="1"/><Activate Id="2"/><Activate Id="3"/><Activate Id="4" moved="0"/><Activate Id="5"/><Activate Id="99"/></Step>
                    <Step><Activate Id="99"/></Step>
                </Staging></Pod></Part>
                <Part partType="pod" id="1"><Pod name="root"/></Part>
                <Part partType="pod" id="3"><Pod><Staging currentStage="3">
                    <Step><Activate Id="1"/><Activate Id="2"/><Activate Id="3"/><Activate Id="4" moved="0"/><Activate Id="5"/><Activate Id="99"/></Step>
                    <Step><Activate Id="99"/></Step>
                </Staging></Pod></Part>
                <Part partType="tank" id="5"/>
                <Part partType="tank" id="99"/>
            </Parts>
            <Connections>
                <Connection parentPart="2" childPart="4" parentAttachPoint="1" childAttachPoint="1"/>
                <Connection parentPart="1" childPart="3" parentAttachPoint="1" childAttachPoint="1"/>
                <Connection parentPart="3" childPart="4" parentAttachPoint="2" childAttachPoint="2"/>
                <Connection parentPart="1" childPart="2" parentAttachPoint="2" childAttachPoint="1"/>
                <Connection parentPart="4" childPart="5" parentAttachPoint="1" childAttachPoint="1"/>
            </Connections>
            <Staging>
                <Step><Activate Id="1"/><Activate Id="2"/><Activate Id="3"/><Activate Id="4"/><Activate Id="5"/><Activate Id="99"/></Step>
                <Step><Activate Id="99"/></Step>
            </Staging>
        </Ship>"#).unwrap()
}

#[test]
fn indexed_walk_matches_full_scan_for_small_graphs_and_duplicate_edges() {
    // 特意交错不同 parent；穷举三节点有向图，包括自环和不可达环。
    let candidates = [
        (2, 1),
        (0, 2),
        (1, 0),
        (1, 2),
        (2, 0),
        (0, 1),
        (2, 2),
        (0, 0),
        (1, 1),
    ];
    for mask in 0..(1 << candidates.len()) {
        for repeat_first in [false, true] {
            let mut connections = Vec::new();
            for (bit, &(parent, child)) in candidates.iter().enumerate() {
                if mask & (1 << bit) != 0 {
                    connections.push(Connection {
                        parent,
                        child,
                        kind: ConnectionKind::Normal {
                            parent_attach_point: 1,
                            child_attach_point: 1,
                        },
                    });
                }
            }
            if repeat_first && let Some(first) = connections.first().cloned() {
                connections.push(first);
            }
            let outgoing_edges = index_outgoing_edges(3, &connections);
            for root in 0..3 {
                assert_eq!(
                    walk(root, &connections, &outgoing_edges),
                    reference_walk(root, 3, &connections),
                    "mask={mask} repeat_first={repeat_first} root={root}",
                );
            }
        }
    }
}

#[test]
fn multiple_pods_share_subtree_without_leaking_staging_or_losing_visit_order() {
    let ship = Ship::from_element(&shared_subtree_source(), &catalog()).unwrap();
    assert_eq!(ship.tree.root_index(), Some(2));
    assert_eq!(ship.selected_pod_id(), Some(1));
    assert_eq!(
        ship.tree
            .traversal()
            .map(|part| part.id)
            .collect::<Vec<_>>(),
        [1, 3, 4, 5, 2, 4, 5]
    );
    assert_eq!(
        ship.tree
            .unreachable_parts()
            .map(|part| part.id)
            .collect::<Vec<_>>(),
        [99]
    );
    let expected_staging: &[(usize, i32, &[i32])] = &[
        (2, 7, &[1, 2, 3, 4, 5]),
        (1, 2, &[2, 4, 5]),
        (3, 3, &[3, 4, 5]),
    ];
    for &(index, current_stage, expected_ids) in expected_staging {
        let staging = ship.tree.parts()[index]
            .pod()
            .unwrap()
            .staging
            .as_ref()
            .unwrap();
        assert_eq!(staging.current_stage, current_stage);
        assert_eq!(
            staging.steps[0]
                .parts
                .iter()
                .map(|part| part.part_id)
                .collect::<Vec<_>>(),
            expected_ids
        );
        assert!(staging.steps[0].parts.iter().all(|part| part.moved));
        assert!(staging.steps[1].parts.is_empty());
    }
    let reference = reference_walk(
        ship.tree.root_index().unwrap(),
        ship.tree.parts().len(),
        ship.tree.connections(),
    )
    .unwrap();
    assert_eq!(
        (&ship.tree.part_order, &ship.tree.connection_order),
        (&reference.0, &reference.1)
    );
    let output = ship.to_element();
    assert_eq!(
        output
            .child("Parts")
            .unwrap()
            .children
            .iter()
            .map(|part| part.attr("id").unwrap())
            .collect::<Vec<_>>(),
        ["1", "3", "4", "5", "2", "4", "5"]
    );
    assert_eq!(
        output
            .child("Connections")
            .unwrap()
            .children
            .iter()
            .map(|connection| (
                connection.attr("parentPart").unwrap(),
                connection.attr("childPart").unwrap()
            ))
            .collect::<Vec<_>>(),
        [
            ("1", "3"),
            ("3", "4"),
            ("4", "5"),
            ("1", "2"),
            ("2", "4"),
            ("4", "5")
        ]
    );
}

#[test]
fn shared_subtree_cycle_is_detected_after_acyclic_last_parent_root_selection() {
    let mut source = shared_subtree_source();
    // 先写回边，后续 1→3 覆盖父指针：父链无环，出边遍历仍必须报环。
    source.child_mut("Connections").unwrap().children.insert(0, xml::parse(r#"<Connection parentPart="5" childPart="3" parentAttachPoint="2" childAttachPoint="1"/>"#).unwrap());
    let error = Ship::from_element(&source, &catalog()).unwrap_err();
    assert_eq!(
        error,
        Error::InvalidData("PartTree 子树遍历在实例索引 3 遇到环".into())
    );
}

#[test]
fn pod_deferred_load_still_detects_cycles_outside_main_root() {
    let mut source = shared_subtree_source();
    let parts = source.child_mut("Parts").unwrap();
    parts.children[5].set_attr("partType", "pod");
    parts
        .children
        .push(xml::parse(r#"<Part partType="tank" id="100"/>"#).unwrap());
    let connections = source.child_mut("Connections").unwrap();
    for child in [
        r#"<Connection parentPart="99" childPart="100" parentAttachPoint="1" childAttachPoint="1"/>"#,
        r#"<Connection parentPart="100" childPart="99" parentAttachPoint="2" childAttachPoint="2"/>"#,
    ] {
        connections.children.push(xml::parse(child).unwrap());
    }
    let error = Ship::from_element(&source, &catalog()).unwrap_err();
    assert_eq!(
        error,
        Error::InvalidData("PartTree 子树遍历在实例索引 5 遇到环".into())
    );
}

#[test]
fn native_ship_defaults_and_save_version_differ_from_strict_editor_contract() {
    let ship = load(
        r#"<Ship name="unused" currentStage="9"><Parts><Part partType="pod" id="1"/></Parts><Connections/></Ship>"#,
    );
    assert_eq!(ship.version, 0);
    assert!(!ship.lifted_off);
    assert!(!ship.touching_ground);
    let saved = ship.to_element();
    assert_eq!(saved.attr("version"), Some("1"));
    assert_eq!(saved.attr("liftedOff"), Some("0"));
    assert_eq!(saved.attr("touchingGround"), Some("0"));
    assert_eq!(saved.attr("name"), None);
    assert_eq!(saved.attr("currentStage"), None);
}

#[test]
fn legacy_staging_migrates_only_for_zero_version_and_does_not_overwrite_pod() {
    for version in ["", "version=\"0\"", "version=\"1\"", "version=\"-1\""] {
        let xml = format!(
            r#"<Ship {version} currentStage="4"><Parts><Part partType="pod" id="1"/></Parts><Connections/><Staging><Step><Activate Id="1"/><Activate Id="404"/></Step></Staging></Ship>"#
        );
        let ship = load(&xml);
        let staging = &ship.tree.root().unwrap().pod().unwrap().staging;
        if ship.version == 0 {
            let staging = staging.as_ref().unwrap();
            assert_eq!(staging.current_stage, 4);
            assert_eq!(staging.steps[0].parts.len(), 1);
        } else {
            assert!(staging.is_none());
        }
    }
    let ship = load(
        r#"<Ship currentStage="4"><Parts><Part partType="pod" id="1"><Pod><Staging currentStage="8"><Step/></Staging></Pod></Part></Parts><Connections/><Staging><Step><Activate Id="1"/></Step></Staging></Ship>"#,
    );
    let staging = ship
        .tree
        .root()
        .unwrap()
        .pod()
        .unwrap()
        .staging
        .as_ref()
        .unwrap();
    assert_eq!(staging.current_stage, 8);
    assert!(staging.steps[0].parts.is_empty());
}

#[test]
fn legacy_staging_uses_parent_chain_root_not_first_xml_part_or_first_pod() {
    let ship = load(
        r#"<Ship><Parts><Part partType="pod" id="2"/><Part partType="tank" id="1"/></Parts><Connections><Connection parentPart="1" childPart="2" parentAttachPoint="1" childAttachPoint="1"/></Connections><Staging><Step><Activate Id="2"/></Step></Staging></Ship>"#,
    );
    assert_eq!(ship.tree.root().unwrap().id, 1);
    assert_eq!(ship.selected_pod_id(), Some(2));
    assert!(ship.tree.parts()[0].pod().unwrap().staging.is_none());
    assert_eq!(
        ship.tree.traversal().map(|p| p.id).collect::<Vec<_>>(),
        [1, 2]
    );
}

#[test]
fn pod_staging_filters_against_its_own_subtree() {
    let ship = load(
        r#"<Ship version="1"><Parts><Part partType="pod" id="1"/><Part partType="pod" id="2"><Pod><Staging currentStage="12"><Step><Activate Id="1"/><Activate Id="2"/><Activate Id="3"/></Step><Step><Activate Id="99"/></Step></Staging></Pod></Part><Part partType="tank" id="3"/></Parts><Connections><Connection parentPart="1" childPart="2" parentAttachPoint="1" childAttachPoint="1"/><Connection parentPart="2" childPart="3" parentAttachPoint="2" childAttachPoint="1"/></Connections></Ship>"#,
    );
    let staging = ship.tree.parts()[1]
        .pod()
        .unwrap()
        .staging
        .as_ref()
        .unwrap();
    assert_eq!(staging.current_stage, 12);
    assert_eq!(
        staging.steps[0]
            .parts
            .iter()
            .map(|p| p.part_id)
            .collect::<Vec<_>>(),
        [2, 3]
    );
    assert!(staging.steps[1].parts.is_empty());
    assert_eq!(ship.selected_pod_id(), Some(1));
}

#[test]
fn invalid_attach_points_are_skipped_and_do_not_merge_isolated_parts() {
    let ship = load(
        r#"<Ship version="1"><Parts><Part partType="pod" id="1"/><Part partType="tank" id="2"/></Parts><Connections><Connection parentPart="1" childPart="2" parentAttachPoint="0" childAttachPoint="1"/></Connections></Ship>"#,
    );
    assert!(ship.tree.connections().is_empty());
    assert_eq!(ship.tree.parts().len(), 2);
    assert_eq!(
        ship.tree
            .unreachable_parts()
            .map(|p| p.id)
            .collect::<Vec<_>>(),
        [2]
    );
    assert_eq!(ship.to_element().child("Parts").unwrap().children.len(), 1);
}

#[test]
fn duplicate_part_ids_resolve_normal_endpoints_to_last_instances() {
    let ship = load(
        r#"<Ship version="1"><Parts><Part partType="pod" id="1"/><Part partType="tank" id="2"/><Part partType="pod" id="1"/><Part partType="tank" id="2"/></Parts><Connections><Connection parentPart="1" childPart="2" parentAttachPoint="1" childAttachPoint="2"/></Connections></Ship>"#,
    );
    let connection = &ship.tree.connections()[0];
    assert_eq!((connection.parent, connection.child), (2, 3));
    assert_eq!(ship.tree.root_index(), Some(0));
    assert_eq!(ship.tree.traversal().count(), 1);
}

#[test]
fn unknown_connection_names_use_dock_branch_and_dock_id_uses_first_match() {
    let ship = load(
        r#"<Ship version="1"><Parts><Part partType="pod" id="1"/><Part partType="tank" id="2"/><Part partType="dock" id="3"/><Part partType="dock" id="3"/></Parts><Connections><Other parentPart="1" childPart="2" dockPart="3"/></Connections></Ship>"#,
    );
    assert_eq!(
        ship.tree.connections()[0].kind,
        ConnectionKind::Dock { dock_part: 2 }
    );
    let output = ship.to_element();
    let connection = &output.child("Connections").unwrap().children[0];
    assert_eq!(connection.name, "DockConnection");
    assert_eq!(connection.attr("dockPart"), Some("3"));
    assert_eq!(connection.attr("parentPart"), Some("1"));
    assert_eq!(connection.attr("childPart"), Some("2"));
}

#[test]
fn dangling_references_and_cycles_return_explicit_errors() {
    let catalog = catalog();
    for source in [
        r#"<Ship><Parts/><Connections><Connection parentPart="1" childPart="2"/></Connections></Ship>"#,
        r#"<Ship><Parts><Part partType="pod" id="1"/><Part partType="tank" id="2"/></Parts><Connections><DockConnection parentPart="1" childPart="2" dockPart="404"/></Connections></Ship>"#,
        r#"<Ship><Parts><Part partType="pod" id="1"/><Part partType="tank" id="2"/></Parts><Connections><Connection parentPart="1" childPart="2" parentAttachPoint="1" childAttachPoint="1"/><Connection parentPart="2" childPart="1" parentAttachPoint="2" childAttachPoint="2"/></Connections></Ship>"#,
        r#"<Ship/>"#,
    ] {
        assert!(matches!(
            Ship::from_xml(source, &catalog),
            Err(Error::InvalidData(_))
        ));
    }
}

#[test]
fn editor_groups_keep_order_local_ids_and_prepend_container() {
    let xml = r#"<Ship><Parts><Part partType="pod" id="1"/></Parts><Connections/><DisconnectedParts><Ignored/><DisconnectedPart><Parts><Part partType="tank" id="1"/></Parts><Connections/></DisconnectedPart><Ignored/><DisconnectedPart><Parts><Part partType="pod" id="7"/></Parts><Connections/></DisconnectedPart></DisconnectedParts></Ship>"#;
    let catalog = catalog();
    assert!(
        Ship::from_xml(xml, &catalog)
            .unwrap()
            .disconnected
            .is_empty()
    );
    let ship = Ship::from_editor_xml(xml, &catalog).unwrap();
    assert_eq!(ship.disconnected.len(), 2);
    assert_eq!(ship.disconnected[0].root().unwrap().id, 1);
    assert_eq!(ship.disconnected[1].root().unwrap().id, 7);
    assert_eq!(ship.selected_pod_id(), Some(1));
    let output = ship.to_editor_element();
    assert_eq!(
        output
            .children
            .iter()
            .map(|e| e.name.as_str())
            .collect::<Vec<_>>(),
        ["DisconnectedParts", "Parts", "Connections"]
    );
    let groups = &output.children[0].children;
    assert_eq!(groups[0].name, "DisconnectedPart");
    assert_eq!(
        groups[0].child("Parts").unwrap().children[0].attr("id"),
        Some("1")
    );
    assert_eq!(
        groups[1].child("Parts").unwrap().children[0].attr("id"),
        Some("7")
    );
    assert!(ship.to_element().child("DisconnectedParts").is_none());
}

#[test]
fn native_tree_output_uses_depth_first_part_and_connection_order() {
    let ship = load(
        r#"<Ship version="1"><Parts><Part partType="tank" id="4"/><Part partType="tank" id="2"/><Part partType="pod" id="1"/><Part partType="tank" id="3"/></Parts><Connections><Connection parentPart="2" childPart="4" parentAttachPoint="1" childAttachPoint="1"/><Connection parentPart="1" childPart="2" parentAttachPoint="1" childAttachPoint="1"/><Connection parentPart="1" childPart="3" parentAttachPoint="2" childAttachPoint="1"/></Connections></Ship>"#,
    );
    assert_eq!(ship.tree.root_index(), Some(2));
    assert_eq!(
        ship.tree
            .traversal()
            .map(|part| part.id)
            .collect::<Vec<_>>(),
        [1, 2, 4, 3]
    );
    let output = ship.to_element();
    assert_eq!(
        output
            .child("Parts")
            .unwrap()
            .children
            .iter()
            .map(|part| part.attr("id").unwrap())
            .collect::<Vec<_>>(),
        ["1", "2", "4", "3"]
    );
    assert_eq!(
        output
            .child("Connections")
            .unwrap()
            .children
            .iter()
            .map(|connection| (
                connection.attr("parentPart").unwrap(),
                connection.attr("childPart").unwrap()
            ))
            .collect::<Vec<_>>(),
        [("1", "2"), ("2", "4"), ("1", "3")]
    );
}

#[test]
fn repeated_paths_are_not_silently_deduplicated_and_last_parent_selects_root() {
    let ship = load(
        r#"<Ship version="1"><Parts><Part partType="tank" id="3"/><Part partType="pod" id="1"/><Part partType="tank" id="2"/></Parts><Connections><Connection parentPart="1" childPart="3" parentAttachPoint="1" childAttachPoint="1"/><Connection parentPart="1" childPart="2" parentAttachPoint="2" childAttachPoint="1"/><Connection parentPart="2" childPart="3" parentAttachPoint="2" childAttachPoint="2"/></Connections></Ship>"#,
    );
    assert_eq!(ship.tree.root().unwrap().id, 1);
    assert_eq!(
        ship.tree
            .traversal()
            .map(|part| part.id)
            .collect::<Vec<_>>(),
        [1, 3, 2, 3]
    );
    assert_eq!(ship.to_element().child("Parts").unwrap().children.len(), 4);
}

#[test]
fn numeric_defaults_and_native_sibling_walk_are_applied_to_tree_items() {
    let ship = load(
        r#"<Ship version="bad" liftedOff="true" touchingGround="2tail"><Parts><Other partType="pod"/><Other partType="tank" id="-2tail"/></Parts><Connections><Connection parentPart="bad" childPart="-2" parentAttachPoint="1tail" childAttachPoint="2"/></Connections></Ship>"#,
    );
    assert_eq!(ship.version, 0);
    assert!(ship.lifted_off);
    assert!(ship.touching_ground);
    assert_eq!(
        ship.tree
            .traversal()
            .map(|part| part.id)
            .collect::<Vec<_>>(),
        [0, -2]
    );
    assert_eq!(
        ship.tree.connections()[0].kind,
        ConnectionKind::Normal {
            parent_attach_point: 1,
            child_attach_point: 2
        }
    );
}

#[test]
fn editor_disconnected_groups_use_ship_version_and_reject_cross_group_references() {
    let source = r#"<Ship currentStage="3"><Parts><Part partType="pod" id="1"/></Parts><Connections/><DisconnectedParts><DisconnectedPart currentStage="7"><Parts><Part partType="pod" id="1"/></Parts><Connections/><Staging><Step><Activate Id="1"/></Step></Staging></DisconnectedPart></DisconnectedParts></Ship>"#;
    let mut root = xml::parse(source).unwrap();
    let catalog = catalog();
    let ship = Ship::from_editor_element(&root, &catalog).unwrap();
    assert_eq!(
        ship.disconnected[0]
            .root()
            .unwrap()
            .pod()
            .unwrap()
            .staging
            .as_ref()
            .unwrap()
            .current_stage,
        7
    );
    root.set_attr("version", "1");
    let ship = Ship::from_editor_element(&root, &catalog).unwrap();
    assert!(
        ship.disconnected[0]
            .root()
            .unwrap()
            .pod()
            .unwrap()
            .staging
            .is_none()
    );
    let source = r#"<Ship version="1"><Parts><Part partType="pod" id="1"/></Parts><Connections/><DisconnectedParts><DisconnectedPart><Parts><Part partType="tank" id="2"/></Parts><Connections><Connection parentPart="1" childPart="2" parentAttachPoint="1" childAttachPoint="1"/></Connections></DisconnectedPart></DisconnectedParts></Ship>"#;
    assert!(matches!(
        Ship::from_editor_xml(source, &catalog),
        Err(Error::InvalidData(_))
    ));
}

#[test]
fn default_ship_resource_migrates_and_saves_pod_staging_canonically() {
    // 自 PC DefaultShip.xml 缩小空白；目录保留相应类型、容量及连接点次序。
    let catalog = Catalog::from_xml(r#"<PartTypes><PartType id="pod-1" name="Command Pod Mk1" type="pod"><AttachPoints><AttachPoint location="TopCenter"/><AttachPoint location="BottomCenter"/></AttachPoints></PartType><PartType id="fueltank-2" name="Sloshy T3000" type="tank"><Tank fuel="3000"/><AttachPoints><AttachPoint location="TopCenter"/><AttachPoint location="BottomCenter"/><AttachPoint location="LeftSide"/><AttachPoint location="RightSide"/></AttachPoints></PartType><PartType id="engine-2" name="Blasto 170" type="engine"><Engine power="2.0" consumption="50" size="0.8" turn="3.0"/><AttachPoints><AttachPoint location="TopCenter" fuelLine="true" order="1"/><AttachPoint location="BottomCenter" order="2"/></AttachPoints></PartType></PartTypes>"#).unwrap();
    let source = r#"<Ship currentStage="0" throttle="0.000000" liftedOff="0"><Parts><Part partType="pod-1" id="1" x="0.000000" y="0.750000" angle="0.000000" angleV="0.000000" activated="0" exploded="0"/><Part partType="fueltank-2" id="9" x="0.000000" y="-2.000000" angle="0.000000" angleV="0.000000"><Tank fuel="3000.000000"/></Part><Part partType="engine-2" id="10" x="0.000000" y="-5.500000" angle="0.000000" angleV="0.000000" activated="0" exploded="0"/></Parts><Connections><Connection parentPart="1" parentAttachPoint="2" childPart="9" childAttachPoint="1"/><Connection parentPart="9" parentAttachPoint="2" childPart="10" childAttachPoint="1"/></Connections><Staging><Step><Activate Id="10"/></Step></Staging></Ship>"#;
    let ship = Ship::from_xml(source, &catalog).unwrap();
    assert_eq!(
        ship.tree
            .traversal()
            .map(|part| part.id)
            .collect::<Vec<_>>(),
        [1, 9, 10]
    );
    assert_eq!(ship.selected_pod_id(), Some(1));
    let output = ship.to_element();
    assert_eq!(output.attr("version"), Some("1"));
    assert!(output.child("Staging").is_none());
    let staging = output.child("Parts").unwrap().children[0]
        .child("Pod")
        .unwrap()
        .child("Staging")
        .unwrap();
    assert_eq!(staging.children[0].children[0].attr("Id"), Some("10"));
    assert_eq!(staging.children[0].children[0].attr("moved"), Some("1"));
    let reloaded = Ship::from_element(&output, &catalog).unwrap();
    assert_eq!(reloaded.to_element(), output);
}

#[test]
fn empty_tree_and_wrong_root_are_explicit_non_crashing_boundaries() {
    let ship = load("<Ship><Parts/><Connections/></Ship>");
    assert!(ship.tree.root().is_none());
    assert!(ship.selected_pod_id().is_none());
    assert!(ship.tree.traversal().next().is_none());
    assert!(matches!(
        Ship::from_xml("<Runtime/>", &catalog()),
        Err(Error::WrongRoot {
            expected: "Ship",
            ..
        })
    ));
}

#[test]
fn editor_save_keeps_empty_disconnected_container_and_selected_pod_is_explicit() {
    let mut ship = load(
        r#"<Ship version="1"><Parts><Part partType="pod" id="1"/></Parts><Connections/></Ship>"#,
    );
    assert_eq!(
        ship.to_editor_element().children[0].name,
        "DisconnectedParts"
    );
    assert!(ship.select_pod(1));
    assert!(!ship.select_pod(404));
    assert_eq!(ship.selected_pod_id(), Some(1));
}
