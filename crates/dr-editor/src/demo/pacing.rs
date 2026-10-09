//! 动作分类及观看、快速回放的节拍计算。
use super::*;

/// 节拍只给真实动作留阅读时间，轮询/截图等技术等待不冒充重要操作。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Action {
    Technical,
    Navigation,
    Click,
    Key,
    Drag,
}

pub(super) fn fast(step: Duration) -> bool {
    step <= Duration::from_millis(60)
}

pub(super) fn action_pause(step: Duration, action: Action) -> Duration {
    if fast(step) {
        return step;
    }
    let (base_ms, min_ms, max_ms) = match action {
        Action::Technical => (80., 20., 160.),
        Action::Navigation => (120., 60., 240.),
        Action::Click => (650., 500., 900.),
        Action::Key => (700., 500., 900.),
        Action::Drag => (200., 100., 400.),
    };
    // 450ms 为观看基准，CLI 调节相对节奏，但重要操作仍保证可读范围。
    Duration::from_millis(
        (base_ms * step.as_secs_f64() / 0.45)
            .clamp(min_ms, max_ms)
            .round() as u64,
    )
}

pub(super) fn input_hold(step: Duration) -> Duration {
    if fast(step) {
        step / 2
    } else {
        Duration::from_millis(45)
    }
}

pub(super) fn chapter_pause(step: Duration, intro: bool) -> Duration {
    if fast(step) {
        step
    } else {
        Duration::from_millis(if intro { 1200 } else { 900 })
    }
}

pub(super) fn movement_duration(step: Duration, distance: f32) -> Duration {
    if fast(step) {
        Duration::ZERO
    } else {
        let millis = distance as f64 / 1.3 * step.as_secs_f64() / 0.45;
        Duration::from_millis(millis.clamp(200., 800.).round() as u64)
    }
}

pub(super) fn classify_action(
    moved: bool,
    before_mouse: &ButtonInput<MouseButton>,
    mouse: &ButtonInput<MouseButton>,
    keys: &ButtonInput<KeyCode>,
    events: &[bevy_egui::egui::Event],
) -> Action {
    use bevy_egui::egui;
    if keys
        .get_just_pressed()
        .any(|key| !is_modifier(&format!("{key:?}")))
        || events.iter().any(|event| {
            matches!(
                event,
                egui::Event::Key {
                    pressed: true,
                    repeat: false,
                    ..
                } | egui::Event::Text(_)
            )
        })
    {
        Action::Key
    } else if before_mouse.get_pressed().next().is_some() && moved {
        Action::Drag
    } else if mouse.get_just_pressed().next().is_some()
        || mouse.get_just_released().next().is_some()
        || events
            .iter()
            .any(|event| matches!(event, egui::Event::PointerButton { pressed: true, .. }))
    {
        Action::Click
    } else if moved
        || events
            .iter()
            .any(|event| matches!(event, egui::Event::MouseWheel { .. }))
    {
        Action::Navigation
    } else {
        Action::Technical
    }
}
