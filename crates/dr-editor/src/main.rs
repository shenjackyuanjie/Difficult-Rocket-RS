mod attachment_hints;
mod canvas_input;
mod connection_lines;
mod connection_smoke;
mod demo;
mod demo_cases;
mod document;
mod egui_ui;
mod files;
mod free_mode;
mod help;
mod icons;
mod interaction_smoke;
mod mobile_input;
mod native_buttons;
mod native_input;
mod native_keys;
mod panels;
mod performance;
mod placement;
mod properties;
mod render;
mod scoped_smoke;
mod selection;
mod selection_smoke;
mod sprite_shader;
mod topology_ui;
mod transform_smoke;
mod view;
mod view_smoke;
mod window;

#[cfg(test)]
mod test_support;

use bevy::prelude::*;
use bevy::ui_widgets::Button;
use dr_core::{
    Connection, EditorCommand, EditorHistory, LinkKind, Part, PartCatalog, PartKey, PartKind, Ship,
    load_ship, save_ship,
};
use dr_core::{SelectionPose, SelectionTransform, ShipFragment};
use std::collections::BTreeSet;

use document::{EditorDocument, load_document, new_ship};
use placement::EditorCursor;
use selection::DragState;
use view::CameraDrag;
use window::{EditorPaths, SmokeTest};

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let smoke_size = window::smoke_window_size(&args)?;
    let ship = args
        .windows(2)
        .find(|w| w[0] == "--ship" || w[0] == "-s")
        .map(|w| w[1].clone());
    let catalog_path = args
        .windows(2)
        .find(|w| w[0] == "--catalog" || w[0] == "-c")
        .map(|w| w[1].clone())
        .unwrap_or_else(|| "../Difficult-Rocket/assets/builtin/PartList.xml".into());
    let document = load_document(ship.as_deref(), &catalog_path)?;
    let assets = args
        .windows(2)
        .find(|w| w[0] == "--assets")
        .map(|w| w[1].clone())
        .unwrap_or_else(|| "../Difficult-Rocket/assets".into());
    let assets = std::path::absolute(&assets)
        .expect("资源路径无效")
        .to_string_lossy()
        .into_owned();
    let showcase = demo::Showcase::from_args(&args)?;
    let presentation = showcase.is_some() && args.iter().any(|arg| arg == "--demo-presentation");
    let automated = showcase.is_some() || args.iter().any(|arg| arg == "--smoke-test");
    let (winit_settings, present_mode) = window::window_runtime_policy(automated);
    let mut app = App::new();
    app.insert_resource(winit_settings);
    if let Some(showcase) = showcase {
        app.insert_resource(showcase);
    }
    app.insert_resource(panels::ShipBrowser::new(
        std::path::Path::new(&assets).join("ships"),
    ))
    .init_resource::<panels::Palette>()
    .init_resource::<panels::UiPointer>()
    .init_resource::<mobile_input::GestureInput>()
    .init_resource::<properties::Inspector>()
    .init_resource::<files::FileMenu>()
    .insert_resource(SmokeTest::from_args(&args, smoke_size))
    .insert_resource(EditorPaths {
        ship,
        catalog: catalog_path,
        assets: assets.clone(),
    })
    .insert_resource(document)
    .init_resource::<DragState>()
    .init_resource::<EditorCursor>()
    .init_resource::<CameraDrag>()
    .init_resource::<connection_lines::Settings>()
    .init_resource::<connection_lines::Controls>()
    .init_resource::<transform_smoke::Captured>()
    .init_resource::<demo_cases::Run>()
    .init_resource::<demo_cases::Captured>()
    .init_resource::<interaction_smoke::Probe>()
    .init_resource::<view::ViewOptions>()
    .add_plugins(
        DefaultPlugins
            .set(bevy::log::LogPlugin {
                filter: format!("{},icu_provider=error", bevy::log::DEFAULT_FILTER),
                ..default()
            })
            // 编辑器优先输入延迟，不让第 N 帧输入等到第 N+1 帧渲染。
            .disable::<bevy::render::pipelined_rendering::PipelinedRenderingPlugin>()
            .set(AssetPlugin {
                file_path: assets,
                ..default()
            })
            .set(WindowPlugin {
                close_when_requested: false,
                primary_window: Some(Window {
                    title: "Difficult Rocket Editor".into(),
                    // 录制使用固定物理像素和无边框窗口，不随桌面 DPI 改变成片尺寸。
                    resolution: window::initial_window_resolution(presentation, smoke_size),
                    decorations: !presentation,
                    resize_constraints: bevy::window::WindowResizeConstraints {
                        min_width: 320.0,
                        min_height: 320.0,
                        ..default()
                    },
                    present_mode,
                    // 普通编辑保留无撕裂显示，避免交换链再积压多帧旧输入。
                    desired_maximum_frame_latency: std::num::NonZeroU32::new(1),
                    ..default()
                }),
                ..default()
            }),
    )
    .add_plugins(sprite_shader::SpriteShaderPlugin)
    .add_plugins(native_buttons::NativeButtonsPlugin)
    .init_gizmo_group::<connection_lines::LineGizmos>()
    .add_plugins(egui_ui::EditorEguiPlugin)
    .add_message::<files::FileAction>()
    .add_message::<panels::PanelButton>()
    .add_systems(
        Startup,
        (
            view::setup_camera,
            render::setup_hud,
            files::setup_file_toolbar,
            help::setup,
            properties::setup,
        ),
    )
    .add_systems(
        Update,
        (
            (
                demo::begin,
                demo::restore_pointer,
                demo_cases::run.run_if(demo::advance_ready),
                (
                    panels::smoke::run,
                    topology_ui::smoke::run,
                    properties::egui_smoke::run,
                    connection_smoke::run,
                    demo::rocket_workflow::run,
                    transform_smoke::run,
                    interaction_smoke::input,
                    performance::run,
                    native_input::run,
                    native_keys::run,
                    scoped_smoke::run,
                    properties::repair_smoke::run,
                    selection_smoke::run,
                    view_smoke::run,
                    panels::browser_smoke::run,
                    properties::staging_smoke::run,
                    properties::native_ime::run,
                    files::native_dialog_test,
                )
                    .chain()
                    .run_if(demo::advance_ready),
                demo::animate,
                (panels::layout_toolbars, mobile_input::input).chain(),
                panels::pointer_over_ui,
                help::input,
                properties::actions,
                files::toolbar_actions,
                egui_ui::prepare_input,
                panels::panel_actions.run_if(properties::closed),
                files::file_inputs,
                properties::cancel_for_file_action,
                placement::cancel_for_file_action,
                files::file_actions,
                view::controls.run_if(egui_ui::canvas_or_fit_available),
                view::camera_controls,
                canvas_input::mouse_editor.run_if(egui_ui::canvas_input_available),
                canvas_input::keyboard_commands.run_if(egui_ui::canvas_input_available),
            )
                .chain(),
            render::sync,
            placement::draw_preview,
            selection::draw_preview,
            connection_lines::configure_system,
            render::connections,
            attachment_hints::draw,
            demo_cases::draw,
            demo_cases::draw_degenerate,
            view::draw_debug,
            render::update_hud,
            files::update_window_title,
            window::capture_screenshot,
            files::native_file_dialog_test,
            demo::pace,
        )
            .chain(),
    )
    .add_systems(
        PostUpdate,
        (interaction_smoke::verify, demo_cases::verify)
            .after(bevy::transform::TransformSystems::Propagate)
            .before(demo::isolate_pointer),
    )
    .add_systems(
        PostUpdate,
        demo::isolate_pointer.after(bevy_egui::EguiPostUpdateSet::EndPass),
    )
    .add_systems(Last, demo::finish)
    .add_systems(
        bevy_egui::EguiPrimaryContextPass,
        demo::overlay.after(topology_ui::draw),
    )
    .run();
    Ok(())
}
