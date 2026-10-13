use super::*;

struct Panel {
    ctx: egui::Context,
    inspector: Inspector,
    document: EditorDocument,
    hits: Vec<(Action, egui::Rect)>,
    size: egui::Vec2,
    dock: Option<Dock>,
}

impl Panel {
    fn new() -> Self {
        Self::at_size(egui::vec2(960.0, 640.0))
    }

    fn at_size(size: egui::Vec2) -> Self {
        let mut document = crate::test_support::document();
        document.ship = dr_core::ship_from_xml(
            r#"<Ship><Parts><Part id="1" partType="pod"><Pod name="原名称" throttle="0"><Staging currentStage="0"/></Pod></Part></Parts></Ship>"#,
        )
        .unwrap();
        document.saved_ship = document.ship.clone();
        let mut panel = Self {
            ctx: crate::egui_ui::test_context(),
            inspector: Inspector::default(),
            document,
            hits: vec![],
            size,
            dock: None,
        };
        // 与目录 draw 的窄屏共享样式一致，使用真实停靠 Panel。
        if size.x < 960.0 {
            panel.ctx.style_mut_of(panel.ctx.theme(), |style| {
                style.spacing.interact_size =
                    style.spacing.interact_size.max(egui::Vec2::splat(44.0));
            });
        }
        open(&mut panel.inspector, &panel.document);
        for _ in 0..3 {
            panel.frame(vec![]);
        }
        panel
    }

    fn frame(&mut self, events: Vec<egui::Event>) {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, self.size)),
            events,
            focused: true,
            ..Default::default()
        };
        self.ctx.begin_pass(input);
        self.hits.clear();
        if let Some(shown) = show(
            &self.ctx,
            &mut self.inspector,
            &self.document,
            &mut self.hits,
            self.ctx.content_rect(),
            &mut |kind| {
                (
                    egui::TextureId::User(321),
                    render::fallback_size(Some(kind)),
                )
            },
        ) {
            self.dock = shown.dock;
            if let Some(action) = shown.action {
                act(&action, &mut self.inspector, &mut self.document);
            }
        }
        // 无头控件测试不上传 GPU 纹理，显式确认丢弃字体图集增量。
        self.ctx.end_pass().textures_delta.clear();
    }

    fn click(&mut self, action: Action) {
        let rect = self
            .hits
            .iter()
            .find(|(candidate, _)| *candidate == action)
            .expect("实际控件不可见")
            .1;
        let pos = rect.center();
        for pressed in [true, false] {
            self.frame(vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ]);
        }
        self.frame(vec![]);
    }
}

fn key(key: egui::Key, modifiers: egui::Modifiers) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: false,
        modifiers,
    }
}

#[test]
fn text_edit_replaces_unicode_in_draft_and_applies_as_one_transaction() {
    let mut panel = Panel::new();
    let before = panel.document.ship.clone();
    panel.click(Action::Focus(Field::Name));
    assert_eq!(panel.inspector.focus, Some(Field::Name));
    panel.frame(vec![
        key(egui::Key::A, egui::Modifiers::COMMAND),
        egui::Event::Text("中文 🚀 & <船体>".into()),
    ]);
    assert_eq!(
        panel.inspector.draft.as_ref().unwrap().name,
        "中文 🚀 & <船体>"
    );
    assert_eq!(panel.document.ship, before);
    assert!(!panel.document.history.can_undo());
    panel.click(Action::Apply);
    assert!(!panel.inspector.is_open());
    let after = panel.document.ship.clone();
    assert_eq!(
        after.parts[0].pod.as_ref().unwrap().name,
        "中文 🚀 & <船体>"
    );
    assert_eq!(
        dr_core::ship_from_xml(&dr_core::ship_to_xml(&after).unwrap()).unwrap(),
        after
    );
    assert!(panel.document.undo());
    assert_eq!(panel.document.ship, before);
    assert!(!panel.document.history.can_undo());
    assert!(panel.document.redo());
    assert_eq!(panel.document.ship, after);
}

#[test]
fn text_edit_navigation_deletes_whole_unicode_characters_only_in_draft() {
    let mut panel = Panel::new();
    let before = panel.document.ship.clone();
    panel.click(Action::Focus(Field::Name));
    panel.frame(vec![
        key(egui::Key::A, egui::Modifiers::COMMAND),
        egui::Event::Text("甲🚀乙".into()),
    ]);
    panel.frame(vec![key(egui::Key::ArrowLeft, egui::Modifiers::NONE)]);
    panel.frame(vec![key(egui::Key::Backspace, egui::Modifiers::NONE)]);
    assert_eq!(panel.inspector.draft.as_ref().unwrap().name, "甲乙");
    panel.frame(vec![key(egui::Key::Delete, egui::Modifiers::NONE)]);
    assert_eq!(panel.inspector.draft.as_ref().unwrap().name, "甲");
    assert_eq!(panel.document.ship, before);
    assert!(!panel.document.history.can_undo());
    panel.click(Action::Cancel);
    assert_eq!(panel.document.ship, before);
}

#[test]
fn ime_cancel_with_escape_does_not_discard_draft() {
    let mut panel = Panel::new();
    let before = panel.document.ship.clone();
    panel.click(Action::Focus(Field::Name));
    panel.frame(vec![egui::Event::Ime(egui::ImeEvent::Preedit {
        text: "huojian".into(),
        active_range_chars: Some(0..7),
    })]);
    panel.frame(vec![
        egui::Event::Ime(egui::ImeEvent::Preedit {
            text: String::new(),
            active_range_chars: None,
        }),
        key(egui::Key::Escape, egui::Modifiers::NONE),
    ]);
    assert!(panel.inspector.is_open());
    assert_eq!(panel.document.ship, before);
    panel.frame(vec![]);
    panel.frame(vec![key(egui::Key::Escape, egui::Modifiers::NONE)]);
    assert!(!panel.inspector.is_open());
    assert_eq!(panel.document.ship, before);
    assert!(!panel.document.history.can_undo());
}

#[test]
fn backdrop_keeps_draft_and_explicit_cancel_does_not_create_history() {
    let mut panel = Panel::new();
    let before = panel.document.ship.clone();
    panel.click(Action::Active);
    assert!(panel.inspector.draft.as_ref().unwrap().active);
    let pos = egui::pos2(5.0, 5.0);
    for pressed in [true, false] {
        panel.frame(vec![
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            },
        ]);
    }
    assert!(panel.inspector.is_open());
    panel.click(Action::Cancel);
    assert!(!panel.inspector.is_open());
    assert_eq!(panel.document.ship, before);
    assert!(!panel.document.history.can_undo());
}

#[test]
fn invalid_throttle_keeps_visible_apply_cancel_and_document_at_minimum_size() {
    let mut panel = Panel::new();
    let before = panel.document.ship.clone();
    panel.click(Action::Focus(Field::Throttle));
    panel.frame(vec![
        key(egui::Key::A, egui::Modifiers::COMMAND),
        egui::Event::Text("NaN".into()),
    ]);
    panel.click(Action::Apply);
    assert!(panel.inspector.is_open());
    assert!(!panel.inspector.error.is_empty());
    assert_eq!(panel.document.ship, before);
    for action in [Action::Apply, Action::Cancel] {
        let rect = panel
            .hits
            .iter()
            .find(|(candidate, _)| *candidate == action)
            .unwrap()
            .1;
        assert!(rect.min.x >= 0.0 && rect.max.x <= 960.0);
        assert!(rect.min.y >= 0.0 && rect.max.y <= 640.0);
    }
    panel.click(Action::Cancel);
    assert_eq!(panel.document.ship, before);
}

#[test]
fn compact_staging_buttons_keep_boundary_hits_and_draft_transaction() {
    let mut panel = Panel::new();
    let before = panel.document.ship.clone();
    panel.click(Action::AddStep);
    panel.click(Action::CycleTarget(true));
    panel.click(Action::AddActivation(0));
    panel.click(Action::AddStep);
    assert!(
        !panel
            .hits
            .iter()
            .any(|(a, _)| *a == Action::MoveStep(0, false))
    );
    assert!(
        !panel
            .hits
            .iter()
            .any(|(a, _)| *a == Action::MoveStep(1, true))
    );
    panel.click(Action::MoveStep(0, true));
    let staging = panel
        .inspector
        .draft
        .as_ref()
        .unwrap()
        .staging
        .as_ref()
        .unwrap();
    assert!(staging.steps[0].activations.is_empty());
    assert_eq!(staging.steps[1].activations[0].id, 1);
    panel.click(Action::RemoveStep(0));
    assert_eq!(panel.document.ship, before, "结构按钮只能修改草稿");
    panel.click(Action::Apply);
    assert_eq!(
        panel.document.ship.parts[0]
            .pod
            .as_ref()
            .unwrap()
            .staging
            .as_ref()
            .unwrap()
            .steps
            .len(),
        1
    );
    assert_eq!(panel.document.history.undo_len(), 1);
    assert!(panel.document.undo());
    assert_eq!(panel.document.ship, before);
}

#[test]
fn disabled_stage_arrow_explains_boundary_without_exposing_hit() {
    let ctx = crate::egui_ui::test_context();
    let mut rect = egui::Rect::NOTHING;
    let mut saw_hint = false;
    for frame in 0..6 {
        let mut hits = vec![];
        let mut pending = None;
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(600.0, 240.0),
                )),
                time: Some(f64::from(frame)),
                // 移入一次后推进静止帧；重复 Move 会重置 egui 的悬停等待。
                events: if frame == 2 {
                    vec![egui::Event::PointerMoved(rect.center())]
                } else {
                    vec![]
                },
                ..Default::default()
            },
            |ui| {
                ui.add_enabled_ui(false, |ui| {
                    action_button(
                        ui,
                        "↑ 上移",
                        Action::MoveStep(0, false),
                        &mut hits,
                        &mut pending,
                    );
                });
                // 唯一控件的矩形来自 egui 实际布局，不是硬编码坐标。
                rect = ui.min_rect();
            },
        );
        output.textures_delta.clear();
        assert!(hits.is_empty());
        assert!(pending.is_none());
        fn has_hint(shape: &egui::epaint::Shape) -> bool {
            match shape {
                egui::epaint::Shape::Text(text) => text.galley.job.text.contains("首级不可上移"),
                egui::epaint::Shape::Vec(shapes) => shapes.iter().any(has_hint),
                _ => false,
            }
        }
        saw_hint |= output.shapes.iter().any(|shape| has_hint(&shape.shape));
    }
    assert!(saw_hint, "禁用的首级上移按钮必须实际生成可用条件 tooltip");
}

fn assert_touch_dock_controls_visible(panel: &Panel) {
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, panel.size);
    let dock = panel.dock.expect("驾驶舱分级必须采用真实停靠布局");
    assert!(screen.expand(0.5).contains_rect(dock.panel));
    assert!(screen.expand(0.5).contains_rect(dock.canvas));
    assert!(!dock.panel.intersect(dock.canvas).is_positive());
    assert!(dock.canvas.width() >= 96.0 && dock.canvas.height() >= 96.0);
    for action in [Action::Apply, Action::Cancel] {
        let rect = panel
            .hits
            .iter()
            .find(|(candidate, _)| *candidate == action)
            .expect("应用与取消必须实际注册可见命中矩形")
            .1;
        assert!(
            screen.contains_rect(rect),
            "{action:?} 在 {screen:?} 内必须可达：{rect:?}"
        );
        assert!(rect.height() >= 43.99, "{action:?} 触摸高度不足：{rect:?}");
    }
    assert!(
        panel
            .hits
            .iter()
            .all(|(_, rect)| screen.contains_rect(*rect))
    );
}

#[test]
fn staging_dock_apply_and_cancel_remain_reachable_on_touch_screens() {
    for size in [
        egui::vec2(320.0, 640.0),
        egui::vec2(390.0, 844.0),
        egui::vec2(640.0, 360.0),
    ] {
        for choice in [Action::Apply, Action::Cancel] {
            let mut panel = Panel::at_size(size);
            let before = panel.document.ship.clone();
            panel.inspector.draft.as_mut().unwrap().name = "窄屏属性草稿".into();
            panel.frame(vec![]);
            assert_touch_dock_controls_visible(&panel);
            panel.click(choice.clone());
            assert!(!panel.inspector.is_open());
            if choice == Action::Apply {
                assert_eq!(
                    panel.document.ship.parts[0].pod.as_ref().unwrap().name,
                    "窄屏属性草稿"
                );
                assert_eq!(panel.document.history.undo_len(), 1);
                assert!(panel.document.undo());
            } else {
                assert!(!panel.document.history.can_undo());
            }
            assert_eq!(panel.document.ship, before);
        }
    }
}

#[test]
fn staging_dock_validation_error_keeps_touch_apply_cancel_reachable() {
    for size in [
        egui::vec2(320.0, 640.0),
        egui::vec2(390.0, 844.0),
        egui::vec2(640.0, 360.0),
    ] {
        let mut panel = Panel::at_size(size);
        let before = panel.document.ship.clone();
        panel.inspector.draft.as_mut().unwrap().throttle = "NaN".into();
        panel.frame(vec![]);
        panel.click(Action::Apply);
        assert!(panel.inspector.is_open());
        assert!(!panel.inspector.error.is_empty());
        assert_touch_dock_controls_visible(&panel);
        assert_eq!(panel.document.ship, before);
        assert!(!panel.document.history.can_undo());
        panel.click(Action::Cancel);
        assert!(!panel.inspector.is_open());
        assert_eq!(panel.document.ship, before);
    }
}

#[test]
fn staging_dock_reserves_real_toolbar_space_and_never_paints_a_modal_backdrop() {
    for size in [
        egui::vec2(1440.0, 900.0),
        egui::vec2(390.0, 844.0),
        egui::vec2(320.0, 640.0),
        egui::vec2(640.0, 360.0),
    ] {
        let mut panel = Panel::at_size(size);
        let top = crate::panels::toolbar_height(size.x) + 80.0;
        let available = egui::Rect::from_min_max(egui::pos2(0.0, top), size.to_pos2());
        for _ in 0..3 {
            panel.ctx.begin_pass(egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
                ..default()
            });
            let mut hits = vec![];
            let shown = show(
                &panel.ctx,
                &mut panel.inspector,
                &panel.document,
                &mut hits,
                available,
                &mut |kind| {
                    (
                        egui::TextureId::User(321),
                        render::fallback_size(Some(kind)),
                    )
                },
            )
            .unwrap();
            let dock = shown.dock.expect("分级必须停靠");
            assert!(available.contains_rect(shown.body));
            if size == egui::vec2(640.0, 360.0) {
                assert!(
                    shown.body.height() >= 44.0,
                    "短横屏正文至少完整显示一个动作行"
                );
            }
            assert!(
                available.contains_rect(dock.panel),
                "{size:?} available={available:?}, dock={dock:?}"
            );
            assert!(available.contains_rect(dock.canvas));
            assert!(!dock.panel.intersect(dock.canvas).is_positive());
            assert!(dock.canvas.width() >= 96.0 && dock.canvas.height() >= 96.0);
            for action in [Action::Apply, Action::Cancel] {
                let rect = hits
                    .iter()
                    .find(|(value, _)| *value == action)
                    .expect("预留工具栏后的应用/取消必须可达")
                    .1;
                assert!(
                    available.contains_rect(rect),
                    "{size:?} {action:?}: {rect:?}"
                );
            }

            assert!(
                panel
                    .ctx
                    .memory(|memory| memory.area_rect(egui::Id::new("part_properties_modal")))
                    .is_none()
            );
            panel.ctx.end_pass().textures_delta.clear();
        }
    }
}

#[test]
fn actual_staging_action_click_selects_its_ship_target_without_editing_the_document() {
    let mut panel = Panel::new();
    panel.document.catalog = crate::test_support::catalog();
    let target = panel
        .document
        .catalog
        .get("tank")
        .unwrap()
        .instantiate(2, (4.0, 0.0));
    panel.document.ship.parts.push(target);
    panel
        .inspector
        .draft
        .as_mut()
        .unwrap()
        .staging
        .as_mut()
        .unwrap()
        .steps
        .push(StageStep {
            activations: vec![Activation {
                id: 2,
                moved: false,
            }],
        });
    let before = panel.document.ship.clone();
    panel.frame(vec![]);
    panel.click(Action::SelectActivation(0, 0));
    assert_eq!(
        panel.inspector.preview_keys(&panel.document),
        vec![PartKey::new(0, 2, 0)]
    );
    assert_eq!(panel.document.ship, before);
    assert!(!panel.document.history.can_undo());
}

#[test]
fn hovering_an_actual_activation_temporarily_highlights_its_part_and_restores_the_selection() {
    let mut panel = Panel::new();
    panel.document.catalog = crate::test_support::catalog();
    for id in [2, 3] {
        panel.document.ship.parts.push(
            panel
                .document
                .catalog
                .get("tank")
                .unwrap()
                .instantiate(id, (id as f64 * 4.0, 0.0)),
        );
    }
    panel
        .inspector
        .draft
        .as_mut()
        .unwrap()
        .staging
        .as_mut()
        .unwrap()
        .steps
        .push(StageStep {
            activations: vec![
                Activation {
                    id: 2,
                    moved: false,
                },
                Activation {
                    id: 3,
                    moved: false,
                },
            ],
        });
    let before = panel.document.ship.clone();
    let selected = panel.document.selected;
    panel.frame(vec![]);
    panel.click(Action::SelectActivation(0, 0));
    let second = panel
        .hits
        .iter()
        .find(|(action, _)| *action == Action::SelectActivation(0, 1))
        .unwrap()
        .1;
    panel.frame(vec![egui::Event::PointerMoved(second.center())]);
    assert_eq!(panel.inspector.hovered, Some(Preview::Activation(0, 1)));
    assert_eq!(panel.inspector.preview, Preview::Activation(0, 0));
    assert_eq!(
        panel.inspector.preview_keys(&panel.document),
        vec![PartKey::new(0, 3, 0)]
    );
    panel.frame(vec![egui::Event::PointerMoved(
        panel.dock.unwrap().canvas.center(),
    )]);
    assert_eq!(panel.inspector.hovered, None);
    assert_eq!(
        panel.inspector.preview_keys(&panel.document),
        vec![PartKey::new(0, 2, 0)]
    );
    assert_eq!(panel.document.ship, before);
    assert_eq!(panel.document.selected, selected);
    assert!(!panel.document.history.can_undo());
}

struct ActivationFrame {
    output: egui::FullOutput,
    hits: Vec<(Action, egui::Rect)>,
    pending: Option<Action>,
}

fn activation_frame(
    ctx: &egui::Context,
    width: f32,
    size: Vec2,
    events: Vec<egui::Event>,
) -> ActivationFrame {
    let mut hits = vec![];
    let mut pending = None;
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(width + 16.0, 100.0),
            )),
            events,
            focused: true,
            ..default()
        },
        |ui| {
            ui.set_width(width);
            activation_row(
                ui,
                (
                    0,
                    0,
                    &Activation {
                        id: 23,
                        moved: false,
                    },
                ),
                (
                    "Long Long Long Long Long Long Long Part",
                    None,
                    Some((egui::TextureId::User(321), size)),
                ),
                Preview::Owner,
                &mut hits,
                &mut pending,
                &mut None,
            );
        },
    );
    output.textures_delta.clear();
    ActivationFrame {
        output,
        hits,
        pending,
    }
}

#[test]
fn activation_columns_keep_icons_proportional_and_every_label_on_one_line() {
    for width in [240.0, 268.0, 430.0] {
        for size in [
            Vec2::new(129.0, 30.0),
            Vec2::new(30.0, 100.0),
            Vec2::splat(32.0),
        ] {
            let ctx = crate::egui_ui::test_context();
            activation_frame(&ctx, width, size, vec![]);
            let frame = activation_frame(&ctx, width, size, vec![]);
            assert_eq!(frame.hits.len(), 3);
            let columns: Vec<_> = frame.hits.iter().map(|(_, rect)| *rect).collect();
            for rect in &columns {
                assert!(rect.height() >= 43.99);
                assert!((rect.center().y - columns[0].center().y).abs() < 0.01);
            }
            assert!(columns[0].right() < columns[1].left());
            assert!(columns[1].right() < columns[2].left());
            assert!((columns[2].right() - columns[0].left() - width).abs() < 0.01);
            assert!((columns[1].width() - 64.0).abs() < 0.01);
            assert!((columns[2].width() - 56.0).abs() < 0.01);
            let mut icon = None;
            let mut labels = 0;
            for shape in frame.output.shapes {
                match shape.shape {
                    egui::Shape::Rect(rect)
                        if rect.brush.as_ref().is_some_and(|brush| {
                            brush.fill_texture_id == egui::TextureId::User(321)
                        }) =>
                    {
                        icon = Some(rect.rect);
                    }
                    egui::Shape::Text(text) if !text.galley.job.text.is_empty() => {
                        labels += 1;
                        assert_eq!(text.galley.rows.len(), 1, "文字不能换行");
                        let column = if text.galley.job.text == "移动" {
                            columns[1]
                        } else if text.galley.job.text == "移除" {
                            columns[2]
                        } else {
                            columns[0]
                        };
                        assert!(
                            column
                                .expand(1.0)
                                .contains_rect(text.visual_bounding_rect()),
                            "文字不得侵占相邻列：{}",
                            text.galley.job.text
                        );
                    }
                    _ => {}
                }
            }
            assert_eq!(labels, 3);
            let icon = icon.expect("必须实际绘制部件贴图");
            let expected = size * (32.0 / size.max_element());
            assert!((icon.width() - expected.x).abs() < 1.01);
            assert!((icon.height() - expected.y).abs() < 1.01);
            assert!(columns[0].contains_rect(icon));
        }
    }
}

#[test]
fn activation_icon_name_and_edit_controls_have_distinct_real_click_targets() {
    let ctx = crate::egui_ui::test_context();
    let size = Vec2::new(129.0, 30.0);
    activation_frame(&ctx, 268.0, size, vec![]);
    let frame = activation_frame(&ctx, 268.0, size, vec![]);
    let select = frame.hits[0].1;
    for (pos, action) in [
        (
            egui::pos2(select.left() + 22.0, select.center().y),
            Action::SelectActivation(0, 0),
        ),
        (
            egui::pos2(select.right() - 12.0, select.center().y),
            Action::SelectActivation(0, 0),
        ),
        (frame.hits[1].1.center(), Action::Moved(0, 0)),
        (frame.hits[2].1.center(), Action::RemoveActivation(0, 0)),
    ] {
        for pressed in [true, false] {
            let frame = activation_frame(
                &ctx,
                268.0,
                size,
                vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            );
            assert_eq!(
                frame.pending,
                if pressed { None } else { Some(action.clone()) }
            );
        }
    }
}

#[test]
fn target_marks_use_current_camera_dpi_rotation_and_clip_outside_the_ship_area() {
    let mut panel = Panel::new();
    panel.document.ship.parts[0].angle = 0.37;
    panel.inspector.draft = Some(Draft::new(
        PartKey::new(0, 1, 0),
        &panel.document.ship.parts[0],
    ));
    for dpi in [1.0, 2.0] {
        for scale in [0.5, 2.0] {
            let window = Window {
                resolution: bevy::window::WindowResolution::new(960, 640)
                    .with_scale_factor_override(dpi),
                ..default()
            };
            let camera = Transform::from_xyz(60.0, 30.0, 0.0);
            let projection = Projection::Orthographic(OrthographicProjection {
                scale,
                ..OrthographicProjection::default_2d()
            });
            panel.ctx.begin_pass(egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, panel.size)),
                ..default()
            });
            let canvas = egui::Rect::from_min_size(egui::Pos2::ZERO, panel.size);
            let marks = paint_targets(
                &panel.ctx,
                &panel.inspector,
                &panel.document,
                canvas,
                (&window, &camera, &projection),
                |_| Some(Vec2::new(100.0, 60.0)),
            );
            assert_eq!(marks.len(), 1);
            let factor = window.scale_factor() / panel.ctx.pixels_per_point();
            let expected = egui::pos2(
                (window.width() * 0.5 - 60.0 / scale) * factor,
                (window.height() * 0.5 + 30.0 / scale) * factor,
            );
            assert!((marks[0].1.center() - expected).length() < 0.001);
            assert!(marks[0].1.width() > 0.0 && marks[0].1.height() > 0.0);
            let outside =
                egui::Rect::from_min_size(egui::pos2(900.0, 600.0), egui::vec2(10.0, 10.0));
            assert!(
                paint_targets(
                    &panel.ctx,
                    &panel.inspector,
                    &panel.document,
                    outside,
                    (&window, &camera, &projection),
                    |_| Some(Vec2::new(100.0, 60.0))
                )
                .is_empty()
            );
            let mut output = panel.ctx.end_pass();
            assert!(!output.shapes.is_empty());
            assert!(output.shapes.iter().any(|shape| matches!(&shape.shape,
                egui::Shape::Path(path) if path.closed && path.points.len() == 4 && path.fill.a() > 0)),
                "对应部件必须实际绘制半透明高亮，不只记录命中矩形");
            for shape in &output.shapes {
                if let egui::Shape::Text(text) = &shape.shape {
                    assert!(
                        canvas.contains_rect(text.visual_bounding_rect()),
                        "ID 标签不能被船体区边缘裁切"
                    );
                }
            }
            output.textures_delta.clear();
        }
    }
}
