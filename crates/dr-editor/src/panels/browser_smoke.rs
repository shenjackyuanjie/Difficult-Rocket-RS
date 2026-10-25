use crate::demo;
use crate::document::EditorDocument;
use crate::panels::*;
use crate::window::EditorPaths;
use crate::window::SmokeTest;
use dr_core::Ship;

#[derive(Default)]
pub struct State {
    phase: u8,
    delay: u8,
    wait_passes: u16,
    input: crate::egui_ui::UiTestInput,
    before: Option<Ship>,
    last: Option<PathBuf>,
    scan_ms: f64,
}

/// 先通过正式 Ships 入口导航，再等待当前目录的实际可见文件命中。
/// 抽屉关闭时 browser_rows/offset 仍可能是旧值，不能拿这些缓存判断布局已就绪。
fn wait_for_browser(
    files: &[PathBuf],
    ui: &egui_panel::UiState,
    input: &mut bevy_egui::EguiInput,
    driver: &mut crate::egui_ui::UiTestInput,
) -> bool {
    if !ui.browser_area.is_some_and(|area| area.is_positive()) {
        if let Some(toggle) = ui.ship_toggle.filter(|rect| rect.is_positive()) {
            driver.click_rect(toggle, input);
        }
        return false;
    }
    !ui.scrolling
        && ui.hits.iter().any(|(action, rect)| {
            rect.is_positive()
                && matches!(action, PanelButton::Open(path) if files.binary_search(path).is_ok())
        })
}

#[allow(clippy::too_many_arguments)]
pub fn run(
    mode: Res<SmokeTest>,
    mut state: Local<State>,
    document: Res<EditorDocument>,
    paths: Res<EditorPaths>,
    mut browser: ResMut<ShipBrowser>,
    mut ui: ResMut<egui_panel::UiState>,
    mut inputs: Query<&mut bevy_egui::EguiInput, With<bevy_egui::PrimaryEguiContext>>,
    mut commands: Commands,
    showcase: Option<Res<demo::Showcase>>,
) {
    if !mode.browser || mode.started.elapsed().as_secs() < 3 {
        return;
    }
    assert!(
        demo::within_timeout(showcase.as_deref(), mode.started, 90),
        "大目录交互自测超时：phase={} ships={:?} rows={} offset={} max={} hits={:?}",
        state.phase,
        ui.browser_area,
        ui.browser_rows,
        ui.browser_offset,
        ui.browser_max_scroll,
        ui.hits
    );
    let Ok(mut input) = inputs.single_mut() else {
        return;
    };
    if state.input.tick(&mut input) {
        return;
    }
    // 详细演示允许超出旧的总时长，但单阶段仍要在有限次实际导航后给出诊断。
    if matches!(state.phase, 1 | 2) {
        state.wait_passes += 1;
        assert!(
            state.wait_passes <= 120,
            "船体目录导航未就绪：phase={} passes={} ships={:?} toggle={:?} \
             rows={} offset={} max={} requested_scroll={:?} scrolling={} hits={:?}",
            state.phase,
            state.wait_passes,
            ui.browser_area,
            ui.ship_toggle,
            ui.browser_rows,
            ui.browser_offset,
            ui.browser_max_scroll,
            ui.browser_scroll,
            ui.scrolling,
            ui.hits,
        );
    }
    match state.phase {
        0 => {
            state.before = Some(document.ship.clone());
            let stamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis();
            let folder = PathBuf::from(format!("target/browser-smoke-{stamp}/samples"));
            std::fs::create_dir_all(&folder).unwrap();
            let xml = dr_core::ship_to_xml(&document.ship).unwrap();
            for index in 0..1000 {
                std::fs::write(folder.join(format!("ship-{index:04}.xml")), &xml).unwrap();
            }
            std::fs::write(folder.join("invalid.xml"), "<Ship><Parts>").unwrap();
            let start = std::time::Instant::now();
            browser.set_folder(folder);
            state.scan_ms = start.elapsed().as_secs_f64() * 1000.0;
            assert_eq!(browser.files.len(), 1000);
            assert_eq!(browser.rejected, 1);
            assert!(browser.files.windows(2).all(|pair| pair[0] < pair[1]));
            state.last = browser.files.last().cloned();
        }
        1 => {
            if !wait_for_browser(&browser.files, &ui, &mut input, &mut state.input) {
                state.delay = 0;
                return;
            }
            state.delay += 1;
            if state.delay < 5 {
                return;
            }
            assert!(
                ui.browser_rows > 0 && ui.browser_rows < 80,
                "虚拟列表应只绘制可见行"
            );
            // 复用 ScrollAreaOutput 实测的最大偏移，不估算行距，也不使用失去像素精度的极大值。
            ui.browser_scroll = Some(ui.browser_max_scroll);
        }
        2 => {
            if !wait_for_browser(&browser.files, &ui, &mut input, &mut state.input) {
                return;
            }
            if ui.browser_offset <= 10000.0 {
                return;
            }
            let last = state.last.clone().unwrap();
            if !state
                .input
                .click_panel(PanelButton::Open(last), &ui, &mut input)
            {
                return;
            }
        }

        3 => {
            assert_eq!(
                paths.ship.as_deref().map(PathBuf::from).as_ref(),
                state.last.as_ref()
            );
            assert_eq!(state.before.as_ref(), Some(&document.ship));
            assert!(!document.dirty);
            assert!(!document.history.can_undo());
            assert!(ui.browser_offset > 10000.0);
        }
        4 => {
            let report = format!(
                "{{\"files\":1000,\"rejected\":1,\"scan_ms\":{:.3}}}\n",
                state.scan_ms
            );
            std::fs::write("target/editor-browser-smoke.json", &report).unwrap();
            info!(
                "大目录交互自测通过：1000 个文件排序与虚拟列表渲染、异常 XML 过滤、滚动到底、末项打开与滚动位置保留，{}",
                report.trim()
            );
            use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk};
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk("target/editor-browser-smoke.png"))
                .observe(
                    |_: On<ScreenshotCaptured>, mut exit: MessageWriter<AppExit>| {
                        exit.write(AppExit::Success);
                    },
                );
        }
        _ => return,
    }
    state.wait_passes = 0;
    state.phase += 1;
}

#[cfg(test)]
mod tests;
