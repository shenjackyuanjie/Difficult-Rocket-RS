"""演示驱动单元测试：仅临时文件与 subprocess mock，无 GUI、键鼠、通知或构建副作用。"""
from contextlib import redirect_stderr, redirect_stdout
import copy
import editor_session
import importlib.util
import io
import json
import os
import re
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import Mock, patch

from test_rocket_workflow_report import write_catalog, write_workflow_evidence


SPEC = importlib.util.spec_from_file_location("demo_editor", Path(__file__).with_name("demo_editor.py"))
demo = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(demo)


def complete_report(mode="brief"):
    return {
        "completed": True,
        "mode": mode,
        "chapters": [
            {"id": chapter_id, "title": "演示章节", "status": "passed",
             "elapsed_seconds": 0.45,
             "artifacts": (list(demo.rocket_workflow.ARTIFACTS) if chapter_id == "rocket-workflow" else
                           [chapter_id + ".json", *["case-" + case_id + ".png" for case_id in demo.DETAILED_CASE_IDS[chapter_id]]]
                           if chapter_id in demo.DETAILED_CASE_IDS else [chapter_id + ".png", *(["editor-staging-performance.json"] if mode == "detailed" and chapter_id == "staging" else [])])}
            for chapter_id in demo.chapter_ids(mode)
        ],
    }


GEOMETRY_CASE_RATIOS = {
    "overlap-below": (0.04899999871850014, 0.049),
    "overlap-exact": (0.05000000074505806, 0.05),
    "overlap-above": (0.050999999046325684, 0.051),
    "overlap-rotated-mirrors": (0.05000000074505806, 0.05),
    "overlap-relative-45": (0.05000000063678113, 0.05),
    "overlap-contained": (1.0, 1.0),
}


def case_metrics(chapter_id, case_id):
    """与 Rust 一致的最小通用指标，手势与几何再补各自的真实字段。"""
    metrics = {"part_count": 2, "undo_len": 0, "redo_len": 0, "dirty": False}
    if case_id in demo.GESTURE_CASE_CHECKS:
        checks, release = demo.GESTURE_CASE_CHECKS[case_id]
        metrics.update(same_frame_checks=checks, release_frame_checks=release,
                       max_world_pixel_error=0.0000674079783493653,
                       physical_display_latency_measured=False)
    if chapter_id == "geometry-cases":
        ratio, expected = GEOMETRY_CASE_RATIOS[case_id]
        metrics.update(overlap_ratio=ratio, expected_ratio=expected,
                       accepted=expected < demo.OVERLAP_THRESHOLD)
    return metrics


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
        self.catalog = write_catalog(self.base / "Difficult-Rocket/assets/builtin/PartList.xml")
        catalog_patch = patch.object(demo.rocket_workflow, "CATALOG_PATH", self.catalog)
        catalog_patch.start()
        self.addCleanup(catalog_patch.stop)
        self.report = complete_report()
        self.write_evidence(self.report)
        self.process = Mock()
        self.process.poll.return_value = None
        self.process.wait.return_value = 0
        self.captured = io.StringIO()
        self.silence = redirect_stdout(self.captured)
        self.silence.__enter__()
        self.addCleanup(self.silence.__exit__, None, None, None)

    def write_evidence(self, report, fresh=True):
        stamp = self.started_ns + 1_000_000 if fresh else self.started_ns - 1
        for chapter in report["chapters"]:
            if chapter["id"] == "rocket-workflow":
                write_workflow_evidence(self.output, stamp, self.catalog)
                continue
            for name in chapter["artifacts"]:
                artifact = self.output / name
                artifact.write_bytes(b"mock screenshot evidence")
                if name.endswith("-cases.json"):
                    artifact.write_text(json.dumps({"completed": True, "chapter": chapter["id"], "cases": [
                        {"id": case_id, "status": "passed", "title": "样例", "expected": "预期", "input_path": "内部注入",
                         "screenshot": "case-" + case_id + ".png",
                         "metrics": case_metrics(chapter["id"], case_id)}
                        for case_id in demo.DETAILED_CASE_IDS[chapter["id"]]
                    ]}), encoding="utf-8")
                if name == "editor-staging-performance.json":
                    artifact.write_text(json.dumps({"stages":64, "activations":1024, "edited_stage":63, "edited_activation":15,
                        "preview": {"panel_rect": [0, 220, 380, 900], "canvas_rect": [380, 220, 1440, 900],
                                    "target_rect": [600, 320, 650, 360], "target_group": 0, "target_id": 17,
                                    "panel_overlaps_canvas": False, "edited_target_visible": True,
                                    "hover_target_visible": True, "hover_restored": True,
                                    "hover_target_rect": [600, 320, 650, 360]}}), encoding="utf-8")
                os.utime(artifact, ns=(stamp, stamp))
        path = self.output / demo.REPORT_NAME
        path.write_text(json.dumps(report), encoding="utf-8")
        os.utime(path, ns=(stamp, stamp))

    def detailed_report(self):
        report = complete_report("detailed")
        self.write_evidence(report)
        return report

    def rewrite_case_metrics(self, chapter_id, case_id, mutate):
        path = self.output / (chapter_id + ".json")
        document = json.loads(path.read_text(encoding="utf-8"))
        case = next(case for case in document["cases"] if case["id"] == case_id)
        mutate(case["metrics"])
        path.write_text(json.dumps(document), encoding="utf-8")

    def assert_metrics_rejected(self, report, chapter_id, case_id, mutate):
        self.write_evidence(report)  # 每个变体都从完整夹具出发，避免坏字段互相污染。
        self.rewrite_case_metrics(chapter_id, case_id, mutate)
        with self.subTest(chapter=chapter_id, case=case_id), self.assertRaises(demo.DemoError) as raised:
            demo.validate_report(report, self.output, self.started_ns, "detailed")
        self.assertIn(case_id, str(raised.exception))

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
        copy_path = Path(command[0])
        self.assertEqual(command[1:], ["--demo-showcase", str(self.output.resolve()),
                                       "--demo-mode", "brief", "--demo-step-ms", "450"])
        self.assertEqual(copy_path, copy_path.resolve())
        self.assertEqual(copy_path.parent, self.output.resolve())
        self.assertNotEqual(copy_path, self.binary.resolve())
        self.assertEqual(copy_path.suffix, self.binary.suffix)
        self.assertTrue(copy_path.name.startswith("dr-editor-"))
        self.assertEqual(copy_path.read_bytes(), self.binary.read_bytes())
        self.assertEqual(self.binary.read_bytes(), b"mock exe")
        session = json.loads((self.output / editor_session.SESSION_NAME).read_text(encoding="utf-8"))
        self.assertEqual(session, {"source_path": str(self.binary.resolve()),
                                   "source_sha256": editor_session.file_sha256(self.binary),
                                   "copy_path": str(copy_path),
                                   "copy_sha256": editor_session.file_sha256(self.binary)})
        self.assertIn(str(copy_path), self.captured.getvalue())
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
        report = complete_report("detailed")
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

    def test_modes_have_independent_complete_chapter_and_case_contracts(self):
        self.assertEqual(len(demo.CHAPTER_IDS), 11)
        self.assertEqual(len(demo.DETAILED_CHAPTER_IDS), 20)
        self.assertEqual(sum(map(len, demo.DETAILED_CASE_IDS.values())), 45)
        self.assertEqual(demo.CHAPTER_IDS[1:3], ("connections", "rocket-workflow"))
        self.assertEqual(demo.DETAILED_CHAPTER_IDS[2:5], ("connections", "rocket-workflow", "geometry-cases"))
        self.assertEqual(tuple(chapter for chapter in demo.DETAILED_CHAPTER_IDS if chapter not in demo.DETAILED_CASE_IDS), demo.CHAPTER_IDS)
        report = complete_report("detailed")
        self.write_evidence(report)
        self.assertEqual(demo.validate_report(report, self.output, self.started_ns, "detailed"), report)
        report["chapters"] = complete_report()["chapters"]
        with self.assertRaisesRegex(demo.DemoError, "完整 20"):
            demo.validate_report(report, self.output, self.started_ns, "detailed")

    def test_python_contract_matches_rust_case_manifest(self):
        source = (demo.ROOT / "crates/dr-editor/src/demo_cases/manifest.rs").read_text(encoding="utf-8")
        manifests = {name: tuple(re.findall(r'case!\(\s*"([^"]+)"', body))
                     for name, body in re.findall(r'pub\(crate\) const (\w+):.*?= &\[(.*?)\n\];', source, re.S)}
        mapping = {"placement-cases": "PLACEMENT", "geometry-cases": "GEOMETRY", "free-cases": "FREE",
                   "group-cases": "GROUPS", "line-cases": "LINES", "interaction-cases": "INTERACTION",
                   "history-cases": "HISTORY", "topology-cases": "TOPOLOGY", "file-cases": "FILES"}
        for chapter, rust_name in mapping.items():
            self.assertEqual(demo.DETAILED_CASE_IDS[chapter], manifests[rust_name])

    def test_legacy_19_chapters_45_cases_95_artifacts_remain_plus_1_3_9(self):
        source = (demo.ROOT / "crates/dr-editor/src/demo/chapters.rs").read_text(encoding="utf-8")
        entries = re.findall(r'id: "([^"]+)",.*?artifacts: &\[(.*?)\]', source, re.S)
        artifacts = {chapter: re.findall(r'"([^"]+)"', body) for chapter, body in entries}
        self.assertEqual(set(artifacts), set(demo.DETAILED_CHAPTER_IDS))
        self.assertEqual(artifacts["rocket-workflow"], list(demo.rocket_workflow.ARTIFACTS))
        for chapter, cases in demo.DETAILED_CASE_IDS.items():
            artifacts[chapter].extend("case-" + case_id + ".png" for case_id in cases)
        legacy_chapters = tuple(chapter for chapter in demo.DETAILED_CHAPTER_IDS if chapter != "rocket-workflow")
        self.assertEqual(len(legacy_chapters), 19)
        self.assertEqual(sum(map(len, demo.DETAILED_CASE_IDS.values())), 45)
        self.assertEqual(sum(len(artifacts[chapter]) for chapter in legacy_chapters), 95)
        self.assertEqual(len(artifacts), 20)
        self.assertEqual(45 + len(demo.rocket_workflow.CASE_IDS), 48)
        self.assertEqual(sum(map(len, artifacts.values())), 104)
        self.assertIn("editor-staging-performance.json", artifacts["staging"])
        self.assertIn("64 级 / 1024 动作", source)

    def test_rocket_scale_and_chapter_copy_match_eight_card_contract(self):
        self.assertEqual(demo.rocket_workflow.AUTO_STEPS,
                         [[11, 13], [9], [10, 12], [7], [6], [4], [3], [14]])
        self.assertEqual(demo.rocket_workflow.MANUAL_STEPS,
                         [[11, 13], [], [10, 12, 9], [7], [6], [4], [3], [14]])
        self.assertEqual(len(demo.rocket_workflow.PART_TYPES), 14)
        self.assertEqual(sorted(Path(name).suffix for name in demo.rocket_workflow.ARTIFACTS),
                         [".json", *[".png"] * 5, *[".xml"] * 3])
        source = (demo.ROOT / "crates/dr-editor/src/demo/chapters.rs").read_text(encoding="utf-8")
        chapter = re.search(r'const ROCKET_WORKFLOW: Chapter = Chapter \{(.*?)\n\};', source, re.S).group(1)
        for phrase in ("三级芯级 + 双固体助推器", "四个物理推进级", "14 部件",
                       "13 条 Normal 树边", "自动八卡", "移出、移回、再移出",
                       "整卡下移回置", "取消后再应用", "SRB 为叶子", "仅接侧挂分离器",
                       "自带固体 Tank", "无需额外挂罐"):
            self.assertIn(phrase, chapter)
        for old_copy in ("三级火箭：", "五步草稿", "9 部件", "8 条 Normal", "16 部件", "15 条 Normal"):
            self.assertNotIn(old_copy, chapter)
        review = (demo.ROOT / "docs/demo-review-suggestions.md").read_text(encoding="utf-8")
        for phrase in ("ID 10 分离器 → ID 11 SRB", "ID 12 分离器 → ID 13 SRB",
                       "返回降落伞为 ID 14", "SRB 是 Normal 树的叶子",
                       "自带固体 Tank，无需额外挂罐", "顶部也不连接油箱"):
            self.assertIn(phrase, review)
        self.assertNotIn("和小燃料罐组成", review)

    def test_rocket_contract_is_mandatory_in_both_modes_and_wraps_validation_error(self):
        for mode in ("brief", "detailed"):
            report = complete_report(mode)
            self.write_evidence(report)
            chapter = next(chapter for chapter in report["chapters"] if chapter["id"] == "rocket-workflow")
            chapter["artifacts"] = []
            with self.subTest(mode=mode, missing="declaration"), self.assertRaisesRegex(demo.DemoError, "四物理推进级火箭"):
                demo.validate_report(report, self.output, self.started_ns, mode)
            chapter["artifacts"] = list(demo.rocket_workflow.ARTIFACTS)
            path = self.output / "rocket-workflow.json"
            document = json.loads(path.read_text(encoding="utf-8"))
            document["cases"][2]["metrics"]["drags"][0]["events"][1]["frame"] = document["cases"][2]["metrics"]["drags"][0]["events"][0]["frame"]
            path.write_text(json.dumps(document), encoding="utf-8")
            with self.subTest(mode=mode, invalid="same-frame drag"), self.assertRaisesRegex(demo.DemoError, "四物理推进级火箭"):
                demo.validate_report(report, self.output, self.started_ns, mode)

    def test_detailed_cases_cannot_be_missing_faked_or_unillustrated(self):
        report = complete_report("detailed")
        self.write_evidence(report)
        path = self.output / "geometry-cases.json"
        original = json.loads(path.read_text(encoding="utf-8"))
        variants = []
        for key, value in (("completed", False), ("completed", 1), ("chapter", "free-cases"), ("cases", [])):
            bad = copy.deepcopy(original)
            bad[key] = value
            variants.append(bad)
        for key, value in (("id", "unknown"), ("status", "failed"), ("expected", ""), ("input_path", ""),
                           ("screenshot", "missing.png"), ("metrics", None)):
            bad = copy.deepcopy(original)
            bad["cases"][0][key] = value
            variants.append(bad)
        for bad in variants:
            path.write_text(json.dumps(bad), encoding="utf-8")
            with self.subTest(bad=bad), self.assertRaises(demo.DemoError):
                demo.validate_report(report, self.output, self.started_ns, "detailed")
        path.write_text(json.dumps(original), encoding="utf-8")
        for chapter in report["chapters"]:
            if chapter["id"] == "geometry-cases":
                chapter["artifacts"].remove("case-overlap-exact.png")
        with self.assertRaisesRegex(demo.DemoError, "截图"):
            demo.validate_report(report, self.output, self.started_ns, "detailed")

    def test_case_metrics_must_be_real_counts_and_bool_dirty(self):
        report = self.detailed_report()
        variants = [
            ("placement-cases", "palette-cancel", lambda metrics: metrics.clear()),
            ("placement-cases", "palette-cancel", lambda metrics: metrics.pop("undo_len")),
            ("placement-cases", "palette-cancel", lambda metrics: metrics.update(part_count=True)),
            ("placement-cases", "palette-cancel", lambda metrics: metrics.update(redo_len=-1)),
            ("placement-cases", "palette-cancel", lambda metrics: metrics.update(part_count="2")),
            ("placement-cases", "palette-cancel", lambda metrics: metrics.update(dirty="false")),
            ("placement-cases", "palette-cancel", lambda metrics: metrics.update(dirty=1)),
            ("free-cases", "manual-same", lambda metrics: metrics.pop("part_count")),
        ]
        for chapter_id, case_id, mutate in variants:
            self.assert_metrics_rejected(report, chapter_id, case_id, mutate)
        self.write_evidence(report)
        self.assertEqual(demo.validate_report(report, self.output, self.started_ns, "detailed"), report)

    def test_gesture_cases_must_report_same_frame_evidence(self):
        report = self.detailed_report()
        variants = [
            ("drag-continuous", lambda metrics: metrics.update(physical_display_latency_measured=True)),
            ("drag-continuous", lambda metrics: metrics.pop("physical_display_latency_measured")),
            ("drag-continuous", lambda metrics: metrics.update(same_frame_checks=9)),
            ("drag-continuous", lambda metrics: metrics.update(same_frame_checks=True)),
            ("drag-continuous", lambda metrics: metrics.update(release_frame_checks=2)),
            ("pan-gesture", lambda metrics: metrics.update(same_frame_checks=11)),
            ("pan-gesture", lambda metrics: metrics.update(release_frame_checks=3)),
            ("pan-gesture", lambda metrics: metrics.update(max_world_pixel_error=float("nan"))),
            ("pan-gesture", lambda metrics: metrics.update(max_world_pixel_error="0.001")),
            ("drag-continuous", lambda metrics: metrics.update(max_world_pixel_error=float("inf"))),
            ("drag-continuous", lambda metrics: metrics.update(max_world_pixel_error=0.002)),
            ("drag-continuous", lambda metrics: metrics.update(max_world_pixel_error=True)),
        ]
        for case_id, mutate in variants:
            self.assert_metrics_rejected(report, "interaction-cases", case_id, mutate)
        self.write_evidence(report)
        self.assertEqual(demo.validate_report(report, self.output, self.started_ns, "detailed"), report)

    def test_geometry_cases_only_check_the_ratio_fields_they_actually_carry(self):
        report = self.detailed_report()
        variants = [
            ("overlap-below", lambda metrics: metrics.update(overlap_ratio=float("nan"))),
            ("overlap-below", lambda metrics: metrics.update(expected_ratio="0.049")),
            ("overlap-below", lambda metrics: metrics.update(expected_ratio=True)),
            ("overlap-below", lambda metrics: metrics.update(accepted=1)),
            ("overlap-below", lambda metrics: metrics.update(accepted="true")),
            ("overlap-exact", lambda metrics: metrics.update(accepted=True)),
            ("overlap-below", lambda metrics: metrics.update(accepted=False)),
            ("overlap-below", lambda metrics: metrics.pop("expected_ratio")),
        ]
        for case_id, mutate in variants:
            self.assert_metrics_rejected(report, "geometry-cases", case_id, mutate)

        def common_only(metrics):
            metrics.clear()
            metrics.update(part_count=2, undo_len=0, redo_len=0, dirty=False)

        for case_id in demo.DETAILED_CASE_IDS["geometry-cases"]:
            self.rewrite_case_metrics("geometry-cases", case_id, common_only)
        self.assertEqual(demo.validate_report(report, self.output, self.started_ns, "detailed"), report)
        self.rewrite_case_metrics("geometry-cases", "overlap-below",
                                  lambda metrics: metrics.update(expected_ratio=float("inf")))
        with self.assertRaisesRegex(demo.DemoError, "expected_ratio"):
            demo.validate_report(report, self.output, self.started_ns, "detailed")

    def test_detailed_staging_must_use_the_full_list_and_edit_its_last_action(self):
        report = complete_report("detailed")
        self.write_evidence(report)
        path = self.output / "editor-staging-performance.json"
        original = json.loads(path.read_text(encoding="utf-8"))
        for field, value in (("stages", 8), ("activations", 48), ("edited_stage", 7), ("edited_activation", 0)):
            bad = dict(original, **{field: value})
            path.write_text(json.dumps(bad), encoding="utf-8")
            with self.subTest(field=field), self.assertRaisesRegex(demo.DemoError, "64 级"):
                demo.validate_report(report, self.output, self.started_ns, "detailed")

    def test_staging_cannot_claim_visible_target_with_missing_or_overlapping_geometry(self):
        report = self.detailed_report()
        path = self.output / "editor-staging-performance.json"
        original = json.loads(path.read_text(encoding="utf-8"))
        for preview in [None, {}, dict(original["preview"], panel_overlaps_canvas=True),
                        dict(original["preview"], edited_target_visible=False),
                        dict(original["preview"], hover_target_visible=False),
                        dict(original["preview"], hover_restored=False),
                        dict(original["preview"], hover_target_rect=[0, 0, 20, 20]),
                        dict(original["preview"], panel_rect=[300, 220, 500, 900]),
                        dict(original["preview"], target_rect=[0, 0, 20, 20])]:
            with self.subTest(preview=preview), self.assertRaisesRegex(demo.DemoError, "分级"):
                path.write_text(json.dumps(dict(original, preview=preview)), encoding="utf-8")
                demo.validate_report(report, self.output, self.started_ns, "detailed")

    def test_staging_preview_requires_finite_positive_rectangles_and_real_identity(self):
        report = self.detailed_report()
        path = self.output / "editor-staging-performance.json"
        original = json.loads(path.read_text(encoding="utf-8"))
        for field, value in [("target_group", True), ("target_id", False),
                             ("canvas_rect", [0, 0, 0, 10]), ("target_rect", [0, 1, float("nan"), 10]),
                             ("panel_rect", [0, True, 10, 20]), ("panel_rect", [0, 1, 10])]:
            with self.subTest(field=field, value=value), self.assertRaisesRegex(demo.DemoError, "分级"):
                path.write_text(json.dumps(dict(original, preview=dict(original["preview"], **{field:value}))), encoding="utf-8")
                demo.validate_report(report, self.output, self.started_ns, "detailed")

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
