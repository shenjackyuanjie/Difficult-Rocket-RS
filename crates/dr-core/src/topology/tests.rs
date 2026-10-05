use super::*;
use crate::{ShipGroup, catalog_from_xml};

fn ship() -> Ship {
    let catalog =
        catalog_from_xml(r#"<PartTypes><PartType id="p" width="2" height="2"/></PartTypes>"#)
            .unwrap();
    let mut ship = Ship::default();
    ship.parts = (1..=4)
        .map(|id| catalog.get("p").unwrap().instantiate(id, (id as f64, 0.0)))
        .collect();
    ship
}
fn edge(parent: i64, child: i64) -> Connection {
    Connection::Normal {
        parent,
        child,
        parent_attach: 1,
        child_attach: 1,
    }
}
#[test]
fn forest_keeps_cycles_multiple_parents_and_isolated_nodes_without_editing() {
    let mut ship = ship();
    ship.connections = vec![edge(1, 2), edge(2, 3), edge(3, 1), edge(1, 3)];
    let before = ship.clone();
    let graph = Topology::from_ship(&ship);
    let forest = graph.forest();
    assert_eq!(graph.edges.len(), 4);
    assert_eq!(forest.extra_edges.len(), 2);
    let mut represented: Vec<_> = forest
        .roots
        .iter()
        .flat_map(|root| forest.subtree(*root))
        .map(|(node, _)| node)
        .collect();
    represented.sort_unstable();
    assert_eq!(represented, vec![0, 1, 2, 3]);
    assert_eq!(graph.reachable(0, false).len(), 3);
    assert_eq!(ship, before);
}
#[test]
fn duplicate_and_dangling_references_are_reported_without_guessing() {
    let mut ship = ship();
    ship.parts[1].id = 1;
    ship.connections = vec![edge(1, 3), edge(3, 99)];
    ship.disconnected.push(ShipGroup {
        parts: vec![ship.parts[2].clone(), ship.parts[3].clone()],
        connections: vec![edge(3, 4)],
    });
    let graph = Topology::from_ship(&ship);
    assert_eq!(graph.nodes.len(), 6);
    assert_eq!(graph.unresolved.len(), 2);
    assert_eq!(graph.edges.len(), 1);
    assert_eq!(graph.nodes[graph.edges[0].parent].group, 1);
    assert_eq!(graph.reachable(graph.edges[0].parent, false).len(), 2);
}
#[test]
fn docking_connector_is_included_in_component_and_tree() {
    let mut ship = ship();
    ship.connections = vec![Connection::Dock {
        parent: 1,
        child: 2,
        dock: 3,
    }];
    let graph = Topology::from_ship(&ship);
    assert_eq!(graph.edges[0].dock, Some(2));
    assert_eq!(graph.reachable(1, false).len(), 3);
    assert_eq!(graph.forest().subtree(0).len(), 3);
}
#[test]
fn long_link_tree_uses_iterative_traversal() {
    let mut ship = ship();
    let template = ship.parts[0].clone();
    ship.parts = (0..12_000)
        .map(|id| {
            let mut p = template.clone();
            p.id = id;
            p
        })
        .collect();
    ship.connections = (0..11_999).map(|id| edge(id, id + 1)).collect();
    let forest = Topology::from_ship(&ship).forest();
    let subtree = forest.subtree(0);
    assert_eq!(subtree.len(), 12_000);
    assert_eq!(subtree.last(), Some(&(11_999, 11_999)));
}

fn square() -> (Ship, crate::PartCatalog) {
    let catalog = catalog_from_xml(
        r#"<PartTypes><PartType id="p" width="2" height="2"><AttachPoints>
    <AttachPoint location="LeftCenter"/><AttachPoint location="RightCenter"/>
    <AttachPoint location="TopCenter"/><AttachPoint location="BottomCenter"/>
    </AttachPoints></PartType></PartTypes>"#,
    )
    .unwrap();
    let mut ship = Ship::default();
    ship.parts = [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)]
        .into_iter()
        .enumerate()
        .map(|(i, pos)| catalog.get("p").unwrap().instantiate(i as i64 + 1, pos))
        .collect();
    ship.connections = vec![
        Connection::Normal {
            parent: 1,
            child: 2,
            parent_attach: 2,
            child_attach: 1,
        },
        Connection::Normal {
            parent: 2,
            child: 3,
            parent_attach: 3,
            child_attach: 4,
        },
        Connection::Normal {
            parent: 3,
            child: 4,
            parent_attach: 1,
            child_attach: 2,
        },
    ];
    (ship, catalog)
}
#[test]
fn graph_accepts_a_geometric_cycle_but_tree_rejects_it_atomically() {
    use crate::{EditorCommand, EditorHistory, LinkKind};
    let (mut ship, catalog) = square();
    let before = ship.clone();
    let mut history = EditorHistory::default();
    let parent = PartKey::new(0, 4, 0);
    let child = PartKey::new(0, 1, 0);
    let kind = LinkKind::Normal {
        parent_attach: 4,
        child_attach: 3,
    };
    assert!(
        history
            .execute_with_catalog(
                &mut ship,
                &catalog,
                EditorCommand::Reparent {
                    parent,
                    child,
                    kind: kind.clone()
                }
            )
            .is_err()
    );
    assert_eq!(ship, before);
    assert_eq!(history.undo_len(), 0);
    history
        .execute_with_catalog(
            &mut ship,
            &catalog,
            EditorCommand::ConnectParts {
                parent,
                child,
                kind,
            },
        )
        .unwrap();
    assert_eq!(Topology::from_ship(&ship).forest().extra_edges.len(), 1);
    assert!(history.undo(&mut ship));
    assert_eq!(ship, before);
    assert!(history.redo(&mut ship));
    assert_eq!(ship.connections.len(), 4);
}
#[test]
fn tree_reparent_is_one_history_entry_and_invalid_replacement_keeps_redo() {
    use crate::{EditorCommand, EditorHistory, LinkKind};
    let (mut ship, catalog) = square();
    ship.connections.truncate(1);
    let before = ship.clone();
    let mut history = EditorHistory::default();
    history
        .execute_with_catalog(
            &mut ship,
            &catalog,
            EditorCommand::Reparent {
                parent: PartKey::new(0, 3, 0),
                child: PartKey::new(0, 2, 0),
                kind: LinkKind::Normal {
                    parent_attach: 4,
                    child_attach: 3,
                },
            },
        )
        .unwrap();
    assert_eq!(ship.connections.len(), 1);
    assert!(matches!(
        ship.connections[0],
        Connection::Normal {
            parent: 3,
            child: 2,
            ..
        }
    ));
    assert_eq!(history.undo_len(), 1);
    assert!(history.undo(&mut ship));
    assert_eq!(ship, before);
    assert!(
        history
            .execute_with_catalog(
                &mut ship,
                &catalog,
                EditorCommand::Reparent {
                    parent: PartKey::new(0, 3, 0),
                    child: PartKey::new(0, 2, 0),
                    kind: LinkKind::Normal {
                        parent_attach: 1,
                        child_attach: 1
                    }
                }
            )
            .is_err()
    );
    assert_eq!(ship, before);
    assert!(history.can_redo());
}
#[test]
fn unlink_checks_edge_identity_and_keeps_unrelated_connections() {
    use crate::{EditorCommand, EditorHistory};
    let (mut ship, catalog) = square();
    let before = ship.clone();
    let mut history = EditorHistory::default();
    let reference = Topology::from_ship(&ship).edges[1].reference.clone();
    history
        .execute_with_catalog(
            &mut ship,
            &catalog,
            EditorCommand::RemoveConnection(reference.clone()),
        )
        .unwrap();
    assert_eq!(
        ship.connections,
        vec![before.connections[0].clone(), before.connections[2].clone()]
    );
    let changed = ship.clone();
    assert!(
        history
            .execute_with_catalog(
                &mut ship,
                &catalog,
                EditorCommand::RemoveConnection(reference)
            )
            .is_err()
    );
    assert_eq!(ship, changed);
    assert!(history.undo(&mut ship));
    assert_eq!(ship, before);
}

#[test]
fn tree_reparent_does_not_silently_remove_multiple_parent_edges() {
    use crate::{EditorCommand, EditorHistory, LinkKind};
    let (mut ship, catalog) = square();
    ship.connections.push(Connection::Normal {
        parent: 4,
        child: 2,
        parent_attach: 1,
        child_attach: 1,
    });
    let before = ship.clone();
    let mut history = EditorHistory::default();
    assert!(
        history
            .execute_with_catalog(
                &mut ship,
                &catalog,
                EditorCommand::Reparent {
                    parent: PartKey::new(0, 1, 0),
                    child: PartKey::new(0, 2, 0),
                    kind: LinkKind::Normal {
                        parent_attach: 2,
                        child_attach: 1
                    }
                }
            )
            .is_err()
    );
    assert_eq!(ship, before);
    assert_eq!(history.undo_len(), 0);
}
