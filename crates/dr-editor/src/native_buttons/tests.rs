use super::*;
use crate::mobile_input::GestureInput;
use bevy::input::ButtonState;
use bevy::input::mouse::MouseButton;
use bevy::picking::backend::PointerHits;
use bevy::picking::input::{mouse_pick_events, spawn_mouse_pointer};
use bevy::picking::pointer::PointerAction;
use bevy::picking::{InteractionPlugin, PickingPlugin};
use bevy::ui::Pressed;
use bevy::ui_widgets::ButtonPlugin;
use bevy::window::{PrimaryWindow, WindowEvent};

#[derive(Resource, Default)]
struct ActivatedHits(Vec<Entity>);

fn collect_activated(mut reader: MessageReader<Activated>, mut hits: ResMut<ActivatedHits>) {
    hits.0.extend(reader.read().map(|event| event.entity));
}

/// 本帧读到的指针输入：核对被过滤的按键没进处理链，也确认上一帧不会重发。
#[derive(Resource, Default)]
struct PinLog(Vec<PointerInput>);

fn clear_pin_log(mut log: ResMut<PinLog>) {
    log.0.clear();
}

fn log_pointer_inputs(mut reader: MessageReader<PointerInput>, mut log: ResMut<PinLog>) {
    log.0.extend(reader.read().cloned());
}

#[derive(Resource, Clone, Copy)]
struct HoverTarget(Entity);

#[derive(Resource, Clone, Copy)]
struct TestCamera(Entity);

/// 手动后端：每帧把鼠标指针指向按钮，替代真实 UI 后端，因此不需要 GPU 或窗口渲染。
fn feed_pointer_hits(
    target: Res<HoverTarget>,
    camera: Res<TestCamera>,
    mut hits: MessageWriter<PointerHits>,
) {
    hits.write(test_support::pointer_hits(target.0, camera.0));
}

/// 跨生产链路：WindowEvent -> 官方 mouse_pick_events -> PointerInput -> ProcessInput/Backend/Hover -> 按钮事件。
fn app() -> (App, Entity, Entity) {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        PickingPlugin,
        InteractionPlugin,
        ButtonPlugin,
        NativeButtonsPlugin,
    ))
    .init_resource::<ActivatedHits>()
    .init_resource::<PinLog>()
    .add_message::<WindowEvent>()
    .add_message::<PointerHits>()
    .add_systems(Startup, spawn_mouse_pointer)
    .add_systems(First, mouse_pick_events.in_set(PickingSystems::Input))
    .add_systems(PreUpdate, feed_pointer_hits.before(PickingSystems::Backend))
    .add_systems(
        Update,
        (clear_pin_log, log_pointer_inputs, collect_activated).chain(),
    );
    let window = app
        .world_mut()
        .spawn((Window::default(), PrimaryWindow))
        .id();
    let camera = app.world_mut().spawn_empty().id();
    app.insert_resource(TestCamera(camera));
    let button = app.world_mut().spawn(press_triggered()).id();
    app.insert_resource(HoverTarget(button));
    (app, button, window)
}

fn send(app: &mut App, event: WindowEvent) {
    app.world_mut().write_message(event);
    app.update();
}

fn activated(app: &App) -> Vec<Entity> {
    app.world().resource::<ActivatedHits>().0.clone()
}

/// 中键与右键不得驱动原生按钮，也不得打断左键按住的按下态。
#[test]
fn foreign_mouse_buttons_never_activate_or_break_the_primary_press() {
    let (mut app, button, window) = app();

    send(
        &mut app,
        test_support::mouse_button_event(window, MouseButton::Right, ButtonState::Pressed),
    );
    assert!(
        app.world().get::<Pressed>(button).is_none(),
        "右键不应进入按下态"
    );
    assert!(activated(&app).is_empty(), "右键不应激活按钮");
    send(
        &mut app,
        test_support::mouse_button_event(window, MouseButton::Right, ButtonState::Released),
    );

    send(
        &mut app,
        test_support::mouse_button_event(window, MouseButton::Middle, ButtonState::Pressed),
    );
    assert!(
        app.world().get::<Pressed>(button).is_none(),
        "中键不应进入按下态"
    );
    assert!(activated(&app).is_empty(), "中键不应激活按钮");
    send(
        &mut app,
        test_support::mouse_button_event(window, MouseButton::Middle, ButtonState::Released),
    );

    send(
        &mut app,
        test_support::mouse_button_event(window, MouseButton::Left, ButtonState::Pressed),
    );
    assert!(
        app.world().get::<Pressed>(button).is_some(),
        "左键按下应进入按下态"
    );
    assert_eq!(activated(&app), vec![button], "左键按下当帧激活一次");

    send(
        &mut app,
        test_support::mouse_button_event(window, MouseButton::Right, ButtonState::Pressed),
    );
    assert!(
        app.world().get::<Pressed>(button).is_some(),
        "右键按下不得清掉左键按下态"
    );
    send(
        &mut app,
        test_support::mouse_button_event(window, MouseButton::Right, ButtonState::Released),
    );
    assert!(
        app.world().get::<Pressed>(button).is_some(),
        "右键抬起不得清掉左键按下态"
    );
    assert_eq!(activated(&app), vec![button], "右键不得重复激活");

    send(
        &mut app,
        test_support::mouse_button_event(window, MouseButton::Left, ButtonState::Released),
    );
    assert!(
        app.world().get::<Pressed>(button).is_none(),
        "左键抬起应清除按下态"
    );
    assert_eq!(activated(&app), vec![button], "抬起不得二次激活");
}

/// 移动与滚动原样保留；被丢弃的按键不会在下一帧重新变成输入。
#[test]
fn pointer_motion_and_scroll_pass_through_and_inputs_are_not_replayed() {
    let (mut app, _button, window) = app();

    app.world_mut()
        .write_message(test_support::mouse_wheel_event(window));
    app.world_mut()
        .write_message(test_support::cursor_moved_event(
            window,
            Vec2::new(12.0, 8.0),
        ));
    app.update();
    let kept = app.world().resource::<PinLog>().0.clone();
    assert!(
        kept.iter()
            .any(|input| matches!(input.action, PointerAction::Scroll { .. })),
        "滚动输入应保留"
    );
    assert!(
        kept.iter()
            .any(|input| matches!(input.action, PointerAction::Move { .. })),
        "移动输入应保留"
    );

    app.update();
    assert!(
        app.world().resource::<PinLog>().0.is_empty(),
        "上一帧输入不得重发"
    );

    send(
        &mut app,
        test_support::mouse_button_event(window, MouseButton::Right, ButtonState::Pressed),
    );
    assert!(
        !app.world()
            .resource::<PinLog>()
            .0
            .iter()
            .any(|input| matches!(input.action, PointerAction::Press(PointerButton::Secondary))),
        "被过滤的右键输入不应出现在指针输入里"
    );
    app.update();
    assert!(
        app.world().resource::<PinLog>().0.is_empty(),
        "被过滤的输入不得重发"
    );
}

/// 外壳提供 widget 需要的状态与焦点组件。
#[test]
fn press_triggered_bundle_provides_widget_state_components() {
    let mut app = App::new();
    let button = app.world_mut().spawn(press_triggered()).id();
    let world = app.world();
    assert!(
        world.get::<Button>(button).is_some(),
        "应包含 widget Button"
    );
    assert!(
        world.get::<ActivateOnPress>(button).is_some(),
        "应按下即触发"
    );
    assert_eq!(
        world.get::<Hovered>(button).map(Hovered::get),
        Some(false),
        "悬停状态由 picking 更新，初始为未悬停"
    );
    let pickable = world
        .get::<Pickable>(button)
        .expect("应显式包含 picking 命中配置");
    assert!(pickable.should_block_lower, "按钮命中应阻断下层命中");
    assert!(pickable.is_hoverable, "按钮应参与悬停命中");
}

#[derive(Resource, Default)]
struct ConsumedHits(Vec<Entity>);

/// 消费者侧的同帧策略：本帧输入更新后若仍接管指针，就整帧丢弃激活。
fn consume_activated(
    mut reader: MessageReader<Activated>,
    mut consumed: ResMut<ConsumedHits>,
    touch: Option<Res<GestureInput>>,
) {
    if touch.is_some_and(|input| input.override_mouse) {
        // 先清游标再返回：解除接管后不补触发这一帧的旧消息。
        reader.clear();
        return;
    }
    consumed.0.extend(reader.read().map(|event| event.entity));
}

/// 桥接不受触摸状态影响：接管期间照样产生消息，由消费者丢弃；解除后的新激活必须能正常消费。
#[test]
fn consumers_drop_activation_while_touch_override_is_active() {
    let mut app = App::new();
    app.add_plugins(NativeButtonsPlugin)
        .init_resource::<ConsumedHits>()
        .init_resource::<GestureInput>()
        .add_message::<Activated>()
        .add_systems(Update, consume_activated);
    let button = app.world_mut().spawn(press_triggered()).id();
    app.world_mut()
        .resource_mut::<GestureInput>()
        .override_mouse = true;
    app.world_mut().write_message(Activated { entity: button });
    app.update();
    assert_eq!(
        app.world().resource::<ConsumedHits>().0.len(),
        0,
        "接管期间消费者应丢弃该帧激活"
    );

    app.world_mut()
        .resource_mut::<GestureInput>()
        .override_mouse = false;
    app.world_mut().write_message(Activated { entity: button });
    app.update();
    assert_eq!(
        app.world().resource::<ConsumedHits>().0,
        vec![button],
        "解除接管后的新激活应正常消费"
    );
}
