"""前台驱动的重试/提醒及 CLI 映射回归，不操作桌面、不发送通知。"""
import ast
from contextlib import redirect_stdout
import io
from pathlib import Path
import re
import sys
import tempfile
import time
import os
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch

SOURCE = Path(__file__).with_name("check_native_input.py")
TREE = ast.parse(SOURCE.read_text(encoding="utf-8"))
GUARD = next(node for node in TREE.body if isinstance(node, ast.If) and "__name__" in ast.unparse(node.test))
CODE = compile(ast.Module(body=[GUARD], type_ignores=[]), str(SOURCE), "exec")


class DriverTests(unittest.TestCase):
    def run_wrapper(self, effects):
        main = Mock(side_effect=effects)
        report = Mock()
        scope = {"__name__": "__main__", "main": main, "Path": Path, "sys": sys,
                 "subprocess": SimpleNamespace(run=report, CREATE_NO_WINDOW=0, CalledProcessError=RuntimeError),
                 "time": SimpleNamespace(sleep=Mock())}
        with patch.object(Path, "is_file", return_value=True), redirect_stdout(io.StringIO()):
            try:
                exec(CODE, scope)
            except AssertionError:
                pass
        return main, report, scope["time"].sleep

    def test_three_focus_failures_notify_once(self):
        main, report, sleep = self.run_wrapper([AssertionError("系统拒绝将测试窗口置前")] * 3)
        self.assertEqual(main.call_count, 3)
        self.assertEqual(report.call_count, 1)
        arguments = report.call_args.args[0]
        self.assertEqual(arguments[arguments.index("--room") + 1], "sr1")
        self.assertEqual(sleep.call_count, 2)
        arguments = report.call_args.args[0]
        self.assertIn("noticer-progress", arguments[5])
        self.assertIn("--message", arguments)

    def test_program_failure_is_not_retried_or_reported_as_focus_failure(self):
        main, report, sleep = self.run_wrapper([AssertionError("编辑器运行失败：退出码 101")])
        self.assertEqual(main.call_count, 1)
        report.assert_not_called()
        sleep.assert_not_called()

    def test_focus_recovers_before_the_third_attempt_without_a_warning(self):
        main, report, sleep = self.run_wrapper([AssertionError("已失去真实前台"), AssertionError("已失去真实前台"), None])
        self.assertEqual(main.call_count, 3)
        report.assert_not_called()
        self.assertEqual(sleep.call_count, 2)

    def test_window_cases_map_to_flags_supported_by_the_editor(self):
        mapping = next(ast.literal_eval(node) for node in ast.walk(TREE)
                       if isinstance(node, ast.Dict)
                       and any(isinstance(value, ast.Constant) and value.value == "panel-smoke-test"
                               for value in node.values))
        choices = next(ast.literal_eval(keyword.value) for node in ast.walk(TREE)
                       if isinstance(node, ast.Call) and node.args and isinstance(node.args[0], ast.Constant)
                       and node.args[0].value == "--window-case" for keyword in node.keywords if keyword.arg == "choices")
        editor = (SOURCE.parents[1] / "crates/dr-editor/src/main.rs").read_text(encoding="utf-8")
        supported = set(re.findall(r'"(--[a-z-]+test)"', editor))
        for case in choices:
            if case == "keys":
                self.assertIn("--native-keys-test", supported)
                continue
            if case not in ["native", "performance"]:
                flag = "--" + mapping.get(case, case + "-smoke-test")
                self.assertIn(flag, supported, f"{case} 的标志未启用测试，编辑器会一直停在普通窗口")


class CompletionTests(unittest.TestCase):
    def checker(self):
        nodes = [node for node in TREE.body if
                 isinstance(node, ast.FunctionDef) and node.name == "completed_window_case"
                 or isinstance(node, ast.Assign) and any(isinstance(t, ast.Name) and t.id == "WINDOW_ARTIFACTS" for t in node.targets)]
        scope = {}
        exec(compile(ast.Module(body=nodes, type_ignores=[]), str(SOURCE), "exec"), scope)
        return scope["completed_window_case"]

    def test_clean_exit_race_requires_a_fresh_final_artifact(self):
        check = self.checker()
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "target").mkdir()
            started = time.time_ns()
            artifact = root / "target/editor-connections-smoke.png"
            self.assertFalse(check(root, "connections", started, 0))
            artifact.write_bytes("本次窗口产物".encode())
            os.utime(artifact, ns=(started + 1_000_000, started + 1_000_000))
            self.assertTrue(check(root, "connections", started, 0))
            self.assertFalse(check(root, "connections", started, 101))
            self.assertFalse(check(root, "connections", started, None))

    def test_old_artifact_does_not_turn_early_close_into_success(self):
        check = self.checker()
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "target").mkdir()
            artifact = root / "target/editor-topology-smoke.png"
            artifact.write_bytes("旧产物".encode())
            os.utime(artifact, ns=(1_000_000, 1_000_000))
            self.assertFalse(check(root, "topology", time.time_ns(), 0))


class SegmentationLogTests(unittest.TestCase):
    def checker(self):
        node = next(node for node in TREE.body if isinstance(node, ast.FunctionDef)
                    and node.name == "assert_no_segmentation_warnings")
        scope = {}
        exec(compile(ast.Module(body=[node], type_ignores=[]), str(SOURCE), "exec"), scope)
        return scope["assert_no_segmentation_warnings"]

    def test_clean_chinese_window_log_is_accepted(self):
        check = self.checker()
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "editor.log"
            path.write_text("INFO 中文窗口正常退出", encoding="utf-8")
            check(path)

    def test_both_known_icu_message_forms_are_rejected(self):
        check = self.checker()
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "editor.log"
            for message in ["No segmentation model for language: ja",
                            "No segmentation model for complex script: Chinese/Japanese"]:
                with self.subTest(message=message):
                    path.write_text("ICU4X data error: " + message, encoding="utf-8")
                    with self.assertRaisesRegex(AssertionError, "ICU4X"):
                        check(path)


if __name__ == "__main__":
    unittest.main()
