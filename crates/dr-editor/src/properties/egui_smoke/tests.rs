
use super::*;
use crate::egui_ui::UiHits;

fn rect(y: f32) -> egui::Rect {
    egui::Rect::from_min_size(egui::pos2(12.0, y), egui::vec2(44.0, 44.0))
}

fn assert_click_at(input: &EguiInput, rect: egui::Rect) {
    assert!(matches!(
        input.0.events.as_slice(),
        [
            egui::Event::PointerMoved(moved),
            egui::Event::PointerButton {
                pos: pressed,
                button: egui::PointerButton::Primary,
                pressed: true,
                ..
            },
            egui::Event::PointerButton {
                pos: released,
                button: egui::PointerButton::Primary,
                pressed: false,
                ..
            }
        ] if *moved == rect.center()
            && *pressed == rect.center()
            && *released == rect.center()
    ));
}

#[test]
fn smoke_name_focus_navigates_default_advanced_before_advancing() {
    let toggle = rect(12.0);
    let field = rect(72.0);
    let mut state = State {
        phase: 1,
        ..default()
    };
    let mut input = EguiInput::default();
    let mut hits = UiHits {
        actions: vec![(Action::ToggleAdvanced, toggle)],
        ..default()
    };
    assert!(!click(
        Action::Focus(Field::Name),
        &hits,
        &mut input,
        &mut state
    ));
    assert_eq!(state.phase, 1, "导航点击不得推进到文本注入阶段");
    assert_click_at(&input, toggle);
    assert!(state.input.tick(&mut input));
    assert!(state.input.tick(&mut input));
    assert!(!state.input.tick(&mut input));
    input.0.events.clear();
    hits.advanced_open = true;
    hits.actions = vec![(Action::Focus(Field::Name), field)];
    assert!(click(
        Action::Focus(Field::Name),
        &hits,
        &mut input,
        &mut state
    ));
    assert_click_at(&input, field);
}

#[test]
fn smoke_moved_navigates_real_activation_context_before_advancing() {
    let tile = rect(120.0);
    let moved = rect(200.0);
    let mut state = State {
        phase: 5,
        ..default()
    };
    let mut input = EguiInput::default();
    let mut hits = UiHits {
        actions: vec![(Action::SelectActivation(0, 0), tile)],
        ..default()
    };
    assert!(!click(Action::Moved(0, 0), &hits, &mut input, &mut state));
    assert_eq!(state.phase, 5, "选择缩略图不得当作已改变 moved");
    assert_click_at(&input, tile);
    assert!(state.input.tick(&mut input));
    assert!(state.input.tick(&mut input));
    assert!(!state.input.tick(&mut input));
    input.0.events.clear();
    hits.selected_activation = Some((0, 0));
    hits.actions = vec![(Action::Moved(0, 0), moved)];
    assert!(click(Action::Moved(0, 0), &hits, &mut input, &mut state));
    assert_click_at(&input, moved);
}
