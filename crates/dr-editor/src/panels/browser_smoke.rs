use super::*;

#[derive(Default)]
pub(crate) struct State {
    phase: u8,
    delay: u8,
    before: Option<Ship>,
    last: Option<PathBuf>,
    scan_ms: f64,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn run(
    mode: Res<SmokeTest>,
    mut state: Local<State>,
    document: Res<EditorDocument>,
    paths: Res<EditorPaths>,
    mut browser: ResMut<ShipBrowser>,
    mut scrolling: Query<(&ComputedNode, &mut ScrollPosition), With<BrowserScroll>>,
    mut buttons: Query<(&PanelButton, &mut Interaction)>,
    mut commands: Commands,
) {
    if !mode.browser || mode.started.elapsed().as_secs() < 3 {
        return;
    }
    assert!(mode.started.elapsed().as_secs() < 90, "大目录交互自测超时");
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
            state.delay += 1;
            if state.delay < 5 {
                return;
            }
            assert_eq!(
                buttons
                    .iter()
                    .filter(|(button, _)| matches!(button, PanelButton::Open(_)))
                    .count(),
                1000
            );
            let (node, mut scroll) = scrolling.single_mut().unwrap();
            scroll.y = (node.content_size().y - node.size().y) * node.inverse_scale_factor();
            assert!(scroll.y > 10000.0);
        }
        2 => {
            assert!(scrolling.single().unwrap().1.y > 10000.0);
            let (_, mut interaction) = buttons.iter_mut().find(|(button, _)| matches!(button, PanelButton::Open(path) if Some(path) == state.last.as_ref())).unwrap();
            *interaction = Interaction::Pressed;
        }
        3 => {
            assert_eq!(
                paths.ship.as_deref().map(PathBuf::from).as_ref(),
                state.last.as_ref()
            );
            assert_eq!(state.before.as_ref(), Some(&document.ship));
            assert!(!document.dirty);
            assert!(!document.history.can_undo());
            assert!(scrolling.single().unwrap().1.y > 10000.0);
        }
        4 => {
            let report = format!(
                "{{\"files\":1000,\"rejected\":1,\"scan_ms\":{:.3}}}\n",
                state.scan_ms
            );
            std::fs::write("target/editor-browser-smoke.json", &report).unwrap();
            info!(
                "大目录交互自测通过：1000 个文件排序与渲染、异常 XML 过滤、滚动到底、末项打开与滚动位置保留，{}",
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
    state.phase += 1;
}
