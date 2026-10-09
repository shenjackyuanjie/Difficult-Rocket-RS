use super::*;

pub(super) fn setup(ctx: &mut Context, scenario: Scenario) {
    match scenario {
        Scenario::PaletteCancel => {
            ctx.cursor.placing = true;
            ctx.cursor.catalog_index = ctx
                .document
                .catalog
                .visible()
                .position(|kind| kind.id == "fuselage-1")
                .unwrap();
        }
        Scenario::QuantityLimit => {
            ctx.document.ship.parts = vec![
                ctx.document
                    .catalog
                    .get("dock-1")
                    .unwrap()
                    .instantiate(1, (-2., 1.)),
            ];
            ctx.document.ship.connections.clear();
            ctx.cursor.catalog_index = ctx
                .document
                .catalog
                .visible()
                .position(|kind| kind.id == "dock-1")
                .unwrap();
            ctx.cursor.placing = true;
        }
        Scenario::DisabledRotation => {
            ctx.document.ship.parts = vec![
                ctx.document
                    .catalog
                    .get("wheel-2")
                    .unwrap()
                    .instantiate(1, (0., 0.)),
            ];
            ctx.document.ship.connections.clear();
            ctx.document.select_only(Some(key(1)));
        }
        Scenario::ManualMismatch => {
            // 原版插头的 BottomCenter 是普通安装点，不是 dock=true 对接点。
            // 使用明确标记的临时目录夹具，不能为了演示改写原版安装点语义。
            let mut kind = ctx.document.catalog.get("dock-1").unwrap().clone();
            kind.id = "demo-explicit-dock".into();
            kind.name = "显式对接点（临时夹具）".into();
            kind.attach_points[0].dock = true;
            ctx.document.ship.parts[0] = kind.instantiate(1, (-2.5, 1.5));
            ctx.document.catalog.types.push(kind);
            ctx.document.ship.connections.clear();
        }
        Scenario::EmptySelection => {
            ctx.document.ship = Ship::default();
        }
        Scenario::SingleFine => {
            ctx.document.select_only(Some(key(2)));
        }
        Scenario::Disconnected => {
            ctx.document.ship.connections.clear();
            ctx.document.selection = [key(1), key(2), key(3)].into();
        }
        Scenario::MirrorTwice | Scenario::TransformOrder => {
            for part in &mut ctx.document.ship.parts[..3] {
                part.angle = 37_f64.to_radians();
            }
            ctx.document.selection = [key(1), key(2), key(3)].into();
        }
        Scenario::Descendants => {
            ctx.options.follow_children = true;
        }
        Scenario::LineWidth(_) => {
            ctx.lines.open = true;
        }
        Scenario::LineReset => {
            *ctx.lines = connection_lines::Settings {
                open: true,
                enabled: false,
                rgba: [0.75, 0.4, 1., 0.35],
                width: 9.,
                pattern: connection_lines::Pattern::Dashed,
                effect: connection_lines::Effect::Flow,
                arrows: true,
                speed: 3.,
                spacing: 10.,
            };
        }
        Scenario::ZeroLength => {
            let center = (ctx.document.ship.parts[0].x, ctx.document.ship.parts[0].y);
            ctx.document.ship.parts.truncate(2);
            ctx.document.ship.parts[1].x = center.0;
            ctx.document.ship.parts[1].y = center.1;
            ctx.document.ship.connections = vec![Connection::Normal {
                parent: 1,
                child: 2,
                parent_attach: 1,
                child_attach: 1,
            }];
            ctx.lines.effect = connection_lines::Effect::Flow;
            ctx.lines.arrows = true;
        }
        Scenario::HiddenEndpoint => {
            ctx.lines.enabled = false;
        }
        _ => {}
    }
}

fn line_defaults_match(settings: &connection_lines::Settings) -> bool {
    let expected = connection_lines::Settings {
        open: true,
        ..default()
    };
    // egui 色板的颜色空间往返会产生末位浮点误差；其它配置仍逐字段精确核验。
    let color_matches = settings
        .rgba
        .iter()
        .zip(expected.rgba)
        .all(|(a, b)| (*a - b).abs() < 1e-6);
    let mut actual = settings.clone();
    actual.rgba = expected.rgba;
    color_matches && actual == expected
}

fn group(state: &Run, ctx: &Context) {
    let before = state.before.as_ref().unwrap();
    assert_eq!(ctx.document.ship.part(4), before.part(4));
    assert_eq!(ctx.document.ship.connections, before.connections);
    for (a, b) in [(1, 2), (2, 3), (1, 3)] {
        let distance = |ship: &Ship| {
            let a = ship.part(a).unwrap();
            let b = ship.part(b).unwrap();
            (a.x - b.x).hypot(a.y - b.y)
        };
        assert!((distance(&ctx.document.ship) - distance(before)).abs() < 1e-8);
    }
}

fn manual(state: &mut Run, ctx: &mut Context, scenario: Scenario) -> bool {
    match state.step {
        1 => {
            let (id, attach) = if matches!(scenario, Scenario::ManualReverse) {
                (2, 2)
            } else {
                (1, 0)
            };
            let point = ctx.endpoint(id, attach);
            ctx.click_point(state, point);
        }
        2 => {
            ctx.mouse.release(MouseButton::Left);
        }
        3 => {
            assert!(
                ctx.cursor.manual_connection.is_some(),
                "端点点击没有进入待连接状态"
            );
            match scenario {
                Scenario::ManualCancel(Cancel::Same) => {
                    let point = ctx.endpoint(1, 0);
                    ctx.click_point(state, point);
                }
                Scenario::ManualCancel(Cancel::Escape) => ctx.key(KeyCode::Escape, false, false),
                Scenario::ManualCancel(Cancel::Blank) => ctx.click_point(state, (-3.8, -3.4)),
                Scenario::ManualCancel(Cancel::Revision) => {
                    let active = ctx.document.ship.part(3).unwrap().active;
                    assert!(ctx.document.execute(EditorCommand::SetActive(3, !active)));
                }
                Scenario::ManualSelf => {
                    let point = ctx.endpoint(1, 1);
                    ctx.click_point(state, point);
                }
                Scenario::ManualMismatch => {
                    let point = ctx.endpoint(2, 0);
                    ctx.click_point(state, point);
                }
                Scenario::ManualReverse => {
                    let point = ctx.endpoint(1, 3);
                    ctx.click_point(state, point);
                }
                Scenario::HiddenEndpoint => {}
                _ => unreachable!(),
            }
        }
        4 => {
            ctx.mouse.release(MouseButton::Left);
        }
        5 => match scenario {
            Scenario::ManualSelf | Scenario::ManualMismatch => {
                unchanged(state, &ctx.document);
                assert!(!ctx.document.status.is_empty());
                state.metrics = serde_json::json!({"rejected":true, "error":ctx.document.status});
                return true;
            }
            Scenario::ManualReverse => {
                assert_eq!(ctx.document.ship.connections, vec![normal(2, 3)]);
                assert_eq!(ctx.document.history.undo_len(), 1);
                ctx.key(KeyCode::KeyZ, true, false);
            }
            Scenario::ManualCancel(Cancel::Revision) => {
                assert!(ctx.cursor.manual_connection.is_none());
                assert_eq!(ctx.document.history.undo_len(), 1);
                ctx.key(KeyCode::KeyZ, true, false);
            }
            Scenario::HiddenEndpoint => {
                assert!(!ctx.lines.enabled);
                assert!(ctx.cursor.manual_connection.is_some());
                unchanged(state, &ctx.document);
                state.metrics =
                    serde_json::json!({"lines_visible":false, "endpoint_click_works":true});
                return true;
            }
            _ => {
                assert!(ctx.cursor.manual_connection.is_none());
                unchanged(state, &ctx.document);
                state.metrics =
                    serde_json::json!({"pending_cleared":true, "history_unchanged":true});
                return true;
            }
        },
        6 => {
            unchanged(state, &ctx.document);
            assert!(ctx.cursor.manual_connection.is_none());
            state.metrics = serde_json::json!({"undo":true, "original_connections_restored":true});
            return true;
        }
        _ => unreachable!(),
    }
    state.step += 1;
    false
}

pub(super) fn advance(state: &mut Run, ctx: &mut Context, scenario: Scenario) -> bool {
    if matches!(
        scenario,
        Scenario::ManualCancel(_)
            | Scenario::ManualSelf
            | Scenario::ManualMismatch
            | Scenario::ManualReverse
            | Scenario::HiddenEndpoint
    ) {
        return manual(state, ctx, scenario);
    }
    match scenario {
        Scenario::PaletteCancel => match state.step {
            1 => {
                ctx.point(state, (1., -2.8));
                ctx.key(KeyCode::KeyE, false, false);
            }
            2 => {
                ctx.key(KeyCode::KeyX, false, false);
            }
            3 => {
                assert!(ctx.cursor.placing);
                ctx.key(KeyCode::Escape, false, false);
            }
            4 => {
                assert!(!ctx.cursor.placing);
                unchanged(state, &ctx.document);
                return true;
            }
            _ => unreachable!(),
        },
        Scenario::QuantityLimit => match state.step {
            1 => {
                ctx.point(state, (1., -2.));
            }
            2 => {
                assert!(ctx.cursor.valid);
                assert!(!placement::preview(&ctx.document, &ctx.cursor).unwrap().2);
                assert!(!placement::place(&mut ctx.document, &ctx.cursor));
            }
            3 => {
                unchanged(state, &ctx.document);
                state.metrics =
                    serde_json::json!({"free_mode":true, "quantity_limit_respected":true});
                return true;
            }
            _ => unreachable!(),
        },
        Scenario::DisabledRotation => match state.step {
            1 => ctx.key(KeyCode::KeyR, false, false),
            2 => {
                unchanged(state, &ctx.document);
                ctx.key(KeyCode::KeyE, false, true);
            }
            3 => {
                unchanged(state, &ctx.document);
                return true;
            }
            _ => unreachable!(),
        },
        Scenario::EmptySelection => match state.step {
            1 => ctx.key(KeyCode::KeyE, false, false),
            2 => ctx.key(KeyCode::KeyX, false, false),
            3 => ctx.key(KeyCode::KeyY, false, false),
            4 => {
                unchanged(state, &ctx.document);
                return true;
            }
            _ => unreachable!(),
        },
        Scenario::SingleFine => match state.step {
            1 => ctx.key(KeyCode::KeyQ, false, true),
            2 => {
                assert!(
                    (ctx.document.ship.part(2).unwrap().angle.to_degrees() - 359.).abs() < 1e-8
                );
                let before = state.before.as_ref().unwrap().part(2).unwrap();
                let after = ctx.document.ship.part(2).unwrap();
                assert_eq!((before.x, before.y), (after.x, after.y));
                ctx.key(KeyCode::KeyZ, true, false);
            }
            3 => {
                unchanged(state, &ctx.document);
                return true;
            }
            _ => unreachable!(),
        },
        Scenario::Disconnected => match state.step {
            1 => ctx.key(KeyCode::KeyE, false, false),
            2 => {
                group(state, ctx);
                ctx.key(KeyCode::KeyX, false, false);
            }
            3 => {
                group(state, ctx);
                ctx.key(KeyCode::KeyY, false, false);
            }
            4 => {
                group(state, ctx);
                assert_eq!(ctx.document.history.undo_len(), 3);
                state.metrics = serde_json::json!({"rigid_distance_preserved":true, "unselected_unchanged":true, "connections":0});
                return true;
            }
            _ => unreachable!(),
        },
        Scenario::MirrorTwice => match state.step {
            1 | 2 => ctx.key(KeyCode::KeyX, false, false),
            3 | 4 => ctx.key(KeyCode::KeyY, false, false),
            5 => {
                close_enough(state.before.as_ref().unwrap(), &ctx.document.ship);
                assert_eq!(ctx.document.history.undo_len(), 4);
                ctx.key(KeyCode::KeyZ, true, false);
            }
            6..=8 => ctx.key(KeyCode::KeyZ, true, false),
            9 => {
                unchanged(state, &ctx.document);
                return true;
            }
            _ => unreachable!(),
        },
        Scenario::TransformOrder => match state.step {
            1 => ctx.key(KeyCode::KeyE, false, false),
            2 => ctx.key(KeyCode::KeyX, false, false),
            3 => {
                group(state, ctx);
                state.after = Some(ctx.document.ship.clone());
                ctx.key(KeyCode::KeyZ, true, false);
            }
            4 => ctx.key(KeyCode::KeyZ, true, false),
            5 => {
                unchanged(state, &ctx.document);
                ctx.document.selection = [key(1), key(2), key(3)].into();
                ctx.key(KeyCode::KeyX, false, false);
            }
            6 => ctx.key(KeyCode::KeyE, false, false),
            7 => {
                group(state, ctx);
                assert_ne!(
                    state.after.as_ref(),
                    Some(&ctx.document.ship),
                    "旋转和镜像被错误当作可交换"
                );
                let first = state.after.as_ref().unwrap().part(2).unwrap().angle;
                let second = ctx.document.ship.part(2).unwrap().angle;
                assert!(((first - second).abs().to_degrees() - 30.).abs() < 1e-8);
                state.metrics = serde_json::json!({"order_is_noncommutative":true, "angle_difference_degrees":30});
                return true;
            }
            _ => unreachable!(),
        },
        Scenario::Descendants => match state.step {
            1 => {
                let part = ctx.document.ship.part(2).unwrap();
                let p = (part.x, part.y);
                ctx.click_point(state, p);
            }
            2 => {
                assert_eq!(ctx.drag.keys(), [key(2), key(3)].into());
                ctx.point(state, (0.4, -1.5));
                ctx.key(KeyCode::KeyE, false, false);
            }
            3 => {
                assert_eq!(Some(&ctx.document.ship), state.before.as_ref());
                ctx.key(KeyCode::KeyX, false, false);
            }
            4 => {
                assert_eq!(Some(&ctx.document.ship), state.before.as_ref());
                ctx.mouse.release(MouseButton::Left);
            }
            5 => {
                let before = state.before.as_ref().unwrap();
                assert_eq!(ctx.document.ship.part(1), before.part(1));
                assert_eq!(ctx.document.ship.part(4), before.part(4));
                assert_eq!(ctx.document.ship.connections, before.connections);
                let distance = |ship: &Ship| {
                    let a = ship.part(2).unwrap();
                    let b = ship.part(3).unwrap();
                    (a.x - b.x).hypot(a.y - b.y)
                };
                assert!((distance(&ctx.document.ship) - distance(before)).abs() < 1e-8);
                assert_eq!(ctx.document.history.undo_len(), 1);
                state.metrics = serde_json::json!({"drag_members":[2,3], "ancestor_unchanged":true, "external_connection_preserved_in_free_mode":true, "atomic_history":true});
                return true;
            }
            _ => unreachable!(),
        },
        Scenario::LineWidth(maximum) => {
            let limit = if maximum { 12. } else { 1. };
            if state.step <= 13 {
                if !ctx.line_button(
                    state,
                    if maximum {
                        connection_lines::Control::Thicker
                    } else {
                        connection_lines::Control::Thinner
                    },
                ) {
                    return false;
                }
            } else {
                assert_eq!(ctx.lines.width, limit);
                assert_eq!(
                    ctx.configs
                        .config::<connection_lines::LineGizmos>()
                        .0
                        .line
                        .width,
                    limit
                );
                unchanged(state, &ctx.document);
                state.metrics = serde_json::json!({"width_px":limit, "history_unchanged":true});
                return true;
            }
        }
        Scenario::LineReset => match state.step {
            1 => {
                if !ctx.line_button(state, connection_lines::Control::Reset) {
                    return false;
                }
            }
            2 => {
                assert!(
                    line_defaults_match(&ctx.lines),
                    "实际重置没有恢复默认连线配置"
                );
                unchanged(state, &ctx.document);
                return true;
            }
            _ => unreachable!(),
        },
        Scenario::ZeroLength => {
            if !state.zero_path_rendered {
                return false;
            }
            unchanged(state, &ctx.document);
            state.metrics = serde_json::json!({"coincident_parts":true, "zero_length_path_actually_drawn":true, "rendered_flow_and_arrows":true});
            return true;
        }
        _ => unreachable!(),
    }
    state.step += 1;
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resetting_line_color_accepts_only_color_space_roundoff_not_real_configuration_errors() {
        let mut settings = connection_lines::Settings {
            open: true,
            ..default()
        };
        settings.rgba[2] = 0.54999995;
        assert!(line_defaults_match(&settings));
        settings.width = 3.0;
        assert!(!line_defaults_match(&settings));
        settings.width = 2.0;
        settings.rgba[2] = 0.54;
        assert!(!line_defaults_match(&settings));
    }
}
