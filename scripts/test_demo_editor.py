"""演示驱动单元测试：仅临时文件与 subprocess mock，无 GUI、键鼠、通知或构建副作用。"""
from contextlib import redirect_stderr, redirect_stdout
import copy
import importlib.util
import io
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import Mock, patch


SPEC = importlib.util.spec_from_file_location("demo_editor", Path(__file__).with_name("demo_editor.py"))
demo = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(demo)


def complete_report():
    return {
        "completed": True,
        "chapters": [
            {"id": chapter_id, "title": "演示章节", "status": "passed",
             "elapsed_seconds": 0.45, "artifacts": [chapter_id + ".png"]}
            for chapter_id in demo.CHAPTER_IDS
        ],
    }


class DemoTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.base = Path(self.temporary.name)
        self.root = self.base / "dr-rs"
        self.root.mkdir()
        self.output = self.root / "output"
        self.output.mkdir()
        self.binary = self.root / "target/debug/dr-editor.exe"
        self.binary.parent.mkdir(parents=True)
        self.binary.write_bytes(b"mock exe")
        self.samples = self.base / "Difficult-Rocket/assets/ships"
        self.samples.mkdir(parents=True)
        for name in ("Test.xml", "Heronb.xml"):
            (self.samples / name).write_text("<Ship/>", encoding="utf-8")
        self.started_ns = 2_000_000_000
        self.report = complete_report()
        self.write_evidence(self.report)
        self.process = Mock()
        self.process.poll.return_value = None
        self.process.wait.return_value = 0
        self.silence = redirect_stdout(io.StringIO())
        self.silence.__enter__()
        self.addCleanup(self.silence.__exit__, None, None, None)

    def write_evidence(self, report, fresh=True):
        stamp = self.started_ns + 1_000_000 if fresh else self.started_ns - 1
        for chapter in report["chapters"]:
            for name in chapter["artifacts"]:
                artifact = self.output / name
                artifact.write_bytes(b"mock screenshot evidence")
                os.utime(artifact, ns=(stamp, stamp))
        path = self.output / demo.REPORT_NAME
        path.write_text(json.dumps(report), encoding="utf-8")
        os.utime(path, ns=(stamp, stamp))

    def options(self, *args):
        return demo.parse_args(["--no-build", "--output", str(self.output), *args])

    def invoke(self, options=None, launch=None, monitor=None):
        def default_launch(*args, **kwargs):
            if launch is not None:
                launch(*args, **kwargs)
            return self.process

        with patch.object(demo.time, "time_ns", return_value=self.started_ns), \
                patch.object(demo.subprocess, "Popen", side_effect=default_launch) as popen, \
                patch.object(demo.subprocess, "run") as build:
            if monitor is None:
                result = demo.run_demo(options or self.options(), root=self.root)
            else:
                with patch.object(demo, "wait_for_demo", side_effect=monitor):
                    result = demo.run_demo(options or self.options(), root=self.root)
        build.assert_not_called()
        return result, popen

    def test_cli_defaults(self):
        options = demo.parse_args([])
        self.assertEqual(options.step_ms, 450)
        self.assertEqual(options.timeout, 600)
        self.assertEqual(options.mode, "brief")
        self.assertFalse(options.exit_on_complete)
        self.assertFalse(options.no_build)
        self.assertIsNone(options.output)

    def test_fast_always_forces_40ms_and_exit(self):
        for args in (["--fast"], ["--fast", "--step-ms", "900"], ["--step-ms", "1", "--fast"]):
            options = demo.parse_args(args)
            self.assertEqual(options.step_ms, 40)
            self.assertTrue(options.exit_on_complete)

    def test_reject_nonpositive_and_nonfinite_options(self):
        for option, value in (("--step-ms", "0"), ("--step-ms", "-1"), ("--step-ms", "1.5"),
                              ("--timeout", "0"), ("--timeout", "nan"), ("--timeout", "inf")):
            with self.subTest(option=option, value=value), redirect_stderr(io.StringIO()):
                with self.assertRaises(SystemExit):
                    demo.parse_args([option, value])

    def test_complete_report_and_live_window_are_success_without_termination(self):
        original = demo.sample_hashes(self.root)
        result, popen = self.invoke()
        self.assertEqual(result, self.report)
        command = popen.call_args.args[0]
        self.assertEqual(command, [str(self.binary.resolve()), "--demo-showcase",
                                   str(self.output.resolve()), "--demo-mode", "brief", "--demo-step-ms", "450"])
        self.assertEqual(popen.call_args.kwargs["cwd"], self.root.resolve())
        self.assertIs(popen.call_args.kwargs["stdout"], popen.call_args.kwargs["stderr"])
        self.process.terminate.assert_not_called()
        self.process.kill.assert_not_called()
        self.process.wait.assert_not_called()
        self.assertEqual(demo.sample_hashes(self.root), original)

    def test_fast_contract_waits_for_zero_exit(self):
        _, popen = self.invoke(self.options("--fast"))
        command = popen.call_args.args[0]
        self.assertEqual(command[-3:], ["--demo-step-ms", "40", "--demo-exit-on-complete"])
        self.process.wait.assert_called_once()
        self.process.terminate.assert_not_called()

    def test_detailed_has_no_implicit_timeout_and_fast_does_not_change_content_mode(self):
        options = self.options("--mode", "detailed", "--fast")
        self.assertIsNone(options.timeout)
        self.assertEqual(options.mode, "detailed")
        report = complete_report()
        report["mode"] = "detailed"
        self.write_evidence(report)
        result, popen = self.invoke(options)
        self.assertEqual(result, report)
        command = popen.call_args.args[0]
        self.assertEqual(command[command.index("--demo-mode") + 1], "detailed")
        self.process.wait.assert_called_once_with(timeout=None)
        self.assertEqual(self.options("--mode", "detailed", "--timeout", "7200").timeout, 7200)

    def test_report_mode_and_chapter_order_must_match_request(self):
        for mode in ("unknown", "detailed", []):
            report = copy.deepcopy(self.report)
            report["mode"] = mode
            with self.subTest(mode=mode), self.assertRaises(demo.DemoError):
                demo.validate_report(report, self.output, self.started_ns, "brief")
        report = copy.deepcopy(self.report)
        report["chapters"][0], report["chapters"][1] = report["chapters"][1], report["chapters"][0]
        with self.assertRaisesRegex(demo.DemoError, "顺序"):
            demo.validate_report(report, self.output, self.started_ns)

    def test_unlimited_wait_still_rejects_crashes_and_early_closes(self):
        for code in (0, 101):
            self.process.poll.return_value = code
            with patch.object(demo, "read_completed_report", return_value=None), self.subTest(code=code):
                with self.assertRaises(demo.DemoError):
                    demo.wait_for_demo(self.process, self.output, self.started_ns, None, True, "detailed")

    def test_exit_mode_nonzero_after_report_fails_and_cleans_owned_process(self):
        self.process.wait.return_value = 101
        with self.assertRaisesRegex(demo.DemoError, "异常退出"):
            self.invoke(self.options("--exit-on-complete"))
        self.process.terminate.assert_called_once()

    def test_default_output_is_absolute_timestamp_directory(self):
        with patch.object(demo.subprocess, "Popen", return_value=self.process) as popen, \
                patch.object(demo, "wait_for_demo", return_value=self.report):
            demo.run_demo(demo.parse_args(["--no-build"]), root=self.root)
        output = Path(popen.call_args.args[0][2])
        self.assertTrue(output.is_absolute())
        self.assertEqual(output.parent, self.root / "target")
        self.assertTrue(output.name.startswith("demo-"))
        self.assertTrue((output / "demo-editor.log").is_file())

    def test_output_cannot_contain_protected_original_samples(self):
        for directory in (self.samples, self.samples.parent, self.base):
            options = demo.parse_args(["--no-build", "--output", str(directory)])
            with self.subTest(directory=directory), patch.object(demo.subprocess, "Popen") as popen:
                with self.assertRaisesRegex(demo.DemoError, "受保护的原版样本"):
                    demo.run_demo(options, root=self.root)
                popen.assert_not_called()
        self.assertEqual((self.samples / "Test.xml").read_text(encoding="utf-8"), "<Ship/>")

    def test_valid_schema_can_have_empty_artifacts_without_claiming_screenshot(self):
        report = copy.deepcopy(self.report)
        report["chapters"][0]["artifacts"] = []
        self.assertEqual(demo.validate_report(report, self.output, self.started_ns), report)

    def test_reject_missing_duplicate_unknown_and_malformed_chapters(self):
        variants = []
        report = copy.deepcopy(self.report)
        report["chapters"].pop()
        variants.append(report)
        for value in ("panels", "unknown", ["panels"]):
            report = copy.deepcopy(self.report)
            report["chapters"][1]["id"] = value
            variants.append(report)
        report = copy.deepcopy(self.report)
        report["chapters"][0] = None
        variants.append(report)
        for report in variants:
            with self.subTest(report=report), self.assertRaises(demo.DemoError):
                demo.validate_report(report, self.output, self.started_ns)

    def test_reject_bad_completion_title_status_duration_and_artifacts_type(self):
        for key, value in (("title", " "), ("title", 1), ("status", "failed"),
                           ("elapsed_seconds", -1), ("elapsed_seconds", True),
                           ("elapsed_seconds", float("nan")), ("elapsed_seconds", float("inf")),
                           ("elapsed_seconds", "1"), ("artifacts", None)):
            report = copy.deepcopy(self.report)
            report["chapters"][0][key] = value
            with self.subTest(key=key, value=value), self.assertRaises(demo.DemoError):
                demo.validate_report(report, self.output, self.started_ns)
        for value in (False, 1, "true", None):
            report = copy.deepcopy(self.report)
            report["completed"] = value
            with self.subTest(completed=value), self.assertRaises(demo.DemoError):
                demo.validate_report(report, self.output, self.started_ns)

    def test_reject_missing_empty_and_old_artifacts(self):
        path = self.output / "panels.png"
        path.unlink()
        with self.assertRaisesRegex(demo.DemoError, "不存在"):
            demo.validate_report(self.report, self.output, self.started_ns)
        path.write_bytes(b"")
        with self.assertRaisesRegex(demo.DemoError, "为空"):
            demo.validate_report(self.report, self.output, self.started_ns)
        path.write_bytes(b"old")
        os.utime(path, ns=(self.started_ns - 1, self.started_ns - 1))
        with self.assertRaisesRegex(demo.DemoError, "本次新产物"):
            demo.validate_report(self.report, self.output, self.started_ns)

    def test_artifact_paths_cannot_escape_output(self):
        names = ["../outside.png", "..\\outside.png", "/absolute.png", "C:\\outside.png",
                 "C:outside.png", "\\\\server\\share\\outside.png", "", 5]
        for name in names:
            report = copy.deepcopy(self.report)
            report["chapters"][0]["artifacts"] = [name]
            with self.subTest(name=name), self.assertRaises(demo.DemoError):
                demo.validate_report(report, self.output, self.started_ns)

    def test_nested_artifacts_and_windows_separator_are_supported(self):
        nested = self.output / "shots"
        nested.mkdir()
        shot = nested / "panels.png"
        shot.write_bytes(b"evidence")
        report = copy.deepcopy(self.report)
        report["chapters"][0]["artifacts"] = ["shots\\panels.png"]
        demo.validate_report(report, self.output, self.started_ns)

    def test_old_report_is_not_accepted(self):
        self.write_evidence(self.report, fresh=False)
        self.assertIsNone(demo.read_completed_report(self.output, self.started_ns))

    def test_partial_report_and_incomplete_report_remain_pending(self):
        path = self.output / demo.REPORT_NAME
        for content in ("{", '{"completed": false}'):
            path.write_text(content, encoding="utf-8")
            self.assertIsNone(demo.read_completed_report(self.output, self.started_ns))

    def test_zero_exit_without_complete_report_is_failure(self):
        self.process.poll.return_value = 0
        self.write_evidence(self.report, fresh=False)
        with self.assertRaisesRegex(demo.DemoError, "提前退出"):
            self.invoke()
        self.process.terminate.assert_not_called()

    def test_zero_exit_with_fresh_complete_report_is_success(self):
        self.process.poll.return_value = 0
        self.invoke()
        self.process.terminate.assert_not_called()

    def test_nonzero_exit_is_failure_even_with_complete_report(self):
        self.process.poll.return_value = 101
        with self.assertRaisesRegex(demo.DemoError, "101"):
            self.invoke()
        self.process.terminate.assert_not_called()

    def test_timeout_cleans_only_owned_process(self):
        self.write_evidence(self.report, fresh=False)
        with patch.object(demo.time, "monotonic", side_effect=[0, 601]), \
                patch.object(demo.time, "sleep") as sleep:
            with self.assertRaisesRegex(demo.DemoError, "超时"):
                self.invoke()
        self.process.terminate.assert_called_once()
        self.process.wait.assert_called_once_with(timeout=5)
        sleep.assert_not_called()

    def test_pending_report_can_complete_on_later_poll(self):
        with patch.object(demo, "read_completed_report", side_effect=[None, self.report]), \
                patch.object(demo.time, "sleep") as sleep:
            self.invoke()
        sleep.assert_called_once_with(0.1)
        self.process.terminate.assert_not_called()

    def test_exit_mode_timeout_cleans_owned_process(self):
        self.process.wait.side_effect = [subprocess.TimeoutExpired("editor", 600), 0]
        with self.assertRaisesRegex(demo.DemoError, "自动退出超时"):
            self.invoke(self.options("--exit-on-complete"))
        self.process.terminate.assert_called_once()

    def test_cleanup_escalates_only_owned_process_when_terminate_times_out(self):
        self.process.wait.side_effect = [subprocess.TimeoutExpired("editor", 5), 0]
        demo.stop_owned_process(self.process)
        self.process.terminate.assert_called_once()
        self.process.kill.assert_called_once()
        self.assertEqual(self.process.wait.call_count, 2)

    def test_sample_change_fails_without_restoring_original(self):
        path = self.samples / "Heronb.xml"
        def launch(*args, **kwargs):
            path.write_text("changed", encoding="utf-8")
        with self.assertRaisesRegex(demo.DemoError, "原版样本被修改"):
            self.invoke(launch=launch)
        self.process.terminate.assert_called_once()
        self.assertEqual(path.read_text(encoding="utf-8"), "changed")

    def test_sample_missing_is_detected_after_termination(self):
        path = self.samples / "Test.xml"
        self.process.terminate.side_effect = path.unlink
        with self.assertRaisesRegex(demo.DemoError, "原版样本被修改或丢失"):
            self.invoke(monitor=demo.DemoError("演示失败"))
        self.process.terminate.assert_called_once()
        self.assertFalse(path.exists())

    def test_keyboard_interrupt_cleans_owned_process(self):
        with self.assertRaises(KeyboardInterrupt):
            self.invoke(monitor=KeyboardInterrupt())
        self.process.terminate.assert_called_once()

    def test_popen_failure_does_not_touch_an_unowned_process(self):
        with patch.object(demo.subprocess, "Popen", side_effect=OSError("启动失败")), \
                patch.object(demo, "stop_owned_process") as stop:
            with self.assertRaises(OSError):
                demo.run_demo(self.options(), root=self.root)
        stop.assert_called_once_with(None)

    def test_needs_build_for_missing_binary_or_updated_sources_and_vendor(self):
        self.assertFalse(demo.needs_build(self.root, self.binary))
        built_ns = 3_000_000_000
        os.utime(self.binary, ns=(built_ns, built_ns))
        for relative in ("Cargo.toml", "Cargo.lock", "crates/dr-editor/src/main.rs",
                         "vendor/example/src/lib.rs", ".cargo/config.toml"):
            path = self.root / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text("source", encoding="utf-8")
            os.utime(path, ns=(built_ns - 1, built_ns - 1))
        self.assertFalse(demo.needs_build(self.root, self.binary))
        for relative in ("Cargo.toml", "Cargo.lock", "crates/dr-editor/src/main.rs",
                         "vendor/example/src/lib.rs", ".cargo/config.toml"):
            path = self.root / relative
            os.utime(path, ns=(built_ns + 1_000_000, built_ns + 1_000_000))
            self.assertTrue(demo.needs_build(self.root, self.binary))
            os.utime(path, ns=(built_ns - 1, built_ns - 1))
        self.binary.unlink()
        self.assertTrue(demo.needs_build(self.root, self.binary))

    def test_default_binary_builds_when_outdated(self):
        with patch.object(demo, "needs_build", return_value=True), \
                patch.object(demo.subprocess, "run") as build:
            self.assertEqual(demo.prepare_binary(demo.parse_args([]), self.root), self.binary.resolve())
        build.assert_called_once_with(["cargo", "build", "-p", "dr-editor"], cwd=self.root, check=True)

    def test_no_build_and_explicit_custom_binary_never_build(self):
        custom = self.root / "custom.exe"
        custom.write_bytes(b"custom")
        for options in (demo.parse_args(["--no-build"]), demo.parse_args(["--editor-bin", str(custom)])):
            with patch.object(demo.subprocess, "run") as build, \
                    patch.object(demo, "needs_build") as stale:
                demo.prepare_binary(options, self.root)
            build.assert_not_called()
            stale.assert_not_called()

    def test_missing_custom_binary_and_no_build_missing_default_fail(self):
        with patch.object(demo.subprocess, "run") as build:
            with self.assertRaisesRegex(demo.DemoError, "编辑器不存在"):
                demo.prepare_binary(demo.parse_args(["--editor-bin", str(self.root / "missing.exe")]), self.root)
            self.binary.unlink()
            with self.assertRaisesRegex(demo.DemoError, "编辑器不存在"):
                demo.prepare_binary(self.options(), self.root)
        build.assert_not_called()

    def test_build_failure_propagates_without_launching_gui(self):
        with patch.object(demo, "needs_build", return_value=True), \
                patch.object(demo.subprocess, "run", side_effect=subprocess.CalledProcessError(1, "cargo")), \
                patch.object(demo.subprocess, "Popen") as popen:
            with self.assertRaises(subprocess.CalledProcessError):
                demo.run_demo(demo.parse_args([]), root=self.root)
        popen.assert_not_called()

    def test_main_exit_codes(self):
        for error, expected in ((None, 0), (demo.DemoError("证据无效"), 1),
                                (OSError("启动失败"), 1), (KeyboardInterrupt(), 130)):
            with patch.object(demo, "run_demo", side_effect=error), redirect_stderr(io.StringIO()):
                self.assertEqual(demo.main([]), expected)


if __name__ == "__main__":
    unittest.main()
