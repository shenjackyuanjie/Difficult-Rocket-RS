use super::*;

pub(super) fn setup(state: &mut Run, ctx: &mut Context, scenario: Scenario) {
    let Scenario::Overlap {
        ratio,
        degrees,
        relative,
        mirrors,
    } = scenario
    else {
        if matches!(scenario, Scenario::Contained) {
            ctx.document.free_mode = false;
            ctx.document.ship.parts.truncate(2);
            ctx.document.ship.connections.clear();
            ctx.document.ship.parts[1] = ctx
                .document
                .catalog
                .get("nosecone-1")
                .unwrap()
                .instantiate(2, (2.2, -1.8));
            let mut candidate = ctx.document.ship.parts[1].clone();
            candidate.x = ctx.document.ship.parts[0].x;
            candidate.y = ctx.document.ship.parts[0].y;
            state.candidate = Some(candidate);
        }
        return;
    };
    ctx.document.free_mode = false;
    ctx.document.ship.parts.truncate(2);
    ctx.document.ship.connections.clear();
    let angle = degrees.to_radians();
    ctx.document.ship.parts[0].angle = angle;
    ctx.document.ship.parts[0].flip_x = mirrors;
    ctx.document.ship.parts[0].flip_y = mirrors;
    ctx.document.ship.parts[1].angle = angle
        + if relative {
            std::f64::consts::FRAC_PI_4
        } else {
            0.0
        };
    ctx.document.ship.parts[1].flip_x = mirrors;
    ctx.document.ship.parts[1].flip_y = mirrors;
    let mut candidate = ctx.document.ship.parts[1].clone();
    // 原版机身实体为 2×2 Ship 单位；相对 45° 时尖角三角面积为深度平方。
    let distance = if relative {
        1.0 + 2.0_f64.sqrt() - 2.0 * ratio.sqrt()
    } else {
        2.0 * (1.0 - ratio)
    };
    candidate.x = ctx.document.ship.parts[0].x + distance * angle.cos();
    candidate.y = ctx.document.ship.parts[0].y + distance * angle.sin();
    state.candidate = Some(candidate);
}

pub(super) fn advance(state: &mut Run, ctx: &mut Context, scenario: Scenario) -> bool {
    let candidate = state.candidate.as_ref().unwrap().clone();
    let target = &ctx.document.ship.parts[0];
    let kind = ctx.document.catalog.get(&candidate.part_type).unwrap();
    let target_kind = ctx.document.catalog.get(&target.part_type).unwrap();
    let ratio = dr_core::geometry::overlap_ratio(&candidate, kind, target, target_kind);
    let expected = match scenario {
        Scenario::Overlap { ratio, .. } => ratio,
        Scenario::Contained => 1.0,
        _ => unreachable!(),
    };
    assert!(
        (ratio - expected).abs() < 2e-7,
        "几何夹具不符合样例：预期 {expected}，实际 {ratio}"
    );
    let allowed = expected < 0.05;
    match state.step {
        1 => {
            let original = ctx.document.ship.parts[1].clone();
            let accepted = ctx.document.execute(EditorCommand::TransformSelection {
                parts: vec![key(2)],
                transform: SelectionTransform::Translate {
                    dx: candidate.x - original.x,
                    dy: candidate.y - original.y,
                },
            });
            assert_eq!(accepted, allowed, "严格 <5% 的事务判定错误");
            state.metrics = serde_json::json!({"overlap_ratio":ratio, "expected_ratio":expected, "accepted":accepted, "threshold_strict_less_than":0.05});
        }
        2 => {
            if !allowed {
                unchanged(state, &ctx.document);
                return true;
            }
            assert_eq!(ctx.document.history.undo_len(), 1);
            assert!((ctx.document.ship.parts[1].x - candidate.x).abs() < 1e-9);
            state.after = Some(ctx.document.ship.clone());
            ctx.key(KeyCode::KeyZ, true, false);
        }
        3 => {
            unchanged(state, &ctx.document);
            ctx.key(KeyCode::KeyY, true, false);
        }
        4 => {
            assert_eq!(state.after.as_ref(), Some(&ctx.document.ship));
            let xml = dr_core::ship_to_xml(&ctx.document.ship).unwrap();
            assert_eq!(dr_core::ship_from_xml(&xml).unwrap(), ctx.document.ship);
            state.metrics["undo_redo"] = true.into();
            state.metrics["xml_roundtrip"] = true.into();
            return true;
        }
        _ => unreachable!(),
    }
    state.step += 1;
    false
}
