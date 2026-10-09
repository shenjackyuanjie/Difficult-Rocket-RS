//! 内部指针和输入回放；隔离桌面误点，抵达后再应用边沿。
use super::*;

pub(super) struct Motion {
    pub(super) from: Vec2,
    pub(super) to: Vec2,
    pub(super) started: Instant,
    pub(super) duration: Duration,
    pub(super) ui: bool,
    pub(super) action: Action,
    pub(super) events: Vec<bevy_egui::egui::Event>,
    pub(super) mouse: ButtonInput<MouseButton>,
    pub(super) keys: ButtonInput<KeyCode>,
}

/// 只保存演示自身消费过的输入，下一帧不接受桌面误点或按键覆盖。
pub(super) struct ReplayInput {
    pub(super) mouse: ButtonInput<MouseButton>,
    pub(super) keys: ButtonInput<KeyCode>,
    pub(super) focused: bool,
}

impl Default for ReplayInput {
    fn default() -> Self {
        Self {
            mouse: default(),
            keys: default(),
            focused: true,
        }
    }
}

impl ReplayInput {
    pub(super) fn restore(
        &self,
        mouse: &mut ButtonInput<MouseButton>,
        keys: &mut ButtonInput<KeyCode>,
    ) {
        *mouse = self.mouse.clone();
        *keys = self.keys.clone();
        mouse.clear();
        keys.clear();
    }
}

/// 在所有 Update 注入之后消费节拍。截图观察者不受节拍限制。
pub(crate) fn pace(
    mut showcase: Option<ResMut<Showcase>>,
    windows: Query<&Window>,
    inputs: Query<&bevy_egui::EguiInput>,
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
) {
    if let Some(showcase) = showcase.as_mut() {
        showcase.pointer = windows.iter().next().and_then(Window::cursor_position);
        showcase.replay.mouse = mouse.clone();
        showcase.replay.keys = keys.clone();
        if let Some(window) = windows.iter().next() {
            showcase.replay.focused = window.focused;
        }
        for input in &inputs {
            for event in &input.0.events {
                if let bevy_egui::egui::Event::PointerMoved(pos) = event {
                    showcase.ui_pointer = Some(*pos);
                }
            }
        }
    }
    if let Some(showcase) = showcase.as_mut()
        && !showcase.initialize
        && showcase.finishing.is_none()
        && showcase.motion.is_none()
        && Instant::now() >= showcase.next_tick
    {
        // animate 已为实际动作或到达设置停顿；这里只消费无输入的技术步骤。
        showcase.next_tick = Instant::now() + action_pause(showcase.step, Action::Technical);
    }
}

/// 注入坐标只服务本帧编辑器；PostUpdate 后设 None，阻止 Winit 在 Last 移动系统光标。
/// 下一帧 Update 再恢复坐标，因此无需把桌面前台或系统鼠标当作演示先决条件。
pub(crate) fn restore_pointer(
    showcase: Option<ResMut<Showcase>>,
    mut windows: Query<&mut Window>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut inputs: Query<&mut bevy_egui::EguiInput>,
    wheels: Option<ResMut<Messages<bevy::input::mouse::MouseWheel>>>,
) {
    if let Some(mut showcase) = showcase
        && !showcase.completed
    {
        showcase.replay.restore(&mut mouse, &mut keys);
        showcase.before_mouse = mouse.clone();
        showcase.before_keys = keys.clone();
        // 原生鼠标、按键、文本和滚轮不得混入内部回放；章节稍后注入自己的输入。
        for mut input in &mut inputs {
            input.0.events.clear();
            input.0.focused = showcase.replay.focused;
        }
        if let Some(mut wheels) = wheels {
            wheels.clear();
        }
        for mut window in &mut windows {
            window.set_cursor_position(showcase.pointer);
            window.focused = showcase.replay.focused;
        }
    }
}
/// 线性插值的是编辑器实际使用的指针；按键/按钮边沿在到达目标时再应用。
/// egui 点击采用短按，观看节拍只控制下一动作，不把按钮一直按到下一节拍。
#[allow(clippy::too_many_arguments)]
pub(crate) fn animate(
    showcase: Option<ResMut<Showcase>>,
    mut windows: Query<&mut Window, With<bevy::window::PrimaryWindow>>,
    mut inputs: Query<&mut bevy_egui::EguiInput, With<bevy_egui::PrimaryEguiContext>>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
) {
    use bevy_egui::egui;
    let Some(mut showcase) = showcase else {
        return;
    };
    if showcase.completed || showcase.initialize {
        return;
    }
    let (Ok(mut window), Ok(mut input)) = (windows.single_mut(), inputs.single_mut()) else {
        return;
    };
    let now = Instant::now();
    let mut events = std::mem::take(&mut input.0.events);
    let mut applied_action = None;
    if let Some((deadline, held)) = &showcase.key_release
        && now >= *deadline
    {
        for key in held {
            keys.release(*key);
        }
        showcase.key_release = None;
    }
    if showcase.motion.is_none() {
        let ui_target = events.iter().rev().find_map(|event| match event {
            egui::Event::PointerMoved(point) => Some(Vec2::new(point.x, point.y)),
            _ => None,
        });
        // 显式 egui 坐标始终优先，包括在原位置释放的 PointerMoved。
        // smoke 保存的窗口坐标可能仍在画布/滚轮处，不能在短按期间回退过去。
        let ui = ui_target.is_some();
        let target = ui_target.or_else(|| window.cursor_position());
        if let Some(to) = target {
            let from = showcase
                .display_pointer
                .unwrap_or(Vec2::new(window.width() / 2., window.height() / 2.));
            let moved = from.distance(to) > 1.;
            let action = classify_action(moved, &showcase.before_mouse, &mouse, &keys, &events);
            if moved && !fast(showcase.step) {
                showcase.motion = Some(Motion {
                    from,
                    to,
                    started: now,
                    duration: movement_duration(showcase.step, from.distance(to)),
                    ui,
                    action,
                    events: std::mem::take(&mut events),
                    mouse: mouse.clone(),
                    keys: keys.clone(),
                });
                *mouse = showcase.before_mouse.clone();
                *keys = showcase.before_keys.clone();
            } else {
                // fast/原位点击也必须同步实际窗口指针，pace 才不会保存旧画布坐标。
                window.set_cursor_position(Some(to));
                showcase.display_pointer = Some(to);
                if now >= showcase.next_tick {
                    applied_action = Some(action);
                }
            }
        } else if now >= showcase.next_tick {
            applied_action = Some(classify_action(
                false,
                &showcase.before_mouse,
                &mouse,
                &keys,
                &events,
            ));
        }
    }
    if let Some(motion) = showcase.motion.as_ref() {
        let t =
            (motion.started.elapsed().as_secs_f32() / motion.duration.as_secs_f32()).clamp(0., 1.);
        let point = motion.from.lerp(motion.to, t);
        window.set_cursor_position(Some(point));
        showcase.display_pointer = Some(point);
        if t >= 1. {
            let motion = showcase.motion.take().unwrap();
            events.extend(motion.events);
            *mouse = motion.mouse;
            *keys = motion.keys;
            applied_action = Some(motion.action);
            if motion.ui {
                showcase.ui_pointer = Some(egui::pos2(point.x, point.y));
            }
        }
    }
    let point = showcase.display_pointer.or(window.cursor_position());
    if let Some(point) = point {
        // 每一帧保持 egui 的内部指针连续，不只更新说明区的装饰圆环。
        input
            .0
            .events
            .push(egui::Event::PointerMoved(egui::pos2(point.x, point.y)));
        showcase.ui_pointer = Some(egui::pos2(point.x, point.y));
    }
    if let Some((deadline, point)) = showcase.click_release
        && now >= deadline
    {
        input.0.events.push(egui::Event::PointerButton {
            pos: point,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        });
        showcase.click_release = None;
    }
    for event in &events {
        if let egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: true,
            ..
        } = event
        {
            showcase.click_release = Some((now + input_hold(showcase.step), *pos));
        }
    }
    if let Some(action) = applied_action {
        // 停顿从边沿实际重播/指针抵达后开始，而不是从注入帧开始。
        let pause_action = if action == Action::Technical && showcase.description_changed {
            Action::Key
        } else {
            action
        };
        showcase.next_tick = now + action_pause(showcase.step, pause_action);
        showcase.description_changed = false;
        let held: Vec<_> = keys
            .get_just_pressed()
            .copied()
            .filter(|key| !is_modifier(&format!("{key:?}")))
            .collect();
        if !held.is_empty() {
            showcase.key_release = Some((now + input_hold(showcase.step), held));
        }
    }
    let key_actions = showcase.key_hints.collect(&keys, &events, now);
    let mut actions = showcase.mouse_hints.collect(&mouse, &events, now);
    actions.extend(key_actions.into_iter().map(|label| ("key", label)));
    append_actions(
        &showcase.output,
        showcase.chapter(),
        &actions,
        SystemTime::now(),
    );
    input.0.events.extend(events);
}

pub(crate) fn isolate_pointer(showcase: Option<Res<Showcase>>, mut windows: Query<&mut Window>) {
    if showcase.is_some_and(|showcase| !showcase.completed) {
        for mut window in &mut windows {
            window.set_cursor_position(None);
        }
    }
}
