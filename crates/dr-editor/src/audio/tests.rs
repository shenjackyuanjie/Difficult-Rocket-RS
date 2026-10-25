//! 合成音效回归：确定性、WAV 头、电平与时长、同帧节流、无声卡降级与导出。
use crate::audio::*;
use crate::document::EditorDocument;
use crate::test_support;
use dr_core::EditorCommand;

/// 归一化后的峰值上限，换算到 i16 量级。
fn peak_limit() -> f32 {
    10f32.powf(PEAK_DBFS / 20.0) * f32::from(i16::MAX)
}

fn u16_at(bytes: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([bytes[at], bytes[at + 1]])
}

fn u32_at(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
}

fn peak(samples: &[i16]) -> i32 {
    samples
        .iter()
        .map(|sample| i32::from(*sample).abs())
        .max()
        .unwrap_or(0)
}

#[test]
fn synthesis_is_deterministic() {
    for event in SfxEvent::ALL {
        assert_eq!(pcm(event), pcm(event), "{} 采样不稳定", event.name());
        assert_eq!(
            wav_bytes(event),
            wav_bytes(event),
            "{} 字节不稳定",
            event.name()
        );
    }
}

#[test]
fn wav_header_is_mono_16_bit_48k_pcm() {
    for event in SfxEvent::ALL {
        let bytes = wav_bytes(event);
        assert_eq!(&bytes[0..4], b"RIFF");
        assert_eq!(u32_at(&bytes, 4) as usize, bytes.len() - 8);
        assert_eq!(&bytes[8..12], b"WAVE");
        assert_eq!(&bytes[12..16], b"fmt ");
        assert_eq!(u32_at(&bytes, 16), 16);
        assert_eq!(u16_at(&bytes, 20), 1, "应为 PCM");
        assert_eq!(u16_at(&bytes, 22), 1, "应为单声道");
        assert_eq!(u32_at(&bytes, 24), SAMPLE_RATE);
        assert_eq!(u32_at(&bytes, 28), SAMPLE_RATE * 2, "字节率");
        assert_eq!(u16_at(&bytes, 32), 2, "块对齐");
        assert_eq!(u16_at(&bytes, 34), 16, "位深");
        assert_eq!(&bytes[36..40], b"data");
        assert_eq!(u32_at(&bytes, 40) as usize, bytes.len() - 44);
    }
}

#[test]
fn every_event_is_short_and_quiet() {
    for event in SfxEvent::ALL {
        let samples = pcm(event);
        let millis = samples.len() * 1000 / SAMPLE_RATE as usize;
        assert!(
            (40..=120).contains(&millis),
            "{} 时长 {millis}ms 超出 40-120ms",
            event.name()
        );
        let measured = peak(&samples);
        assert!(measured > 0, "{} 没有可听内容", event.name());
        assert!(
            measured as f32 <= peak_limit() + 1.0,
            "{} 峰值 {measured} 超过 -24 dBFS",
            event.name()
        );
    }
}

#[test]
fn names_are_unique_for_export() {
    let mut names: Vec<_> = SfxEvent::ALL.iter().map(|event| event.name()).collect();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), SfxEvent::ALL.len());
}

#[test]
fn same_tick_repeats_play_once() {
    let planned = plan(
        true,
        [
            SfxEvent::Click,
            SfxEvent::Click,
            SfxEvent::Place,
            SfxEvent::Click,
        ],
    );
    assert_eq!(planned, vec![SfxEvent::Click, SfxEvent::Place]);
}

#[test]
fn no_sfx_plans_nothing() {
    let planned = plan(false, [SfxEvent::Click, SfxEvent::Place]);
    assert!(planned.is_empty());
    assert!(!SfxConfig::from_args(&["dr-editor".into(), "--no-sfx".into()]).enabled);
    assert!(SfxConfig::from_args(&["dr-editor".into()]).enabled);
}

#[test]
fn export_flag_parses_directory() {
    let args = [
        "dr-editor".to_owned(),
        "--sfx-export".to_owned(),
        "sfx-wav".to_owned(),
    ];
    assert_eq!(export_dir(&args), Some(PathBuf::from("sfx-wav")));
    assert_eq!(export_dir(&["dr-editor".to_owned()]), None);
}

#[test]
fn commands_map_to_place_connect_or_silence() {
    let part = test_support::document().ship.parts[0].clone();
    let connect = EditorCommand::ConnectParts {
        parent: dr_core::PartKey::new(0, 1, 0),
        child: dr_core::PartKey::new(0, 2, 0),
        kind: dr_core::LinkKind::Normal {
            parent_attach: 1,
            child_attach: 1,
        },
    };
    assert_eq!(
        command_sfx(&EditorCommand::Place(Box::new(part.clone()))),
        Some(SfxEvent::Place)
    );
    assert_eq!(command_sfx(&connect), Some(SfxEvent::Connect));
    assert_eq!(
        command_sfx(&EditorCommand::Batch(vec![
            EditorCommand::FreeEdit(Box::new(EditorCommand::Place(Box::new(part)))),
            connect,
        ])),
        Some(SfxEvent::Place)
    );
    assert_eq!(command_sfx(&EditorCommand::Delete(1)), None);
}

#[test]
fn export_writes_every_event() -> std::io::Result<()> {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(export_all(dir.path())?, SfxEvent::ALL.len());
    for event in SfxEvent::ALL {
        let bytes = std::fs::read(dir.path().join(format!("{}.wav", event.name())))?;
        assert_eq!(bytes, wav_bytes(event));
    }
    Ok(())
}

/// 不含音频后端的测试应用：可选参数让系统自行静默降级。
fn app(enabled: bool, with_backend: bool) -> App {
    let mut app = App::new();
    app.add_plugins(SfxPlugin {
        config: SfxConfig { enabled },
    });
    app.insert_resource(test_support::document());
    if with_backend {
        app.insert_resource(Assets::<AudioSource>::default());
    }
    app
}

fn players(app: &mut App) -> usize {
    let mut query = app
        .world_mut()
        .query_filtered::<Entity, With<AudioPlayer<AudioSource>>>();
    query.iter(app.world()).count()
}

#[test]
fn repeated_events_spawn_one_player_per_kind() {
    let mut app = app(true, true);
    app.world_mut().write_message(SfxEvent::Click);
    app.world_mut().write_message(SfxEvent::Click);
    app.world_mut().write_message(SfxEvent::Place);
    app.update();
    assert_eq!(players(&mut app), 2);
    assert_eq!(
        app.world().resource::<Assets<AudioSource>>().iter().count(),
        2,
        "每种音效只合成一次"
    );
}

#[test]
fn no_sfx_swallows_every_intent() {
    let mut app = app(false, true);
    app.world_mut().write_message(SfxEvent::Click);
    app.world_mut()
        .resource_mut::<EditorDocument>()
        .sfx
        .push(SfxEvent::Reject);
    app.update();
    assert_eq!(players(&mut app), 0);
    assert_eq!(app.world().resource::<Assets<AudioSource>>().len(), 0);
}

#[test]
fn missing_audio_backend_stays_silent() {
    let mut app = app(true, false);
    app.world_mut().write_message(SfxEvent::Click);
    app.world_mut()
        .resource_mut::<EditorDocument>()
        .sfx
        .push(SfxEvent::Chapter);
    // 没有 Assets<AudioSource>：不 panic、不生成实体，事件被取走。
    app.update();
    assert_eq!(players(&mut app), 0);
    assert!(app.world().resource::<EditorDocument>().sfx.is_empty());
}
