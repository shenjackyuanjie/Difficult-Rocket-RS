//! 真实输入边沿的提示采集、寿命与非交互绘制。
use super::*;

/// 提示寿命独立于演示节拍：fast 模式也有可见的停留与淡出。
pub(super) const KEY_HINT_HOLD: Duration = Duration::from_millis(250);
pub(super) const KEY_HINT_FADE: Duration = Duration::from_millis(650);

pub(super) struct KeyHint {
    pub(super) text: String,
    pub(super) applied: Instant,
}

impl KeyHint {
    pub(super) fn opacity(&self, now: Instant) -> f32 {
        let elapsed = now.saturating_duration_since(self.applied);
        if elapsed <= KEY_HINT_HOLD {
            1.
        } else {
            (1. - (elapsed - KEY_HINT_HOLD).as_secs_f32() / KEY_HINT_FADE.as_secs_f32())
                .clamp(0., 1.)
        }
    }
}

#[derive(Default)]
pub(super) struct KeyHints {
    pub(super) latest: Option<KeyHint>,
    // 只记已采集的边沿；即使 ButtonInput 在下一帧仍保留 just_pressed，也不重置寿命。
    pub(super) seen_edges: std::collections::HashSet<KeyCode>,
}

pub(super) fn is_modifier(name: &str) -> bool {
    matches!(
        name,
        "ControlLeft"
            | "ControlRight"
            | "ShiftLeft"
            | "ShiftRight"
            | "AltLeft"
            | "AltRight"
            | "SuperLeft"
            | "SuperRight"
    )
}

pub(super) fn button_key_name(key: KeyCode) -> String {
    use bevy_egui::egui;
    let name = format!("{key:?}");
    let name = name
        .strip_prefix("Key")
        .or_else(|| name.strip_prefix("Digit"))
        .unwrap_or(&name);
    egui::Key::from_name(name).map_or_else(|| name.to_owned(), |key| key.name().to_owned())
}

pub(super) fn key_combination(name: &str, modifiers: bevy_egui::egui::Modifiers) -> String {
    let mut parts = Vec::new();
    if modifiers.ctrl || (modifiers.command && !modifiers.mac_cmd) {
        parts.push("Ctrl");
    }
    if modifiers.alt {
        parts.push("Alt");
    }
    if modifiers.shift {
        parts.push("Shift");
    }
    if modifiers.mac_cmd {
        parts.push("Cmd");
    }
    parts.push(name);
    parts.join("+")
}

impl KeyHints {
    /// 必须在 Motion 的键与事件已重播之后调用；不读取待应用的 Motion 快照。
    pub(super) fn collect(
        &mut self,
        keys: &ButtonInput<KeyCode>,
        events: &[bevy_egui::egui::Event],
        now: Instant,
    ) -> Vec<String> {
        use bevy_egui::egui;
        let mut labels = BTreeSet::new();
        let mut event_keys = BTreeSet::new();
        for event in events {
            if let egui::Event::Key {
                key,
                physical_key,
                pressed,
                repeat,
                modifiers,
            } = event
            {
                // egui 的逻辑键用于显示，物理键用于和 ButtonInput 去重。
                // repeat/release 也占据该键，防止 fallback 把重复事件重新当作新按键。
                event_keys.insert(key.name().to_owned());
                event_keys.insert(physical_key.unwrap_or(*key).name().to_owned());
                if *pressed && !*repeat && !is_modifier(key.name()) {
                    labels.insert(key_combination(key.name(), *modifiers));
                }
            }
        }
        let modifiers = egui::Modifiers {
            ctrl: keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight),
            alt: keys.pressed(KeyCode::AltLeft) || keys.pressed(KeyCode::AltRight),
            shift: keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight),
            mac_cmd: keys.pressed(KeyCode::SuperLeft) || keys.pressed(KeyCode::SuperRight),
            ..Default::default()
        };
        self.seen_edges.retain(|key| keys.just_pressed(*key));
        for key in keys.get_just_pressed() {
            let fresh = self.seen_edges.insert(*key);
            let name = button_key_name(*key);
            if fresh && !is_modifier(&name) && !event_keys.contains(&name) {
                labels.insert(key_combination(&name, modifiers));
            }
        }
        let labels: Vec<_> = labels.into_iter().collect();
        if !labels.is_empty() {
            self.latest = Some(KeyHint {
                text: labels.join(" · "),
                applied: now,
            });
        }
        labels
    }
}

#[derive(Default)]
pub(super) struct MouseHints {
    pub(super) latest: Option<KeyHint>,
    pub(super) seen_edges: std::collections::HashSet<MouseButton>,
}

pub(super) fn mouse_label(button: MouseButton) -> Option<(&'static str, &'static str)> {
    match button {
        MouseButton::Left => Some(("left", "左键")),
        MouseButton::Middle => Some(("middle", "中键")),
        MouseButton::Right => Some(("right", "右键")),
        _ => None,
    }
}

impl MouseHints {
    pub(super) fn collect(
        &mut self,
        mouse: &ButtonInput<MouseButton>,
        events: &[bevy_egui::egui::Event],
        now: Instant,
    ) -> Vec<(&'static str, String)> {
        use bevy_egui::egui;
        let mut presses = BTreeSet::new();
        let mut event_buttons = BTreeSet::new();
        let mut actions = Vec::new();
        for event in events {
            match event {
                egui::Event::PointerButton {
                    button, pressed, ..
                } => {
                    let mapped = match button {
                        egui::PointerButton::Primary => Some(MouseButton::Left),
                        egui::PointerButton::Middle => Some(MouseButton::Middle),
                        egui::PointerButton::Secondary => Some(MouseButton::Right),
                        _ => None,
                    };
                    if let Some((kind, label)) = mapped.and_then(mouse_label) {
                        event_buttons.insert(kind);
                        if *pressed {
                            presses.insert((kind, label));
                        }
                    }
                }
                egui::Event::MouseWheel { delta, .. } if delta.length_sq() > 0. => {
                    actions.push(("scroll", format!("滚轮 ({:.0}, {:.0})", delta.x, delta.y)));
                }
                _ => {}
            }
        }
        self.seen_edges.retain(|button| mouse.just_pressed(*button));
        for button in mouse.get_just_pressed() {
            let fresh = self.seen_edges.insert(*button);
            if let Some((kind, label)) = mouse_label(*button)
                && fresh
                && !event_buttons.contains(kind)
            {
                presses.insert((kind, label));
            }
        }
        actions.extend(
            presses
                .into_iter()
                .map(|(kind, label)| (kind, label.to_owned())),
        );
        if !actions.is_empty() {
            self.latest = Some(KeyHint {
                text: actions
                    .iter()
                    .map(|(_, label)| label.as_str())
                    .collect::<Vec<_>>()
                    .join(" · "),
                applied: now,
            });
        }
        actions
    }
}

/// 仅向 egui pass 的图层添加形状，不创建 Area/控件，也不请求焦点或捕获输入。
pub(super) fn paint_key_hint(
    ctx: &bevy_egui::egui::Context,
    hint: &KeyHint,
    now: Instant,
    below_help: bool,
) {
    paint_input_hint(ctx, hint, now, below_help, false);
}

pub(super) fn paint_input_hint(
    ctx: &bevy_egui::egui::Context,
    hint: &KeyHint,
    now: Instant,
    below_help: bool,
    mouse: bool,
) {
    use bevy_egui::egui;
    let opacity = hint.opacity(now);
    if opacity <= 0. {
        return;
    }
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Tooltip,
        egui::Id::new(if mouse {
            "demo-mouse-hint"
        } else {
            "demo-key-hint"
        }),
    ));
    let color = egui::Color32::from_rgb(239, 226, 255).gamma_multiply(opacity);
    let galley = painter.layout_no_wrap(hint.text.clone(), egui::FontId::proportional(24.), color);
    let center = egui::pos2(
        ctx.content_rect().center().x,
        if below_help {
            ctx.content_rect().bottom()
                - 12.
                - (galley.size().y + 16.) / 2.
                - if mouse { 50. } else { 0. }
        } else {
            ctx.content_rect().top() + if mouse { 122. } else { 72. }
        },
    );
    let rect = egui::Rect::from_center_size(center, galley.size() + egui::vec2(28., 16.));
    painter.rect_filled(
        rect,
        7.,
        egui::Color32::from_rgb(25, 23, 34).gamma_multiply(opacity),
    );
    painter.galley(rect.min + egui::vec2(14., 8.), galley, color);
    ctx.request_repaint();
}
