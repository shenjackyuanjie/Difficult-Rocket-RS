use super::*;

#[derive(Default)]
pub(crate) struct State {
    phase: u8,
    before: Option<Ship>,
    after: Option<Ship>,
}

/// 使用实际 UI 实体与输入消息覆盖草稿、输入法、应用、撤销及保存往返。
#[allow(clippy::too_many_arguments)]
pub(crate) fn run(
    mode: Res<SmokeTest>,
    mut state: Local<State>,
    inspector: Res<Inspector>,
    document: Res<EditorDocument>,
    mut buttons: Query<(&Action, &mut Interaction)>,
    windows: Query<Entity, With<bevy::window::PrimaryWindow>>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut ime: MessageWriter<Ime>,
    mut events: MessageWriter<KeyboardInput>,
    mut commands: Commands,
) {
    if !mode.properties || mode.started.elapsed().as_secs() < 3 {
        return;
    }
    assert!(mode.started.elapsed().as_secs() < 60, "属性交互自测超时");
    let Ok(window) = windows.single() else {
        return;
    };
    let mut press = |action: Action| {
        let (_, mut interaction) = buttons
            .iter_mut()
            .find(|(item, _)| **item == action)
            .unwrap_or_else(|| panic!("未找到属性按钮：{action:?}"));
        *interaction = Interaction::Pressed;
    };
    match state.phase {
        0 => {
            state.before = Some(document.ship.clone());
            press(Action::Open);
        }
        1 => {
            assert!(inspector.draft.is_some());
            assert_eq!(state.before.as_ref(), Some(&document.ship));
            press(Action::Focus(Field::Name));
        }
        2 => {
            ime.write(Ime::Commit {
                window,
                value: "分级自测 & 火箭".into(),
            });
            keys.press(KeyCode::Delete);
            keys.press(KeyCode::KeyR);
        }
        3 => {
            keys.reset_all();
            assert_eq!(inspector.draft.as_ref().unwrap().name, "分级自测 & 火箭");
            assert_eq!(
                state.before.as_ref(),
                Some(&document.ship),
                "文字输入穿透到画布"
            );
            press(Action::AddStep);
        }
        4 => press(Action::Focus(Field::Target)),
        5 => {
            let target = document
                .ship
                .all_parts()
                .find(|part| part.pod.is_none())
                .unwrap()
                .id;
            events.write(KeyboardInput {
                key_code: KeyCode::Digit2,
                logical_key: bevy::input::keyboard::Key::Character(target.to_string().into()),
                state: ButtonState::Pressed,
                text: Some(target.to_string().into()),
                repeat: false,
                window,
            });
        }
        6 => {
            let last = inspector
                .draft
                .as_ref()
                .unwrap()
                .staging
                .as_ref()
                .unwrap()
                .steps
                .len()
                - 1;
            press(Action::AddActivation(last));
        }
        7 => {
            let last = inspector
                .draft
                .as_ref()
                .unwrap()
                .staging
                .as_ref()
                .unwrap()
                .steps
                .len()
                - 1;
            press(Action::Moved(last, 0));
        }
        8 => {
            assert_eq!(state.before.as_ref(), Some(&document.ship));
            use bevy::render::view::screenshot::{Screenshot, save_to_disk};
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk("target/editor-properties-draft.png"));
        }
        9 => press(Action::Apply),
        10 => {
            assert!(inspector.draft.is_none());
            assert!(document.dirty);
            let pod = document
                .ship
                .all_parts()
                .find_map(|part| part.pod.as_ref())
                .unwrap();
            assert_eq!(pod.name, "分级自测 & 火箭");
            assert!(
                pod.staging
                    .as_ref()
                    .unwrap()
                    .steps
                    .last()
                    .unwrap()
                    .activations[0]
                    .moved
            );
            state.after = Some(document.ship.clone());
            save_ship("target/properties-smoke.xml", &document.ship).unwrap();
            assert_eq!(
                load_ship("target/properties-smoke.xml").unwrap(),
                document.ship
            );
            keys.press(KeyCode::ControlLeft);
            keys.press(KeyCode::KeyZ);
        }
        11 => {
            assert_eq!(state.before.as_ref(), Some(&document.ship));
            assert!(!document.dirty);
            keys.reset_all();
            keys.press(KeyCode::ControlLeft);
            keys.press(KeyCode::KeyY);
        }
        12 => {
            assert_eq!(state.after.as_ref(), Some(&document.ship));
            keys.reset_all();
            press(Action::Open);
        }
        13 => press(Action::Active),
        14 => {
            keys.press(KeyCode::Escape);
        }
        15 => {
            assert!(inspector.draft.is_none());
            assert_eq!(state.after.as_ref(), Some(&document.ship));
            keys.reset_all();
            use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk};
            commands.spawn(Screenshot::primary_window())
                .observe(save_to_disk("target/editor-properties-smoke.png"))
                .observe(|_: On<ScreenshotCaptured>, mut exit: MessageWriter<AppExit>| {
                    info!("属性交互自测通过：中文输入、草稿隔离、分级动作、应用、撤销重做、取消及 XML 保存往返");
                    exit.write(AppExit::Success);
                });
        }
        _ => return,
    }
    state.phase += 1;
}
