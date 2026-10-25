use crate::document::EditorDocument;
use crate::properties::egui_panel::*;
use crate::render;
use dr_core::PartKey;

struct Panel {
    ctx: egui::Context,
    inspector: Inspector,
    document: EditorDocument,
    hits: Vec<(Action, egui::Rect)>,
    size: egui::Vec2,
    dock: Option<Dock>,
    body: egui::Rect,
    last_action: Option<Action>,
    time: f64,
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
            body: egui::Rect::NOTHING,
            last_action: None,
            time: 0.0,
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
        self.frame_focused(events, true);
    }

    fn frame_focused(&mut self, events: Vec<egui::Event>, focused: bool) {
        self.time += 1.0 / 60.0;
        self.last_action = None;
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, self.size)),
            events,
            focused,
            time: Some(self.time),
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
            self.body = shown.body;
            self.last_action = shown.action.clone();
            if let Some(action) = shown.action {
                act(&action, &mut self.inspector, &mut self.document);
            }
        }
        // 无头控件测试不上传 GPU 纹理，显式确认丢弃字体图集增量。
        self.ctx.end_pass().textures_delta.clear();
    }

    fn click(&mut self, action: Action) {
        if !self.hits.iter().any(|(candidate, _)| *candidate == action) {
            match action {
                Action::Focus(Field::Target) | Action::CycleTarget(_)
                    if !self.inspector.targets_open =>
                {
                    self.click(Action::ToggleTargets)
                }
                Action::Focus(_) | Action::Active if !self.inspector.advanced_open => {
                    self.click(Action::ToggleAdvanced);
                }
                Action::Moved(stage, index) | Action::RemoveActivation(stage, index) => {
                    self.click(Action::SelectActivation(stage, index));
                }
                Action::MoveStep(stage, _) => self.click(Action::SelectStage(stage)),
                _ => {}
            }
        }
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

    fn rect(&self, action: Action) -> egui::Rect {
        self.hits
            .iter()
            .find(|(candidate, _)| *candidate == action)
            .unwrap_or_else(|| panic!("正式控件不可见：{action:?}"))
            .1
    }

    fn pointer(&mut self, pos: egui::Pos2, pressed: bool) {
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

    fn begin_drag(&mut self, source: egui::Pos2) {
        self.pointer(source, true);
        assert!(self.last_action.is_none());
        self.frame(vec![egui::Event::PointerMoved(
            source + egui::vec2(12.0, 0.0),
        )]);
        assert!(self.last_action.is_none());
        assert!(
            self.ctx
                .data_mut(|data| data.get_temp::<StagingDrag>(drag_id()))
                .is_some(),
            "正式 click_and_drag 必须跨帧启动拖动"
        );
    }

    fn drag_to(&mut self, source: egui::Pos2, destination: egui::Pos2) {
        self.begin_drag(source);
        self.frame(vec![egui::Event::PointerMoved(destination)]);
        assert!(self.last_action.is_none());
        self.pointer(destination, false);
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
        let available =
            egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(size.x, size.y - 56.0));
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
fn activation_tiles_keep_icons_proportional_and_only_show_the_short_id() {
    for width in [240.0, 268.0, 430.0] {
        for size in [
            Vec2::new(129.0, 30.0),
            Vec2::new(30.0, 100.0),
            Vec2::splat(32.0),
        ] {
            let ctx = crate::egui_ui::test_context();
            activation_frame(&ctx, width, size, vec![]);
            let frame = activation_frame(&ctx, width, size, vec![]);
            assert_eq!(frame.hits.len(), 1);
            assert_eq!(frame.hits[0].0, Action::SelectActivation(0, 0));
            let tile = frame.hits[0].1;
            assert!((tile.width() - 56.0).abs() < 0.01);
            assert!((tile.height() - 56.0).abs() < 0.01);
            let mut icon = None;
            let mut labels = 0;
            for shape in frame.output.shapes {
                match shape.shape {
                    egui::Shape::Rect(rect)
                        if rect.brush.as_ref().is_some_and(|brush| {
                            brush.fill_texture_id == egui::TextureId::User(321)
                        }) =>
                    {
                        icon = Some(rect.rect)
                    }
                    egui::Shape::Text(text) if !text.galley.job.text.is_empty() => {
                        labels += 1;
                        assert_eq!(text.galley.job.text, "#23");
                        assert_eq!(text.galley.rows.len(), 1);
                        assert!(tile.expand(1.0).contains_rect(text.visual_bounding_rect()));
                    }
                    _ => {}
                }
            }
            assert_eq!(labels, 1, "长名称留在 tooltip，卡片仅显示短编号");
            let icon = icon.expect("必须实际绘制部件贴图");
            let expected = size * (32.0 / size.max_element());
            assert!((icon.width() - expected.x).abs() < 1.01);
            assert!((icon.height() - expected.y).abs() < 1.01);
            assert!(tile.contains_rect(icon));
        }
    }
}

#[test]
fn activation_tile_click_and_selected_edit_controls_have_distinct_real_targets() {
    let ctx = crate::egui_ui::test_context();
    let size = Vec2::new(129.0, 30.0);
    activation_frame(&ctx, 268.0, size, vec![]);
    let frame = activation_frame(&ctx, 268.0, size, vec![]);
    let tile = frame.hits[0].1;
    for pos in [
        tile.center(),
        egui::pos2(tile.right() - 12.0, tile.center().y),
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
                if pressed {
                    None
                } else {
                    Some(Action::SelectActivation(0, 0))
                }
            );
        }
    }
    let mut panel = Panel::new();
    panel.click(Action::AddStep);
    panel.inspector.draft.as_mut().unwrap().target = "1".into();
    panel.click(Action::AddActivation(0));
    panel.click(Action::SelectActivation(0, 0));
    let select = panel.rect(Action::SelectActivation(0, 0));
    let moved = panel.rect(Action::Moved(0, 0));
    let remove = panel.rect(Action::RemoveActivation(0, 0));
    assert!(!select.intersects(moved) && !moved.intersects(remove));
    assert!(moved.height() >= 43.99 && remove.height() >= 43.99);
    panel.click(Action::Moved(0, 0));
    assert!(plan(&panel).steps[0].activations[0].moved);
    panel.click(Action::RemoveActivation(0, 0));
    assert!(plan(&panel).steps[0].activations.is_empty());
    assert!(!panel.document.history.can_undo());
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

fn plan(panel: &Panel) -> &StagingState {
    panel
        .inspector
        .draft
        .as_ref()
        .unwrap()
        .staging
        .as_ref()
        .unwrap()
}

fn ids(panel: &Panel) -> Vec<Vec<i64>> {
    plan(panel)
        .steps
        .iter()
        .map(|step| step.activations.iter().map(|a| a.id).collect())
        .collect()
}

fn generated_panel() -> Panel {
    let mut panel = Panel::at_size(egui::vec2(1440.0, 900.0));
    panel.document.catalog = dr_core::catalog_from_xml(
        r#"<PartTypes><PartType id="pod" type="pod"/>
        <PartType id="tank" type="tank"><Tank fuel="20"/></PartType>
        <PartType id="engine" type="engine"><Engine/></PartType>
        <PartType id="detacher" type="detacher"/></PartTypes>"#,
    )
    .unwrap();
    panel.document.ship = dr_core::Ship {
        parts: [
            (1, "pod"),
            (2, "tank"),
            (3, "engine"),
            (4, "detacher"),
            (5, "tank"),
            (6, "engine"),
            (7, "detacher"),
            (8, "tank"),
            (9, "engine"),
        ]
        .into_iter()
        .map(|(id, kind)| {
            panel
                .document
                .catalog
                .get(kind)
                .unwrap()
                .instantiate(id, (0.0, 0.0))
        })
        .collect(),
        connections: (1..9)
            .map(|parent| dr_core::Connection::Normal {
                parent,
                child: parent + 1,
                parent_attach: 0,
                child_attach: 1,
            })
            .collect(),
        ..default()
    };
    panel.document.saved_ship = panel.document.ship.clone();
    panel.document.select_only(Some(PartKey::new(0, 1, 0)));
    panel.inspector = Inspector::default();
    open(&mut panel.inspector, &panel.document);
    for _ in 0..3 {
        panel.frame(vec![]);
    }
    panel.click(Action::GenerateStaging);
    assert!(panel.inspector.error.is_empty());
    assert_eq!(
        ids(&panel),
        vec![vec![9], vec![7], vec![6], vec![4], vec![3]]
    );
    assert!(!panel.document.history.can_undo());
    panel
}

#[test]
fn compact_dock_defaults_to_six_real_icons_and_explicit_property_drawers() {
    let mut panel = Panel::new();
    for action in [
        Action::GenerateStaging,
        Action::AddStep,
        Action::ToggleTargets,
        Action::ToggleAdvanced,
        Action::Apply,
        Action::Cancel,
    ] {
        let rect = panel.rect(action);
        assert!(rect.width() >= 43.99 && rect.height() >= 43.99);
    }
    assert!(!panel.inspector.advanced_open && !panel.inspector.targets_open);
    assert!(
        !panel
            .hits
            .iter()
            .any(|(action, _)| matches!(action, Action::Focus(_) | Action::Active))
    );
    panel.click(Action::ToggleAdvanced);
    assert!(
        panel
            .hits
            .iter()
            .any(|(action, _)| *action == Action::Focus(Field::Name))
    );
    panel.click(Action::ToggleAdvanced);
    panel.click(Action::ToggleTargets);
    assert!(
        panel
            .hits
            .iter()
            .any(|(action, _)| *action == Action::Focus(Field::Target))
    );
    panel.click(Action::ToggleTargets);
    assert!(!panel.document.history.can_undo());
}

#[test]
fn four_frame_activation_drag_uses_real_centers_and_header_tail_then_one_apply() {
    let mut panel = generated_panel();
    let before = panel.document.ship.clone();
    let source = panel.rect(Action::SelectActivation(2, 0)).center();
    let destination = panel.rect(Action::SelectStage(1)).center();
    let original = plan(&panel).clone();
    panel.begin_drag(source);
    assert_eq!(plan(&panel), &original);
    panel.frame(vec![egui::Event::PointerMoved(destination)]);
    assert_eq!(plan(&panel), &original);
    assert!(panel.last_action.is_none());
    panel.pointer(destination, false);
    assert_eq!(
        panel.last_action,
        Some(Action::MoveActivation {
            from_stage: 2,
            index: 0,
            to_stage: 1,
            to_index: 1,
        })
    );
    assert_eq!(
        ids(&panel),
        vec![vec![9], vec![7, 6], vec![], vec![4], vec![3]]
    );
    assert!(plan(&panel).steps[1].activations[1].moved);
    assert!(!plan(&panel).steps[1].activations[0].moved);
    assert_eq!(panel.document.ship, before);
    assert!(!panel.document.history.can_undo());
    panel.frame(vec![]);
    panel.click(Action::Apply);
    assert!(!panel.inspector.is_open());
    let after = panel.document.ship.clone();
    assert_eq!(panel.document.history.undo_len(), 1);
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
fn four_frame_stage_drag_inserts_before_or_after_and_keeps_activation_flags() {
    for after in [false, true] {
        let mut panel = generated_panel();
        panel
            .inspector
            .draft
            .as_mut()
            .unwrap()
            .staging
            .as_mut()
            .unwrap()
            .steps[0]
            .activations[0]
            .moved = true;
        panel.frame(vec![]);
        let before = panel.document.ship.clone();
        let source = panel.rect(Action::SelectStage(0)).center();
        let target = panel.rect(Action::SelectStage(2));
        let destination = egui::pos2(
            target.center().x,
            if after {
                target.bottom() - 5.0
            } else {
                target.top() + 5.0
            },
        );
        panel.drag_to(source, destination);
        let to = if after { 2 } else { 1 };
        assert_eq!(panel.last_action, Some(Action::ReorderStep { from: 0, to }));
        let expected = if after {
            vec![vec![7], vec![6], vec![9], vec![4], vec![3]]
        } else {
            vec![vec![7], vec![9], vec![6], vec![4], vec![3]]
        };
        assert_eq!(ids(&panel), expected);
        assert!(plan(&panel).steps[to].activations[0].moved);
        assert!(
            plan(&panel)
                .steps
                .iter()
                .filter(|step| step.activations[0].id != 9)
                .all(|step| !step.activations[0].moved)
        );
        assert_eq!(panel.document.ship, before);
        assert!(!panel.document.history.can_undo());
    }
}

#[test]
fn same_card_tile_drag_uses_final_insertion_index_and_keeps_moved_flags() {
    let mut panel = generated_panel();
    panel.inspector.preview = Preview::Owner;
    panel.inspector.draft.as_mut().unwrap().staging = Some(StagingState {
        current_stage: 0,
        steps: vec![StageStep {
            activations: vec![
                Activation {
                    id: 9,
                    moved: false,
                },
                Activation { id: 7, moved: true },
                Activation {
                    id: 6,
                    moved: false,
                },
            ],
        }],
    });
    panel.frame(vec![]);
    let source = panel.rect(Action::SelectActivation(0, 0)).center();
    let target = panel.rect(Action::SelectActivation(0, 2));
    panel.drag_to(source, egui::pos2(target.right() - 3.0, target.center().y));
    assert_eq!(
        panel.last_action,
        Some(Action::MoveActivation {
            from_stage: 0,
            index: 0,
            to_stage: 0,
            to_index: 2,
        })
    );
    assert_eq!(
        plan(&panel).steps[0].activations,
        vec![
            Activation { id: 7, moved: true },
            Activation {
                id: 6,
                moved: false
            },
            Activation {
                id: 9,
                moved: false
            },
        ]
    );
    assert!(!panel.document.history.can_undo());
}

#[test]
fn escape_cancels_only_the_active_drag_and_a_later_escape_discards_the_draft() {
    let mut panel = generated_panel();
    let before = panel.document.ship.clone();
    let original = plan(&panel).clone();
    let source = panel.rect(Action::SelectActivation(2, 0)).center();
    let destination = panel.rect(Action::SelectStage(1)).center();
    panel.begin_drag(source);
    panel.frame(vec![
        egui::Event::PointerMoved(destination),
        key(egui::Key::Escape, egui::Modifiers::NONE),
    ]);
    assert!(panel.inspector.is_open());
    assert!(panel.last_action.is_none());
    assert_eq!(plan(&panel), &original);
    assert!(
        panel
            .ctx
            .data_mut(|data| data.get_temp::<StagingDrag>(drag_id()))
            .is_none()
    );
    panel.pointer(destination, false);
    assert!(panel.inspector.is_open());
    assert_eq!(plan(&panel), &original);
    panel.frame(vec![key(egui::Key::Escape, egui::Modifiers::NONE)]);
    assert!(!panel.inspector.is_open());
    assert_eq!(panel.document.ship, before);
    assert!(!panel.document.history.can_undo());
}

#[test]
fn escape_in_the_drag_threshold_frame_keeps_the_draft_and_cancels_release() {
    for action in [Action::SelectActivation(2, 0), Action::SelectStage(0)] {
        let mut panel = generated_panel();
        let before = panel.document.ship.clone();
        let original = plan(&panel).clone();
        let source = panel.rect(action).center();
        let destination = panel.rect(Action::SelectStage(1)).center();
        panel.pointer(source, true);
        assert!(panel.last_action.is_none());
        assert!(
            panel
                .ctx
                .data_mut(|data| data.get_temp::<StagingDrag>(press_id()))
                .is_some(),
            "正式卡片或部件控件应在跨起拖阈值前记录按下来源"
        );
        panel.frame(vec![
            egui::Event::PointerMoved(source + egui::vec2(12.0, 0.0)),
            key(egui::Key::Escape, egui::Modifiers::NONE),
        ]);
        assert!(panel.inspector.is_open(), "起拖同帧的 Esc 只能取消拖动");
        assert!(panel.last_action.is_none());
        assert!(
            panel
                .ctx
                .data_mut(|data| data.get_temp::<StagingDrag>(press_id()))
                .is_none(),
            "Esc 应同时清除待起拖来源"
        );
        assert_eq!(plan(&panel), &original);
        assert!(
            panel
                .ctx
                .data_mut(|data| data.get_temp::<StagingDrag>(drag_id()))
                .is_none()
        );
        panel.pointer(destination, false);
        assert!(panel.inspector.is_open());
        assert!(panel.last_action.is_none());
        assert_eq!(plan(&panel), &original, "取消后的释放不能提交拖放");
        panel.frame(vec![key(egui::Key::Escape, egui::Modifiers::NONE)]);
        assert!(
            !panel.inspector.is_open(),
            "手势取消后后续独立 Esc 仍可关闭草稿"
        );
        assert_eq!(panel.document.ship, before);
        assert!(!panel.document.history.can_undo());
    }
}

#[test]
fn canvas_release_focus_loss_pointer_gone_and_touch_cancel_never_commit_a_drag() {
    for cancellation in 0..4 {
        let mut panel = generated_panel();
        let original = plan(&panel).clone();
        let before = panel.document.ship.clone();
        let source = panel.rect(Action::SelectActivation(2, 0)).center();
        let destination = panel.rect(Action::SelectStage(1)).center();
        panel.begin_drag(source);
        match cancellation {
            0 => panel.pointer(panel.dock.unwrap().canvas.center(), false),
            1 => panel.frame_focused(vec![egui::Event::WindowFocused(false)], false),
            2 => panel.frame(vec![egui::Event::PointerGone]),
            _ => panel.frame(vec![egui::Event::Touch {
                device_id: egui::TouchDeviceId(1),
                id: egui::TouchId(1),
                phase: egui::TouchPhase::Cancel,
                pos: destination,
                force: None,
            }]),
        }
        assert!(panel.last_action.is_none());
        assert!(panel.inspector.is_open());
        assert_eq!(plan(&panel), &original);
        assert!(
            panel
                .ctx
                .data_mut(|data| data.get_temp::<StagingDrag>(drag_id()))
                .is_none()
        );
        panel.pointer(destination, false);
        assert_eq!(plan(&panel), &original, "取消后释放不能重新提交");
        assert_eq!(panel.document.ship, before);
        assert!(!panel.document.history.can_undo());
    }
}

#[test]
fn narrow_activation_tiles_wrap_and_the_sixteenth_tile_remains_clickable() {
    let mut panel = Panel::at_size(egui::vec2(390.0, 844.0));
    let part = panel.document.ship.parts[0].clone();
    for id in 2..=16 {
        let mut activation_part = part.clone();
        activation_part.id = id;
        activation_part.pod = None;
        panel.document.ship.parts.push(activation_part);
    }
    panel.document.ship.parts[0].pod.as_mut().unwrap().staging = Some(StagingState {
        current_stage: 0,
        steps: vec![StageStep {
            activations: (1..=16).map(|id| Activation { id, moved: false }).collect(),
        }],
    });
    panel.document.saved_ship = panel.document.ship.clone();
    panel.inspector = Inspector::default();
    open(&mut panel.inspector, &panel.document);
    for _ in 0..3 {
        panel.frame(vec![]);
    }
    let before = panel.document.ship.clone();
    let original = plan(&panel).clone();
    let first = panel.rect(Action::SelectActivation(0, 0));
    let last = panel.rect(Action::SelectActivation(0, 15));
    assert!(
        last.top() > first.bottom(),
        "16 个 tile 必须真实换行，而非横向越出正文"
    );
    for index in 0..16 {
        let rect = panel.rect(Action::SelectActivation(0, index));
        assert!(
            panel.body.contains_rect(rect),
            "第 {index} 个正式命中必须在正文内：{rect:?}"
        );
        assert_eq!(
            rect.size(),
            egui::vec2(56.0, 56.0),
            "第 {index} 个 tile 不能只剩裁剪碎片"
        );
    }
    panel.click(Action::SelectActivation(0, 15));
    assert_eq!(panel.inspector.preview, Preview::Activation(0, 15));
    assert_eq!(plan(&panel), &original);
    assert_eq!(panel.document.ship, before);
    assert!(!panel.document.history.can_undo());
}

#[test]
fn clipped_stage_hits_and_real_edge_autoscroll_keep_long_drafts_atomic() {
    let mut panel = Panel::new();
    panel.inspector.preview = Preview::Owner;
    panel.inspector.draft.as_mut().unwrap().staging = Some(StagingState {
        current_stage: 0,
        steps: (0..40)
            .map(|_| StageStep {
                activations: vec![Activation {
                    id: 1,
                    moved: false,
                }],
            })
            .collect(),
    });
    for _ in 0..3 {
        panel.frame(vec![]);
    }
    assert!(
        !panel
            .hits
            .iter()
            .any(|(action, _)| *action == Action::SelectStage(39))
    );
    let maximum_stage = |panel: &Panel| {
        panel
            .hits
            .iter()
            .filter_map(|(action, _)| {
                if let Action::SelectStage(stage) = action {
                    Some(*stage)
                } else {
                    None
                }
            })
            .max()
            .unwrap()
    };
    let initial_max = maximum_stage(&panel);
    let before = plan(&panel).clone();
    let source = panel.rect(Action::SelectActivation(0, 0)).center();
    panel.begin_drag(source);
    let edge = egui::pos2(panel.body.left() + 30.0, panel.body.bottom() - 2.0);
    for _ in 0..80 {
        panel.frame(vec![egui::Event::PointerMoved(edge)]);
        for (action, rect) in &panel.hits {
            if matches!(
                action,
                Action::SelectStage(_) | Action::SelectActivation(_, _)
            ) {
                assert!(
                    panel.body.contains_rect(*rect),
                    "裁剪外不能注册命中：{action:?} {rect:?}"
                );
            }
        }
        assert_eq!(plan(&panel), &before);
        assert!(panel.last_action.is_none());
    }
    assert!(
        maximum_stage(&panel) > initial_max,
        "保持在真实正文下边缘必须向下滚动"
    );
    panel.pointer(panel.dock.unwrap().canvas.center(), false);
    assert_eq!(plan(&panel), &before);
    for _ in 0..12 {
        panel.frame(vec![
            egui::Event::PointerMoved(panel.body.center()),
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                phase: egui::TouchPhase::Move,
                delta: egui::vec2(0.0, -2000.0),
                modifiers: egui::Modifiers::NONE,
            },
        ]);
    }
    panel.click(Action::SelectActivation(39, 0));
    for _ in 0..8 {
        panel.frame(vec![egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            phase: egui::TouchPhase::Move,
            delta: egui::vec2(0.0, -200.0),
            modifiers: egui::Modifiers::NONE,
        }]);
    }
    panel.click(Action::Moved(39, 0));
    assert!(plan(&panel).steps[39].activations[0].moved);
    assert!(!panel.document.history.can_undo());
}
