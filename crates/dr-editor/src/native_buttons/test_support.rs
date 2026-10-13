//! 测试用的指针事件构造：正式 Button 事件、真实窗口事件，以及手动后端命中。
use super::*;
use bevy::camera::NormalizedRenderTarget;
use bevy::input::ButtonState;
use bevy::input::mouse::{MouseButtonInput, MouseScrollUnit, MouseWheel};
use bevy::input::touch::TouchPhase;
use bevy::picking::backend::{HitData, PointerHits};
use bevy::picking::events::{Pointer, PointerPress};
use bevy::picking::pointer::{Location, PointerButton, PointerId};
use bevy::window::{CursorMoved, WindowEvent};

fn pointer() -> Pointer {
    Pointer::new(
        PointerId::Mouse,
        Location {
            // 非窗口目标：事件自动传播时不会去访问可能不存在的窗口实体。
            target: NormalizedRenderTarget::None {
                width: 1440,
                height: 900,
            },
            position: Vec2::ZERO,
        },
    )
}

fn hit(camera: Entity) -> HitData {
    HitData::new(camera, 0.0, None, None)
}

pub(crate) fn press(entity: Entity, camera: Entity) -> PointerPress {
    PointerPress {
        entity,
        pointer: pointer(),
        button: PointerButton::Primary,
        hit: hit(camera),
        count: 1,
    }
}

/// 真实窗口按键事件：交给官方 mouse_pick_events 生成指针输入。
pub(crate) fn mouse_button_event(
    window: Entity,
    button: MouseButton,
    state: ButtonState,
) -> WindowEvent {
    WindowEvent::MouseButtonInput(MouseButtonInput {
        button,
        state,
        window,
    })
}

/// 真实光标移动事件。
pub(crate) fn cursor_moved_event(window: Entity, position: Vec2) -> WindowEvent {
    WindowEvent::CursorMoved(CursorMoved {
        window,
        position,
        delta: Some(Vec2::ZERO),
    })
}

/// 真实滚轮事件。
pub(crate) fn mouse_wheel_event(window: Entity) -> WindowEvent {
    WindowEvent::MouseWheel(MouseWheel {
        unit: MouseScrollUnit::Line,
        x: 0.0,
        y: 1.0,
        window,
        phase: TouchPhase::Moved,
    })
}

/// 手动后端命中：替代真实 UI 后端把鼠标指针指向目标实体。
pub(crate) fn pointer_hits(target: Entity, camera: Entity) -> PointerHits {
    PointerHits::new(
        PointerId::Mouse,
        vec![(target, HitData::new(camera, 0.0, None, None))],
        0.0,
    )
}
