"""Windows owned 编辑器物理客户区的进度片；默认静音，可选合成操作音效。

Vulkan的HWND GDI表面可能冻结，故gdigrab desktop只截取已验证owned窗口的
1920x1080物理客户区，绝不录制全桌面。窗口必须真实可见无遮挡；脚本只对
owned PID置顶（不激活、不改位置大小），持续核对客户区、TOPMOST与遮挡。
录制期间不得最小化、移动或遮挡窗口；不是后台捕获，也不验收原生输入。

python scripts/render_demo_video.py --no-build
python scripts/render_demo_video.py --render-only target/video-YYYYmmdd-HHMMSS
仅当九章原报告、样本保护与 ffprobe 全部通过才发布最终 MP4。
"""
import argparse
from array import array
import ctypes
from ctypes import wintypes
from datetime import datetime
from fractions import Fraction
import hashlib
import json
import math
import re
from pathlib import Path
import shutil
import subprocess
import sys
import time
import wave

import demo_editor as demo

ROOT = demo.ROOT
RAW_NAME = "raw.mp4"
TIMELINE_NAME = "demo-timeline.jsonl"
ACTIONS_NAME = "demo-actions.jsonl"
SOUND_NAME = "progress-effects.wav"
MANIFEST_NAME = "video-manifest.json"
FINAL_NAME = "dr-rs-progress-1080p.mp4"
FONT = "C:/Windows/Fonts/msyh.ttc"
INTRO_SECONDS = 4.0
OUTRO_SECONDS = 5.0
NOT_ACCEPTANCE = "内部输入展示；原生路径与输入法不在本片验收范围"
# 说明覆盖本章，不承诺字幕与某一单帧动作相位一致。
CAPTIONS = {
    "panels": ("部件目录：选取部件并预览旋转与镜像", "分类筛选：点击切换部件分类，再恢复全部", "碰撞拒绝：重叠放置不会改写船体或新增撤销记录", "合法放置：添加部件并展示撤销与重做"),
    "connections": ("紫色吸附提示：显示候选连接点与长梁沿边吸附", "同边多点连接：展示连接、撤销重做与 XML 往返"),
    "selection": ("默认左键选择/拖动/空白平移；中键框选；R 拖拽旋转，右键取消", "点击切换框选键：左键框选、中键平移；重叠保留位置角度，不新增外部连接", "点击后代跟随开关：保留内部连接，断开外部连接", "拖到右侧部件列表：删除整个连接组件，支持撤销"),
    "view": ("视角控制：平移、缩放，以及整船与选区适配", "视角帮助：F1 查看操作说明", "F3：单独展示调试信息", "F4：单独展示船体隐藏"),
    "staging": ("属性草稿：F2 打开属性编辑，修改后统一应用", "分级长列表：切换动作条目的移动标记", "应用后撤销重做，并进行 XML 往返检查"),
    "topology": ("树图切换与换父：查看并调整连接关系", "环路保护：拒绝循环关系；断开连接后可撤销"),
    "repair": ("Heronb 歧义修复：为真实重号部件逐一分配引用", "未分配引用时拒绝应用；完成分配后展示撤销重做", "只读原版样本：修复结果写入独立目录，不覆盖源文件"),
    "browser": ("千船虚拟列表：扫描临时目录中的 1000 份船体", "坏 XML 跳过：列表只绘制可见行", "滚动到末尾并点击打开，保留浏览位置"),
    "unsaved": ("未保存模态：窗口内暗色确认提示保护当前修改", "取消保留文档，放弃修改后继续；不演示原生保存路径"),
}
SHORT_TITLES = {
    "panels": "面板碰撞", "connections": "连接吸附", "selection": "选择编辑",
    "view": "视角帮助", "staging": "属性分级", "topology": "树图拓扑",
    "repair": "歧义修复", "browser": "千船浏览", "unsaved": "未保存确认",
}


class VideoError(demo.DemoError):
    pass


class WindowOwnershipError(VideoError):
    pass


def write_json(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def sha256(path):
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def timestamp(seconds, ass=False):
    if type(seconds) not in (int, float) or not math.isfinite(seconds) or seconds < 0:
        raise VideoError("字幕时间必须是有限非负数")
    units = 100 if ass else 1000
    ticks = int(math.floor(seconds * units + 0.5))
    whole, fraction = divmod(ticks, units)
    minutes, second = divmod(whole, 60)
    hour, minute = divmod(minutes, 60)
    if ass:
        return f"{hour}:{minute:02}:{second:02}.{fraction:02}"
    return f"{hour:02}:{minute:02}:{second:02},{fraction:03}"


def filter_path(path):
    """FFmpeg filter 内引用路径，不通过 shell。"""
    text = str(path).replace("\\", "/")
    text = text.replace(":", "\\:").replace("'", "'\\\\\\''")
    return "'" + text + "'"


def capture_origin(log):
    matches = re.findall(r"Duration: N/A, start: ([0-9]+(?:\.[0-9]+)?),", log)
    if len(matches) != 1:
        raise VideoError("无法从gdigrab日志唯一确定Unix录制起点")
    seconds = float(matches[0])
    if not math.isfinite(seconds) or seconds < 1000000000:
        raise VideoError("gdigrab起点不是Unix壁钟")
    return int(math.floor(seconds * 1000 + 0.5))


def load_timeline(path):
    try:
        rows = [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines() if line.strip()]
    except (OSError, ValueError) as error:
        raise VideoError(f"无法读取 timeline：{error}") from error
    return validate_timeline(rows)


def validate_timeline(rows):
    if not isinstance(rows, list) or len(rows) != 2 * len(demo.CHAPTER_IDS):
        raise VideoError("timeline 必须有九章完整的开始/结束事件")
    chapters = []
    last_ms = -1
    for index, chapter_id in enumerate(demo.CHAPTER_IDS):
        pair = rows[index * 2:index * 2 + 2]
        for row, event in zip(pair, ("chapter_started", "chapter_finished")):
            if not isinstance(row, dict) or row.get("event") != event or row.get("id") != chapter_id:
                raise VideoError("timeline 事件顺序、章节 ID 或事件类型无效")
            value = row.get("unix_ms")
            if type(value) is not int or value <= 0 or value < last_ms:
                raise VideoError("timeline unix_ms 必须为单调非降的正整数")
            if not isinstance(row.get("title"), str) or not row["title"].strip():
                raise VideoError("timeline 缺少章节标题")
            last_ms = value
        start, end = pair
        if end["unix_ms"] <= start["unix_ms"] or end["title"] != start["title"]:
            raise VideoError("timeline 章节时间或标题不一致")
        chapters.append({"id": chapter_id, "title": start["title"],
                         "started_unix_ms": start["unix_ms"], "finished_unix_ms": end["unix_ms"]})
    return chapters


def plan_clips(chapters, capture_start_unix_ms, raw_duration):
    if type(capture_start_unix_ms) is not int or capture_start_unix_ms <= 0:
        raise VideoError("缺少录制起点壁钟时间")
    if not math.isfinite(raw_duration) or raw_duration <= 0:
        raise VideoError("raw 时长无效")
    clips = []
    cursor = INTRO_SECONDS
    for chapter in chapters:
        start = (chapter["started_unix_ms"] - capture_start_unix_ms) / 1000
        end = (chapter["finished_unix_ms"] - capture_start_unix_ms) / 1000
        # 截掉启动等待与章内前约2s；短章不强行切掉动作。
        cut = min(2.0, max(0.0, (end - start) - 1.0))
        start = max(0.0, start + cut)
        if end > raw_duration + 0.25 or start >= end or end <= 0:
            raise VideoError(f"raw 未覆盖完整章节：{chapter['id']}")
        end = min(end, raw_duration)
        clip = dict(chapter, raw_start_seconds=start, raw_end_seconds=end,
                    trimmed_intro_seconds=cut, video_start_seconds=cursor,
                    video_end_seconds=cursor + end - start)
        clips.append(clip)
        cursor = clip["video_end_seconds"]
    return clips, cursor + OUTRO_SECONDS


def caption_entries(clips, duration):
    entries = [(0.0, INTRO_SECONDS, "dr-rs 开发进度"),
               (0.0, INTRO_SECONDS, NOT_ACCEPTANCE)]
    for clip in clips:
        lines = CAPTIONS[clip["id"]]
        part = (clip["video_end_seconds"] - clip["video_start_seconds"]) / len(lines)
        for index, text in enumerate(lines):
            start = clip["video_start_seconds"] + part * index
            entries.append((start, start + part, text))
    entries.extend([(duration - OUTRO_SECONDS, duration, "当前完成：九章编辑器展示与报告检查通过；非完整游戏"),
                    (duration - OUTRO_SECONDS, duration, NOT_ACCEPTANCE)])
    return entries


def ass_escape(text):
    return text.replace("\\", "\\\\").replace("{", "\\{").replace("}", "\\}").replace("\n", "\\N")


def write_subtitles(output, clips, duration):
    entries = caption_entries(clips, duration)
    # SRT 用单条两行首尾，避免播放器重叠条目。
    srt_entries = [(0.0, INTRO_SECONDS, "dr-rs 开发进度\n" + NOT_ACCEPTANCE)]
    srt_entries.extend(entries[2:-2])
    srt_entries.append((duration - OUTRO_SECONDS, duration,
                        "当前完成：九章编辑器展示与报告检查通过；非完整游戏\n" + NOT_ACCEPTANCE))
    srt = "\n\n".join(f"{i}\n{timestamp(start)} --> {timestamp(end)}\n{text}"
                       for i, (start, end, text) in enumerate(srt_entries, 1)) + "\n"
    (output / "progress.srt").write_text(srt, encoding="utf-8")
    header = """[Script Info]
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080
WrapStyle: 2
ScaledBorderAndShadow: yes

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Subtitle,Microsoft YaHei,36,&H00FFFFFF,&H00FFFFFF,&H00202020,&H80000000,0,0,0,0,100,100,0,0,1,1,0,2,360,70,38,1
Style: Chapter,Microsoft YaHei,36,&H00FFFFFF,&H00FFFFFF,&H00202020,&H80000000,0,0,0,0,100,100,0,0,1,1,0,4,36,0,0,1
Style: Scope,Microsoft YaHei,36,&H00FFFFFF,&H00FFFFFF,&H00202020,&H80000000,0,0,0,0,100,100,0,0,1,1,0,2,125,125,38,1
Style: Card,Microsoft YaHei,38,&H00FFFFFF,&H00FFFFFF,&H00202020,&H80000000,0,0,0,0,100,100,0,0,1,1,0,5,80,80,0,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
"""
    dialogues = []
    def add(start, end, style, text):
        dialogues.append(f"Dialogue: 0,{timestamp(start, True)},{timestamp(end, True)},{style},,0,0,0,,{text}")
    add(0, INTRO_SECONDS, "Card", ass_escape("dr-rs 开发进度\n九章编辑器能力展示"))
    add(0, INTRO_SECONDS, "Scope", ass_escape(NOT_ACCEPTANCE))
    for start, end, text in entries[2:-2]:
        add(start, end, "Subtitle", ass_escape(text))
    for index, clip in enumerate(clips, 1):
        # 短章名水平置于底部独立字幕栏左侧；正文从x360起，不遮UI或章名。
        title = f"{index:02} " + SHORT_TITLES[clip["id"]]
        add(clip["video_start_seconds"], clip["video_end_seconds"], "Chapter",
            "{\\pos(36,1020)}" + ass_escape(title))
    add(duration - OUTRO_SECONDS, duration, "Card", ass_escape(
        "当前完成\n九章编辑器展示与报告检查通过\n这是开发进度展示，并非完整游戏"))
    add(duration - OUTRO_SECONDS, duration, "Scope", ass_escape(NOT_ACCEPTANCE))
    (output / "progress.ass").write_text(header + "\n".join(dialogues) + "\n", encoding="utf-8")


def probe(path):
    result = subprocess.run(["ffprobe", "-v", "error", "-show_streams", "-show_format", "-of", "json", str(path)],
                            capture_output=True, text=True, encoding="utf-8", check=True)
    return json.loads(result.stdout)


def validate_raw_probe(info):
    """原生录制必须1080p且无音轨，拒绝靠成片缩放掩盖错误捕获。"""
    streams = info.get("streams", [])
    videos = [stream for stream in streams if stream.get("codec_type") == "video"]
    if len(videos) != 1 or any(stream.get("codec_type") == "audio" for stream in streams):
        raise VideoError("raw必须仅有一个视频流且无音频")
    if (videos[0].get("width"), videos[0].get("height")) != (1920, 1080):
        raise VideoError("raw原生捕获不是1920x1080，禁止缩放伪装1080p")
    try:
        duration = float(info.get("format", {}).get("duration", 0))
        size = int(info.get("format", {}).get("size", 0))
    except (TypeError, ValueError) as error:
        raise VideoError("raw规格字段无效") from error
    if not math.isfinite(duration) or duration <= 0 or size <= 0:
        raise VideoError("raw时长或数据为空")
    return duration


def validate_dynamic_frames(payload, clips, width=128, height=64, sample_fps=2):
    """每章至少一次实质画面变化；排除底部进度框，避免仅计时器掩盖冻结。"""
    frame_size = width * height
    if not payload or len(payload) % frame_size:
        raise VideoError("raw动态检测解码为空或帧数据不完整")
    frame_count = len(payload) // frame_size
    summaries = []
    minimum_pixels = max(8, math.ceil(frame_size * 0.001))
    for clip in clips:
        indices = [i for i in range(frame_count)
                   if clip["raw_start_seconds"] <= i / sample_fps < clip["raw_end_seconds"]]
        if len(indices) < 2:
            raise VideoError(f"章节动态检测采样不足：{clip['id']}")
        changed = 0
        previous = None
        for index in indices:
            current = payload[index * frame_size:(index + 1) * frame_size]
            if previous is not None:
                difference = sum(abs(a - b) >= 8 for a, b in zip(previous, current))
                if difference >= minimum_pixels:
                    changed += 1
            previous = current
        if not changed:
            raise VideoError(f"raw章节画面冻结或无可辨变化：{clip['id']}")
        summaries.append({"id": clip["id"], "sample_frames": len(indices), "changed_pairs": changed})
    if not summaries:
        raise VideoError("raw动态检测缺少章节")
    return {"passed": True, "sample_fps": sample_fps, "gray_size": [width, height],
            "excluded_bottom_pixels": 120, "pixel_delta_threshold": 8,
            "minimum_changed_pixels": minimum_pixels, "chapters": summaries,
            "scope": "低分辨率实质帧变化检查，不代替报告断言或人工内容审查"}


def check_raw_dynamics(raw, clips):
    command = ["ffmpeg", "-v", "error", "-i", str(raw), "-an", "-vf",
               "crop=iw:ih-120:0:0,fps=2:start_time=0,scale=128:64,format=gray",
               "-f", "rawvideo", "-pix_fmt", "gray", "pipe:1"]
    decoded = subprocess.run(command, capture_output=True, check=True)
    return validate_dynamic_frames(decoded.stdout, clips)


def load_actions(path):
    try:
        rows = [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines() if line.strip()]
    except (OSError, ValueError) as error:
        raise VideoError(f"无法读取真实replay音效事件：{error}") from error
    last = 0
    for row in rows:
        if (not isinstance(row, dict) or row.get("event") != "action" or type(row.get("unix_ms")) is not int
                or row["unix_ms"] <= 0 or row["unix_ms"] < last
                or row.get("kind") not in ("left", "middle", "right", "key", "scroll")):
            raise VideoError("音效事件须为有序真实Unix毫秒及已知replay种类")
        last = row["unix_ms"]
    if not rows:
        raise VideoError("真实replay音效事件为空，禁止编造操作音")
    return rows


def map_actions(actions, clips, capture_start_unix_ms):
    """仅保留实际剪辑半开区间内事件；片头/片尾及裁掉等待不补造声音。"""
    mapped = []
    for row in actions:
        raw_seconds = (row["unix_ms"] - capture_start_unix_ms) / 1000
        for clip in clips:
            if clip["raw_start_seconds"] <= raw_seconds < clip["raw_end_seconds"]:
                mapped.append({"kind": row["kind"], "unix_ms": row["unix_ms"], "chapter": clip["id"],
                               "video_seconds": clip["video_start_seconds"] + raw_seconds - clip["raw_start_seconds"],
                               "clip_end_seconds": clip["video_end_seconds"]})
                break
    if not mapped:
        raise VideoError("保留区间没有真实replay事件，禁止生成伪音效")
    return mapped


def synthesize_effects(path, events, duration, sample_rate=48000):
    """stdlib确定性低音量短音色；自合成，无配音/音乐/外部版权资源。"""
    if not events or not math.isfinite(duration) or duration <= 0:
        raise VideoError("音效合成缺少真实事件或有效时长")
    count = round(duration * sample_rate)
    samples = array("f", [0.0]) * count
    frequencies = {"left": 600, "middle": 440, "right": 720, "key": 520, "scroll": 280}
    for event in events:
        start = round(event["video_seconds"] * sample_rate)
        end_limit = round(min(duration, event["clip_end_seconds"]) * sample_rate)
        length = min(round(0.075 * sample_rate), end_limit - start)
        if start < 0 or start >= count or length <= 0 or event["kind"] not in frequencies:
            raise VideoError("音效事件时间或种类无效")
        frequency = frequencies[event["kind"]]
        for offset in range(length):
            seconds = offset / sample_rate
            envelope = min(1.0, seconds / 0.005) * max(0.0, 1 - offset / length) ** 2
            samples[start + offset] += 0.018 * envelope * math.sin(2 * math.pi * frequency * seconds)
    pcm = array("h", (round(max(-0.04, min(0.04, value)) * 32767) for value in samples))
    if sys.byteorder != "little":
        pcm.byteswap()
    with wave.open(str(path), "wb") as output:
        output.setnchannels(1)
        output.setsampwidth(2)
        output.setframerate(sample_rate)
        output.writeframes(pcm.tobytes())
    return {"source": "stdlib deterministic synthesized replay tones", "voice": False, "music": False,
            "external_resources": False, "sample_rate": sample_rate, "channels": 1,
            "duration_seconds": count / sample_rate, "tone_max_seconds": 0.075,
            "tone_amplitude": 0.018, "mixed_peak_limit": 0.04, "event_count": len(events)}


def render_command(raw, output, crf, sound_effects=False):
    command = ["ffmpeg", "-hide_banner", "-y", "-i", str(raw)]
    if sound_effects:
        command += ["-i", str(output / SOUND_NAME)]
    command += ["-filter_complex_script", str(output / "render-filter.txt"), "-map", "[v]"]
    if sound_effects:
        command += ["-map", "1:a:0", "-c:a", "aac", "-b:a", "96k", "-ar", "48000", "-ac", "1"]
    else:
        command += ["-an"]
    return command + ["-c:v", "libx264", "-preset", "medium", "-crf", str(crf),
                      "-pix_fmt", "yuv420p", "-r", "30", "-movflags", "+faststart", str(output / "render.partial.mp4")]


def validate_probe(info, expected_duration=None, sound_effects=False):
    streams = info.get("streams", [])
    videos = [stream for stream in streams if stream.get("codec_type") == "video"]
    audios = [s for s in streams if s.get("codec_type") == "audio"]
    if len(videos) != 1 or len(audios) != int(sound_effects):
        raise VideoError("成片视频/音频流数量与静音或音效模式不符")
    if sound_effects:
        audio = audios[0]
        try:
            audio_duration = float(audio.get("duration", 0))
        except (TypeError, ValueError) as error:
            raise VideoError("音效音轨时长无效") from error
        if (audio.get("codec_name") != "aac" or str(audio.get("sample_rate")) != "48000"
                or audio.get("channels") != 1 or not math.isfinite(audio_duration) or audio_duration <= 0
                or expected_duration is None or abs(audio_duration - expected_duration) > 0.35):
            raise VideoError("音效必须AAC/48000Hz/单声道且完整覆盖成片时长")
    stream = videos[0]
    if (stream.get("width"), stream.get("height")) != (1920, 1080):
        raise VideoError("成片必须是1920x1080")
    try:
        fps = Fraction(stream.get("avg_frame_rate", "0"))
        duration = float(info.get("format", {}).get("duration", 0))
        size = int(info.get("format", {}).get("size", 0))
    except (TypeError, ValueError, ZeroDivisionError) as error:
        raise VideoError("ffprobe 规格字段无效") from error
    if fps != 30 or not math.isfinite(duration) or duration <= 0 or size <= 0:
        raise VideoError("成片必须30fps且具有非空时长和数据")
    if stream.get("codec_name") != "h264" or stream.get("pix_fmt") != "yuv420p":
        raise VideoError("成片编码必须是H.264 yuv420p")
    if expected_duration is not None and abs(duration - expected_duration) > 0.35:
        raise VideoError("成片时长与章节裁剪计划不符")
    return {"width": 1920, "height": 1080, "fps": 30, "audio": bool(sound_effects),
            "audio_codec": "aac" if sound_effects else None,
            "codec": "h264", "pixel_format": "yuv420p", "duration_seconds": duration, "bytes": size}


def selection_expression(clips):
    """有序半开区间的并集；不含重复末边界，不改变保留帧的30fps速度。"""
    if not clips:
        raise VideoError("不能渲染空章节列表")
    terms = []
    previous_end = 0.0
    for clip in clips:
        start, end = clip["raw_start_seconds"], clip["raw_end_seconds"]
        if (type(start) not in (int, float) or type(end) not in (int, float)
                or not math.isfinite(start) or not math.isfinite(end)
                or start < previous_end or end <= start):
            raise VideoError("章节裁剪区间无效或重叠")
        terms.append(f"gte(t,{start:.6f})*lt(t,{end:.6f})")
        previous_end = end
    return "+".join(terms)


def build_filter(clips, subtitle_path):
    selection = selection_expression(clips)
    # 原录制只过一个分支，避免split九路让concat积压尚未消费的1080p帧。
    # fps先固定30，select只删区间外帧，N/(30*TB)只压紧被删等待，不加速操作。
    filters = [f"color=c=black:s=1920x1080:r=30:d={INTRO_SECONDS}[intro]",
               f"[0:v]fps=30,select='{selection}',setpts=N/(30*TB),"
               "scale=1706:960:force_original_aspect_ratio=decrease,"
               "pad=1706:960:(ow-iw)/2:(oh-ih)/2:color=black,setsar=1,"
               "pad=1920:1080:107:0:color=black[chapters]",
               f"color=c=black:s=1920x1080:r=30:d={OUTRO_SECONDS}[outro]",
               "[intro][chapters][outro]concat=n=3:v=1:a=0,"
               f"ass=filename={filter_path(subtitle_path)}:fontsdir={filter_path(Path(FONT).parent)},"
               "format=yuv420p[v]"]
    return ";\n".join(filters)


def cleanup_partial(output):
    """只删自己固定名称的临时成片；保留 raw、日志和失败证据。"""
    path = output.resolve() / "render.partial.mp4"
    if path.parent != output.resolve() or path.name != "render.partial.mp4":
        raise VideoError("拒绝清理未经确认的路径")
    if path.is_file():
        path.unlink()


def stop_recorder(process):
    if process is None:
        return
    if process.poll() is None:
        try:
            process.stdin.write("q\n")
            process.stdin.flush()
            process.wait(timeout=30)
        except (OSError, BrokenPipeError, subprocess.TimeoutExpired):
            demo.stop_owned_process(process)
            raise VideoError("ffmpeg 无法正常发送 q 并结束，raw 不可发布")
    if process.returncode != 0:
        raise VideoError(f"ffmpeg 录制失败：退出码{process.returncode}")


def configure_dpi_awareness(user32):
    """枚举前关闭本进程坐标虚拟化，与已成功的捕获探针保持一致。"""
    user32.SetProcessDPIAware.argtypes = []
    user32.SetProcessDPIAware.restype = wintypes.BOOL
    user32.IsProcessDPIAware.argtypes = []
    user32.IsProcessDPIAware.restype = wintypes.BOOL
    user32.SetProcessDPIAware()
    # 已由宿主设为aware时Set可能失败；仅在实际状态已aware时允许继续。
    if not user32.IsProcessDPIAware():
        raise VideoError("无法启用DPI aware，禁止以虚拟化坐标录制")


def owned_window_geometry(user32, hwnd, pid):
    """返回物理客户区/外框坐标；两者都必须严格1920x1080。"""
    window_pid = wintypes.DWORD()
    user32.GetWindowThreadProcessId(hwnd, ctypes.byref(window_pid))
    if window_pid.value != pid:
        raise WindowOwnershipError("HWND不再属于owned PID，拒绝操作")
    if not user32.IsWindowVisible(hwnd):
        raise VideoError("owned窗口不可见")
    bounds = []
    for getter in (user32.GetClientRect, user32.GetWindowRect):
        rect = wintypes.RECT()
        if not getter(hwnd, ctypes.byref(rect)):
            raise VideoError("owned窗口客户区或物理外框读取失败")
        coordinates = (rect.left, rect.top, rect.right, rect.bottom)
        if (rect.right - rect.left, rect.bottom - rect.top) != (1920, 1080):
            raise VideoError("owned窗口客户区或物理外框不是1920x1080")
        bounds.append(coordinates)
    return tuple(bounds)


def pin_owned_window(user32, hwnd, pid):
    """只置顶再次核对PID/双尺寸的owned窗口；GDI不是后台捕获。"""
    owned_window_geometry(user32, hwnd, pid)
    # HWND_TOPMOST=-1；NOSIZE|NOMOVE|NOACTIVATE：不聚焦，不注入输入。
    if not user32.SetWindowPos(hwnd, -1, 0, 0, 0, 0, 0x0013):
        raise VideoError("owned窗口SetWindowPos TOPMOST失败")
    # SetWindowPos可同步触发系统启动定位；后续独立稳态阶段再核双尺寸。
    window_pid = wintypes.DWORD()
    user32.GetWindowThreadProcessId(hwnd, ctypes.byref(window_pid))
    if window_pid.value != pid:
        raise WindowOwnershipError("置顶后HWND不再属于owned PID，拒绝录制")
    if not user32.IsWindowVisible(hwnd):
        raise VideoError("置顶后owned窗口不可见")
    if not user32.GetWindowLongW(hwnd, -20) & 0x00000008:
        raise VideoError("owned窗口未获得WS_EX_TOPMOST，禁止录制")


def wait_for_stable_window(user32, pid, timeout, process, enumerate_candidates):
    """先稳态0.6秒、只置顶一次，再等待置顶后物理几何稳态0.6秒。"""
    deadline = time.monotonic() + timeout
    stable_key = None
    stable_since = None
    pinned_hwnd = None
    while time.monotonic() < deadline:
        if process.poll() is not None:
            raise VideoError("等待窗口时 owned 编辑器已退出")
        if pinned_hwnd is not None:
            window_pid = wintypes.DWORD()
            user32.GetWindowThreadProcessId(pinned_hwnd, ctypes.byref(window_pid))
            if window_pid.value != pid:
                raise WindowOwnershipError("等待置顶稳态时HWND失去owned所有权")
        candidates = enumerate_candidates()
        current_key = None
        if candidates:
            _, hwnd = max(candidates)
            if pinned_hwnd is not None and hwnd != pinned_hwnd:
                raise WindowOwnershipError("置顶后主窗口HWND发生替换，拒绝录制")
            try:
                current_key = (hwnd, owned_window_geometry(user32, hwnd, pid))
            except WindowOwnershipError:
                raise
            except VideoError:
                pass  # 创建或置顶后的尺寸/样式过渡只重置稳态，不能捕获。
        now = time.monotonic()
        if current_key != stable_key:
            stable_key, stable_since = current_key, now if current_key else None
        if current_key is not None and now - stable_since >= 0.6:
            if pinned_hwnd is None:
                pin_owned_window(user32, hwnd, pid)
                pinned_hwnd = hwnd
                stable_key = stable_since = None  # 重新开始置顶后稳态阶段。
            else:
                # 返回前的瞬间变化同样重新等待，不捕获启动过渡帧。
                try:
                    final_geometry = owned_window_geometry(user32, hwnd, pid)
                except WindowOwnershipError:
                    raise
                except VideoError:
                    final_geometry = None
                if final_geometry == current_key[1]:
                    if not user32.GetWindowLongW(hwnd, -20) & 0x00000008:
                        raise VideoError("置顶稳态后丢失WS_EX_TOPMOST，禁止录制")
                    return hwnd
                stable_key = stable_since = None
        time.sleep(0.05)
    raise VideoError("找不到客户区和物理外框均1920x1080且置顶前后分别稳定0.6秒的owned主窗口")


def find_owned_window(pid, timeout, process):
    if sys.platform != "win32":
        raise VideoError("实际录制仅支持Windows")
    user32 = ctypes.WinDLL("user32", use_last_error=True)
    configure_dpi_awareness(user32)
    user32.GetWindowThreadProcessId.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.DWORD)]
    user32.GetWindowThreadProcessId.restype = wintypes.DWORD
    user32.IsWindowVisible.argtypes = [wintypes.HWND]
    user32.IsWindowVisible.restype = wintypes.BOOL
    user32.GetWindow.argtypes = [wintypes.HWND, wintypes.UINT]
    user32.GetWindow.restype = wintypes.HWND
    for getter in (user32.GetClientRect, user32.GetWindowRect):
        getter.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.RECT)]
        getter.restype = wintypes.BOOL
    user32.SetWindowPos.argtypes = [wintypes.HWND, wintypes.HWND, ctypes.c_int, ctypes.c_int,
                                    ctypes.c_int, ctypes.c_int, wintypes.UINT]
    user32.SetWindowPos.restype = wintypes.BOOL
    user32.GetWindowLongW.argtypes = [wintypes.HWND, ctypes.c_int]
    user32.GetWindowLongW.restype = ctypes.c_long
    callback_type = ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.HWND, wintypes.LPARAM)
    user32.EnumWindows.argtypes = [callback_type, wintypes.LPARAM]
    user32.EnumWindows.restype = wintypes.BOOL
    candidates = []
    @callback_type
    def callback(hwnd, _):
        window_pid = wintypes.DWORD()
        user32.GetWindowThreadProcessId(hwnd, ctypes.byref(window_pid))
        if window_pid.value == pid and user32.IsWindowVisible(hwnd) and not user32.GetWindow(hwnd, 4):
            rect = wintypes.RECT()
            if user32.GetClientRect(hwnd, ctypes.byref(rect)):
                width, height = rect.right - rect.left, rect.bottom - rect.top
                if width > 500 and height > 500:
                    candidates.append((width * height, int(hwnd)))
        return True
    def enumerate_candidates():
        candidates.clear()
        if not user32.EnumWindows(callback, 0):
            raise VideoError("EnumWindows失败，禁止录制")
        return candidates
    return wait_for_stable_window(user32, pid, timeout, process, enumerate_candidates)


def open_capture_api():
    user32 = ctypes.WinDLL("user32", use_last_error=True)
    configure_dpi_awareness(user32)
    signatures = {
        "GetWindowThreadProcessId": ([wintypes.HWND, ctypes.POINTER(wintypes.DWORD)], wintypes.DWORD),
        "IsWindowVisible": ([wintypes.HWND], wintypes.BOOL),
        "IsIconic": ([wintypes.HWND], wintypes.BOOL),
        "GetClientRect": ([wintypes.HWND, ctypes.POINTER(wintypes.RECT)], wintypes.BOOL),
        "GetWindowRect": ([wintypes.HWND, ctypes.POINTER(wintypes.RECT)], wintypes.BOOL),
        "ClientToScreen": ([wintypes.HWND, ctypes.POINTER(wintypes.POINT)], wintypes.BOOL),
        "GetWindowLongW": ([wintypes.HWND, ctypes.c_int], ctypes.c_long),
        "GetSystemMetrics": ([ctypes.c_int], ctypes.c_int),
        "WindowFromPoint": ([wintypes.POINT], wintypes.HWND),
        "GetAncestor": ([wintypes.HWND, wintypes.UINT], wintypes.HWND),
    }
    for name, (arguments, result) in signatures.items():
        function = getattr(user32, name)
        function.argtypes, function.restype = arguments, result
    return user32


def owned_capture_region(user32, hwnd, pid):
    """只允许可见无遮挡的owned客户区；desktop从输入端限制到此矩形。"""
    _, outer = owned_window_geometry(user32, hwnd, pid)
    if user32.IsIconic(hwnd) or not user32.GetWindowLongW(hwnd, -20) & 0x8:
        raise VideoError("owned窗口已最小化或不再置顶，拒绝区域捕获")
    point = wintypes.POINT(0, 0)
    if not user32.ClientToScreen(hwnd, ctypes.byref(point)):
        raise VideoError("无法确定owned物理客户区屏幕原点")
    x, y = point.x, point.y
    if (x, y, x + 1920, y + 1080) != outer:
        raise VideoError("owned物理客户区和无边框外框不一致")
    left, top, width, height = [user32.GetSystemMetrics(index) for index in (76, 77, 78, 79)]
    if not (left <= x and top <= y and x + 1920 <= left + width and y + 1080 <= top + height):
        raise VideoError("owned客户区未完全位于可见虚拟屏幕内")
    # 检测中心及边角/边中9个点；无法保证抵御采样间隙或任意微小覆盖。
    for dx in (8, 960, 1911):
        for dy in (8, 540, 1071):
            visible_hwnd = user32.WindowFromPoint(wintypes.POINT(x + dx, y + dy))
            if not visible_hwnd or user32.GetAncestor(visible_hwnd, 2) != hwnd:
                raise VideoError("owned客户区采样点被其它窗口遮挡，停止捕获")
    return x, y, 1920, 1080


def capture_command(output, region, crf):
    x, y, width, height = region
    if (width, height) != (1920, 1080) or any(type(value) is not int for value in region):
        raise VideoError("desktop输入必须严格限定owned1920x1080物理客户区")
    return ["ffmpeg", "-hide_banner", "-y", "-f", "gdigrab", "-framerate", "30", "-draw_mouse", "0",
            "-offset_x", str(x), "-offset_y", str(y), "-video_size", "1920x1080", "-i", "desktop",
            "-an", "-c:v", "libx264", "-preset", "ultrafast", "-crf", str(crf),
            "-pix_fmt", "yuv420p", "-r", "30", str(output / RAW_NAME)]


def record(options, output, manifest):
    binary = demo.prepare_binary(options, ROOT)
    command = [str(binary), "--demo-showcase", str(output), "--demo-step-ms", "450", "--demo-presentation"]
    editor = recorder = None
    started_ns = time.time_ns()
    manifest.update({"editor_command": command, "editor_started_unix_ms": started_ns // 1000000,
                     "report_started_ns": started_ns, "status": "recording"})
    write_json(output / MANIFEST_NAME, manifest)
    try:
        with (output / "editor.log").open("w", encoding="utf-8") as editor_log,                 (output / "capture.log").open("w", encoding="utf-8") as capture_log:
            editor = subprocess.Popen(command, cwd=ROOT, stdout=editor_log, stderr=subprocess.STDOUT)
            hwnd = find_owned_window(editor.pid, min(options.timeout, 60), editor)
            capture_api = open_capture_api()
            region = owned_capture_region(capture_api, hwnd, editor.pid)
            capture = capture_command(output, region, options.crf)
            manifest.update({"owned_editor_pid": editor.pid, "hwnd": f"0x{hwnd:x}", "capture_command": capture,
                             "capture_source": "gdigrab desktop cropped to owned physical client",
                             "capture_region_xywh": list(region),
                             "occlusion_check": "9-point WindowFromPoint every report poll; not an absolute occlusion guarantee",
                             "window_topmost_verified": True,
                             "window_dpi_aware_verified": True,
                             "window_client_and_outer_size": [1920, 1080],
                             "window_geometry_stable_min_seconds": 0.6,
                             "window_stability_phases": "置顶前0.6秒；只置顶一次；置顶后重新稳定0.6秒",
                             "visibility_requirement": "gdigrab需owned窗口真实可见且无遮挡；不是后台捕获",
                             "capture_requested_unix_ms": time.time_ns() // 1000000,
                             "capture_clock_note": "gdigrab Input start Unix timestamp；Popen壁钟另存capture_requested_unix_ms"})
            recorder = subprocess.Popen(capture, stdin=subprocess.PIPE, stdout=capture_log,
                                        stderr=subprocess.STDOUT, text=True, encoding="utf-8")
            write_json(output / MANIFEST_NAME, manifest)
            deadline = time.monotonic() + options.timeout
            while True:
                if recorder.poll() is not None:
                    raise VideoError(f"录制中ffmpeg提前退出：{recorder.returncode}")
                if editor.poll() is not None:
                    raise VideoError("报告完成前编辑器退出；禁止autoexit")
                if owned_capture_region(capture_api, hwnd, editor.pid) != region:
                    raise VideoError("录制中owned物理客户区移动；禁止继续截取旧区域")
                report = demo.read_completed_report(output, started_ns)
                if report is not None:
                    break
                if time.monotonic() >= deadline:
                    raise VideoError("九章报告等待超时")
                time.sleep(0.10)
            # 报告 passed 后才发 q；整个等待期间编辑器保持存活。
            stop_recorder(recorder)
            if editor.poll() is not None:
                raise VideoError("ffmpeg结束前编辑器退出，录制不完整")
            origin = capture_origin((output / "capture.log").read_text(encoding="utf-8"))
            manifest.update({"status": "recorded", "capture_start_unix_ms": origin,
                             "capture_end_unix_ms": time.time_ns() // 1000000})
            write_json(output / MANIFEST_NAME, manifest)
    finally:
        # 即使报告失败，也先结束录制，再仅清理自己创建的编辑器。
        try:
            if recorder is not None and recorder.poll() is None:
                stop_recorder(recorder)
        finally:
            demo.stop_owned_process(editor)


def render(options, output, manifest, originals):
    final = output / FINAL_NAME
    if final.exists():
        raise VideoError("最终成片已存在；不覆盖既有交付或字幕")
    started_ns = manifest.get("report_started_ns")
    if type(started_ns) is not int or started_ns <= 0:
        raise VideoError("manifest缺少本次报告起点，无法验证原始证据")
    report = demo.read_completed_report(output, started_ns)
    if report is None:
        raise VideoError("原始报告未通过，禁止成片")
    demo.check_samples(originals)
    raw = output / RAW_NAME
    raw_info = probe(raw)
    raw_duration = validate_raw_probe(raw_info)
    manifest["raw_ffprobe"] = raw_info
    chapters = load_timeline(output / TIMELINE_NAME)
    report_titles = {chapter["id"]: chapter["title"] for chapter in report["chapters"]}
    if any(report_titles[c["id"]] != c["title"] for c in chapters):
        raise VideoError("timeline标题与原报告不一致")
    clips, duration = plan_clips(chapters, manifest.get("capture_start_unix_ms"), raw_duration)
    manifest["raw_dynamic_validation"] = check_raw_dynamics(raw, clips)
    sound_effects = bool(getattr(options, "sound_effects", False))
    manifest["sound_effects"] = {"enabled": sound_effects, "voice": False, "music": False}
    if sound_effects:
        actions_path = output / ACTIONS_NAME
        actions = load_actions(actions_path)
        mapped = map_actions(actions, clips, manifest["capture_start_unix_ms"])
        sound_path = output / SOUND_NAME
        sound = synthesize_effects(sound_path, mapped, duration)
        sound.update({"enabled": True, "actions_sha256": sha256(actions_path),
                      "wav_sha256": sha256(sound_path), "events": mapped,
                      "scope": "真实内部replay事件提示音，不是录到的系统/游戏声音，不含配音"})
        manifest["sound_effects"] = sound
    manifest.setdefault("presentation", {}).update({
        "chapter_title": "底部独立字幕栏左侧水平短章名，位置36,1020",
        "subtitle_margin_left": 360, "subtitle_margin_right": 70})
    write_subtitles(output, clips, duration)
    graph = build_filter(clips, output / "progress.ass")
    (output / "render-filter.txt").write_text(graph, encoding="utf-8")
    partial = output / "render.partial.mp4"
    cleanup_partial(output)
    command = render_command(raw, output, options.crf, sound_effects)
    manifest.update({"status": "rendering", "render_command": command, "chapters": clips,
                     "planned_duration_seconds": duration, "raw_sha256": sha256(raw),
                     "timeline_sha256": sha256(output / TIMELINE_NAME), "report_sha256": sha256(output / demo.REPORT_NAME)})
    write_json(output / MANIFEST_NAME, manifest)
    try:
        with (output / "render.log").open("w", encoding="utf-8") as log:
            subprocess.run(command, stdout=log, stderr=subprocess.STDOUT, check=True)
        info = probe(partial)
        spec = validate_probe(info, duration, sound_effects)
        demo.check_samples(originals)
        manifest.update({"status": "validated", "specification": spec, "ffprobe": info,
                         "video_sha256": sha256(partial), "video_file": FINAL_NAME,
                         "subtitles": {name: sha256(output / name) for name in ("progress.srt", "progress.ass")},
                         "scope": NOT_ACCEPTANCE + "；本片为编辑器开发进度，不是完整游戏"})
        write_json(output / MANIFEST_NAME, manifest)
        # 不同于 replace，Windows rename 不覆盖已有文件。只在验证后发布 final 名称。
        partial.rename(final)
        manifest["status"] = "completed"
        write_json(output / MANIFEST_NAME, manifest)
        return final
    finally:
        cleanup_partial(output)


def parse_args(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--render-only", type=Path, metavar="INPUTDIR", help="复用raw、timeline和原报告，不启动窗口")
    parser.add_argument("--output", type=Path, help="默认target/video-时间戳；录制要求全新目录")
    parser.add_argument("--editor-bin", type=Path)
    parser.add_argument("--no-build", action="store_true")
    parser.add_argument("--timeout", type=demo.positive_seconds, default=600)
    parser.add_argument("--crf", type=int, choices=(18, 20), default=18)
    parser.add_argument("--sound-effects", action="store_true", help="按真实replay事件自合成低量提示音；默认无音轨，无配音")
    options = parser.parse_args(argv)
    if options.render_only and options.output:
        parser.error("--render-only不能与--output同时使用")
    return options


def main(argv=None):
    options = parse_args(argv)
    output = (options.render_only or options.output or
              ROOT / "target" / ("video-" + datetime.now().strftime("%Y%m%d-%H%M%S-%f"))).resolve()
    manifest = None
    originals = None
    try:
        for program in ("ffmpeg", "ffprobe"):
            if not shutil.which(program):
                raise VideoError(f"{program}不在PATH")
        originals = demo.sample_hashes(ROOT)
        if options.render_only:
            manifest = json.loads((output / MANIFEST_NAME).read_text(encoding="utf-8"))
            expected = manifest.get("samples_sha256")
            if expected != {str(path): digest for path, digest in originals.items()}:
                raise VideoError("原样本SHA与录制manifest不一致；不会恢复覆盖样本")
        else:
            output.mkdir(parents=True, exist_ok=False)
            manifest = {"schema_version": 1, "status": "prepared", "output_directory": str(output),
                        "samples_sha256": {str(path): digest for path, digest in originals.items()},
                        "recording": {"source": "gdigrab desktop restricted to visible TOPMOST owned physical client region",
                                      "requires_visible_unoccluded_window": True, "background_capture": False, "fps": 30, "audio": False,
                                      "native_size": [1920, 1080], "demo_step_ms": 450},
                        "presentation": {"canvas": [1920, 1080], "picture": [1706, 960], "picture_xy": [107, 0],
                                         "subtitle_bar_height": 120, "font": FONT, "font_size": 36,
                                         "chapter_title": "底部独立字幕栏左侧水平短章名，位置36,1020",
                                         "subtitle_margin_left": 360, "subtitle_margin_right": 70}}
            record(options, output, manifest)
        final = render(options, output, manifest, originals)
        print(f"成片验证通过：{final}", flush=True)
        return 0
    except (OSError, ValueError, subprocess.SubprocessError, demo.DemoError) as error:
        print(f"未交付成片：{error}", file=sys.stderr, flush=True)
        if manifest is not None and output.is_dir():
            # 既有成功交付不因render-only误调用被降级。
            if not (output / FINAL_NAME).exists():
                manifest.update({"status": "failed", "error": str(error)})
                write_json(output / MANIFEST_NAME, manifest)
            cleanup_partial(output)
        return 1
    finally:
        if originals is not None:
            demo.check_samples(originals)


if __name__ == "__main__":
    raise SystemExit(main())
