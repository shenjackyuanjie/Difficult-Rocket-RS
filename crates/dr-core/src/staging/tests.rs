use crate::staging::*;
use crate::{
    EditorCommand, EditorHistory, FuelKind, Part, ShipGroup, catalog_from_xml, ship_from_xml,
    ship_to_xml,
};

fn catalog() -> PartCatalog {
    catalog_from_xml(
        r#"<PartTypes name="分级测试">
            <PartType id="pod" type="pod" hidden="true"/>
            <PartType id="tank" type="tank"><Tank fuel="20"/></PartType>
            <PartType id="engine" type="engine"><Engine/></PartType>
            <PartType id="engine-4" type="engine"><Engine/><Tank fuel="12"/></PartType>
            <PartType id="ion-0" type="engine"><Engine/></PartType>
            <PartType id="detacher" type="detacher"/>
            <PartType id="side-detacher" type="detacher"/>
            <PartType id="parachute" type="parachute"/>
            <PartType id="rcs" type="rcs"/>
            <PartType id="solar" type="solar"/>
            <PartType id="lander" type="lander"/>
            <PartType id="wheel-1" type="wheel"/>
        </PartTypes>"#,
    )
    .unwrap()
}

fn part(catalog: &PartCatalog, id: PartId, kind: &str) -> Part {
    catalog.get(kind).unwrap().instantiate(id, (0.0, 0.0))
}

fn connection(parent: PartId, child: PartId) -> Connection {
    Connection::Normal {
        parent,
        child,
        parent_attach: 0,
        child_attach: 1,
    }
}

fn basic_ship(catalog: &PartCatalog) -> Ship {
    Ship {
        parts: vec![part(catalog, 1, "pod"), part(catalog, 2, "engine")],
        connections: vec![connection(1, 2)],
        ..Ship::default()
    }
}

fn three_stage_ship(catalog: &PartCatalog) -> Ship {
    Ship {
        parts: vec![
            part(catalog, 1, "pod"),
            part(catalog, 2, "tank"),
            part(catalog, 3, "engine"),
            part(catalog, 4, "detacher"),
            part(catalog, 5, "tank"),
            part(catalog, 6, "engine"),
            part(catalog, 7, "detacher"),
            part(catalog, 8, "tank"),
            part(catalog, 9, "engine"),
            part(catalog, 10, "parachute"),
        ],
        connections: (1..9)
            .map(|id| connection(id, id + 1))
            .chain([connection(1, 10)])
            .collect(),
        ..Ship::default()
    }
}

fn stage_ids(staging: &StagingState) -> Vec<Vec<PartId>> {
    staging
        .steps
        .iter()
        .map(|step| {
            step.activations
                .iter()
                .map(|activation| activation.id)
                .collect()
        })
        .collect()
}

fn assert_automatic(staging: &StagingState) {
    assert_eq!(staging.current_stage, 0);
    let mut seen = HashSet::new();
    for step in &staging.steps {
        assert!(!step.activations.is_empty());
        for activation in &step.activations {
            assert!(!activation.moved);
            assert!(seen.insert(activation.id));
        }
    }
}

fn assert_roundtrip(ship: &Ship) {
    let xml = ship_to_xml(ship).unwrap();
    assert_eq!(ship_from_xml(&xml).unwrap(), *ship);
}

#[test]
fn three_propulsion_stages_follow_full_tree_depth_in_execution_order() {
    let catalog = catalog();
    let ship = three_stage_ship(&catalog);
    let before = ship.clone();
    let staging = generate_basic_staging(&ship, &catalog, PartKey::new(0, 1, 0)).unwrap();
    assert_eq!(
        stage_ids(&staging),
        vec![vec![9], vec![7], vec![6], vec![4], vec![3], vec![10]]
    );
    assert_automatic(&staging);
    assert_eq!(ship, before);
}

#[test]
fn every_connection_adds_depth_without_forcing_parachutes_to_the_end() {
    let catalog = catalog();
    let ship = Ship {
        parts: vec![
            part(&catalog, 1, "pod"),
            part(&catalog, 2, "engine"),
            part(&catalog, 3, "tank"),
            part(&catalog, 4, "engine"),
            part(&catalog, 5, "parachute"),
        ],
        connections: vec![
            connection(1, 2),
            connection(1, 3),
            connection(3, 4),
            connection(4, 5),
        ],
        ..Ship::default()
    };
    let staging = generate_basic_staging(&ship, &catalog, PartKey::new(0, 1, 0)).unwrap();
    assert_eq!(stage_ids(&staging), vec![vec![5], vec![4], vec![2]]);
    assert_automatic(&staging);
}

#[test]
fn equal_depth_groups_by_kind_then_sorts_ids_including_srb_and_ion() {
    let catalog = catalog();
    let mut ship = Ship {
        parts: vec![
            part(&catalog, 1, "pod"),
            part(&catalog, 2, "tank"),
            part(&catalog, 30, "ion-0"),
            part(&catalog, 25, "engine"),
            part(&catalog, 20, "engine-4"),
            part(&catalog, 15, "side-detacher"),
            part(&catalog, 14, "detacher"),
            part(&catalog, 10, "parachute"),
        ],
        connections: vec![
            connection(1, 2),
            connection(2, 30),
            connection(2, 25),
            connection(2, 20),
            connection(2, 15),
            connection(2, 14),
            connection(2, 10),
        ],
        ..Ship::default()
    };
    assert_eq!(ship.part(20).unwrap().fuel_kind, Some(FuelKind::Tank));
    ship.part_mut(2).unwrap().fuel_kind = Some(FuelKind::Engine);
    ship.part_mut(30).unwrap().fuel_kind = None;
    let staging = generate_basic_staging(&ship, &catalog, PartKey::new(0, 1, 0)).unwrap();
    assert_eq!(
        stage_ids(&staging),
        vec![vec![20, 25, 30], vec![14, 15], vec![10]]
    );
    assert_automatic(&staging);
}

#[test]
fn endpoints_storage_order_and_geometry_do_not_change_staging() {
    let catalog = catalog();
    let mut ship = three_stage_ship(&catalog);
    let expected = generate_basic_staging(&ship, &catalog, PartKey::new(0, 1, 0)).unwrap();
    ship.parts.reverse();
    ship.connections.reverse();
    for connection in &mut ship.connections {
        if let Connection::Normal {
            parent,
            child,
            parent_attach,
            child_attach,
        } = connection
        {
            std::mem::swap(parent, child);
            std::mem::swap(parent_attach, child_attach);
        }
    }
    for part in &mut ship.parts {
        part.x = part.id as f64 * 7.0;
        part.y = -(part.id as f64) * 11.0;
        part.angle = 1.25;
        part.editor_angle = 3;
        part.flip_x = true;
        part.flip_y = true;
    }
    assert_eq!(
        generate_basic_staging(&ship, &catalog, PartKey::new(0, 1, 0)).unwrap(),
        expected
    );
}

#[test]
fn catalog_kind_is_read_fresh_without_an_id_or_fuel_heuristic() {
    let mut catalog = catalog();
    let ship = basic_ship(&catalog);
    let pod = PartKey::new(0, 1, 0);
    assert_eq!(
        stage_ids(&generate_basic_staging(&ship, &catalog, pod).unwrap()),
        vec![vec![2]]
    );
    catalog
        .types
        .iter_mut()
        .find(|part| part.id == "engine")
        .unwrap()
        .kind = PartKind::Tank;
    assert_eq!(
        generate_basic_staging(&ship, &catalog, pod).unwrap(),
        StagingState::default()
    );
}

#[test]
fn groups_and_disconnected_bad_components_are_out_of_scope() {
    let catalog = catalog();
    let mut ship = basic_ship(&catalog);
    ship.parts.extend([
        part(&catalog, 100, "pod"),
        part(&catalog, 99, "tank"),
        part(&catalog, 99, "tank"),
    ]);
    ship.parts.last_mut().unwrap().part_type = "不在目录".into();
    ship.connections.extend([
        connection(500, 600),
        connection(100, 100),
        Connection::Dock {
            dock: 99,
            parent: 100,
            child: 101,
        },
    ]);
    ship.disconnected.push(ShipGroup {
        parts: vec![
            part(&catalog, 1, "pod"),
            part(&catalog, 2, "engine"),
            part(&catalog, 3, "detacher"),
        ],
        connections: vec![connection(1, 2), connection(2, 3)],
    });
    ship.disconnected.push(ShipGroup {
        parts: vec![part(&catalog, 1, "pod")],
        connections: vec![connection(1, 404)],
    });
    assert_eq!(
        stage_ids(&generate_basic_staging(&ship, &catalog, PartKey::new(0, 1, 0)).unwrap()),
        vec![vec![2]]
    );
    assert_eq!(
        stage_ids(&generate_basic_staging(&ship, &catalog, PartKey::new(1, 1, 0)).unwrap()),
        vec![vec![3], vec![2]]
    );
    assert_eq!(
        generate_basic_staging(&ship, &catalog, PartKey::new(2, 1, 0)),
        Err(StagingError::MissingPart { group: 2, id: 404 })
    );
}

#[test]
fn empty_plan_and_single_candidates_are_valid() {
    let catalog = catalog();
    let mut ship = Ship {
        parts: vec![part(&catalog, 1, "pod")],
        ..Ship::default()
    };
    for (id, kind) in [
        (2, "tank"),
        (3, "rcs"),
        (4, "solar"),
        (5, "lander"),
        (6, "wheel-1"),
    ] {
        ship.parts.push(part(&catalog, id, kind));
        ship.connections.push(connection(1, id));
    }
    assert_eq!(
        generate_basic_staging(&ship, &catalog, PartKey::new(0, 1, 0)).unwrap(),
        StagingState::default()
    );
    for kind in ["engine", "parachute"] {
        ship.parts.push(part(&catalog, 7, kind));
        ship.connections.push(connection(1, 7));
        let staging = generate_basic_staging(&ship, &catalog, PartKey::new(0, 1, 0)).unwrap();
        assert_eq!(stage_ids(&staging), vec![vec![7]]);
        assert_automatic(&staging);
        ship.parts.pop();
        ship.connections.pop();
    }
}

#[test]
fn missing_or_non_pod_selection_has_an_explicit_error() {
    let catalog = catalog();
    let mut ship = basic_ship(&catalog);
    for key in [
        PartKey::new(99, 1, 0),
        PartKey::new(0, 404, 0),
        PartKey::new(0, 1, 1),
    ] {
        assert_eq!(
            generate_basic_staging(&ship, &catalog, key),
            Err(StagingError::PodNotFound(key))
        );
    }
    let engine = PartKey::new(0, 2, 0);
    assert_eq!(
        generate_basic_staging(&ship, &catalog, engine),
        Err(StagingError::NotPod(engine))
    );
    ship.part_mut(1).unwrap().pod = None;
    let pod = PartKey::new(0, 1, 0);
    assert_eq!(
        generate_basic_staging(&ship, &catalog, pod),
        Err(StagingError::NotPod(pod))
    );
}

#[test]
fn missing_catalog_entries_are_reported_for_root_and_reachable_parts() {
    let catalog = catalog();
    let pod = PartKey::new(0, 1, 0);
    for id in [1, 2] {
        let mut ship = basic_ship(&catalog);
        ship.part_mut(id).unwrap().part_type = "未知类型".into();
        let before = ship.clone();
        assert_eq!(
            generate_basic_staging(&ship, &catalog, pod),
            Err(StagingError::MissingPartType {
                part: PartKey::new(0, id, 0),
                part_type: "未知类型".into(),
            })
        );
        assert_eq!(ship, before);
    }
}

#[test]
fn ambiguous_and_missing_references_are_not_guessed() {
    let catalog = catalog();
    let pod = PartKey::new(0, 1, 0);
    for id in [1, 2] {
        let mut ship = basic_ship(&catalog);
        ship.parts.push(ship.part(id).unwrap().clone());
        assert_eq!(
            generate_basic_staging(&ship, &catalog, pod),
            Err(StagingError::AmbiguousPart { group: 0, id })
        );
    }
    let mut ship = basic_ship(&catalog);
    ship.connections.push(connection(2, 404));
    assert_eq!(
        generate_basic_staging(&ship, &catalog, pod),
        Err(StagingError::MissingPart { group: 0, id: 404 })
    );
}

#[test]
fn docking_is_rejected_when_any_of_its_three_roles_touches_the_component() {
    let catalog = catalog();
    for connection in [
        Connection::Dock {
            parent: 1,
            child: 3,
            dock: 4,
        },
        Connection::Dock {
            parent: 3,
            child: 1,
            dock: 4,
        },
        Connection::Dock {
            parent: 3,
            child: 4,
            dock: 1,
        },
    ] {
        let mut ship = basic_ship(&catalog);
        ship.connections.push(connection);
        assert_eq!(
            generate_basic_staging(&ship, &catalog, PartKey::new(0, 1, 0)),
            Err(StagingError::UnsupportedDock {
                group: 0,
                connection: 1
            })
        );
    }
}

#[test]
fn cycles_self_loops_and_parallel_edges_are_rejected() {
    let catalog = catalog();
    for extra in [connection(1, 1), connection(1, 2), connection(2, 1)] {
        let mut ship = basic_ship(&catalog);
        ship.connections.push(extra);
        assert_eq!(
            generate_basic_staging(&ship, &catalog, PartKey::new(0, 1, 0)),
            Err(StagingError::Cycle {
                group: 0,
                connection: 1
            })
        );
    }
    let mut ship = basic_ship(&catalog);
    ship.parts.push(part(&catalog, 3, "tank"));
    ship.connections
        .extend([connection(2, 3), connection(3, 1)]);
    assert_eq!(
        generate_basic_staging(&ship, &catalog, PartKey::new(0, 1, 0)),
        Err(StagingError::Cycle {
            group: 0,
            connection: 1
        })
    );
}

#[test]
fn another_pod_in_the_component_requires_manual_staging() {
    let catalog = catalog();
    let mut ship = basic_ship(&catalog);
    ship.parts[1] = part(&catalog, 2, "pod");
    assert_eq!(
        generate_basic_staging(&ship, &catalog, PartKey::new(0, 1, 0)),
        Err(StagingError::MultiplePods {
            pod: PartKey::new(0, 1, 0),
            other: PartKey::new(0, 2, 0),
        })
    );
}

#[test]
fn generated_plan_uses_one_existing_transaction_and_preserves_legacy_undo_xml() {
    let catalog = catalog();
    let mut ship = three_stage_ship(&catalog);
    let pod = PartKey::new(0, 1, 0);
    let legacy = StagingState {
        current_stage: 42,
        steps: vec![
            StageStep::default(),
            StageStep {
                activations: vec![Activation { id: 3, moved: true }],
            },
            StageStep {
                activations: vec![Activation {
                    id: 3,
                    moved: false,
                }],
            },
        ],
    };
    ship.part_at_mut(pod).unwrap().pod.as_mut().unwrap().staging = Some(legacy.clone());
    let before = ship.clone();
    assert_roundtrip(&before);
    let generated = generate_basic_staging(&ship, &catalog, pod).unwrap();
    assert_eq!(ship, before);
    let command = EditorCommand::SetStaging(pod.id, Some(generated.clone())).at(pod);
    let mut history = EditorHistory::default();
    history
        .execute_with_catalog(&mut ship, &catalog, command.clone())
        .unwrap();
    let after = ship.clone();
    let mut expected = before.clone();
    expected
        .part_at_mut(pod)
        .unwrap()
        .pod
        .as_mut()
        .unwrap()
        .staging = Some(generated.clone());
    assert_eq!(ship, expected);
    assert_eq!(history.undo_len(), 1);
    assert_roundtrip(&ship);

    // 旧计划不影响重新计算；相同结果是 no-op，不新增历史。
    let regenerated = generate_basic_staging(&ship, &catalog, pod).unwrap();
    assert_eq!(regenerated, generated);
    history
        .execute_with_catalog(
            &mut ship,
            &catalog,
            EditorCommand::SetStaging(pod.id, Some(regenerated)).at(pod),
        )
        .unwrap();
    assert_eq!(history.undo_len(), 1);
    assert!(history.undo(&mut ship));
    assert_eq!(ship, before);
    assert_eq!(history.redo_len(), 1);

    // 通用入口仍接受旧的空步骤、跨步骤重复、超出长度的进度；no-op 保留 redo。
    history
        .execute_with_catalog(
            &mut ship,
            &catalog,
            EditorCommand::SetStaging(pod.id, Some(legacy)).at(pod),
        )
        .unwrap();
    assert_eq!(ship, before);
    assert_eq!(history.undo_len(), 0);
    assert_eq!(history.redo_len(), 1);

    // 批处理中后续的非法计划失败，前面的重命名也不落地、不清 redo。
    let mut invalid = generated;
    let duplicate = invalid.steps[0].activations[0].clone();
    invalid.steps[0].activations.push(duplicate);
    let batch = EditorCommand::Batch(vec![
        EditorCommand::RenamePod(pod.id, "不应落地".into()).at(pod),
        EditorCommand::SetStaging(pod.id, Some(invalid)).at(pod),
    ]);
    assert!(
        history
            .execute_with_catalog(&mut ship, &catalog, batch)
            .is_err()
    );
    assert_eq!(ship, before);
    assert_eq!(history.redo_len(), 1);
    assert!(history.redo(&mut ship));
    assert_eq!(ship, after);
}

#[test]
fn scoped_generated_plan_only_updates_its_pod_and_roundtrips() {
    let catalog = catalog();
    let mut ship = basic_ship(&catalog);
    ship.disconnected.push(ShipGroup {
        parts: vec![part(&catalog, 1, "pod"), part(&catalog, 2, "parachute")],
        connections: vec![connection(2, 1)],
    });
    let before = ship.clone();
    let pod = PartKey::new(1, 1, 0);
    let staging = generate_basic_staging(&ship, &catalog, pod).unwrap();
    let mut history = EditorHistory::default();
    history
        .execute_with_catalog(
            &mut ship,
            &catalog,
            EditorCommand::SetStaging(1, Some(staging.clone())).at(pod),
        )
        .unwrap();
    assert_eq!(ship.parts, before.parts);
    assert_eq!(ship.connections, before.connections);
    assert_eq!(
        ship.part_at(pod).unwrap().pod.as_ref().unwrap().staging,
        Some(staging)
    );
    assert_roundtrip(&ship);
    assert!(history.undo(&mut ship));
    assert_eq!(ship, before);
}

#[test]
fn calculation_failure_leaves_ship_and_existing_redo_untouched() {
    let catalog = catalog();
    let mut ship = basic_ship(&catalog);
    ship.connections.push(connection(2, 1));
    let mut history = EditorHistory::default();
    history
        .execute_with_catalog(&mut ship, &catalog, EditorCommand::SetActive(2, true))
        .unwrap();
    assert!(history.undo(&mut ship));
    let before = ship.clone();
    assert!(matches!(
        generate_basic_staging(&ship, &catalog, PartKey::new(0, 1, 0)),
        Err(StagingError::Cycle { .. })
    ));
    assert_eq!(ship, before);
    assert_eq!(history.undo_len(), 0);
    assert_eq!(history.redo_len(), 1);
}
