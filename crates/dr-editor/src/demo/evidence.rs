//! 新鲜证据的章节时间线及实际输入动作日志。
use super::*;
use std::{io::Write, time::UNIX_EPOCH};

pub(super) fn unix_ms(time: SystemTime) -> u64 {
    time.duration_since(UNIX_EPOCH)
        .expect("系统时间早于 UNIX_EPOCH")
        .as_millis()
        .try_into()
        .expect("时间戳溢出")
}

pub(super) fn append_timeline(
    output: &std::path::Path,
    event: &str,
    chapter: &Chapter,
    time: SystemTime,
) {
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open(output.join("demo-timeline.jsonl"))
        .expect("无法打开演示时间线");
    serde_json::to_writer(&mut file, &serde_json::json!({"event": event, "id": chapter.id, "title": chapter.title, "unix_ms": unix_ms(time)})).expect("无法写演示时间线");
    file.write_all(b"\n").expect("无法结束时间线记录");
    file.flush().expect("无法刷新演示时间线");
}

/// 只在边沿真正应用后写入；与章节时间线分离，移动帧和松手不制造音效点。
pub(super) fn append_actions(
    output: &std::path::Path,
    chapter: &Chapter,
    actions: &[(&str, String)],
    time: SystemTime,
) {
    if actions.is_empty() {
        return;
    }
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open(output.join("demo-actions.jsonl"))
        .expect("无法打开演示动作日志");
    let timestamp = unix_ms(time);
    for (kind, detail) in actions
        .iter()
        .filter(|(kind, _)| matches!(*kind, "left" | "middle" | "right" | "key" | "scroll"))
    {
        serde_json::to_writer(&mut file, &serde_json::json!({
            "event": "action", "id": chapter.id, "chapter_id": chapter.id, "title": chapter.title,
            "unix_ms": timestamp, "kind": kind, "detail": detail,
        })).expect("无法写演示动作日志");
        file.write_all(
            b"
",
        )
        .expect("无法结束动作记录");
    }
    file.flush().expect("无法刷新演示动作日志");
}
