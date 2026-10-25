use crate::native_keys::*;
#[test]
fn shortcut_scenario_requires_actual_key_events_and_never_tests_focus_or_text_input() {
    assert!(STEPS.len() > 40);
    for (action, _) in STEPS {
        if action.starts_with("key:") {
            assert!(primary(action).is_some());
        } else {
            assert!(matches!(*action, "click" | "move" | "tree_click"));
        }
    }
    assert!(matches!(STEPS.last(), Some(("key:ctrl:N", Check::New))));
}
