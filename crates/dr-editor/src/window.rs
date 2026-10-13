//! 启动路径、自测配置与窗口截图策略。
use bevy::prelude::*;
use bevy::window::{PresentMode, WindowResolution};

#[derive(Resource, Clone)]
pub(crate) struct EditorPaths {
    pub ship: Option<String>,
    pub catalog: String,
    pub assets: String,
}

#[derive(Resource)]
pub(crate) struct SmokeTest {
    pub(crate) enabled: bool,
    pub(crate) window_size: Option<UVec2>,
    pub(crate) native_dialogs: bool,
    pub(crate) native_file_dialogs: bool,
    pub(crate) interaction: bool,
    pub(crate) topology: bool,
    pub(crate) panels: bool,
    pub(crate) properties: bool,
    pub(crate) connections: bool,
    pub(crate) transforms: bool,
    pub(crate) performance: bool,
    pub(crate) performance_selection_count: usize,
    pub(crate) scoped: bool,
    pub(crate) repair: bool,
    pub(crate) selection: bool,
    pub(crate) view: bool,
    pub(crate) browser: bool,
    pub(crate) staging: bool,
    pub(crate) native_ime: bool,
    pub(crate) native_input: Option<std::path::PathBuf>,
    pub(crate) native_keys: Option<std::path::PathBuf>,
    pub(crate) egui: bool,
    pub(crate) started: std::time::Instant,
}

/// 窗口截图/组合分级验收指定初始尺寸；普通编辑与演示保留既有窗口配置。
pub(crate) fn smoke_window_size(args: &[String]) -> anyhow::Result<Option<UVec2>> {
    let mut indices = args
        .iter()
        .enumerate()
        .filter(|(_, arg)| *arg == "--smoke-window-size");
    let Some((index, _)) = indices.next() else {
        return Ok(None);
    };
    anyhow::ensure!(indices.next().is_none(), "--smoke-window-size 只能指定一次");
    anyhow::ensure!(
        args.iter().any(|arg| arg == "--smoke-test"),
        "--smoke-window-size 需要 --smoke-test"
    );
    anyhow::ensure!(
        !args.iter().any(|arg| arg == "--demo-presentation"),
        "窄窗截图自测不能与演示录制窗口同时启用"
    );
    // Bevy 0.20 的 prelude 也导出 ContextExt；这里显式选择 anyhow 的错误上下文。
    let value = anyhow::Context::context(
        args.get(index + 1),
        "--smoke-window-size 需要 WIDTHxHEIGHT，例如 390x844",
    )?;
    let (width, height) =
        anyhow::Context::context(value.split_once('x'), "窗口尺寸格式应为 WIDTHxHEIGHT")?;
    let width: u32 = anyhow::Context::context(width.parse(), "窗口宽度应为正整数")?;
    let height: u32 = anyhow::Context::context(height.parse(), "窗口高度应为正整数")?;
    anyhow::ensure!(width >= 320 && height >= 320, "窗口截图尺寸至少为 320x320");
    Ok(Some(UVec2::new(width, height)))
}

pub(crate) fn initial_window_resolution(
    presentation: bool,
    smoke_size: Option<UVec2>,
) -> WindowResolution {
    if let Some(size) = smoke_size {
        WindowResolution::new(size.x, size.y).with_scale_factor_override(1.0)
    } else if presentation {
        WindowResolution::new(1920, 1080).with_scale_factor_override(1.0)
    } else {
        WindowResolution::new(1440, 900)
    }
}

/// 自动回放显式持续更新并优先无同步呈现；普通编辑保留原策略。
pub(crate) fn window_runtime_policy(automated: bool) -> (bevy::winit::WinitSettings, PresentMode) {
    if automated {
        (
            bevy::winit::WinitSettings::continuous(),
            PresentMode::AutoNoVsync,
        )
    } else {
        (
            bevy::winit::WinitSettings::default(),
            PresentMode::AutoVsync,
        )
    }
}

pub(crate) fn smoke_window_matches(window: &Window, requested: UVec2) -> bool {
    window.physical_width() == requested.x
        && window.physical_height() == requested.y
        && window.width() == requested.x as f32
        && window.height() == requested.y as f32
        && window.scale_factor() == 1.0
}

impl SmokeTest {
    pub(crate) fn from_args(args: &[String], smoke_size: Option<UVec2>) -> Self {
        Self {
            enabled: args.iter().any(|arg| arg == "--smoke-test"),
            window_size: smoke_size,
            topology: args.iter().any(|arg| arg == "--topology-smoke-test"),
            native_dialogs: args.iter().any(|arg| arg == "--native-dialog-test"),
            native_file_dialogs: args.iter().any(|arg| arg == "--native-file-dialog-test"),
            interaction: args.iter().any(|arg| arg == "--interaction-smoke-test"),
            panels: args.iter().any(|arg| arg == "--panel-smoke-test"),
            properties: args.iter().any(|arg| arg == "--properties-smoke-test"),
            connections: args.iter().any(|arg| arg == "--connection-smoke-test"),
            transforms: args.iter().any(|arg| arg == "--transforms-smoke-test"),
            performance: args.iter().any(|arg| arg == "--performance-test"),
            performance_selection_count: args
                .windows(2)
                .find(|pair| pair[0] == "--performance-selection-count")
                .and_then(|pair| pair[1].parse().ok())
                .unwrap_or(1),
            scoped: args.iter().any(|arg| arg == "--scoped-smoke-test"),
            repair: args.iter().any(|arg| arg == "--repair-smoke-test"),
            selection: args.iter().any(|arg| arg == "--selection-smoke-test"),
            view: args.iter().any(|arg| arg == "--view-smoke-test"),
            browser: args.iter().any(|arg| arg == "--browser-smoke-test"),
            staging: args.iter().any(|arg| arg == "--staging-smoke-test"),
            native_ime: args.iter().any(|arg| arg == "--native-ime-test"),
            native_keys: args
                .windows(2)
                .find(|args| args[0] == "--native-keys-test")
                .map(|args| std::path::PathBuf::from(&args[1])),
            native_input: args
                .windows(2)
                .find(|args| args[0] == "--native-input-test")
                .map(|args| std::path::PathBuf::from(&args[1])),
            egui: args.iter().any(|arg| arg == "--egui-smoke-test"),
            started: std::time::Instant::now(),
        }
    }
}

pub(crate) fn capture_screenshot(
    mut commands: Commands,
    mode: Res<SmokeTest>,
    keys: Res<ButtonInput<KeyCode>>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mut captured: Local<bool>,
    mut stable_size_frames: Local<u8>,
    mut observed_window: Local<Option<(UVec2, f32)>>,
) {
    use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk};
    let window = windows.single().ok();
    let size_ready = if let Some(requested) = mode.window_size {
        if window.is_some_and(|window| smoke_window_matches(window, requested)) {
            *stable_size_frames = stable_size_frames.saturating_add(1);
        } else {
            *stable_size_frames = 0;
        }
        *stable_size_frames >= 2
    } else {
        true
    };
    // 组合分级自测沿用尺寸观测，但由 staging_smoke 完成草稿、历史和 XML 后截图退出。
    let startup_smoke = mode.enabled && !mode.staging;
    let smoke = startup_smoke && !*captured && size_ready && mode.started.elapsed().as_secs() >= 5;
    if mode.enabled
        && let Some(window) = window
    {
        let observed = (window.physical_size(), window.scale_factor());
        if *observed_window != Some(observed) || *stable_size_frames == 2 || smoke {
            info!(
                "截图自测窗口观测：{:?} → {:?}，经过 {:.3}s",
                *observed_window,
                observed,
                mode.started.elapsed().as_secs_f64()
            );
            *observed_window = Some(observed);
            let report = serde_json::json!({
                "requested_size": mode.window_size.map(|size| [size.x, size.y]),
                "logical_size": [window.width(), window.height()],
                "physical_size": [window.physical_width(), window.physical_height()],
                "scale_factor": window.scale_factor(),
                "stable_size_frames": *stable_size_frames,
                "elapsed_seconds": mode.started.elapsed().as_secs_f64(),
                "capture_requested": smoke,
            });
            std::fs::create_dir_all("target").expect("创建截图证据目录");
            std::fs::write(
                "target/editor-smoke-window.json",
                serde_json::to_vec_pretty(&report).unwrap(),
            )
            .expect("记录截图自测窗口尺寸");
        }
    }
    if startup_smoke && mode.started.elapsed().as_secs() > 30 {
        panic!("窗口截图自测超时");
    }
    if smoke || keys.just_pressed(KeyCode::F12) {
        *captured = true;
        let path = if smoke {
            "target/editor-smoke.png".to_owned()
        } else {
            format!(
                "editor-{}.png",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis()
            )
        };
        let mut entity = commands.spawn(Screenshot::primary_window());
        entity.observe(save_to_disk(path));
        if smoke {
            entity.observe(
                |_: On<ScreenshotCaptured>, mut exit: MessageWriter<AppExit>| {
                    exit.write(AppExit::Success);
                },
            );
        }
    }
}

#[cfg(test)]
mod tests;
