"""纯单元回归：不启动GUI、ffmpeg或编辑器。"""
import copy
from array import array
from pathlib import Path
import subprocess
import tempfile
from types import SimpleNamespace
import unittest
import wave
from unittest.mock import Mock, patch

import render_demo_video as video


def timeline():
    rows = []
    clock = 1700000000000
    for index, chapter_id in enumerate(video.demo.CHAPTER_IDS):
        duration = 4000 + index * 500
        for event, stamp in (("chapter_started", clock), ("chapter_finished", clock + duration)):
            rows.append({"event": event, "id": chapter_id, "title": "章节" + chapter_id, "unix_ms": stamp})
        clock += duration + 200
    return rows


def valid_probe():
    return {"streams": [{"codec_type": "video", "width": 1920, "height": 1080,
                         "avg_frame_rate": "30/1", "codec_name": "h264", "pix_fmt": "yuv420p"}],
            "format": {"duration": "45.0", "size": "1000"}}


class TimeTests(unittest.TestCase):
    def test_srt_round_and_carry(self):
        self.assertEqual(video.timestamp(0), "00:00:00,000")
        self.assertEqual(video.timestamp(59.9996), "00:01:00,000")
        self.assertEqual(video.timestamp(3661.234), "01:01:01,234")

    def test_ass_centiseconds(self):
        self.assertEqual(video.timestamp(59.999, True), "0:01:00.00")
        self.assertEqual(video.timestamp(3661.23, True), "1:01:01.23")

    def test_invalid_time(self):
        for value in (-1, float("nan"), float("inf"), True, "1"):
            with self.subTest(value=value), self.assertRaises(video.VideoError):
                video.timestamp(value)

    def test_capture_origin_uses_actual_input_clock(self):
        text = "Input #0, gdigrab\n  Duration: N/A, start: 1791283039.643547, bitrate: 1\n"
        self.assertEqual(video.capture_origin(text), 1791283039644)
        for invalid in ("", "Duration: N/A, start: 0.0,", text + text):
            with self.subTest(invalid=invalid), self.assertRaises(video.VideoError):
                video.capture_origin(invalid)


class TimelineTests(unittest.TestCase):
    def test_complete_ordered_chapters(self):
        chapters = video.validate_timeline(timeline())
        self.assertEqual([c["id"] for c in chapters], list(video.demo.CHAPTER_IDS))

    def test_missing_duplicate_unknown_and_wrong_event(self):
        cases = [timeline()[:-1]]
        for key, value in (("id", "unknown"), ("event", "chapter_finished"), ("title", ""), ("unix_ms", True)):
            rows = timeline()
            rows[0][key] = value
            cases.append(rows)
        rows = timeline()
        rows[2]["id"] = rows[0]["id"]
        cases.append(rows)
        for rows in cases:
            with self.subTest(rows=rows[:2]), self.assertRaises(video.VideoError):
                video.validate_timeline(rows)

    def test_reverse_time_zero_length_and_title_mismatch(self):
        for change in ("reverse", "zero", "title"):
            rows = timeline()
            if change == "title":
                rows[1]["title"] = "不同"
            else:
                rows[1]["unix_ms"] = rows[0]["unix_ms"] - (1 if change == "reverse" else 0)
            with self.subTest(change=change), self.assertRaises(video.VideoError):
                video.validate_timeline(rows)

    def test_load_jsonl_utf8_and_reject_truncated(self):
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / "timeline.jsonl"
            import json
            path.write_text("\n".join(json.dumps(row, ensure_ascii=False) for row in timeline()), encoding="utf-8")
            self.assertEqual(len(video.load_timeline(path)), len(video.demo.CHAPTER_IDS))
            path.write_text('{"event":', encoding="utf-8")
            with self.assertRaises(video.VideoError):
                video.load_timeline(path)

    def test_clip_real_times_remove_wait_not_fixed_chapters(self):
        chapters = video.validate_timeline(timeline())
        origin = chapters[0]["started_unix_ms"] - 7000
        clips, duration = video.plan_clips(chapters, origin, 100)
        self.assertEqual(clips[0]["raw_start_seconds"], 9)
        self.assertEqual(clips[0]["raw_end_seconds"], 11)
        self.assertEqual(clips[0]["video_start_seconds"], 4)
        self.assertEqual(clips[1]["video_start_seconds"], 6)
        self.assertEqual(clips[-1]["video_end_seconds"] + 5, duration)
        self.assertNotEqual(clips[0]["raw_end_seconds"] - clips[0]["raw_start_seconds"],
                            clips[-1]["raw_end_seconds"] - clips[-1]["raw_start_seconds"])

    def test_short_chapter_retains_one_second(self):
        chapters = video.validate_timeline(timeline())[:1]
        chapters[0]["finished_unix_ms"] = chapters[0]["started_unix_ms"] + 1500
        clips, duration = video.plan_clips(chapters, chapters[0]["started_unix_ms"], 10)
        self.assertEqual(clips[0]["trimmed_intro_seconds"], 0.5)
        self.assertEqual(duration, 10)

    def test_missing_raw_tail_and_invalid_origin(self):
        chapters = video.validate_timeline(timeline())
        for origin, duration in ((0, 100), (chapters[0]["started_unix_ms"], 5)):
            with self.assertRaises(video.VideoError):
                video.plan_clips(chapters, origin, duration)


class SubtitleAndFilterTests(unittest.TestCase):
    def setUp(self):
        chapters = video.validate_timeline(timeline())
        self.clips, self.duration = video.plan_clips(chapters, chapters[0]["started_unix_ms"], 100)

    def test_chapter_captions_two_to_four(self):
        self.assertEqual(set(video.CAPTIONS), set(video.demo.CHAPTER_IDS))
        for lines in video.CAPTIONS.values():
            self.assertTrue(2 <= len(lines) <= 4)
        body = " ".join(" ".join(lines) for lines in video.CAPTIONS.values())
        for required in ("碰撞", "紫色吸附", "R", "后代跟随", "删除", "视角帮助", "属性草稿", "分级",
                         "树图", "换父", "环路保护", "Heronb", "只读", "千船", "虚拟列表", "未保存模态"):
            self.assertIn(required, body)

    def test_captions_match_actual_mouse_config_and_separate_debug_visibility(self):
        selection = video.CAPTIONS["selection"]
        self.assertIn("默认左键选择/拖动/空白平移", selection[0])
        self.assertIn("中键框选", selection[0])
        self.assertIn("右键取消", selection[0])
        self.assertIn("点击切换框选键", selection[1])
        self.assertIn("左键框选、中键平移", selection[1])
        self.assertIn("点击后代跟随开关", selection[2])
        self.assertIn("分类筛选", video.CAPTIONS["panels"][1])
        self.assertIn("恢复全部", video.CAPTIONS["panels"][1])
        view = video.CAPTIONS["view"]
        self.assertEqual(len(view), 4)
        self.assertIn("F1", view[1])
        self.assertIn("F3", view[2])
        self.assertIn("调试", view[2])
        self.assertNotIn("F4", view[2])
        self.assertIn("F4", view[3])
        self.assertIn("隐藏", view[3])
        self.assertNotIn("F3", view[3])
        body = " ".join(selection)
        for invented in ("更多设置", "完整游戏", "原生保存", "右键框选"):
            self.assertNotIn(invented, body)

    def test_utf8_srt_ass_and_explicit_scope(self):
        with tempfile.TemporaryDirectory() as folder:
            output = Path(folder)
            video.write_subtitles(output, self.clips, self.duration)
            srt = (output / "progress.srt").read_bytes().decode("utf-8")
            ass = (output / "progress.ass").read_bytes().decode("utf-8")
            for text in (srt, ass):
                self.assertIn("开发进度", text)
                self.assertIn("输入法不在本片验收", text)
                self.assertIn("非完整游戏" if text == srt else "并非完整游戏", text)
            self.assertIn("00:00:00,000 --> 00:00:04,000", srt)
            self.assertIn("PlayResX: 1920", ass)
            self.assertIn("Microsoft YaHei,36", ass)
            self.assertEqual(ass.count(",Chapter,,"), len(video.demo.CHAPTER_IDS))
            self.assertIn(r"{\pos(36,1020)}", ass)

    def test_selection_captions_describe_actual_operations(self):
        lines = video.CAPTIONS["selection"]
        self.assertEqual(len(lines), 4)
        for text, phrase in zip(lines, ("R 拖拽旋转", "不新增外部连接", "点击后代跟随开关", "删除整个连接组件")):
            self.assertIn(phrase, text)
        self.assertIn("拖到右侧部件列表", lines[-1])
        body = " ".join(" ".join(lines) for lines in video.CAPTIONS.values())
        for invented in ("子孙列表", "结合筛选", "保存分支"):
            self.assertNotIn(invented, body)
        self.assertIn("不演示原生保存路径", " ".join(video.CAPTIONS["unsaved"]))
        self.assertIn("临时目录", " ".join(video.CAPTIONS["browser"]))

    def test_horizontal_titles_and_body_stay_inside_independent_bar(self):
        with tempfile.TemporaryDirectory() as folder:
            output = Path(folder)
            video.write_subtitles(output, self.clips, self.duration)
            text = (output / "progress.ass").read_text(encoding="utf-8")
            titles = [line for line in text.splitlines() if line.startswith("Dialogue:") and ",Chapter,," in line]
            self.assertEqual(len(titles), len(video.demo.CHAPTER_IDS))
            for line in titles:
                self.assertIn(r"{\pos(36,1020)}", line)
                self.assertNotIn(r"\N", line)
            styles = {}
            for line in text.splitlines():
                if line.startswith("Style: "):
                    fields = line[7:].split(",")
                    styles[fields[0]] = fields
            self.assertEqual(styles["Chapter"][18], "4")  # 水平左对齐、垂直居中
            self.assertEqual(styles["Subtitle"][19:22], ["360", "70", "38"])
            self.assertEqual(styles["Scope"][19:22], ["125", "125", "38"])
            # 按每字符一个字号保守估计，短章名不越入正文、正文不超字幕栏宽度。
            for title in video.SHORT_TITLES.values():
                self.assertLess(36 + len("00 " + title) * 36, 360)
            for lines in video.CAPTIONS.values():
                for caption in lines:
                    self.assertLessEqual(len(caption) * 36, 1920 - 360 - 70)
            self.assertGreater(1020 - 36, 960)
            self.assertLess(1020 + 36, 1080)

    def test_windows_filter_path(self):
        self.assertEqual(video.filter_path(r"D:\test files\中文.ass"), "'D\\:/test files/中文.ass'")
        escaped = video.filter_path("D:/one's/test.ass")
        self.assertIn("one", escaped)
        self.assertIn(r"\\\'", escaped)

    def test_filter_reserves_subtitle_bar_and_no_audio(self):
        graph = video.build_filter(self.clips, Path("D:/video/progress.ass"))
        self.assertIn("scale=1706:960:force_original_aspect_ratio=decrease", graph)
        self.assertIn("pad=1920:1080:107:0", graph)
        self.assertIn("concat=n=3:v=1:a=0", graph)
        self.assertIn("fps=30", graph)
        self.assertIn("format=yuv420p", graph)
        self.assertIn("gte(t,2.000000)*lt(t,4.000000)", graph)

    def test_single_raw_branch_removes_gaps_without_acceleration(self):
        graph = video.build_filter(self.clips, Path("D:/video/progress.ass"))
        self.assertEqual(graph.count("[0:v]"), 1)
        self.assertNotIn("split=", graph)
        self.assertNotIn("trim=", graph)
        self.assertIn("[0:v]fps=30,select='", graph)
        self.assertIn("setpts=N/(30*TB)", graph)
        self.assertIn("[intro][chapters][outro]concat=n=3:v=1:a=0", graph)
        expression = video.selection_expression(self.clips)
        self.assertEqual(expression.count("gte(t,"), len(video.demo.CHAPTER_IDS))
        self.assertEqual(expression.count("lt(t,"), len(video.demo.CHAPTER_IDS))
        self.assertEqual(expression.count("+"), len(video.demo.CHAPTER_IDS) - 1)
        self.assertNotIn("between(", expression)
        for clip in self.clips:
            self.assertIn(f"gte(t,{clip['raw_start_seconds']:.6f})*lt(t,{clip['raw_end_seconds']:.6f})", expression)

    def test_adjacent_intervals_have_half_open_end_boundaries(self):
        clips = [{"raw_start_seconds": 1, "raw_end_seconds": 2},
                 {"raw_start_seconds": 2, "raw_end_seconds": 3}]
        self.assertEqual(video.selection_expression(clips),
                         "gte(t,1.000000)*lt(t,2.000000)+gte(t,2.000000)*lt(t,3.000000)")
        # 30fps帧的区间筛选并集没有重叠；每个保留帧仍对应1/30秒。
        frames = [i for i in range(120) if any(c["raw_start_seconds"] <= i / 30 < c["raw_end_seconds"] for c in clips)]
        self.assertEqual(len(frames), 60)
        self.assertEqual(len(set(frames)), 60)

    def test_selection_rejects_empty_nonfinite_or_overlapping_intervals(self):
        cases = [[], [{"raw_start_seconds": 2, "raw_end_seconds": 1}],
                 [{"raw_start_seconds": 0, "raw_end_seconds": float("inf")}],
                 [{"raw_start_seconds": 0, "raw_end_seconds": 2},
                  {"raw_start_seconds": 1, "raw_end_seconds": 3}]]
        for clips in cases:
            with self.subTest(clips=clips), self.assertRaises(video.VideoError):
                video.selection_expression(clips)


class OwnedWindowTests(unittest.TestCase):
    def api(self, pid=123, width=1920, height=1080):
        api = Mock()
        def get_pid(hwnd, pointer):
            pointer._obj.value = pid
            return 1
        def get_rect(hwnd, pointer):
            pointer._obj.left = pointer._obj.top = 0
            pointer._obj.right = width
            pointer._obj.bottom = height
            return True
        api.GetWindowThreadProcessId.side_effect = get_pid
        api.GetClientRect.side_effect = get_rect
        api.GetWindowRect.side_effect = get_rect
        api.IsWindowVisible.return_value = True
        api.SetWindowPos.return_value = True
        api.GetWindowLongW.return_value = 0x8
        return api

    def test_only_verified_owned_hwnd_is_pinned_without_activation(self):
        api = self.api()
        video.pin_owned_window(api, 0x1234, 123)
        api.SetWindowPos.assert_called_once_with(0x1234, -1, 0, 0, 0, 0, 0x13)
        api.GetWindowLongW.assert_called_once_with(0x1234, -20)
        api.ShowWindow.assert_not_called()
        api.SetForegroundWindow.assert_not_called()

    def test_foreign_pid_invisible_or_wrong_size_never_operated(self):
        apis = [self.api(pid=456), self.api(width=0, height=0), self.api(width=1440, height=900)]
        invisible = self.api()
        invisible.IsWindowVisible.return_value = False
        apis.append(invisible)
        for api in apis:
            with self.subTest(api=api), self.assertRaises(video.VideoError):
                video.pin_owned_window(api, 0x1234, 123)
            api.SetWindowPos.assert_not_called()

    def test_topmost_failure_or_failed_verification_blocks_recording(self):
        failed = self.api()
        failed.SetWindowPos.return_value = False
        unverified = self.api()
        unverified.GetWindowLongW.return_value = 0
        for api in (failed, unverified):
            with self.subTest(api=api), self.assertRaises(video.VideoError):
                video.pin_owned_window(api, 0x1234, 123)

    def test_reused_hwnd_detected_after_topmost(self):
        api = self.api()
        def get_pid(hwnd, pointer):
            pointer._obj.value = 123 if api.GetWindowThreadProcessId.call_count == 1 else 456
            return 1
        api.GetWindowThreadProcessId.side_effect = get_pid
        with self.assertRaises(video.VideoError):
            video.pin_owned_window(api, 0x1234, 123)
        api.SetWindowPos.assert_called_once()

    def test_dpi_awareness_is_configured_and_verified(self):
        api = Mock()
        api.IsProcessDPIAware.return_value = True
        api.SetProcessDPIAware.return_value = False  # 宿主已设置也可继续。
        video.configure_dpi_awareness(api)
        self.assertEqual([call[0] for call in api.mock_calls],
                         ["SetProcessDPIAware", "IsProcessDPIAware"])
        api.IsProcessDPIAware.return_value = False
        with self.assertRaises(video.VideoError):
            video.configure_dpi_awareness(api)

    def test_outer_size_mismatch_or_read_failure_never_pinned(self):
        wrong = self.api()
        def wrong_rect(hwnd, pointer):
            pointer._obj.right, pointer._obj.bottom = 1456, 939
            return True
        wrong.GetWindowRect.side_effect = wrong_rect
        unreadable = self.api()
        unreadable.GetWindowRect.side_effect = None
        unreadable.GetWindowRect.return_value = False
        for api in (wrong, unreadable):
            with self.subTest(api=api), self.assertRaises(video.VideoError):
                video.pin_owned_window(api, 0x1234, 123)
            api.SetWindowPos.assert_not_called()

    def test_pin_accepts_system_position_initialization_then_requires_later_stability(self):
        api = self.api()
        def system_position(hwnd, pointer):
            pointer._obj.left = pointer._obj.top = 260 if api.SetWindowPos.called else 416
            pointer._obj.right = pointer._obj.left + 1920
            pointer._obj.bottom = pointer._obj.top + 1080
            return True
        api.GetWindowRect.side_effect = system_position
        video.pin_owned_window(api, 0x1234, 123)
        api.SetWindowPos.assert_called_once()
        # pin本身仅前核尺寸，系统后续位置/帧尺寸收敛由第二稳态阶段保证。
        api.GetWindowRect.assert_called_once()

    def wait_mock(self, api, enumerate_at=None, timeout=2):
        clock = {"now": 0.0}
        sleeps = []
        def sleep(seconds):
            sleeps.append(seconds)
            clock["now"] = round(clock["now"] + seconds, 6)
        process = Mock()
        process.poll.return_value = None
        def enumerate_candidates():
            return enumerate_at(clock["now"], api) if enumerate_at else [(1920 * 1080, 0x1234)]
        with patch.object(video.time, "monotonic", side_effect=lambda: clock["now"]), \
                patch.object(video.time, "sleep", side_effect=sleep):
            hwnd = video.wait_for_stable_window(api, 123, timeout, process, enumerate_candidates)
        return hwnd, clock["now"], sleeps

    def test_stable_physical_geometry_waits_at_least_point_six_not_four_seconds(self):
        api = self.api()
        hwnd, elapsed, sleeps = self.wait_mock(api)
        self.assertEqual(hwnd, 0x1234)
        self.assertGreaterEqual(elapsed, 1.2)
        self.assertLess(elapsed, 1.4)
        self.assertTrue(all(delay == 0.05 for delay in sleeps))
        api.SetWindowPos.assert_called_once()
        self.assertGreaterEqual(api.GetWindowRect.call_count, 16)

    def test_temporary_wrong_outer_geometry_resets_stability(self):
        api = self.api()
        def enumerate_at(now, api):
            def get_rect(hwnd, pointer):
                pointer._obj.right, pointer._obj.bottom = (1456, 939) if now == 0.3 else (1920, 1080)
                return True
            api.GetWindowRect.side_effect = get_rect
            if now < 0.95:
                api.SetWindowPos.assert_not_called()
            return [(1920 * 1080, 0x1234)]
        _, elapsed, _ = self.wait_mock(api, enumerate_at)
        self.assertGreaterEqual(elapsed, 1.55)
        api.SetWindowPos.assert_called_once()

    def test_changed_hwnd_or_missing_window_resets_stability(self):
        for missing in (True, False):
            api = self.api()
            def enumerate_at(now, api):
                if missing and now == 0.3:
                    return []
                return [(1920 * 1080, 0x1234 if now < 0.3 or missing else 0x5678)]
            hwnd, elapsed, _ = self.wait_mock(api, enumerate_at)
            with self.subTest(missing=missing):
                self.assertGreaterEqual(elapsed, 0.9)
                self.assertEqual(hwnd, 0x1234 if missing else 0x5678)

    def test_never_stable_window_times_out_without_pin(self):
        api = self.api(width=1440, height=900)
        with self.assertRaises(video.VideoError):
            self.wait_mock(api, timeout=0.8)
        api.SetWindowPos.assert_not_called()

    def test_geometry_changed_after_pin_converges_without_repeated_pin(self):
        for resize in (False, True):
            api = self.api()
            times = []
            def enumerate_at(now, api):
                if api.SetWindowPos.called:
                    times.append(now)
                def get_rect(hwnd, pointer):
                    pinned = api.SetWindowPos.called
                    pointer._obj.left = pointer._obj.top = 260 if pinned else 416
                    transient = resize and pinned and now < 0.85
                    pointer._obj.right = pointer._obj.left + (1456 if transient else 1920)
                    pointer._obj.bottom = pointer._obj.top + (939 if transient else 1080)
                    return True
                api.GetWindowRect.side_effect = get_rect
                return [(1920 * 1080, 0x1234)]
            hwnd, elapsed, _ = self.wait_mock(api, enumerate_at)
            with self.subTest(resize=resize):
                self.assertEqual(hwnd, 0x1234)
                self.assertGreaterEqual(elapsed, 1.45 if resize else 1.25)
                api.SetWindowPos.assert_called_once()
                self.assertGreaterEqual(len(times), 13)

    def test_nonconverging_post_pin_geometry_times_out_without_second_pin(self):
        api = self.api()
        def enumerate_at(now, api):
            def get_rect(hwnd, pointer):
                pointer._obj.right = 1456 if api.SetWindowPos.called else 1920
                pointer._obj.bottom = 939 if api.SetWindowPos.called else 1080
                return True
            api.GetWindowRect.side_effect = get_rect
            return [(1920 * 1080, 0x1234)]
        with self.assertRaises(video.VideoError):
            self.wait_mock(api, enumerate_at, timeout=1.5)
        api.SetWindowPos.assert_called_once()

    def test_ownership_loss_during_either_stability_phase_fails_immediately(self):
        for after_pin in (False, True):
            api = self.api()
            def enumerate_at(now, api):
                def get_pid(hwnd, pointer):
                    lost = api.SetWindowPos.called if after_pin else now >= 0.3
                    pointer._obj.value = 456 if lost else 123
                    return 1
                api.GetWindowThreadProcessId.side_effect = get_pid
                return [(1920 * 1080, 0x1234)]
            with self.subTest(after_pin=after_pin), self.assertRaises(video.WindowOwnershipError):
                self.wait_mock(api, enumerate_at)
            self.assertEqual(api.SetWindowPos.call_count, 1 if after_pin else 0)


class ProbeTests(unittest.TestCase):
    def test_raw_requires_native_1080p_no_audio_and_nonempty(self):
        self.assertEqual(video.validate_raw_probe(valid_probe()), 45)
        cases = []
        wrong_size = valid_probe()
        wrong_size["streams"][0].update(width=1456, height=939)
        cases.append(wrong_size)
        audio = valid_probe()
        audio["streams"].append({"codec_type": "audio"})
        cases.append(audio)
        missing = valid_probe()
        missing["streams"] = []
        cases.append(missing)
        for field, value in (("duration", "nan"), ("duration", "0"), ("size", "0")):
            info = valid_probe()
            info["format"][field] = value
            cases.append(info)
        for info in cases:
            with self.subTest(info=info), self.assertRaises(video.VideoError):
                video.validate_raw_probe(info)

    def test_good_video(self):
        self.assertEqual(video.validate_probe(valid_probe(), 45)["fps"], 30)

    def test_audio_bad_dimensions_fps_and_empty(self):
        cases = []
        info = valid_probe()
        info["streams"].append({"codec_type": "audio"})
        cases.append(info)
        for field, value in (("width", 1280), ("avg_frame_rate", "30000/1001"), ("pix_fmt", "yuv444p")):
            info = valid_probe()
            info["streams"][0][field] = value
            cases.append(info)
        for field, value in (("duration", "0"), ("size", "0"), ("duration", "nan")):
            info = valid_probe()
            info["format"][field] = value
            cases.append(info)
        for info in cases:
            with self.subTest(info=info), self.assertRaises(video.VideoError):
                video.validate_probe(info)
        with self.assertRaises(video.VideoError):
            video.validate_probe(valid_probe(), 44)


class ProtectionAndCleanupTests(unittest.TestCase):
    def test_cleanup_keeps_raw_and_existing_final(self):
        with tempfile.TemporaryDirectory() as folder:
            output = Path(folder)
            for name in ("render.partial.mp4", video.RAW_NAME, video.FINAL_NAME):
                (output / name).write_bytes(b"evidence")
            video.cleanup_partial(output)
            self.assertFalse((output / "render.partial.mp4").exists())
            self.assertTrue((output / video.RAW_NAME).exists())
            self.assertTrue((output / video.FINAL_NAME).exists())

    def test_existing_delivery_is_not_overwritten(self):
        with tempfile.TemporaryDirectory() as folder:
            output = Path(folder)
            for name in (video.FINAL_NAME, "progress.ass", "progress.srt"):
                (output / name).write_bytes(b"existing")
            with patch.object(video.demo, "read_completed_report") as read:
                with self.assertRaises(video.VideoError):
                    video.render(SimpleNamespace(crf=18), output, {}, {})
                read.assert_not_called()
            for name in (video.FINAL_NAME, "progress.ass", "progress.srt"):
                self.assertEqual((output / name).read_bytes(), b"existing")

    def test_record_stops_ffmpeg_before_owned_editor(self):
        with tempfile.TemporaryDirectory() as folder:
            output = Path(folder)
            editor, recorder = Mock(), Mock()
            editor.pid = 123
            editor.poll.return_value = None
            recorder.poll.return_value = None
            stopped = []
            def stop_capture(process):
                stopped.append("ffmpeg")
                process.poll.return_value = 0
                (output / "capture.log").write_text(
                    "Duration: N/A, start: 1700000000.123000, bitrate: 1", encoding="utf-8")
            options = SimpleNamespace(timeout=1, crf=18)
            with patch.object(video.demo, "prepare_binary", return_value=Path("editor.exe")), \
                    patch.object(video, "find_owned_window", return_value=0x1234), \
                    patch.object(video, "open_capture_api", return_value=Mock()), \
                    patch.object(video, "owned_capture_region", return_value=(260, 260, 1920, 1080)), \
                    patch.object(video.subprocess, "Popen", side_effect=[editor, recorder]) as popen, \
                    patch.object(video.demo, "read_completed_report", return_value={"completed": True}), \
                    patch.object(video, "stop_recorder", side_effect=stop_capture), \
                    patch.object(video.demo, "stop_owned_process", side_effect=lambda p: stopped.append("editor")):
                manifest = {}
                video.record(options, output, manifest)
            self.assertEqual(stopped, ["ffmpeg", "editor"])
            commands = [call.args[0] for call in popen.call_args_list]
            self.assertNotIn("--demo-autoexit", commands[0])
            self.assertIn("--demo-presentation", commands[0])
            self.assertEqual(commands[1][commands[1].index("-i") + 1], "desktop")
            self.assertEqual(commands[1][commands[1].index("-offset_x") + 1], "260")
            self.assertEqual(commands[1][commands[1].index("-video_size") + 1], "1920x1080")
            self.assertEqual(manifest["capture_start_unix_ms"], 1700000000123)

    def test_sample_protection_detects_change_without_restore(self):
        with tempfile.TemporaryDirectory() as folder:
            sample = Path(folder) / "sample.xml"
            sample.write_bytes(b"original")
            originals = {sample: video.sha256(sample)}
            video.demo.check_samples(originals)
            sample.write_bytes(b"changed")
            with self.assertRaises(video.demo.DemoError):
                video.demo.check_samples(originals)
            self.assertEqual(sample.read_bytes(), b"changed")

    def test_q_only_owned_recorder(self):
        recorder = Mock()
        recorder.poll.return_value = None
        recorder.returncode = 0
        video.stop_recorder(recorder)
        recorder.stdin.write.assert_called_once_with("q\n")
        recorder.wait.assert_called_once_with(timeout=30)
        recorder.terminate.assert_not_called()

    def test_recorder_failure_is_not_success(self):
        recorder = Mock()
        recorder.poll.return_value = 1
        recorder.returncode = 1
        with self.assertRaises(video.VideoError):
            video.stop_recorder(recorder)

    def test_failed_report_never_runs_ffmpeg(self):
        with tempfile.TemporaryDirectory() as folder, \
                patch.object(video.demo, "read_completed_report", return_value=None), \
                patch.object(video.subprocess, "run") as run:
            with self.assertRaises(video.VideoError):
                video.render(SimpleNamespace(crf=18), Path(folder), {"report_started_ns": 1}, {})
            run.assert_not_called()
            self.assertFalse((Path(folder) / video.FINAL_NAME).exists())

    def test_wrong_native_raw_never_renders_or_writes_subtitles(self):
        with tempfile.TemporaryDirectory() as folder:
            output = Path(folder)
            raw = valid_probe()
            raw["streams"][0].update(width=1456, height=939)
            with patch.object(video.demo, "read_completed_report", return_value={"completed": True}), \
                    patch.object(video, "probe", return_value=raw), \
                    patch.object(video.subprocess, "run") as run, \
                    self.assertRaises(video.VideoError):
                video.render(SimpleNamespace(crf=18), output, {"report_started_ns": 1}, {})
            run.assert_not_called()
            for name in (video.FINAL_NAME, "progress.ass", "progress.srt"):
                self.assertFalse((output / name).exists())

    def test_render_failure_cleans_partial_does_not_publish(self):
        with tempfile.TemporaryDirectory() as folder:
            output = Path(folder)
            for name in (video.RAW_NAME, video.TIMELINE_NAME, video.demo.REPORT_NAME):
                (output / name).write_bytes(b"evidence")
            rows = timeline()
            chapters = video.validate_timeline(rows)
            report = {"chapters": [{"id": c["id"], "title": c["title"]} for c in chapters]}
            manifest = {"report_started_ns": 1, "capture_start_unix_ms": chapters[0]["started_unix_ms"]}
            def fail(command, **kwargs):
                (output / "render.partial.mp4").write_bytes(b"invalid")
                raise subprocess.CalledProcessError(1, command)
            with patch.object(video.demo, "read_completed_report", return_value=report), \
                    patch.object(video, "probe", return_value=dict(valid_probe(), format={"duration": "100", "size": "1000"})), \
                    patch.object(video, "load_timeline", return_value=chapters), \
                    patch.object(video, "check_raw_dynamics", return_value={"passed": True}), \
                    patch.object(video.subprocess, "run", side_effect=fail):
                with self.assertRaises(subprocess.CalledProcessError):
                    video.render(SimpleNamespace(crf=18), output, manifest, {})
            self.assertFalse((output / "render.partial.mp4").exists())
            self.assertFalse((output / video.FINAL_NAME).exists())
            self.assertTrue((output / video.RAW_NAME).exists())

    def test_invalid_probe_never_publishes(self):
        with tempfile.TemporaryDirectory() as folder:
            output = Path(folder)
            for name in (video.RAW_NAME, video.TIMELINE_NAME, video.demo.REPORT_NAME):
                (output / name).write_bytes(b"evidence")
            chapters = video.validate_timeline(timeline())
            report = {"chapters": [{"id": c["id"], "title": c["title"]} for c in chapters]}
            manifest = {"report_started_ns": 1, "capture_start_unix_ms": chapters[0]["started_unix_ms"]}
            def fake_render(*args, **kwargs):
                (output / "render.partial.mp4").write_bytes(b"invalid")
            with patch.object(video.demo, "read_completed_report", return_value=report), \
                    patch.object(video, "probe", side_effect=[dict(valid_probe(), format={"duration": "100", "size": "1000"}), {}]), \
                    patch.object(video, "load_timeline", return_value=chapters), \
                    patch.object(video, "check_raw_dynamics", return_value={"passed": True}), \
                    patch.object(video.subprocess, "run", side_effect=fake_render):
                with self.assertRaises(video.VideoError):
                    video.render(SimpleNamespace(crf=18), output, manifest, {})
            self.assertFalse((output / video.FINAL_NAME).exists())
            self.assertFalse((output / "render.partial.mp4").exists())



class DesktopRegionTests(unittest.TestCase):
    def api(self):
        api = OwnedWindowTests().api()
        def outer(hwnd, pointer):
            pointer._obj.left = pointer._obj.top = 260
            pointer._obj.right, pointer._obj.bottom = 2180, 1340
            return True
        def screen(hwnd, pointer):
            pointer._obj.x = pointer._obj.y = 260
            return True
        api.GetWindowRect.side_effect = outer
        api.ClientToScreen.side_effect = screen
        api.IsIconic.return_value = False
        api.GetSystemMetrics.side_effect = lambda index: {76: 0, 77: 0, 78: 3840, 79: 2160}[index]
        api.WindowFromPoint.return_value = 0x2000
        api.GetAncestor.return_value = 0x1234
        return api

    def test_owned_physical_client_only_and_nine_point_occlusion_check(self):
        api = self.api()
        self.assertEqual(video.owned_capture_region(api, 0x1234, 123), (260, 260, 1920, 1080))
        self.assertEqual(api.WindowFromPoint.call_count, 9)
        self.assertEqual(api.GetAncestor.call_count, 9)
        api.SetWindowPos.assert_not_called()

    def test_minimized_not_topmost_outside_screen_or_occluded_are_rejected(self):
        minimized = self.api()
        minimized.IsIconic.return_value = True
        not_topmost = self.api()
        not_topmost.GetWindowLongW.return_value = 0
        outside = self.api()
        outside.GetSystemMetrics.side_effect = lambda index: {76: 0, 77: 0, 78: 1920, 79: 1080}[index]
        occluded = self.api()
        occluded.GetAncestor.return_value = 0x9999
        for api in (minimized, not_topmost, outside, occluded):
            with self.subTest(api=api), self.assertRaises(video.VideoError):
                video.owned_capture_region(api, 0x1234, 123)

    def test_foreign_hwnd_and_wrong_client_origin_are_rejected(self):
        api = self.api()
        api.GetWindowThreadProcessId.side_effect = lambda hwnd, pointer: setattr(pointer._obj, "value", 456)
        with self.assertRaises(video.WindowOwnershipError):
            video.owned_capture_region(api, 0x1234, 123)
        api.WindowFromPoint.assert_not_called()
        api = self.api()
        api.ClientToScreen.side_effect = lambda hwnd, pointer: True  # 原点0，不匹配外框260。
        with self.assertRaises(video.VideoError):
            video.owned_capture_region(api, 0x1234, 123)

    def test_desktop_input_is_always_cropped_before_input_not_after(self):
        command = video.capture_command(Path("D:/video"), (260, 260, 1920, 1080), 18)
        self.assertEqual(command[command.index("-i") + 1], "desktop")
        self.assertEqual(command[command.index("-offset_x") + 1], "260")
        self.assertEqual(command[command.index("-offset_y") + 1], "260")
        self.assertEqual(command[command.index("-video_size") + 1], "1920x1080")
        self.assertLess(command.index("-video_size"), command.index("-i"))
        self.assertIn("-an", command)
        self.assertNotIn("-vf", command)
        self.assertNotIn("hwnd=", " ".join(command))
        with self.assertRaises(video.VideoError):
            video.capture_command(Path("D:/video"), (0, 0, 3840, 2160), 18)


class DynamicFrameTests(unittest.TestCase):
    def clips(self):
        return [{"id": "panels", "raw_start_seconds": 0, "raw_end_seconds": 2},
                {"id": "connections", "raw_start_seconds": 2, "raw_end_seconds": 4}]

    def test_each_chapter_requires_substantial_decoded_changes(self):
        payload = b"".join(bytes([value]) * 64 for value in (0, 20, 40, 60, 80, 100, 120, 140))
        result = video.validate_dynamic_frames(payload, self.clips(), 8, 8)
        self.assertTrue(result["passed"])
        self.assertEqual([chapter["changed_pairs"] for chapter in result["chapters"]], [3, 3])

    def test_frozen_all_or_one_chapter_is_rejected(self):
        for values in ((10,) * 8, (0, 20, 40, 60, 60, 60, 60, 60)):
            with self.subTest(values=values), self.assertRaises(video.VideoError):
                video.validate_dynamic_frames(b"".join(bytes([x]) * 64 for x in values), self.clips(), 8, 8)

    def test_codec_noise_and_single_pixel_changes_do_not_mask_freeze(self):
        for payload in (bytes([0]) * 64 + bytes([7]) * 64,
                        bytes([0]) * 64 + bytes([255]) + bytes([0]) * 63):
            with self.subTest(payload=payload), self.assertRaises(video.VideoError):
                video.validate_dynamic_frames(payload, self.clips()[:1], 8, 8)

    def test_empty_truncated_missing_samples_and_no_chapters_rejected(self):
        for payload, clips in ((b"", self.clips()), (b"x", self.clips()),
                               (bytes(64), self.clips()), (bytes(128), [])):
            with self.subTest(size=len(payload)), self.assertRaises(video.VideoError):
                video.validate_dynamic_frames(payload, clips, 8, 8)

    def test_decode_checks_raw_without_progress_bar(self):
        with patch.object(video.subprocess, "run", return_value=SimpleNamespace(stdout=b"decoded")) as run, \
                patch.object(video, "validate_dynamic_frames", return_value={"passed": True}) as validate:
            video.check_raw_dynamics(Path("raw.mp4"), self.clips())
        command = run.call_args.args[0]
        self.assertIn("crop=iw:ih-120:0:0,fps=2:start_time=0,scale=128:64,format=gray", command)
        self.assertIn("rawvideo", command)
        validate.assert_called_once_with(b"decoded", self.clips())


class SoundEffectsTests(unittest.TestCase):
    def test_cli_is_silent_by_default_and_sound_is_explicit(self):
        self.assertFalse(video.parse_args([]).sound_effects)
        self.assertTrue(video.parse_args(["--sound-effects"]).sound_effects)

    def test_actions_utf8_load_validate_and_reject_invented_kinds_or_times(self):
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / video.ACTIONS_NAME
            path.write_text('{"event":"action","unix_ms":1700000000000,"kind":"left","note":"真实"}\n', encoding="utf-8")
            self.assertEqual(video.load_actions(path)[0]["kind"], "left")
            for text in ('', '{"event":"move","unix_ms":1700000000000,"kind":"left"}',
                         '{"unix_ms":1700000000000,"kind":"left"}',
                         '{"event":"action","unix_ms":true,"kind":"left"}',
                         '{"event":"action","unix_ms":1700000000000,"kind":"voice"}', '{"event":"action","unix_ms":'):
                path.write_text(text, encoding="utf-8")
                with self.subTest(text=text), self.assertRaises(video.VideoError):
                    video.load_actions(path)
            path.write_text('{"event":"action","unix_ms":2,"kind":"key"}\n{"event":"action","unix_ms":1,"kind":"key"}', encoding="utf-8")
            with self.assertRaises(video.VideoError):
                video.load_actions(path)

    def test_real_event_mapping_half_open_cuts_intro_and_gaps(self):
        origin = 1700000000000
        clips = [{"id": "panels", "raw_start_seconds": 2, "raw_end_seconds": 4,
                  "video_start_seconds": 4, "video_end_seconds": 6},
                 {"id": "connections", "raw_start_seconds": 6, "raw_end_seconds": 8,
                  "video_start_seconds": 6, "video_end_seconds": 8}]
        events = [{"event":"action","unix_ms": origin + value, "kind": "left"} for value in (1000, 2000, 3500, 4000, 5000, 6000, 8000)]
        result = video.map_actions(events, clips, origin)
        self.assertEqual([item["video_seconds"] for item in result], [4, 5.5, 6])
        self.assertEqual([item["chapter"] for item in result], ["panels", "panels", "connections"])
        with self.assertRaises(video.VideoError):
            video.map_actions(events[:1], clips, origin)

    def test_stdlib_wav_deterministic_quiet_and_exact_duration_without_intro_voice(self):
        events = [{"kind": kind, "video_seconds": 4, "clip_end_seconds": 4.04}
                  for kind in ("left", "middle", "right", "key", "scroll")]
        with tempfile.TemporaryDirectory() as folder:
            first, second = Path(folder) / "a.wav", Path(folder) / "b.wav"
            spec = video.synthesize_effects(first, events, 5, sample_rate=8000)
            video.synthesize_effects(second, events, 5, sample_rate=8000)
            self.assertEqual(first.read_bytes(), second.read_bytes())
            with wave.open(str(first), "rb") as stream:
                self.assertEqual((stream.getnchannels(), stream.getsampwidth(), stream.getframerate()), (1, 2, 8000))
                self.assertEqual(stream.getnframes(), 40000)
                pcm = array("h")
                pcm.frombytes(stream.readframes(stream.getnframes()))
            self.assertFalse(any(pcm[:32000]))
            self.assertTrue(any(pcm[32000:32320]))
            self.assertFalse(any(pcm[32320:]))  # 不越过实际保留章末边界。
            self.assertLessEqual(max(abs(sample) for sample in pcm), round(0.04 * 32767))
            self.assertFalse(spec["voice"])
            self.assertFalse(spec["external_resources"])

    def test_silent_and_aac_render_commands_are_separate(self):
        silent = video.render_command(Path("raw.mp4"), Path("output"), 18)
        audio = video.render_command(Path("raw.mp4"), Path("output"), 20, True)
        self.assertIn("-an", silent)
        self.assertNotIn("aac", silent)
        self.assertNotIn("-an", audio)
        self.assertIn("aac", audio)
        self.assertIn("1:a:0", audio)
        self.assertIn(str(Path("output") / video.SOUND_NAME), audio)
        self.assertIn("48000", audio)

    def test_aac_probe_requires_explicit_sound_mode_and_full_duration(self):
        info = valid_probe()
        audio = {"codec_type": "audio", "codec_name": "aac", "sample_rate": "48000",
                 "channels": 1, "duration": "45"}
        info["streams"].append(audio)
        self.assertTrue(video.validate_probe(info, 45, True)["audio"])
        with self.assertRaises(video.VideoError):
            video.validate_probe(info, 45)
        for field, value in (("codec_name", "mp3"), ("channels", 2), ("sample_rate", "44100"),
                             ("duration", "44"), ("duration", "nan")):
            wrong = copy.deepcopy(info)
            wrong["streams"][-1][field] = value
            with self.subTest(field=field), self.assertRaises(video.VideoError):
                video.validate_probe(wrong, 45, True)
        with self.assertRaises(video.VideoError):
            video.validate_probe(valid_probe(), 45, True)



class RenderGateTests(unittest.TestCase):
    def setup_evidence(self, output):
        chapters = video.validate_timeline(timeline())
        for name in (video.RAW_NAME, video.TIMELINE_NAME, video.demo.REPORT_NAME):
            (output / name).write_bytes(b"evidence")
        report = {"chapters": [{"id": row["id"], "title": row["title"]} for row in chapters]}
        manifest = {"report_started_ns": 1, "capture_start_unix_ms": chapters[0]["started_unix_ms"]}
        return chapters, report, manifest

    def test_frozen_raw_is_rejected_before_subtitles_or_render(self):
        with tempfile.TemporaryDirectory() as folder:
            output = Path(folder)
            chapters, report, manifest = self.setup_evidence(output)
            with patch.object(video.demo, "read_completed_report", return_value=report), \
                    patch.object(video, "probe", return_value=dict(valid_probe(), format={"duration": "100", "size": "1000"})), \
                    patch.object(video, "load_timeline", return_value=chapters), \
                    patch.object(video, "check_raw_dynamics", side_effect=video.VideoError("raw章节画面冻结")), \
                    patch.object(video.subprocess, "run") as run, \
                    self.assertRaises(video.VideoError):
                video.render(SimpleNamespace(crf=18), output, manifest, {})
            run.assert_not_called()
            for name in (video.FINAL_NAME, "progress.ass", "progress.srt"):
                self.assertFalse((output / name).exists())

    def test_missing_real_actions_cannot_generate_sound(self):
        with tempfile.TemporaryDirectory() as folder:
            output = Path(folder)
            chapters, report, manifest = self.setup_evidence(output)
            with patch.object(video.demo, "read_completed_report", return_value=report), \
                    patch.object(video, "probe", return_value=dict(valid_probe(), format={"duration": "100", "size": "1000"})), \
                    patch.object(video, "load_timeline", return_value=chapters), \
                    patch.object(video, "check_raw_dynamics", return_value={"passed": True}), \
                    patch.object(video.subprocess, "run") as run, \
                    self.assertRaises(video.VideoError):
                video.render(SimpleNamespace(crf=18, sound_effects=True), output, manifest, {})
            run.assert_not_called()
            self.assertFalse((output / video.SOUND_NAME).exists())
            self.assertFalse((output / video.FINAL_NAME).exists())

    def test_sound_pipeline_records_real_events_hash_and_validates_aac_before_publish(self):
        with tempfile.TemporaryDirectory() as folder:
            output = Path(folder)
            chapters, report, manifest = self.setup_evidence(output)
            action_ms = manifest["capture_start_unix_ms"] + 2500
            (output / video.ACTIONS_NAME).write_text(
                f'{{"event":"action","unix_ms":{action_ms},"kind":"left","chapter_id":"panels"}}\n', encoding="utf-8")
            clips, duration = video.plan_clips(chapters, manifest["capture_start_unix_ms"], 100)
            final_probe = valid_probe()
            final_probe["format"]["duration"] = str(duration)
            final_probe["streams"].append({"codec_type": "audio", "codec_name": "aac", "sample_rate": "48000",
                                          "channels": 1, "duration": str(duration)})
            def fake_render(command, **kwargs):
                self.assertIn("aac", command)
                (output / "render.partial.mp4").write_bytes(b"validated-by-mock-only")
            with patch.object(video.demo, "read_completed_report", return_value=report), \
                    patch.object(video, "probe", side_effect=[dict(valid_probe(), format={"duration": "100", "size": "1000"}), final_probe]), \
                    patch.object(video, "load_timeline", return_value=chapters), \
                    patch.object(video, "check_raw_dynamics", return_value={"passed": True}), \
                    patch.object(video.subprocess, "run", side_effect=fake_render):
                final = video.render(SimpleNamespace(crf=18, sound_effects=True), output, manifest, {})
            self.assertEqual(final, output / video.FINAL_NAME)
            self.assertTrue(final.exists())
            self.assertTrue(manifest["sound_effects"]["enabled"])
            self.assertEqual(manifest["sound_effects"]["event_count"], 1)
            self.assertEqual(manifest["sound_effects"]["events"][0]["video_seconds"], 4.5)
            self.assertEqual(manifest["sound_effects"]["actions_sha256"], video.sha256(output / video.ACTIONS_NAME))
            self.assertEqual(manifest["sound_effects"]["wav_sha256"], video.sha256(output / video.SOUND_NAME))
            self.assertTrue(manifest["specification"]["audio"])
            self.assertEqual(manifest["status"], "completed")


if __name__ == "__main__":
    unittest.main()
