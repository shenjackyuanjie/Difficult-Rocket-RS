use super::*;

struct Panel {
    ctx: egui::Context,
    inspector: Inspector,
    document: EditorDocument,
    hits: Vec<(Action, egui::Rect)>,
}

impl Panel {
    fn new() -> Self {
        let mut document = crate::tests::document();
        document.ship = dr_core::ship_from_xml(
            r#"<Ship><Parts><Part id="1" partType="pod"><Pod name="原名称" throttle="0"><Staging currentStage="0"/></Pod></Part></Parts></Ship>"#,
        )
        .unwrap();
        document.saved_ship = document.ship.clone();
        let mut panel = Self {
            ctx: egui::Context::default(),
            inspector: Inspector::default(),
            document,
            hits: vec![],
        };
        open(&mut panel.inspector, &panel.document);
        for _ in 0..3 {
            panel.frame(vec![]);
        }
        panel
    }

    fn frame(&mut self, events: Vec<egui::Event>) {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(960.0, 640.0),
            )),
            events,
            focused: true,
            ..Default::default()
        };
        self.ctx.begin_pass(input);
        self.hits.clear();
        if let Some(action) = show(
            &self.ctx,
            &mut self.inspector,
            &self.document,
            &mut self.hits,
        ) {
            act(&action, &mut self.inspector, &mut self.document);
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
