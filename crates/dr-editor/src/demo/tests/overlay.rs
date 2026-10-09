use super::*;
use crate::demo::caption::paint;
use bevy_egui::egui;

#[test]
fn detailed_property_caption_leaves_real_error_and_apply_cancel_visible_without_taking_input() {
    for width in [1440., 1920.] {
        let ctx = egui::Context::default();
        let mut document = crate::tests::document();
        document.select_only(Some(PartKey::new(0, 1, 0)));
        let mut inspector = properties::Inspector::default();
        properties::act(&properties::Action::Open, &mut inspector, &mut document);
        assert!(inspector.is_open());
        inspector.error = "燃料必须是有限数值".into();
        let details = "样例 4/7 · 燃料 NaN 草稿\n预期：留在草稿并显示错误，文档不改。\n路径：草稿参数设置 + 实际 egui 应用按钮 · 断言通过";
        let mut hits = Vec::new();
        let mut cancel = None;
        // 首次 pass 只测量 Area，和实际回放一样等待可交互控件布局。
        for frame in 0..6 {
            let click = frame == 5;
            let mut input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(width, 900.),
                )),
                ..Default::default()
            };
            if click {
                let pos = cancel.unwrap();
                input.events.push(egui::Event::PointerMoved(pos));
                for pressed in [true, false] {
                    input.events.push(egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    });
                }
            }
            ctx.begin_pass(input);
            hits.clear();
            let action = properties::egui_panel::show(&ctx, &mut inspector, &document, &mut hits);
            let rect = |action| {
                hits.iter()
                    .find(|(candidate, _)| *candidate == action)
                    .map(|(_, rect)| *rect)
            };
            let (Some(apply), Some(cancel_rect)) = (
                rect(properties::Action::Apply),
                rect(properties::Action::Cancel),
            ) else {
                let mut output = ctx.end_pass();
                output.textures_delta.clear();
                assert!(frame < 2, "测量后仍缺少真实应用/取消控件");
                continue;
            };
            cancel = Some(cancel_rect.center());
            let before = ctx.input(|input| input.events.clone());
            let focus = ctx.memory(|memory| memory.focused());
            let caption = paint(
                &ctx,
                "详细演示 12/18 · 历史与属性边界",
                "非法草稿实际应用：显示错误并保留文档。",
                Some(details),
                Duration::from_millis(450),
                Some(apply.left()),
            );
            assert_eq!(ctx.input(|input| input.events.clone()), before);
            assert_eq!(ctx.memory(|memory| memory.focused()), focus);
            let mut output = ctx.end_pass();
            output.textures_delta.clear();
            if click {
                assert_eq!(action, Some(properties::Action::Cancel));
            } else if frame >= 2 {
                assert!(
                    ctx.content_rect().contains_rect(caption),
                    "说明必须在窗口内完整可见"
                );
                assert!(!caption.intersects(apply), "说明遮住应用按钮");
                assert!(!caption.intersects(cancel_rect), "说明遮住取消按钮");
                let error = output.shapes.iter().find_map(|shape| match &shape.shape {
                    egui::epaint::Shape::Text(text) if text.galley.text() == inspector.error => {
                        Some(egui::Rect::from_min_size(text.pos, text.galley.size()))
                    }
                    _ => None,
                });
                let error = error.expect("布局稳定后必须绘制真实错误文字");
                assert!(!caption.intersects(error), "说明遮住真实错误文字");
            }
        }
    }
}

#[test]
fn ordinary_chapter_caption_keeps_the_original_center_bottom_placement() {
    let ctx = egui::Context::default();
    for frame in 0..3 {
        ctx.begin_pass(egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1440., 900.),
            )),
            ..Default::default()
        });
        let rect = paint(
            &ctx,
            "简略演示 1/10",
            "原有节奏和位置",
            None,
            Duration::from_millis(450),
            None,
        );
        let mut output = ctx.end_pass();
        output.textures_delta.clear();
        if frame >= 2 {
            assert!((rect.center().x - 720.).abs() < 1.);
            assert!(rect.bottom() <= 887.);
        }
    }
}
