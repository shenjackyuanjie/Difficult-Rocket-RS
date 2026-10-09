use super::*;

pub(super) fn setup(ctx: &mut Context, scenario: Scenario) {
    match scenario {
        Scenario::SelfLink | Scenario::Cycle(_) => {
            ctx.topology.open = true;
            ctx.topology.refresh(&ctx.document);
            ctx.topology.parent = Some(if matches!(scenario, Scenario::SelfLink) {
                key(1)
            } else {
                key(3)
            });
            ctx.topology.child = Some(key(1));
            ctx.topology.parent_attach = 1;
            ctx.topology.child_attach = 1;
            ctx.topology.mode = if matches!(scenario, Scenario::Cycle(true)) {
                topology_ui::Mode::Graph
            } else {
                topology_ui::Mode::Tree
            };
        }
        Scenario::CrossGroup => {
            let fuselage = ctx.document.catalog.get("fuselage-1").unwrap();
            ctx.document.ship = Ship {
                parts: vec![fuselage.instantiate(1, (-2., 1.))],
                disconnected: vec![dr_core::ShipGroup {
                    parts: vec![
                        fuselage.instantiate(1, (1., -1.)),
                        fuselage.instantiate(2, (3.2, -1.)),
                    ],
                    connections: vec![normal(1, 2)],
                }],
                ..default()
            };
        }
        Scenario::UndoBranch => {
            ctx.document.select_only(Some(key(2)));
        }
        Scenario::InvalidFuel(_) | Scenario::StaleDraft => {
            let tank = ctx
                .document
                .catalog
                .visible()
                .find(|kind| kind.tank.is_some())
                .unwrap();
            ctx.document.ship.parts = vec![tank.instantiate(1, (0., 0.))];
            ctx.document.ship.connections.clear();
            ctx.document.select_only(Some(key(1)));
        }
        _ => {}
    }
}

fn topology(state: &mut Run, ctx: &mut Context, graph_cycle: bool) -> bool {
    match state.step {
        1 => {
            if !ctx.topology_button(state, topology_ui::Action::Connect) {
                return false;
            }
        }
        2 => {
            if graph_cycle {
                assert_eq!(ctx.document.ship.connections.len(), 3);
                assert_eq!(ctx.document.history.undo_len(), 1);
                assert!(ctx.topology.graph.edges.len() >= 3);
                if !ctx.topology_button(state, topology_ui::Action::Undo) {
                    return false;
                }
            } else {
                unchanged(state, &ctx.document);
                assert!(!ctx.document.status.is_empty());
                state.metrics = serde_json::json!({"cycle_rejected":true, "original_edges_preserved":true, "error":ctx.document.status});
                return true;
            }
        }
        3 => {
            unchanged(state, &ctx.document);
            state.metrics = serde_json::json!({"graph_cycle_allowed":true, "atomic_undo":true});
            return true;
        }
        _ => unreachable!(),
    }
    state.step += 1;
    false
}

fn draft(state: &mut Run, ctx: &mut Context, scenario: Scenario) -> bool {
    match state.step {
        1 => {
            ctx.key(KeyCode::F2, false, false);
        }
        2 => {
            let original = ctx.document.ship.part(1).unwrap().clone();
            let draft = ctx.inspector.draft.as_mut().expect("F2 未打开属性草稿");
            if let Scenario::InvalidFuel(value) = scenario {
                draft.fuel = value.into();
            } else {
                draft.active = !original.active;
                assert!(
                    ctx.document
                        .execute(EditorCommand::SetActive(1, !original.active))
                );
                state.after = Some(ctx.document.ship.clone());
            }
        }
        3 => {
            if !ctx.property_button(state, properties::Action::Apply) {
                return false;
            }
        }
        4 => {
            assert!(ctx.inspector.is_open(), "非法草稿被错误应用");
            assert!(!ctx.inspector.error.is_empty());
            if matches!(scenario, Scenario::StaleDraft) {
                assert_eq!(state.after.as_ref(), Some(&ctx.document.ship));
                assert_eq!(ctx.document.history.undo_len(), 1);
            } else {
                unchanged(state, &ctx.document);
            }
            state.metrics = serde_json::json!({"draft_rejected":true, "error":ctx.inspector.error, "document_preserved":true});
            return true;
        }
        _ => unreachable!(),
    }
    state.step += 1;
    false
}

fn scratch(name: &str) -> std::path::PathBuf {
    let root = std::path::PathBuf::from("target/demo-case-inputs");
    std::fs::create_dir_all(&root).unwrap();
    root.join(name)
}

fn files(state: &mut Run, ctx: &mut Context, scenario: Scenario) -> bool {
    match state.step {
        1 => {
            assert!(ctx.document.execute(EditorCommand::Rotate(2)));
            ctx.document.select_only(Some(key(2)));
            state.after = Some(ctx.document.ship.clone());
            ctx.paths.ship = Some(
                scratch("unsaved-current.xml")
                    .to_string_lossy()
                    .into_owned(),
            );
        }
        2 => match scenario {
            Scenario::InvalidOpen => {
                let path = scratch("broken.xml");
                std::fs::write(&path, "<Ship><Parts><Part").unwrap();
                ctx.file_actions.write(files::FileAction::Open(path));
            }
            Scenario::SaveFailure => {
                let path = scratch("save-is-directory");
                std::fs::create_dir_all(&path).unwrap();
                ctx.paths.ship = Some(path.to_string_lossy().into_owned());
                ctx.file_actions.write(files::FileAction::Save);
            }
            Scenario::UnsavedCancel => {
                ctx.file_actions.write(files::FileAction::New);
            }
            _ => unreachable!(),
        },
        3 => {
            assert_eq!(state.after.as_ref(), Some(&ctx.document.ship));
            assert!(ctx.document.dirty);
            assert_eq!(ctx.document.history.undo_len(), 1);
            if matches!(scenario, Scenario::UnsavedCancel) {
                assert!(ctx.pending.is_blocked());
                ctx.key(KeyCode::KeyE, false, false);
                ctx.keys.press(KeyCode::Delete);
            } else {
                assert!(!ctx.document.status.is_empty());
                let expected = scratch(if matches!(scenario, Scenario::SaveFailure) {
                    "save-is-directory"
                } else {
                    "unsaved-current.xml"
                });
                assert_eq!(ctx.paths.ship.as_deref(), Some(expected.to_str().unwrap()));
                state.metrics = serde_json::json!({"failed_without_data_loss":true, "dirty_preserved":true, "history_preserved":true, "path_preserved":true, "error":ctx.document.status});
                return true;
            }
        }
        4 => {
            assert_eq!(
                state.after.as_ref(),
                Some(&ctx.document.ship),
                "模态期间快捷键穿透画布"
            );
            assert_eq!(ctx.document.history.undo_len(), 1);
            let Some((_, rect)) = ctx
                .pending
                .hits
                .iter()
                .find(|(choice, _)| *choice == files::UnsavedChoice::Cancel)
            else {
                return false;
            };
            state
                .input
                .click_rect(*rect, &mut ctx.inputs.single_mut().unwrap());
        }
        5 => {
            if ctx.pending.is_blocked() {
                return false;
            }
            assert_eq!(state.after.as_ref(), Some(&ctx.document.ship));
            assert!(ctx.document.dirty);
            assert_eq!(ctx.document.history.undo_len(), 1);
            state.metrics = serde_json::json!({"cancel_preserved_unsaved_ship":true, "modal_blocks_transform_and_delete":true});
            return true;
        }
        _ => unreachable!(),
    }
    state.step += 1;
    false
}

pub(super) fn advance(state: &mut Run, ctx: &mut Context, scenario: Scenario) -> bool {
    match scenario {
        Scenario::Cycle(graph) => return topology(state, ctx, graph),
        Scenario::InvalidFuel(_) | Scenario::StaleDraft => return draft(state, ctx, scenario),
        Scenario::InvalidOpen | Scenario::SaveFailure | Scenario::UnsavedCancel => {
            return files(state, ctx, scenario);
        }
        Scenario::SelfLink => match state.step {
            1 => {
                if !ctx.topology_button(state, topology_ui::Action::Connect) {
                    return false;
                }
            }
            2 => {
                unchanged(state, &ctx.document);
                assert!(!ctx.document.status.is_empty(), "自连接拒绝未显示错误");
                state.metrics =
                    serde_json::json!({"self_connect_rejected":true, "error":ctx.document.status});
                return true;
            }
            _ => unreachable!(),
        },
        Scenario::CrossGroup => match state.step {
            1 => {
                assert!(ctx.document.execute(EditorCommand::ConnectParts {
                    parent: key(1),
                    child: PartKey::new(1, 1, 0),
                    kind: LinkKind::Normal {
                        parent_attach: 4,
                        child_attach: 3
                    }
                }));
            }
            2 => {
                assert!(ctx.document.ship.disconnected.is_empty());
                assert_eq!(ctx.document.ship.parts.len(), 3);
                let ids: BTreeSet<_> = ctx.document.ship.parts.iter().map(|part| part.id).collect();
                assert_eq!(ids.len(), 3);
                assert_eq!(ctx.document.ship.connections.len(), 2);
                assert!(
                    dr_core::Topology::from_ship(&ctx.document.ship)
                        .unresolved
                        .is_empty()
                );
                let xml = dr_core::ship_to_xml(&ctx.document.ship).unwrap();
                assert_eq!(dr_core::ship_from_xml(&xml).unwrap(), ctx.document.ship);
                assert_eq!(ctx.document.history.undo_len(), 1);
                state.metrics = serde_json::json!({"unique_ids_after_merge":ids, "references_valid":true, "xml_roundtrip":true});
                ctx.key(KeyCode::KeyZ, true, false);
            }
            3 => {
                unchanged(state, &ctx.document);
                state.metrics["atomic_undo"] = true.into();
                return true;
            }
            _ => unreachable!(),
        },
        Scenario::EmptyHistory => match state.step {
            1 => ctx.key(KeyCode::KeyZ, true, false),
            2 => {
                unchanged(state, &ctx.document);
                ctx.key(KeyCode::KeyY, true, false);
            }
            3 => {
                unchanged(state, &ctx.document);
                assert_eq!(ctx.document.history.redo_len(), 0);
                return true;
            }
            _ => unreachable!(),
        },
        Scenario::BatchRollback => {
            assert!(!ctx.document.execute(EditorCommand::Batch(vec![
                EditorCommand::Rotate(1),
                EditorCommand::Delete(i64::MAX)
            ])));
            unchanged(state, &ctx.document);
            state.metrics = serde_json::json!({"entire_batch_rolled_back":true});
            return true;
        }
        Scenario::UndoBranch => match state.step {
            1 => ctx.key(KeyCode::KeyE, false, false),
            2 => {
                assert_eq!(ctx.document.history.undo_len(), 1);
                ctx.key(KeyCode::KeyZ, true, false);
            }
            3 => {
                unchanged(state, &ctx.document);
                assert_eq!(ctx.document.history.redo_len(), 1);
                ctx.document.select_only(Some(key(2)));
                ctx.key(KeyCode::KeyX, false, false);
            }
            4 => {
                assert_eq!(ctx.document.history.redo_len(), 0);
                state.after = Some(ctx.document.ship.clone());
                ctx.key(KeyCode::KeyY, true, false);
            }
            5 => {
                assert_eq!(state.after.as_ref(), Some(&ctx.document.ship));
                assert_eq!(ctx.document.history.undo_len(), 1);
                state.metrics = serde_json::json!({"new_branch_clears_old_redo":true});
                return true;
            }
            _ => unreachable!(),
        },
        _ => unreachable!(),
    }
    state.step += 1;
    false
}
