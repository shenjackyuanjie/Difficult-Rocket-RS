//! TouchInput 的小型适配层：独立指针状态复用画布编辑，不污染真实鼠标输入。
use super::*;
use bevy::input::touch::{TouchInput, TouchPhase};
use bevy::window::{PrimaryWindow, WindowFocused};
use std::collections::BTreeMap;

#[cfg(test)]
mod tests;

#[derive(Clone, Debug)]
pub(crate) enum NativeClick {
    Menu,
    File(files::FileAction),
    Properties,
    Help,
}

#[derive(Clone)]
struct NativeTarget {
    entity: Entity,
    action: NativeClick,
}

#[derive(Clone, Copy)]
struct Contact {
    position: Vec2,
    over_ui: bool,
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Session {
    #[default]
    Idle,
    Single(u64),
    Navigation,
    Blocked,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct NavigationDelta {
    pub from: Vec2,
    pub to: Vec2,
    /// 正交投影的比例：手指张开时值小于 1。
    pub scale_ratio: f32,
}

#[derive(Resource, Default)]
pub(crate) struct GestureInput {
    pub buttons: ButtonInput<MouseButton>,
    pub position: Option<Vec2>,
    pub press_position: Option<Vec2>,
    pub override_mouse: bool,
    pub navigation: Option<NavigationDelta>,
    pub native_click: Option<NativeClick>,
    /// 当前真实触点仍位于起点锁定按钮时的按下反馈，不参与按钮提交。
    pub native_pressed: Option<Entity>,
    contacts: BTreeMap<u64, Contact>,
    session: Session,
    target: Option<NativeTarget>,
    mouse_tail: bool,
}

impl GestureInput {
    pub fn blocked(&self) -> bool {
        self.override_mouse && !matches!(self.session, Session::Single(_) | Session::Idle)
    }

    fn pair(&self) -> Option<(Vec2, f32)> {
        let mut contacts = self.contacts.values();
        let a = contacts.next()?.position;
        let b = contacts.next()?.position;
        Some(((a + b) * 0.5, a.distance(b)))
    }

    fn navigate(&mut self, before: Option<(Vec2, f32)>) {
        let (Some((from, old_distance)), Some((to, new_distance))) = (before, self.pair()) else {
            return;
        };
        let ratio = if old_distance > 1.0 && new_distance > 1.0 {
            old_distance / new_distance
        } else {
            1.0
        };
        if let Some(delta) = &mut self.navigation {
            delta.to = to;
            delta.scale_ratio *= ratio;
        } else {
            self.navigation = Some(NavigationDelta {
                from,
                to,
                scale_ratio: ratio,
            });
        }
    }

    fn block(&mut self) {
        self.session = Session::Blocked;
        self.buttons.reset_all();
        self.navigation = None;
        self.target = None;
    }
}

fn cancel_preview(drag: &mut DragState, cursor: &mut EditorCursor, camera_drag: &mut CameraDrag) {
    canvas_input::cancel_preview(drag, cursor, camera_drag);
    // 触摸取消后需等待下一次有效触点，不能沿用取消前的画布位置。
    cursor.valid = false;
}

/// 起点决定整次触摸归属；UI 滚动越过边界也不会变成画布拖拽。
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(crate) fn input(
    mut events: MessageReader<TouchInput>,
    mut focus: MessageReader<WindowFocused>,
    windows: Query<(Entity, &Window), With<PrimaryWindow>>,
    panels: Query<(&ComputedNode, &UiGlobalTransform), With<panels::EditorPanel>>,
    native_buttons: Query<
        (
            Entity,
            &ComputedNode,
            &UiGlobalTransform,
            Option<&files::FileButton>,
            Option<&files::MenuButton>,
            Option<&properties::Action>,
            Option<&help::HelpButton>,
        ),
        With<Button>,
    >,
    egui_state: Option<Res<panels::egui_panel::UiState>>,
    modal: (
        Option<Res<properties::Inspector>>,
        Option<Res<topology_ui::ConnectionEditor>>,
        Option<Res<files::PendingFileAction>>,
        Option<Res<help::HelpState>>,
        Option<Res<connection_lines::Settings>>,
    ),
    mouse: Res<ButtonInput<MouseButton>>,
    mut state: ResMut<GestureInput>,
    mut drag: ResMut<DragState>,
    mut cursor: ResMut<EditorCursor>,
    mut camera_drag: ResMut<CameraDrag>,
) {
    state.buttons.clear();
    state.press_position = None;
    state.navigation = None;
    state.native_click = None;
    state.native_pressed = None;
    state.override_mouse = !state.contacts.is_empty() || state.mouse_tail;
    if state.contacts.is_empty() && !state.mouse_tail {
        state.session = Session::Idle;
        state.position = None;
    }
    let Ok((window_entity, window)) = windows.single() else {
        events.clear();
        focus.clear();
        return;
    };
    let lost_focus = focus
        .read()
        .any(|event| event.window == window_entity && !event.focused);
    if lost_focus || !window.focused {
        // 已接管的触摸会话保留兼容鼠标尾锁；纯鼠标失焦交给原编辑路径取消。
        if state.override_mouse {
            cancel_preview(&mut drag, &mut cursor, &mut camera_drag);
            state.block();
            state.contacts.clear();
            state.mouse_tail = mouse.pressed(MouseButton::Left);
        }
        events.clear();
        return;
    }
    let modal_open = modal.0.is_some_and(|state| state.is_open())
        || modal.1.is_some_and(|state| state.open)
        || modal.2.is_some_and(|state| state.is_blocked())
        || modal
            .3
            .is_some_and(|state| state.open || state.suppress_frame)
        || modal.4.is_some_and(|state| state.open);
    if modal_open && !state.contacts.is_empty() && state.session != Session::Blocked {
        state.block();
        cancel_preview(&mut drag, &mut cursor, &mut camera_drag);
    }
    for event in events.read().filter(|event| event.window == window_entity) {
        state.override_mouse = true;
        if !event.position.is_finite() {
            continue;
        }
        let physical = event.position * window.scale_factor();
        let over_ui = modal_open || panels::over_ui(physical, &panels, egui_state.as_deref());
        match event.phase {
            TouchPhase::Started => {
                if state.contacts.contains_key(&event.id) {
                    continue;
                }
                let first = state.contacts.is_empty();
                state.contacts.insert(
                    event.id,
                    Contact {
                        position: event.position,
                        over_ui,
                    },
                );
                if first {
                    state.target = None;
                    state.position = Some(event.position);
                    if over_ui {
                        state.session = Session::Blocked;
                        state.buttons.reset_all();
                        if !modal_open {
                            state.target = native_buttons.iter().find_map(
                                |(entity, node, transform, file, menu, property, help)| {
                                    if !node.contains_point(*transform, physical) {
                                        return None;
                                    }
                                    let action = if menu.is_some() {
                                        NativeClick::Menu
                                    } else if let Some(file) = file {
                                        NativeClick::File(file.0.clone())
                                    } else if matches!(property, Some(properties::Action::Open)) {
                                        NativeClick::Properties
                                    } else if help.is_some() {
                                        NativeClick::Help
                                    } else {
                                        return None;
                                    };
                                    Some(NativeTarget { entity, action })
                                },
                            );
                        }
                    } else {
                        // 触摸接管尚未提交的鼠标拖拽，但保留目录待放置部件。
                        drag.cancel();
                        camera_drag.0 = None;
                        state.session = Session::Single(event.id);
                        state.press_position = Some(event.position);
                        state.buttons.press(MouseButton::Left);
                    }
                } else {
                    state.buttons.reset_all();
                    state.target = None;
                    cancel_preview(&mut drag, &mut cursor, &mut camera_drag);
                    if state.contacts.len() == 2
                        && state.session != Session::Blocked
                        && !state.contacts.values().any(|contact| contact.over_ui)
                    {
                        state.session = Session::Navigation;
                    } else {
                        state.block();
                    }
                }
            }
            TouchPhase::Moved | TouchPhase::Ended => {
                let before = state.pair();
                let Some(contact) = state.contacts.get_mut(&event.id) else {
                    continue;
                };
                contact.position = event.position;
                if state.session == Session::Navigation {
                    state.navigate(before);
                }
                if state.session == Session::Single(event.id) {
                    state.position = Some(event.position);
                }
                if event.phase == TouchPhase::Ended {
                    if state.session == Session::Single(event.id) {
                        state.buttons.release(MouseButton::Left);
                    } else if state.contacts.len() == 1
                        && let Some(target) = state.target.take()
                        && let Ok((_, node, transform, ..)) = native_buttons.get(target.entity)
                        && node.contains_point(*transform, physical)
                    {
                        state.native_click = Some(target.action);
                    }
                    state.contacts.remove(&event.id);
                    // Navigation/Blocked 保留至下一帧，释放帧不能交给鼠标或单指编辑。
                    if state.contacts.is_empty() && matches!(state.session, Session::Single(_)) {
                        state.session = Session::Idle;
                    }
                }
            }
            TouchPhase::Canceled => {
                state.contacts.remove(&event.id);
                state.block();
                cancel_preview(&mut drag, &mut cursor, &mut camera_drag);
            }
        }
    }
    if state.contacts.len() == 1
        && let Some(target) = &state.target
        && let Some(contact) = state.contacts.values().next()
        && let Ok((_, node, transform, ..)) = native_buttons.get(target.entity)
        && node.contains_point(*transform, contact.position * window.scale_factor())
    {
        state.native_pressed = Some(target.entity);
    }
    if state.override_mouse {
        // 同设备兼容鼠标事件只能在真实左键释放后重新参与；不修改 ButtonInput。
        state.mouse_tail = mouse.pressed(MouseButton::Left);
    }
    if state.contacts.is_empty() && state.mouse_tail && !mouse.pressed(MouseButton::Left) {
        state.mouse_tail = false;
        state.override_mouse = true;
    }
}
