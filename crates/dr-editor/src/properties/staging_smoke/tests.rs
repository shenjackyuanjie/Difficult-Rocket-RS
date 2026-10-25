use crate::properties::staging_smoke::*;
use bevy_egui::{EguiInput, egui};

#[test]
fn seeking_a_wrapped_activation_scrolls_back_without_clicking_or_guessing_a_hit() {
    let body = egui::Rect::from_min_max(egui::pos2(8.0, 429.0), egui::pos2(382.0, 786.0));
    let mut hits = crate::egui_ui::UiHits {
        body: Some(body),
        actions: vec![(Action::SelectActivation(63, 15), body.shrink(8.0))],
        ..default()
    };
    let mut input = EguiInput::default();
    assert!(seek_activation((63, 0), true, &hits, &mut input).is_none());
    assert!(input.0.events.iter().any(|event| matches!(event,
            egui::Event::MouseWheel { delta, .. } if delta.y == body.height() * 0.75)));
    assert!(
        !input
            .0
            .events
            .iter()
            .any(|event| matches!(event, egui::Event::PointerButton { .. }))
    );
    input.0.events.clear();
    hits.scrolling = true;
    assert!(seek_activation((63, 15), true, &hits, &mut input).is_none());
    assert!(input.0.events.is_empty());
    hits.scrolling = false;
    assert_eq!(
        seek_activation((63, 15), true, &hits, &mut input),
        Some(body.shrink(8.0))
    );
    assert!(
        input.0.events.is_empty(),
        "已记录命中只供悬停，不能自动点击"
    );
    hits.actions.clear();
    assert!(
        seek_activation((63, 15), true, &hits, &mut input).is_none(),
        "没有正式命中记录不能按坐标猜缩略图"
    );
}

#[test]
fn narrow_staging_smoke_reaches_the_last_of_1024_real_tiles_before_editing() {
    let mut document = crate::test_support::document();
    document.ship = dr_core::ship_from_xml(
            r#"<Ship><Parts><Part id="1" partType="pod"><Pod name="分级回归" throttle="0"/></Part></Parts></Ship>"#,
        ).unwrap();
    let original = StagingState {
        current_stage: 32,
        steps: (0..64)
            .map(|_| StageStep {
                activations: (2..=17).map(|id| Activation { id, moved: false }).collect(),
            })
            .collect(),
    };
    document.ship.parts[0].pod.as_mut().unwrap().staging = Some(original.clone());
    document.saved_ship = document.ship.clone();
    let before = document.ship.clone();
    let mut inspector = Inspector::default();
    open(&mut inspector, &document);
    let preview = inspector.preview;
    let ctx = crate::egui_ui::test_context();
    let mut hits = crate::egui_ui::UiHits::default();
    let mut input = EguiInput::default();
    let mut driver = crate::egui_ui::UiTestInput::default();
    let size = egui::vec2(390.0, 844.0);
    let available = egui::Rect::from_min_size(egui::Pos2::ZERO, size - egui::vec2(0.0, 56.0));
    let mut phase = 0;
    for frame in 0..1800 {
        if !driver.tick(&mut input) {
            match phase {
                0 => {
                    if let Some(rect) = seek_activation((63, 15), true, &hits, &mut input) {
                        assert!(hits.body.unwrap().contains_rect(rect));
                        input
                            .0
                            .events
                            .push(egui::Event::PointerMoved(rect.center()));
                        phase = 1;
                    }
                }
                1 => {
                    assert_eq!(inspector.hovered, Some(Preview::Activation(63, 15)));
                    assert_eq!(inspector.preview, preview, "导航和悬停不能替代真实选择");
                    input.0.events.push(egui::Event::PointerMoved(
                        hits.dock.unwrap().canvas.center(),
                    ));
                    phase = 2;
                }
                2 => {
                    assert_eq!(inspector.hovered, None);
                    assert_eq!(inspector.preview, preview);
                    phase = 3;
                }
                _ => {
                    if driver.click(Action::Moved(63, 15), &hits, &mut input) {
                        phase = 4;
                    }
                }
            }
        }
        input.0.screen_rect = Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size));
        input.0.time = Some(frame as f64 / 60.0);
        ctx.begin_pass(std::mem::take(&mut input.0));
        hits.actions.clear();
        hits.scrolling = ctx.input(|input| input.is_scrolling());
        let shown = egui_panel::show(
            &ctx,
            &mut inspector,
            &document,
            &mut hits.actions,
            available,
            &mut |_| (egui::TextureId::User(1), Vec2::splat(60.0)),
        )
        .unwrap();
        hits.body = Some(shown.body);
        hits.dock = shown.dock;
        if let Some(action) = shown.action {
            act(&action, &mut inspector, &mut document);
        }
        hits.selected_activation = match inspector.preview {
            Preview::Activation(stage, index) => Some((stage, index)),
            _ => None,
        };
        ctx.end_pass().textures_delta.clear();
        assert_eq!(document.ship, before, "导航及 moved 编辑只更新草稿");
        assert!(!document.history.can_undo());
        for (action, rect) in &hits.actions {
            if matches!(
                action,
                Action::SelectStage(_) | Action::SelectActivation(_, _)
            ) {
                assert!(shown.body.contains_rect(*rect), "只允许裁剪后真实命中");
            }
        }
        let staging = inspector.draft.as_ref().unwrap().staging.as_ref().unwrap();
        assert_eq!(staging.steps.len(), 64);
        assert_eq!(
            staging
                .steps
                .iter()
                .map(|step| step.activations.len())
                .sum::<usize>(),
            1024
        );
        if phase == 4 {
            let mut expected = original.clone();
            expected.steps[63].activations[15].moved = true;
            assert_eq!(
                staging, &expected,
                "必须通过选中缩略图后的真实 moved 控件编辑末动作"
            );
            return;
        }
        assert_eq!(staging, &original);
    }
    panic!(
        "窄窗末动作导航未完成：phase={phase}，可见={:?}，滚动={}",
        visible_activations(&hits),
        hits.scrolling
    );
}
