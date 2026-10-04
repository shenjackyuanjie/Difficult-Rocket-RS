"""前台驱动的重试/提醒及 CLI 映射回归，不操作桌面、不发送通知。"""
import ast
from contextlib import redirect_stdout
import io
from pathlib import Path
import re
import sys
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
                       if isinstance(node, ast.Dict) and any(isinstance(key, ast.Constant) and key.value == "panels" for key in node.keys))
        choices = next(ast.literal_eval(keyword.value) for node in ast.walk(TREE)
                       if isinstance(node, ast.Call) and node.args and isinstance(node.args[0], ast.Constant)
                       and node.args[0].value == "--window-case" for keyword in node.keywords if keyword.arg == "choices")
        editor = (SOURCE.parents[1] / "crates/dr-editor/src/main.rs").read_text(encoding="utf-8")
        supported = set(re.findall(r'"(--[a-z-]+test)"', editor))
        for case in choices:
            if case not in ["native", "performance"]:
                flag = "--" + mapping.get(case, case + "-smoke-test")
                self.assertIn(flag, supported, f"{case} 的标志未启用测试，编辑器会一直停在普通窗口")


if __name__ == "__main__":
    unittest.main()
