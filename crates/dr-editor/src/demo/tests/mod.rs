//! 演示回归测试共用夹具；按输入、节拍、证据和配置拆分。
use super::chapters::CHAPTERS;
use super::hints::{KEY_HINT_FADE, KEY_HINT_HOLD, button_key_name};
use super::*;

mod configuration;
mod evidence;
mod hints;
mod overlay;
mod pacing;
mod replay;

fn action_log(folder: &std::path::Path) -> Vec<serde_json::Value> {
    std::fs::read_to_string(folder.join("demo-actions.jsonl"))
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

fn animation_app(step_ms: u64, target: Vec2) -> (App, Entity, tempfile::TempDir) {
    let folder = tempfile::tempdir().unwrap();
    let args = vec![
        "editor".into(),
        "--demo-showcase".into(),
        folder.path().to_string_lossy().into_owned(),
        "--demo-step-ms".into(),
        step_ms.to_string(),
    ];
    let mut showcase = Showcase::from_args(&args).unwrap().unwrap();
    showcase.initialize = false;
    showcase.display_pointer = Some(Vec2::ZERO);
    let mut app = App::new();
    app.insert_resource(showcase)
        .init_resource::<ButtonInput<MouseButton>>()
        .init_resource::<ButtonInput<KeyCode>>()
        .add_systems(Update, animate);
    let mut window = Window::default();
    window.set_cursor_position(Some(target));
    let entity = app
        .world_mut()
        .spawn((
            window,
            bevy::window::PrimaryWindow,
            bevy_egui::EguiInput::default(),
            bevy_egui::PrimaryEguiContext,
        ))
        .id();
    (app, entity, folder)
}
