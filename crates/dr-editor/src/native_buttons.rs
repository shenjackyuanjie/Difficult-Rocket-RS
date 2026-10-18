//! 原生按钮到既有消息流的桥接：把 bevy_ui_widgets 的 Activate 事件变成有序的 Activated 消息。
//!
//! bevy 0.20 起按钮改用无样式 widget：按下与悬停状态分别由 bevy_ui::Pressed 与
//! bevy_picking::hover::Hovered 承载，激活以 Activate 事件通知。编辑器各面板只需要
//! 哪个实体被激活这一条消息，因此统一在这里桥接：消费者继续在 Update 里按帧有序消费，
//! 阻塞帧可以整帧丢弃消息，不必等到下一帧再处理。
//!
//! 另外，原生按钮只认鼠标主键：中键与右键的指针输入在 picking 的输入边界就被丢弃，
//! 既不会激活按钮，也不会打断左键按住的状态。触摸走独立的 GestureInput 抬指路径。
use crate::{audio, files, help, mobile_input, properties};
use bevy::picking::hover::Hovered;
use bevy::picking::input::PointerInputSettings;
use bevy::picking::pointer::{PointerAction, PointerButton, PointerId, PointerInput};
use bevy::picking::{Pickable, PickingSystems};
use bevy::prelude::*;
use bevy::ui::Pressed;
use bevy::ui_widgets::{Activate, ActivateOnPress, Button};

// 原版 sandbox 的中性灰控件与青柠强调色；原生入口和 egui 仍共用这一份配置。
pub(crate) const BUTTON: Color = Color::srgb(0.19, 0.19, 0.19);
pub(crate) const HOVERED: Color = Color::srgb(0.27, 0.27, 0.27);
pub(crate) const PRESSED: Color = Color::srgb(0.31, 0.38, 0.09);
pub(crate) const SELECTED: Color = Color::srgb(0.24, 0.29, 0.10);
pub(crate) const TEXT: Color = Color::srgb(0.90, 0.90, 0.90);
pub(crate) const PANEL: Color = Color::srgb(0.13, 0.13, 0.13);
pub(crate) const BORDER: Color = Color::srgb(0.07, 0.07, 0.07);
pub(crate) const ACCENT: Color = Color::srgb(0.70, 0.86, 0.02);
pub(crate) const BUTTON_FONT_SIZE: f32 = 16.0;
pub(crate) const RADIUS: u8 = 2;

/// 0.20 的圆角是 Node 的字段；尺寸、触控高度和命中范围仍由调用方决定。
pub(crate) fn native_corners() -> BorderRadius {
    BorderRadius::all(px(f32::from(RADIUS)))
}

/// 顶栏所有原生按钮共用状态色，包括属性入口；反馈本身不产生 Activate。
type ButtonVisuals = (
    Entity,
    Has<Pressed>,
    &'static Hovered,
    &'static mut BackgroundColor,
    &'static mut BorderColor,
);

pub(crate) fn update_native_buttons(
    mut buttons: Query<ButtonVisuals, With<Button>>,
    touch: Option<Res<mobile_input::GestureInput>>,
) {
    for (entity, pressed, hovered, mut color, mut border) in &mut buttons {
        let next = if let Some(touch) = touch.as_deref().filter(|input| input.override_mouse) {
            if touch.native_pressed == Some(entity) {
                PRESSED
            } else {
                BUTTON
            }
        } else if pressed {
            PRESSED
        } else if hovered.get() {
            HOVERED
        } else {
            BUTTON
        };
        if color.0 != next {
            color.0 = next;
        }
        border.set_if_neq(BorderColor::all(if next == BUTTON {
            BORDER
        } else {
            ACCENT
        }));
    }
}

#[cfg(test)]
pub(crate) mod test_support;
#[cfg(test)]
mod tests;

/// 按钮被激活。消费者按实体查询自己的动作组件。
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Activated {
    pub entity: Entity,
}

/// 注册按钮桥接：Activated 消息、Activate 观察者、指针输入边界过滤，并关闭 picking 的触摸输入。
pub(crate) struct NativeButtonsPlugin;

impl Plugin for NativeButtonsPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Activated>()
            // 不装 PickingPlugin 的 headless 场景也要有输入消息资源；与官方注册幂等。
            .add_message::<PointerInput>()
            .add_observer(observe_activation)
            // 触摸仍由独立的 GestureInput 在抬指时触发，不让 picking 再生成一套触摸指针事件。
            .insert_resource(PointerInputSettings {
                is_touch_enabled: false,
                ..default()
            })
            // PostInput 由 PickingPlugin 排在官方 Input 生成之后、PreUpdate 处理之前。
            .add_systems(
                First,
                drop_foreign_mouse_buttons.in_set(PickingSystems::PostInput),
            )
            .add_systems(
                Update,
                update_native_buttons
                    .after(mobile_input::input)
                    .before(help::input)
                    .before(properties::actions)
                    .before(files::toolbar_actions),
            );
    }
}

/// 按下即触发的按钮外壳：无样式 widget 加上悬停与 picking 命中拦截组件。
pub(crate) fn press_triggered() -> impl Bundle {
    (
        Button,
        ActivateOnPress,
        Hovered::default(),
        Pickable::default(),
        BorderColor::all(BORDER),
    )
}

/// 把 widget 的 Activate 事件桥接成有序消息。
///
/// 这里只看设备无关的 Activate 事件：picking 跑在 PreUpdate，而 GestureInput 在 Update 才更新，
/// 在这一层读 override 只会看到上一帧状态，从而丢掉手指抬起后的第一次真实鼠标按下。
/// 触摸接管期间是否采信，交给消费者在本帧输入更新之后判断。
fn observe_activation(
    event: On<Activate>,
    mut activated: MessageWriter<Activated>,
    mut sfx: Option<MessageWriter<audio::SfxEvent>>,
) {
    activated.write(Activated {
        entity: event.entity,
    });
    audio::emit(&mut sfx, audio::SfxEvent::Click);
}

/// 只让鼠标主键驱动原生按钮：中键与右键的按下、抬起在输入边界就被丢弃。
///
/// 运行在 First 的 PickingSystems::PostInput（官方指针输入生成之后、PreUpdate 处理之前），
/// 只重写本帧新写入的输入消息；其它指针、移动、滚动、取消一律保留。没有需要丢弃的消息时
/// 完全不碰消息缓冲，上一帧的消息不会变成新的输入再被处理一次。
fn drop_foreign_mouse_buttons(mut inputs: ResMut<Messages<PointerInput>>) {
    if !inputs
        .iter_current_update_messages()
        .any(is_foreign_mouse_button)
    {
        return;
    }
    let keep: Vec<PointerInput> = inputs
        .iter_current_update_messages()
        .filter(|input| !is_foreign_mouse_button(input))
        .cloned()
        .collect();
    inputs.clear();
    for input in keep {
        inputs.write(input);
    }
}

/// 是否是鼠标的非主键按下或抬起。其它指针与移动、滚动、取消都不属于这里。
fn is_foreign_mouse_button(input: &PointerInput) -> bool {
    if input.pointer_id != PointerId::Mouse {
        return false;
    }
    match input.action {
        PointerAction::Press(button) | PointerAction::Release(button) => {
            button != PointerButton::Primary
        }
        PointerAction::Move { .. } | PointerAction::Scroll { .. } | PointerAction::Cancel => false,
    }
}
