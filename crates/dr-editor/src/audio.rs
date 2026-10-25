//! 程序化合成音效：运行期生成 48 kHz 单声道 16-bit WAV，交给 bevy_audio 播放。
//!
//! 资源目录没有音频文件，音色全部由参数化正弦/噪声加包络在内存里合成；峰值统一
//! 归一化到 -24 dBFS，双端淡入淡出避免爆音，同一 tick 的相同事件只播一次。
//! 没有音频后端时静默降级：bevy_audio 在 AudioOutput 初始化处 warn 一次并跳过播放，
//! 这里照样取走事件但不生成播放实体，也不 panic，演示与冒烟模式照常跑完。
use crate::document::EditorDocument;
use bevy::audio::{AudioPlayer, AudioSource, PlaybackSettings};
use bevy::prelude::*;
use dr_core::EditorCommand;
use std::path::{Path, PathBuf};

/// 合成采样率。
pub const SAMPLE_RATE: u32 = 48_000;
/// 目标峰值电平；听感轻，不刺耳。
pub const PEAK_DBFS: f32 = -24.0;
/// 播放实体兜底存活时间：无音频设备时 bevy 不会挂上 AudioSink，需要自己回收。
const PLAYER_LIFETIME_SECONDS: f32 = 1.0;
/// 音效种类数量，同时是缓存与去重表的长度。
const COUNT: usize = 8;

/// 音效事件；同帧重复的同类事件只播一次。
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub enum SfxEvent {
    /// UI 按钮激活。
    Click,
    /// 部件放置成功。
    Place,
    /// 吸附或连接建立。
    Connect,
    /// 移动或操作被拒绝、校验失败。
    Reject,
    /// 撤销。
    Undo,
    /// 重做。
    Redo,
    /// 演示章节切换。
    Chapter,
    /// 演示完成。
    Complete,
}

impl SfxEvent {
    pub const ALL: [Self; COUNT] = [
        Self::Click,
        Self::Place,
        Self::Connect,
        Self::Reject,
        Self::Undo,
        Self::Redo,
        Self::Chapter,
        Self::Complete,
    ];

    /// 导出文件名，去掉扩展名后同时用作音色标识。
    pub fn name(self) -> &'static str {
        match self {
            Self::Click => "click",
            Self::Place => "place",
            Self::Connect => "connect",
            Self::Reject => "reject",
            Self::Undo => "undo",
            Self::Redo => "redo",
            Self::Chapter => "chapter",
            Self::Complete => "complete",
        }
    }

    fn index(self) -> usize {
        self as usize
    }

    /// 音色分段：短促高频、中频下滑、上行两音、低频闷响等由这里区分。
    fn voices(self) -> &'static [Voice] {
        match self {
            Self::Click => &CLICK,
            Self::Place => &PLACE,
            Self::Connect => &CONNECT,
            Self::Reject => &REJECT,
            Self::Undo => &UNDO,
            Self::Redo => &REDO,
            Self::Chapter => &CHAPTER,
            Self::Complete => &COMPLETE,
        }
    }
}

/// 一段音色：正弦从 from_hz 滑到 to_hz，可混入确定性噪声。
struct Voice {
    from_hz: f32,
    to_hz: f32,
    millis: u32,
    /// 段前静音，用于两音之间的间隔。
    gap_millis: u32,
    /// 噪声比例 0..1。
    noise: f32,
    /// 相对响度 0..1。
    gain: f32,
}

const CLICK: [Voice; 1] = [Voice {
    from_hz: 1_800.0,
    to_hz: 2_400.0,
    millis: 48,
    gap_millis: 0,
    noise: 0.0,
    gain: 1.0,
}];

const PLACE: [Voice; 1] = [Voice {
    from_hz: 700.0,
    to_hz: 520.0,
    millis: 88,
    gap_millis: 0,
    noise: 0.0,
    gain: 1.0,
}];

const CONNECT: [Voice; 2] = [
    Voice {
        from_hz: 520.0,
        to_hz: 560.0,
        millis: 48,
        gap_millis: 0,
        noise: 0.0,
        gain: 0.85,
    },
    Voice {
        from_hz: 780.0,
        to_hz: 820.0,
        millis: 48,
        gap_millis: 8,
        noise: 0.0,
        gain: 1.0,
    },
];

const REJECT: [Voice; 1] = [Voice {
    from_hz: 170.0,
    to_hz: 110.0,
    millis: 56,
    gap_millis: 0,
    noise: 0.6,
    gain: 1.0,
}];

const UNDO: [Voice; 1] = [Voice {
    from_hz: 880.0,
    to_hz: 520.0,
    millis: 76,
    gap_millis: 0,
    noise: 0.0,
    gain: 1.0,
}];

const REDO: [Voice; 1] = [Voice {
    from_hz: 520.0,
    to_hz: 880.0,
    millis: 76,
    gap_millis: 0,
    noise: 0.0,
    gain: 1.0,
}];

const CHAPTER: [Voice; 2] = [
    Voice {
        from_hz: 440.0,
        to_hz: 440.0,
        millis: 52,
        gap_millis: 0,
        noise: 0.0,
        gain: 0.7,
    },
    Voice {
        from_hz: 660.0,
        to_hz: 660.0,
        millis: 52,
        gap_millis: 8,
        noise: 0.0,
        gain: 0.5,
    },
];

const COMPLETE: [Voice; 3] = [
    Voice {
        from_hz: 523.25,
        to_hz: 523.25,
        millis: 30,
        gap_millis: 0,
        noise: 0.0,
        gain: 0.8,
    },
    Voice {
        from_hz: 659.25,
        to_hz: 659.25,
        millis: 30,
        gap_millis: 8,
        noise: 0.0,
        gain: 0.9,
    },
    Voice {
        from_hz: 783.99,
        to_hz: 783.99,
        millis: 32,
        gap_millis: 8,
        noise: 0.0,
        gain: 1.0,
    },
];

/// 确定性噪声源；同一个事件每次生成完全相同的序列。
struct Noise(u32);

impl Noise {
    fn new(event: SfxEvent) -> Self {
        Self((event.index() as u32).wrapping_mul(0x9E37_79B9) | 1)
    }

    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        self.0 as f32 / u32::MAX as f32 * 2.0 - 1.0
    }
}

fn millis_to_samples(millis: u32) -> usize {
    (u64::from(millis) * u64::from(SAMPLE_RATE) / 1000) as usize
}

/// 合成 16-bit PCM 采样；同参数结果逐字节一致。
pub fn pcm(event: SfxEvent) -> Vec<i16> {
    let target_peak = 10f32.powf(PEAK_DBFS / 20.0);
    let mut noise = Noise::new(event);
    let mut mono: Vec<f32> = Vec::new();
    for voice in event.voices() {
        mono.resize(mono.len() + millis_to_samples(voice.gap_millis), 0.0);
        let count = millis_to_samples(voice.millis);
        let mut phase = 0.0f32;
        for step in 0..count {
            let progress = step as f32 / count as f32;
            let hz = voice.from_hz + (voice.to_hz - voice.from_hz) * progress;
            phase = (phase + std::f32::consts::TAU * hz / SAMPLE_RATE as f32)
                .rem_euclid(std::f32::consts::TAU);
            // sin² 包络：两端归零、段内平滑，不会爆音。
            let envelope = (std::f32::consts::PI * progress).sin().powi(2);
            let tone = phase.sin() * (1.0 - voice.noise) + noise.next() * voice.noise;
            mono.push(envelope * voice.gain * tone);
        }
    }
    let measured = mono
        .iter()
        .fold(0.0f32, |peak, sample| peak.max(sample.abs()));
    let scale = if measured > f32::EPSILON {
        target_peak / measured
    } else {
        0.0
    };
    mono.into_iter()
        .map(|sample| (sample * scale * f32::from(i16::MAX)).round() as i16)
        .collect()
}

/// 完整的 WAV 文件字节：标准 PCM 头 + 单声道 16-bit 数据。
pub fn wav_bytes(event: SfxEvent) -> Vec<u8> {
    let samples = pcm(event);
    let data_len = (samples.len() * 2) as u32;
    let mut bytes = Vec::with_capacity(44 + data_len as usize);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data_len).to_le_bytes());
    bytes.extend_from_slice(b"WAVE");
    bytes.extend_from_slice(b"fmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    bytes.extend_from_slice(&(SAMPLE_RATE * 2).to_le_bytes());
    bytes.extend_from_slice(&2u16.to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_len.to_le_bytes());
    for sample in samples {
        bytes.extend_from_slice(&sample.to_le_bytes());
    }
    bytes
}

/// 单个 tick 的播放意图：去重并按事件枚举顺序输出；关闭时为空。
pub fn plan(enabled: bool, events: impl IntoIterator<Item = SfxEvent>) -> Vec<SfxEvent> {
    if !enabled {
        return Vec::new();
    }
    let mut seen = [false; COUNT];
    for event in events {
        seen[event.index()] = true;
    }
    SfxEvent::ALL
        .into_iter()
        .filter(|event| seen[event.index()])
        .collect()
}

/// 事务命令对应的成功音效：放置优先，其次连接；其余命令保持安静。
pub fn command_sfx(command: &EditorCommand) -> Option<SfxEvent> {
    match command {
        EditorCommand::Place(_) | EditorCommand::Paste { .. } => Some(SfxEvent::Place),
        EditorCommand::ConnectParts { .. } | EditorCommand::Connect(_) => Some(SfxEvent::Connect),
        EditorCommand::FreeEdit(inner) => command_sfx(inner),
        EditorCommand::Batch(commands) => {
            if commands
                .iter()
                .any(|inner| command_sfx(inner) == Some(SfxEvent::Place))
            {
                Some(SfxEvent::Place)
            } else if commands
                .iter()
                .any(|inner| command_sfx(inner) == Some(SfxEvent::Connect))
            {
                Some(SfxEvent::Connect)
            } else {
                None
            }
        }
        _ => None,
    }
}

/// 向可选消息通道写入音效意图；没注册 SfxPlugin 的应用里静默跳过。
pub fn emit(sfx: &mut Option<MessageWriter<'_, SfxEvent>>, event: SfxEvent) {
    if let Some(writer) = sfx.as_mut() {
        writer.write(event);
    }
}

/// 命令行开关：--no-sfx 完全关闭播放。
#[derive(Resource, Clone, Debug)]
pub struct SfxConfig {
    pub enabled: bool,
}

impl SfxConfig {
    pub fn from_args(args: &[String]) -> Self {
        Self {
            enabled: !args.iter().any(|arg| arg == "--no-sfx"),
        }
    }
}

/// --sfx-export 目录参数：把八种音效各导出 WAV，供视频混音脚本复用同一音源。
pub fn export_dir(args: &[String]) -> Option<PathBuf> {
    args.windows(2)
        .find(|window| window[0] == "--sfx-export")
        .map(|window| PathBuf::from(&window[1]))
}

/// 导出全部音效；导出只写文件，与是否播放无关。
pub fn export_all(dir: &Path) -> std::io::Result<usize> {
    std::fs::create_dir_all(dir)?;
    for event in SfxEvent::ALL {
        std::fs::write(dir.join(format!("{}.wav", event.name())), wav_bytes(event))?;
    }
    Ok(SfxEvent::ALL.len())
}

/// 每种音效只合成一次，句柄缓存在资源里。
#[derive(Resource)]
struct SfxClips {
    handles: [Option<Handle<AudioSource>>; COUNT],
}

impl Default for SfxClips {
    fn default() -> Self {
        Self {
            handles: std::array::from_fn(|_| None),
        }
    }
}

impl SfxClips {
    fn handle(&mut self, event: SfxEvent, assets: &mut Assets<AudioSource>) -> Handle<AudioSource> {
        let slot = &mut self.handles[event.index()];
        slot.get_or_insert_with(|| {
            assets.add(AudioSource {
                bytes: wav_bytes(event).into(),
            })
        })
        .clone()
    }
}

/// 播放实体的兜底存活计时；正常播放由 PlaybackSettings::DESPAWN 回收。
#[derive(Component)]
struct SfxLifetime(Timer);

/// 合成音效插件：注册消息、缓存与每帧末的播放系统。
pub struct SfxPlugin {
    pub config: SfxConfig,
}

impl SfxPlugin {
    pub fn from_args(args: &[String]) -> Self {
        Self {
            config: SfxConfig::from_args(args),
        }
    }
}

impl Plugin for SfxPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<SfxEvent>()
            .insert_resource(self.config.clone())
            .init_resource::<SfxClips>()
            .add_systems(Last, (play, despawn_stale).chain());
    }
}

/// 取走本帧全部播放意图；没有音频资产资源时静默降级。
fn play(
    mut commands: Commands,
    config: Res<SfxConfig>,
    mut document: Option<ResMut<EditorDocument>>,
    mut messages: MessageReader<SfxEvent>,
    mut clips: ResMut<SfxClips>,
    mut assets: Option<ResMut<Assets<AudioSource>>>,
) {
    let mut events: Vec<SfxEvent> = messages.read().copied().collect();
    if let Some(document) = document.as_mut()
        && !document.sfx.is_empty()
    {
        events.append(&mut document.sfx);
    }
    // 没有 bevy_audio 资产（无声卡或未注册音频插件）时不生成播放实体。
    let Some(assets) = assets.as_mut() else {
        return;
    };
    for event in plan(config.enabled, events) {
        commands.spawn((
            AudioPlayer::new(clips.handle(event, assets)),
            PlaybackSettings::DESPAWN,
            SfxLifetime(Timer::from_seconds(
                PLAYER_LIFETIME_SECONDS,
                TimerMode::Once,
            )),
        ));
    }
}

/// 兜底回收：音频设备缺失时播放实体拿不到 sink，不会自动 despawn。
fn despawn_stale(
    time: Option<Res<Time>>,
    mut commands: Commands,
    mut players: Query<(Entity, &mut SfxLifetime)>,
) {
    let Some(time) = time else {
        return;
    };
    for (entity, mut lifetime) in &mut players {
        if lifetime.0.tick(time.delta()).is_finished() {
            commands.entity(entity).despawn();
        }
    }
}

#[cfg(test)]
mod tests;
