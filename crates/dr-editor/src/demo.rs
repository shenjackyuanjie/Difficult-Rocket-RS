//! 单窗口演示状态与入口；章节、回放、绘制和证据各自维护。
use super::*;
use std::{
    path::PathBuf,
    time::{Duration, Instant, SystemTime},
};

mod chapters;
mod evidence;
mod hints;
mod lifecycle;
mod overlay;
mod pacing;
mod replay;

use chapters::{Chapter, Mode, staging_fixture};
use evidence::{append_actions, append_timeline, unix_ms};
use hints::{
    KeyHint, KeyHints, MouseHints, is_modifier, mouse_label, paint_input_hint, paint_key_hint,
};
use pacing::{
    Action, action_pause, chapter_pause, classify_action, fast, input_hold, movement_duration,
};
use replay::{Motion, ReplayInput};

pub(crate) use lifecycle::{begin, finish};
pub(crate) use overlay::overlay;
pub(crate) use replay::{animate, isolate_pointer, pace, restore_pointer};

/// 各章节复用已有步骤的说明，不增加虚构设置或重复操作。
pub(crate) fn describe(showcase: &mut Option<ResMut<Showcase>>, text: &'static str) {
    if let Some(showcase) = showcase
        && showcase.description != Some(text)
    {
        showcase.description = Some(text);
        showcase.description_changed = true;
    }
}

#[derive(Resource)]
pub(crate) struct Showcase {
    output: PathBuf,
    mode: Mode,
    chapters: Vec<&'static Chapter>,
    step: Duration,
    exit_on_complete: bool,
    pub(crate) presentation: bool,
    chapter_start_unix_ms: u64,
    finishing: Option<Instant>,
    key_release: Option<(Instant, Vec<KeyCode>)>,
    index: usize,
    initialize: bool,
    next_tick: Instant,
    chapter_started: Instant,
    artifact_since: SystemTime,
    reports: Vec<serde_json::Value>,
    completed: bool,
    pointer: Option<Vec2>,
    ui_pointer: Option<bevy_egui::egui::Pos2>,
    motion: Option<Motion>,
    before_mouse: ButtonInput<MouseButton>,
    before_keys: ButtonInput<KeyCode>,
    replay: ReplayInput,
    display_pointer: Option<Vec2>,
    click_release: Option<(Instant, bevy_egui::egui::Pos2)>,
    key_hints: KeyHints,
    mouse_hints: MouseHints,
    description: Option<&'static str>,
    description_changed: bool,
}

impl Showcase {
    pub fn from_args(args: &[String]) -> anyhow::Result<Option<Self>> {
        let Some(output) = args.windows(2).find(|w| w[0] == "--demo-showcase") else {
            return Ok(None);
        };
        let step_ms = args
            .windows(2)
            .find(|w| w[0] == "--demo-step-ms")
            .map(|w| w[1].parse::<u64>())
            .transpose()?
            .unwrap_or(450);
        anyhow::ensure!(step_ms > 0, "演示步骤间隔必须大于零");
        let mode = if let Some(index) = args.iter().position(|arg| arg == "--demo-mode") {
            match args.get(index + 1).map(String::as_str) {
                Some("brief") => Mode::Brief,
                Some("detailed") => Mode::Detailed,
                _ => anyhow::bail!("--demo-mode 必须为 brief（简略）或 detailed（详细）"),
            }
        } else {
            Mode::Brief
        };
        let output = std::path::absolute(&output[1])?;
        std::fs::create_dir_all(&output)?;
        anyhow::ensure!(
            !output.join("demo-report.json").exists(),
            "演示目录已有报告，请使用新目录"
        );
        // 启动时就发布文件，父录制进程可在第一章初始化完成之前订阅。
        std::fs::File::create(output.join("demo-timeline.jsonl"))?;
        std::fs::File::create(output.join("demo-actions.jsonl"))?;
        Ok(Some(Self {
            output,
            mode,
            chapters: mode.chapters(),
            step: Duration::from_millis(step_ms),
            exit_on_complete: args.iter().any(|arg| arg == "--demo-exit-on-complete"),
            presentation: args.iter().any(|arg| arg == "--demo-presentation"),
            chapter_start_unix_ms: 0,
            finishing: None,
            key_release: None,
            index: 0,
            initialize: true,
            next_tick: Instant::now(),
            chapter_started: Instant::now(),
            artifact_since: SystemTime::now(),
            reports: vec![],
            completed: false,
            pointer: None,
            ui_pointer: None,
            motion: None,
            before_mouse: default(),
            before_keys: default(),
            replay: default(),
            display_pointer: None,
            click_release: None,
            key_hints: KeyHints::default(),
            mouse_hints: MouseHints::default(),
            description: None,
            description_changed: false,
        }))
    }

    fn chapter(&self) -> &'static Chapter {
        self.chapters[self.index]
    }

    pub(crate) fn case_result_pause(&self) -> Duration {
        if self.mode == Mode::Detailed && !fast(self.step) {
            self.step * 4
        } else {
            Duration::ZERO
        }
    }
}

/// 详细演示不以旧专项的固定时长截断样例；显式超时由外部驱动管理。
pub(crate) fn within_timeout(showcase: Option<&Showcase>, started: Instant, seconds: u64) -> bool {
    showcase.is_some_and(|showcase| showcase.mode == Mode::Detailed)
        || started.elapsed().as_secs() < seconds
}

/// 非演示模式不影响任何原有自测；实际编辑器输入、绘制及通道轮询始终每帧运行。
pub(crate) fn advance_ready(showcase: Option<Res<Showcase>>) -> bool {
    showcase.is_none_or(|showcase| {
        !showcase.completed
            && !showcase.initialize
            && showcase.finishing.is_none()
            && showcase.motion.is_none()
            && showcase.click_release.is_none()
            && showcase.key_release.is_none()
            && Instant::now() >= showcase.next_tick
    })
}

#[cfg(test)]
mod tests;
