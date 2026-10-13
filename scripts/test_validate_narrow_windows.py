"""窄窗验收契约测试；纯数据测试不启动 Win32 窗口。"""
import copy
import ctypes
import editor_session
import json
import struct
import tempfile
import unittest
from pathlib import Path
from unittest import mock

from validate_narrow_windows import (
    SIZES,
    TITLE_SUFFIX,
    is_editor_window,
    ownership_lost,
    record_sample,
    run_case,
    sample_owned_window,
    validate_dimensions,
)


def _fill_client(rect, client):
    rect._obj.left = 0
    rect._obj.top = 0
    rect._obj.right, rect._obj.bottom = client
    return True


def fake_user(pid=4321, title=TITLE_SUFFIX, client=(390, 844), alive=True, readable=True, visible=True):
    user = mock.Mock()
    user.IsWindow.return_value = alive
    user.GetWindowThreadProcessId.side_effect = (
        lambda hwnd, pointer: setattr(pointer._obj, "value", pid))
    user.GetWindowTextW.side_effect = (
        lambda hwnd, buffer, length: setattr(buffer, "value", title))
    user.IsWindowVisible.return_value = visible
    user.GetClientRect.side_effect = (
        (lambda hwnd, rect: _fill_client(rect, client)) if readable else (lambda hwnd, rect: False))
    return user


class NarrowWindowEvidenceTests(unittest.TestCase):
    def setUp(self):
        self.size = (390, 844)
        self.window = {"requested_size": [390, 844], "logical_size": [390.0, 844.0],
                       "physical_size": [390, 844], "scale_factor": 1.0, "stable_size_frames": 3}
        self.png = b"\x89PNG\r\n\x1a\n" + struct.pack(">I", 13) + b"IHDR" + struct.pack(">II", *self.size)

    def test_only_owned_visible_nonempty_editor_windows_match(self):
        for title in ("Difficult Rocket Editor", "Test — Difficult Rocket Editor"):
            with self.subTest(title=title):
                self.assertTrue(is_editor_window(7, 7, True, title, self.size))
        for pid, visible, title, client in (
            (8, True, "Difficult Rocket Editor", self.size),
            (7, False, "Difficult Rocket Editor", self.size),
            (7, True, "辅助窗口", self.size),
            (7, True, "Difficult Rocket Editor", (0, 844)),
            (7, True, "Difficult Rocket Editor", (390, 0)),
        ):
            with self.subTest(pid=pid, visible=visible, title=title, client=client):
                self.assertFalse(is_editor_window(pid, 7, visible, title, client))

    def test_three_matching_dimensions_pass(self):
        self.assertEqual(validate_dimensions(self.size, [[390, 844]], self.window, self.png), self.size)

    def test_missing_or_wrong_client_fails(self):
        for clients in ([], [[1440, 900]], [[390, 844], [406, 860]]):
            with self.subTest(clients=clients), self.assertRaises(ValueError):
                validate_dimensions(self.size, clients, self.window, self.png)

    def test_each_bevy_dimension_must_match(self):
        for key in ("requested_size", "logical_size", "physical_size"):
            window = copy.deepcopy(self.window)
            window[key] = [1440, 900]
            with self.subTest(key=key), self.assertRaises(ValueError):
                validate_dimensions(self.size, [[390, 844]], window, self.png)

    def test_dpi_and_stability_must_match(self):
        for key, value in (("scale_factor", 2.0), ("stable_size_frames", 1)):
            window = copy.deepcopy(self.window)
            window[key] = value
            with self.subTest(key=key), self.assertRaises(ValueError):
                validate_dimensions(self.size, [[390, 844]], window, self.png)

    def test_original_png_cannot_be_desktop_sized(self):
        png = self.png[:16] + struct.pack(">II", 1440, 900)
        with self.assertRaises(ValueError):
            validate_dimensions(self.size, [[390, 844]], self.window, png)

    def test_invalid_or_truncated_png_fails(self):
        for png in (b"", self.png[:23], b"not-a-png" + self.png[9:], self.png[:12] + b"NOPE" + self.png[16:]):
            with self.subTest(png=png), self.assertRaises(ValueError):
                validate_dimensions(self.size, [[390, 844]], self.window, png)


class OwnedWindowSamplingTests(unittest.TestCase):
    def test_sample_reports_relative_time_pid_hwnd_title_and_size(self):
        sample, error = sample_owned_window(fake_user(pid=4321, client=(640, 360)), 0x1234, 4321, 1.25)
        self.assertIsNone(error)
        self.assertEqual(sample["elapsed"], 1.25)
        self.assertEqual(sample["pid"], 4321)
        self.assertEqual(sample["hwnd"], 0x1234)
        self.assertEqual(sample["title"], TITLE_SUFFIX)
        self.assertTrue(sample["visible"])
        self.assertEqual(sample["client"], [640, 360])

    def test_dead_handle_fails_before_reading_client(self):
        user = fake_user(alive=False)
        sample, error = sample_owned_window(user, 0x1234, 4321, 2.0)
        self.assertIsNone(sample)
        self.assertIn("HWND", error)
        user.GetClientRect.assert_not_called()

    def test_changed_pid_or_title_is_ownership_lost(self):
        for user, expected in ((fake_user(pid=9999), "9999"), (fake_user(title="别的窗口"), "标题")):
            with self.subTest(expected=expected):
                sample, error = sample_owned_window(user, 0x1234, 4321, 3.0)
                self.assertIsNone(sample)
                self.assertIn(expected, error)
                user.GetClientRect.assert_not_called()

    def test_unreadable_client_area_is_ownership_lost(self):
        user = fake_user(readable=False)
        sample, error = sample_owned_window(user, 0x1234, 4321, 3.5)
        self.assertIsNone(sample)
        self.assertIn("客户区", error)

    def test_sampling_only_reads_window_state(self):
        user = fake_user()
        sample_owned_window(user, 0x1234, 4321, 0.5)
        called = {name for name, _, _ in user.mock_calls}
        self.assertEqual(called, {"IsWindow", "GetWindowThreadProcessId", "GetWindowTextW",
                                  "IsWindowVisible", "GetClientRect"})

    def test_record_sample_keeps_observations_and_details_aligned(self):
        result = {"client_observations": [], "client_samples": []}
        first = {"elapsed": 0.1, "pid": 1, "hwnd": 2, "title": TITLE_SUFFIX,
                 "visible": True, "client": [640, 360]}
        self.assertTrue(record_sample(result, first))
        self.assertFalse(record_sample(result, dict(first, elapsed=0.2)))
        self.assertEqual(result["client_observations"], [[640, 360]])
        self.assertEqual(len(result["client_samples"]), 1)
        self.assertTrue(record_sample(result, dict(first, elapsed=0.3, client=[390, 844])))
        self.assertEqual(result["client_observations"], [[640, 360], [390, 844]])
        self.assertEqual([sample["elapsed"] for sample in result["client_samples"]], [0.1, 0.3])

    def test_ownership_lost_keeps_observations_and_raises(self):
        result = {"client_observations": [[640, 360]],
                  "client_samples": [{"elapsed": 0.5, "pid": 4321, "hwnd": 0x1234,
                                      "title": TITLE_SUFFIX, "visible": True, "client": [640, 360]}]}
        with self.assertRaises(RuntimeError) as caught:
            ownership_lost(result, "PID 已变为 9999，本次进程为 4321", 0x1234, 1.75)
        self.assertIn("所有权丢失", str(caught.exception))
        self.assertIn("9999", str(caught.exception))
        self.assertEqual(result["client_observations"], [[640, 360]])
        self.assertEqual(result["ownership_lost"]["detail"], "PID 已变为 9999，本次进程为 4321")
        self.assertEqual(result["ownership_lost"]["elapsed"], 1.75)
        self.assertEqual(result["ownership_lost"]["hwnd"], 0x1234)
        self.assertEqual(result["ownership_lost"]["last_sample"]["client"], [640, 360])

    def test_ownership_lost_without_samples_still_reports(self):
        result = {"client_observations": [], "client_samples": []}
        with self.assertRaises(RuntimeError):
            ownership_lost(result, "HWND 已失效", 0x2222, 0.25)
        self.assertIsNone(result["ownership_lost"]["last_sample"])
        self.assertEqual(result["ownership_lost"]["elapsed"], 0.25)


class RunCaseExitHandlingTests(unittest.TestCase):
    """进程正常退出时窗口先销毁不算污染；真正换窗口仍立即失败。"""

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.size = (640, 360)
        self.editor = self.root / "dr-editor.exe"
        self.editor.write_bytes(b"")
        self.assets = self.root / "assets"
        self.output = self.root / "out"
        self.output.mkdir()
        self.case = self.output / "640x360"

    @staticmethod
    def png(size):
        return b"\x89PNG\r\n\x1a\n" + struct.pack(">I", 13) + b"IHDR" + struct.pack(">II", *size)

    def preset(self, window_size=None, size=None):
        size = size or self.size
        case = self.output / f"{size[0]}x{size[1]}"
        window_size = window_size or size
        window = {"requested_size": [window_size[0], window_size[1]],
                  "logical_size": [float(window_size[0]), float(window_size[1])],
                  "physical_size": [window_size[0], window_size[1]],
                  "scale_factor": 1.0, "stable_size_frames": 3}
        target = case / "target"
        target.mkdir(parents=True, exist_ok=True)
        (target / "editor-smoke-window.json").write_text(json.dumps(window), encoding="utf-8")
        (target / "editor-smoke.png").write_bytes(self.png(size))
        return window

    @staticmethod
    def process(polls, exit_code=0):
        values = list(polls)
        process = mock.Mock()
        process.pid = 4242
        process.poll.side_effect = lambda: values.pop(0) if len(values) > 1 else values[0]
        process.wait.return_value = exit_code
        return process

    def editor_user(self, alive, pids=(4242,), titles=None, client=None):
        user = mock.Mock()
        alive_values = list(alive)
        user.IsWindow.side_effect = (
            lambda hwnd: alive_values.pop(0) if len(alive_values) > 1 else alive_values[0])
        pid_values = list(pids)
        user.GetWindowThreadProcessId.side_effect = (
            lambda hwnd, pointer: setattr(pointer._obj, "value",
                                          pid_values.pop(0) if len(pid_values) > 1 else pid_values[0]))
        title_values = list(titles or [TITLE_SUFFIX])
        user.GetWindowTextW.side_effect = (
            lambda hwnd, buffer, length: setattr(buffer, "value",
                                                 title_values.pop(0) if len(title_values) > 1 else title_values[0]))
        user.IsWindowVisible.return_value = True
        user.GetClientRect.side_effect = lambda hwnd, rect: _fill_client(rect, client or self.size)
        user.EnumWindows.side_effect = lambda callback, lparam: callback(0x1234, 0)
        return user

    def run_script(self, process, user):
        with mock.patch("validate_narrow_windows.subprocess.Popen", return_value=process):
            return run_case(self.editor, self.assets, self.output, self.size, user, lambda function: function)

    def test_window_destroyed_during_normal_exit_is_not_pollution(self):
        self.preset()
        user = self.editor_user(alive=(True, False), pids=(4242, 4242))
        result = self.run_script(self.process((None, None, 0)), user)
        self.assertNotIn("ownership_lost", result)
        self.assertEqual(result["exit_code"], 0)
        self.assertTrue(result["passed"])
        self.assertEqual(result["client_observations"], [[640, 360]])
        self.assertEqual(result["bevy_window"]["physical_size"], [640, 360])

    def test_bounded_exit_confirmation_accepts_pending_process_state(self):
        user = self.editor_user(alive=(False,))
        result = self.run_script(self.process((None,)), user)
        self.assertNotIn("ownership_lost", result)
        self.assertEqual(result["exit_code"], 0)
        self.assertFalse(result["passed"])

    def test_real_ownership_change_still_fails_immediately(self):
        user = self.editor_user(alive=(True,), pids=(4242, 9999))
        process = self.process((None,))
        result = self.run_script(process, user)
        self.assertIn("所有权丢失", result["error"])
        self.assertIn("9999", result["ownership_lost"]["detail"])
        self.assertNotIn("exit_code", result)
        self.assertFalse(result["passed"])
        process.terminate.assert_called_once()

    def test_title_change_still_fails_immediately(self):
        user = self.editor_user(alive=(True,), pids=(4242, 4242), titles=(TITLE_SUFFIX, "别的进程窗口"))
        result = self.run_script(self.process((None,)), user)
        self.assertIn("所有权丢失", result["error"])
        self.assertIn("标题", result["ownership_lost"]["detail"])

    def test_popen_launches_the_unique_copy_and_records_its_identity(self):
        self.editor.write_bytes(b"MZ unique copy source")
        self.preset()
        user = self.editor_user(alive=(True, False), pids=(4242, 4242))
        process = self.process((None, None, 0))
        with mock.patch("validate_narrow_windows.subprocess.Popen", return_value=process) as popen:
            result = run_case(self.editor, self.assets, self.output, self.size, user, lambda function: function)
        command = popen.call_args.args[0]
        identity = result["identity"]
        self.assertEqual(command, result["command"])
        self.assertEqual(command[0], identity["copy_path"])
        copy_path = Path(command[0])
        self.assertEqual(copy_path.parent, self.case.resolve())
        self.assertEqual(copy_path.suffix, self.editor.suffix)
        self.assertNotEqual(copy_path, self.editor.resolve())
        self.assertEqual(copy_path.read_bytes(), b"MZ unique copy source")
        self.assertEqual(self.editor.read_bytes(), b"MZ unique copy source")
        self.assertEqual(identity["source_path"], str(self.editor.resolve()))
        self.assertEqual(identity["source_sha256"], identity["copy_sha256"])
        self.assertEqual(command[1:], ["--assets", str(self.assets),
                                       "--catalog", str(self.assets / "builtin/PartList.xml"),
                                       "--ship", str(self.assets / "ships/Test.xml"),
                                       "--smoke-test", "--smoke-window-size", "640x360"])
        self.assertEqual(popen.call_args.kwargs["cwd"], self.case)
        session = json.loads((self.case / editor_session.SESSION_NAME).read_text(encoding="utf-8"))
        self.assertEqual(session, identity)
        self.assertTrue(result["passed"])

    def test_each_case_launches_a_process_name_of_its_own(self):
        """三个样例各用自己的进程名，避免桌面工具跨样例沿用上一次窗口尺寸。"""
        names = []
        for size in SIZES:
            self.preset(size=size)
            user = self.editor_user(alive=(True, False), pids=(4242, 4242), client=size)
            process = self.process((None, None, 0))
            with mock.patch("validate_narrow_windows.subprocess.Popen", return_value=process) as popen:
                result = run_case(self.editor, self.assets, self.output, size, user, lambda function: function)
            self.assertTrue(result["passed"])
            names.append(Path(popen.call_args.args[0][0]).name)
        self.assertEqual(len(set(names)), len(SIZES))

    def test_staging_reuses_owned_window_checks_and_requires_the_long_list_preview(self):
        for valid in (True, False):
            with self.subTest(valid=valid):
                self.preset()
                target = self.case / "target"
                (target / "editor-staging-draft.png").write_bytes(self.png(self.size))
                staging = {"stages": 64, "activations": 1024, "edited_stage": 63, "edited_activation": 15}
                if valid:
                    staging["preview"] = {"panel_rect": [0, 198, 320, 360],
                                          "canvas_rect": [320, 198, 640, 360],
                                          "target_rect": [400, 240, 420, 260],
                                          "target_group": 0, "target_id": 17,
                                          "panel_overlaps_canvas": False, "edited_target_visible": True,
                                          "hover_target_visible": True, "hover_restored": True,
                                          "hover_target_rect": [400, 240, 420, 260]}
                (target / "editor-staging-performance.json").write_text(json.dumps(staging), encoding="utf-8")
                user = self.editor_user(alive=(True, False), pids=(4242, 4242))
                with mock.patch("validate_narrow_windows.subprocess.Popen",
                                return_value=self.process((None, None, 0))) as popen:
                    result = run_case(self.editor, self.assets, self.output, self.size,
                                      user, lambda function: function, staging=True)
                command = popen.call_args.args[0]
                self.assertIn("--staging-smoke-test", command)
                self.assertIn("--smoke-test", command)
                self.assertNotIn("--ship", command, "分级必须使用真实 64 级夹具，而非 Test.xml 的短列表")
                self.assertEqual(result["passed"], valid)
                self.assertEqual(result["client_observations"], [[640, 360]])
                if valid:
                    self.assertEqual(result["staging"], staging)
                    self.assertEqual(Path(result["screenshot"]).name, "editor-staging-draft.png")
                else:
                    self.assertIn("停靠布局", result["error"])

    def test_hash_mismatch_stops_before_launching(self):
        self.preset()
        user = self.editor_user(alive=(True, False), pids=(4242, 4242))
        with mock.patch.object(editor_session, "file_sha256", side_effect=["0" * 64, "1" * 64]), \
                mock.patch("validate_narrow_windows.subprocess.Popen") as popen:
            result = run_case(self.editor, self.assets, self.output, self.size, user, lambda function: function)
        popen.assert_not_called()
        self.assertFalse(result["passed"])
        self.assertIsNone(result["identity"])
        self.assertIn("SHA256", result["error"])

    def test_failed_case_still_keeps_editor_side_window_json(self):
        self.preset(window_size=(1440, 900))
        user = self.editor_user(alive=(True,), pids=(4242, 4242))
        result = self.run_script(self.process((None, 0)), user)
        self.assertFalse(result["passed"])
        self.assertIn("requested_size", result["error"])
        self.assertEqual(result["client_observations"], [[640, 360]])
        self.assertEqual(result["bevy_window"]["physical_size"], [1440, 900])


if __name__ == "__main__":
    unittest.main()
