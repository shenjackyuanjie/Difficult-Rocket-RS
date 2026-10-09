//! 章节初始化、成果停留、成功证据收集与失败退出保留。
use super::*;

pub(crate) fn begin(world: &mut World) {
    let Some(showcase) = world.get_resource::<Showcase>() else {
        return;
    };
    if showcase.completed || !showcase.initialize {
        return;
    }
    if Instant::now() < showcase.next_tick {
        return;
    }
    let index = showcase.index;
    let chapter = showcase.chapter();
    let presentation = showcase.presentation;
    let demo_mode = showcase.mode;
    let started = SystemTime::now();
    append_timeline(&showcase.output, "chapter_started", chapter, started);
    {
        let mut showcase = world.resource_mut::<Showcase>();
        showcase.chapter_started = Instant::now();
        showcase.chapter_start_unix_ms = unix_ms(started);
        showcase.artifact_since = started;
    }
    let paths = world.resource::<EditorPaths>().clone();
    let sample = match chapter.id {
        "panels" | "unsaved" => Some("Test.xml"),
        "repair" => Some("Heronb.xml"),
        _ => None,
    };
    let sample = sample.map(|name| {
        std::path::Path::new(&paths.assets)
            .join("ships")
            .join(name)
            .to_string_lossy()
            .into_owned()
    });
    let mut document =
        load_document(sample.as_deref(), &paths.catalog).expect("无法只读加载演示样本");
    // 简略保留原夹具；详细用真实长列表，复用相同的 UI/历史断言而不缩减数据。
    if chapter.id == "staging" {
        staging_fixture(&mut document, demo_mode);
    }
    document.revision = world.resource::<EditorDocument>().revision.wrapping_add(1);
    world.insert_resource(document);
    world.insert_resource(demo_cases::Run::new(chapter.id));
    world.resource_mut::<EditorPaths>().ship = sample;
    world.insert_resource(DragState::default());
    world.insert_resource(EditorCursor::default());
    world.insert_resource(CameraDrag::default());
    world.insert_resource(view::ViewOptions::default());
    world.insert_resource(properties::Inspector::default());
    world.insert_resource(topology_ui::ConnectionEditor::default());
    world.insert_resource(files::PendingFileAction::default());
    world.insert_resource(panels::UiPointer::default());
    world.insert_resource(panels::ShipBrowser::new(
        std::path::Path::new(&paths.assets).join("ships"),
    ));
    world.resource_mut::<ButtonInput<KeyCode>>().reset_all();
    world.resource_mut::<ButtonInput<MouseButton>>().reset_all();
    world.resource_mut::<Messages<files::FileAction>>().clear();
    for (mut transform, mut projection) in world
        .query_filtered::<(&mut Transform, &mut Projection), With<Camera2d>>()
        .iter_mut(world)
    {
        transform.translation.x = 0.;
        transform.translation.y = 0.;
        if let Projection::Orthographic(projection) = &mut *projection {
            projection.scale = 1.;
        }
    }
    // presentation 的物理 1080p 与 scale factor 由 main 启动窗口负责。
    if !presentation {
        for mut window in world.query::<&mut Window>().iter_mut(world) {
            window.resolution.set(1440., 900.);
        }
    }
    {
        let mut mode = world.resource_mut::<SmokeTest>();
        let id = chapter.id;
        mode.panels = id == "panels";
        mode.connections = id == "connections";
        mode.transforms = id == "transforms";
        mode.selection = id == "selection";
        mode.view = id == "view";
        mode.staging = id == "staging";
        mode.topology = id == "topology";
        mode.repair = id == "repair";
        mode.browser = id == "browser";
        mode.native_dialogs = id == "unsaved";
        mode.started = Instant::now();
    }
    let mut showcase = world.resource_mut::<Showcase>();
    showcase.initialize = false;
    showcase.pointer = None;
    showcase.ui_pointer = None;
    showcase.motion = None;
    showcase.replay = ReplayInput::default();
    showcase.click_release = None;
    showcase.key_hints = KeyHints::default();
    showcase.mouse_hints = MouseHints::default();
    showcase.description = None;
    showcase.description_changed = false;
    showcase.key_release = None;
    showcase.next_tick = Instant::now() + chapter_pause(showcase.step, true);
    info!(
        "演示 {}/{}：{}",
        index + 1,
        showcase.chapters.len(),
        chapter.title
    );
}

/// Last 截获章节自测的成功退出；失败退出永不转为通过。
pub(crate) fn finish(world: &mut World) {
    let Some(showcase) = world.get_resource::<Showcase>() else {
        return;
    };
    if showcase.completed || showcase.initialize {
        return;
    }
    // 成果停留期间出现的失败也不得发布 finished 成功事件。
    if world
        .resource::<Messages<AppExit>>()
        .iter_current_update_messages()
        .any(|exit| !matches!(exit, AppExit::Success))
    {
        return;
    }
    if showcase.finishing.is_none() {
        let exits: Vec<_> = world
            .resource::<Messages<AppExit>>()
            .iter_current_update_messages()
            .cloned()
            .collect();
        if exits.is_empty() || exits.iter().any(|exit| !matches!(exit, AppExit::Success)) {
            return;
        }
        // 用户关闭窗口也产生 Success，只有本章所有新鲜证据齐全才接受。
        let chapter = showcase.chapter();
        let evidence = chapter.evidence();
        let fresh = evidence.iter().all(|name| {
            std::fs::metadata(format!("target/{name}")).is_ok_and(|metadata| {
                metadata.len() > 0
                    && metadata
                        .modified()
                        .is_ok_and(|mtime| mtime >= showcase.artifact_since)
            })
        });
        if !fresh {
            return;
        }
        let output = showcase.output.clone();
        for name in &evidence {
            std::fs::copy(format!("target/{name}"), output.join(name)).expect("无法复制演示证据");
        }
        world.resource_mut::<Messages<AppExit>>().clear();
        let mut showcase = world.resource_mut::<Showcase>();
        showcase.finishing = Some(Instant::now() + chapter_pause(showcase.step, false));
        return;
    }
    if Instant::now() < showcase.finishing.unwrap() {
        return;
    }
    let chapter = showcase.chapter();
    let ended = SystemTime::now();
    append_timeline(&showcase.output, "chapter_finished", chapter, ended);
    let output = showcase.output.clone();
    let elapsed = showcase.chapter_started.elapsed().as_secs_f64();
    let report = serde_json::json!({"id":chapter.id, "title":chapter.title, "status":"passed", "elapsed_seconds":elapsed, "artifacts":chapter.evidence(), "start_unix_ms":showcase.chapter_start_unix_ms, "end_unix_ms":unix_ms(ended)});
    let mut showcase = world.resource_mut::<Showcase>();
    showcase.finishing = None;
    showcase.reports.push(report);
    showcase.index += 1;
    if showcase.index < showcase.chapters.len() {
        showcase.initialize = true;
        showcase.next_tick = Instant::now();
        return;
    }
    let report = serde_json::json!({"completed":true, "mode":showcase.mode.name(), "step_ms":showcase.step.as_millis(), "input_mode":"内部 UI/画布输入注入；不是系统键鼠验收", "chapters":showcase.reports});
    let temporary = output.join("demo-report.json.tmp");
    std::fs::write(&temporary, serde_json::to_vec_pretty(&report).unwrap())
        .expect("无法写演示报告");
    std::fs::rename(temporary, output.join("demo-report.json")).expect("无法发布完整演示报告");
    showcase.completed = true;
    let exit = showcase.exit_on_complete;
    let label = showcase.mode.label();
    world.resource_mut::<SmokeTest>().native_dialogs = false;
    info!(
        "{}模式演示完成，报告：{}",
        label,
        output.join("demo-report.json").display()
    );
    if exit {
        world
            .resource_mut::<Messages<AppExit>>()
            .write(AppExit::Success);
    }
}
