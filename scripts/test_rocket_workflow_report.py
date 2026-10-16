"""16 部件分支火箭证据验收回归：临时夹具与只读目录，不启动编辑器。"""
import copy
import json
import math
import os
from pathlib import Path
import tempfile
import unittest
import xml.etree.ElementTree as ET

import rocket_workflow_report as workflow


def write_catalog(path):
    """带 namespace 与 hidden Pod 的独立小目录，索引包含额外可见部件。"""
    root = ET.Element("PartTypes", xmlns="http://example.test/partlist.xsd")
    for part_type, kind, height, hidden in (
            ("pod-1", "pod", 3, True), ("detacher-1", "detacher", 1, False),
            ("hidden-wheel", "wheel", 4, True), ("wheel-2", "wheel", 4, False),
            ("fueltank-0", "tank", 2, False), ("engine-0", "engine", 2, False),
            ("detacher-2", "radial", 4, False), ("engine-4", "srb", 26, False),
            ("parachute-1", "parachute", 1, False)):
        sprite = {"pod": "Pod.png", "tank": "TankTiny.png", "engine": "EngineTiny.png",
                  "detacher": "DetacherVertical.png", "wheel": "Wheel.png",
                  "radial": "DetacherRadial.png", "srb": "SolidRocketBooster.png",
                  "parachute": "ParachuteCanister.png"}[kind]
        item = ET.SubElement(root, "PartType", id=part_type, type={"radial": "detacher", "srb": "engine"}.get(kind, kind),
                             width="1" if kind == "radial" else "4",
                             height=str(height), sprite=sprite)
        if hidden:
            item.set("hidden", "true")
        points = ET.SubElement(item, "AttachPoints")
        for location in (("LeftCenter", "RightCenter") if kind == "radial" else
                         ("BottomCenter",) if kind == "parachute" else ("TopCenter", "BottomCenter")):
            ET.SubElement(points, "AttachPoint", location=location)
        if kind in ("tank", "srb"):
            for location in ("LeftSide", "RightSide"):
                ET.SubElement(points, "AttachPoint", location=location)
    path.parent.mkdir(parents=True, exist_ok=True)
    ET.ElementTree(root).write(path, encoding="utf-8", xml_declaration=True)
    return path


def fixture_document(catalog_path):
    """独立构造已完成记录；几何采用目录 height/4 半高，不调用被测几何函数。"""
    root = ET.parse(catalog_path).getroot()
    definitions = list(root)
    visible = [item.get("id") for item in definitions if item.get("hidden", "false") != "true"]
    heights = {item.get("id"): float(item.get("height")) for item in definitions}
    sprites = {item.get("id"): item.get("sprite") for item in definitions}
    types = ["pod-1", "fueltank-0", "engine-0", "detacher-1", "fueltank-0",
             "engine-0", "detacher-1", "fueltank-0", "engine-0"]
    widths = {item.get("id"): float(item.get("width")) for item in definitions}
    positions, y = [[1.25, 2.5]], 2.5
    for parent, child in zip(types, types[1:]):
        y -= (heights[parent] + heights[child]) / 4
        positions.append([1.25, y])
    types.extend(["detacher-2", "engine-4", "fueltank-0", "detacher-2",
                  "engine-4", "fueltank-0", "parachute-1"])
    for sign in (-1, 1):
        x = 1.25 + sign * (widths["fueltank-0"] + widths["detacher-2"]) / 4
        positions.append([x, positions[7][1]])
        x += sign * (widths["detacher-2"] + widths["engine-4"]) / 4
        positions.append([x, positions[7][1]])
        positions.append([x, positions[7][1] + (heights["engine-4"] + heights["fueltank-0"]) / 4])
    positions.append([1.25, 2.5 + (heights["pod-1"] + heights["parachute-1"]) / 4])
    placements = [{"id": part_id, "part_type": part_type, "catalog_index": visible.index(part_type),
                   "catalog_hit": [12.0, 20.0, 180.0, 68.0], "preview_unchanged": True,
                   "normal_snap": True, "position": positions[part_id - 1]}
                  for part_id, part_type in enumerate(types[1:], 2)]
    drags = [{"kind": kind, "source": [90.0, 200.0 + index * 20],
              "target": [110.0, 300.0 + index * 20], "threshold_distance": 14.0,
              "events": [{"event": event, "frame": 100 + index * 10 + offset}
                         for offset, event in enumerate(("press", "held_threshold", "held_target", "release"))]}
             for index, kind in enumerate(("Activation", "CardDown", "CardUp", "ActivationBack", "Activation", "Activation"))]
    visuals = []
    for part_id, (part_type, position) in enumerate(zip(types, positions), 1):
        path = "textures/parts/" + sprites[part_type]
        image = "fixture-image:" + sprites[part_type]
        color = "fixture opaque white"
        translation = [position[0] * 60, position[1] * 60, (part_id - 1) / 9 * 2]
        visuals.append({"id": part_id, "group": 0, "occurrence": 0, "entity": f"{part_id}v0",
                        "part_type": part_type, "expected_path": path, "expected_image": image,
                        "image": image, "image_path": path, "image_loaded": True, "image_size": [32, 64],
                        "image_load_state": "Some(Loaded)", "material": "fixture-material:" + part_type,
                        "material_image": image, "material_image_path": path,
                        "material_anchor": [0.0, 0.0], "material_custom_size": None,
                        "material_flip": [False, False], "material_color": color,
                        "anchor": [0.0, 0.0], "sprite_custom_size": None,
                        "sprite_flip": [False, False], "sprite_color": color,
                        "local_translation": translation.copy(), "global_translation": translation.copy(),
                        "visibility": "Inherited", "in_view": True})
    report = {
        "chapter": "rocket-workflow", "completed": True,
        "input_mode": "真实目录命中 / 生产画布鼠标 / egui 分帧拖放",
        "cases": [
            {"id": "rocket-assembly", "status": "passed", "metrics": {
                "start_parts": 1, "start_connections": 0, "start_staging_steps": 0,
                "parts": 16, "normal_connections": 15, "engines": 5, "detachers": 4, "physical_stages": 4,
                "placement_history_steps": 15, "endpoint_max_error": 0.0,
                "placements": placements, "xml_roundtrip": True,
                 "visual_entities": 16, "visual_stable_frames": 2, "visuals": visuals}},
            {"id": "basic-staging", "status": "passed", "metrics": {
                "steps": [[11, 14], [9], [10, 13], [7], [6], [4], [3], [16]], "current_stage": 0, "all_moved_false": True,
                "draft_only_before_apply": True, "apply_history_delta": 1,
                "undo_exact": True, "redo_exact": True, "xml_roundtrip": True}},
            {"id": "manual-staging-drag", "status": "passed", "metrics": {
                "steps": [[11, 14], [], [10, 13, 9], [7], [6], [4], [3], [16]], "moved_ids": [9], "activation_multiset": [3, 4, 6, 7, 9, 10, 11, 13, 14, 16],
                "draft_only_before_apply": True, "card_reordered": True, "card_restored": True,
                "cancel_unchanged": True, "apply_history_delta": 1, "undo_exact": True, "redo_exact": True,
                "xml_roundtrip": True, "drags": drags}},
        ],
    }
    return report, positions


def write_workflow_evidence(output, stamp, catalog_path):
    report, positions = fixture_document(catalog_path)
    output.mkdir(parents=True, exist_ok=True)
    for name in workflow.ARTIFACTS[:5]:
        (output / name).write_bytes(b"fixture screenshot evidence")
    for name, rows in zip(workflow.ARTIFACTS[5:8], ([], [[11, 14], [9], [10, 13], [7], [6], [4], [3], [16]], [[11, 14], [], [10, 13, 9], [7], [6], [4], [3], [16]])):
        root = ET.Element("Ship", version="1", liftedOff="0", touchingGround="1")
        parts = ET.SubElement(root, "Parts")
        for part_id, (part_type, (x, y)) in enumerate(zip(["pod-1"] + [item["part_type"] for item in report["cases"][0]["metrics"]["placements"]], positions), 1):
            part = ET.SubElement(parts, "Part", id=str(part_id), partType=part_type, x=str(x), y=str(y),
                                 angle="0", editorAngle="0", angleV="0", flippedX="0", flippedY="0",
                                 activated="0", exploded="0")
            if part_id == 1:
                pod = ET.SubElement(part, "Pod", name="Command Pod", throttle="0")
                staging = ET.SubElement(pod, "Staging", currentStage="0")
                for row in rows:
                    step = ET.SubElement(staging, "Step")
                    for target in row:
                        moved = name == "rocket-manual-staging.xml" and target == 9
                        ET.SubElement(step, "Activate", Id=str(target), moved="1" if moved else "0")
            if part_type == "fueltank-0":
                ET.SubElement(part, "Tank", fuel="750")
            if part_type == "engine-4":
                ET.SubElement(part, "Tank", fuel="9000")
            if part_type in ("engine-0", "engine-4"):
                ET.SubElement(part, "Engine", fuel="0")
        connections = ET.SubElement(root, "Connections")
        edges = [(i, i + 1, 2, 1) for i in range(1, 9)] + [
            (8, 10, 3, 2), (10, 11, 1, 4), (11, 12, 1, 2),
            (8, 13, 4, 1), (13, 14, 2, 3), (14, 15, 1, 2), (1, 16, 1, 1)]
        for parent, child, parent_point, child_point in edges:
            ET.SubElement(connections, "Connection", parentPart=str(parent), childPart=str(child),
                          parentAttachPoint=str(parent_point), childAttachPoint=str(child_point))
        ET.ElementTree(root).write(output / name, encoding="utf-8", xml_declaration=True)
    (output / "rocket-workflow.json").write_text(json.dumps(report), encoding="utf-8")
    for name in workflow.ARTIFACTS:
        os.utime(output / name, ns=(stamp, stamp))
    return report


class WorkflowTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.base = Path(self.temporary.name)
        self.output = self.base / "output"
        self.catalog = write_catalog(self.base / "PartList.xml")
        self.started_ns = 2_000_000_000
        self.chapter = {"id": "rocket-workflow", "artifacts": list(workflow.ARTIFACTS)}
        self.reset()

    def reset(self):
        self.report = write_workflow_evidence(self.output, self.started_ns + 1_000_000, self.catalog)

    def validate(self):
        return workflow.validate_workflow(self.chapter, self.output, self.started_ns, self.catalog)

    def change_report(self, mutate):
        document = copy.deepcopy(self.report)
        mutate(document)
        (self.output / "rocket-workflow.json").write_text(json.dumps(document), encoding="utf-8")

    def reject_report(self, mutate):
        self.change_report(mutate)
        with self.assertRaises(workflow.WorkflowError):
            self.validate()

    def change_xml(self, name, mutate):
        path = self.output / name
        tree = ET.parse(path)
        mutate(tree.getroot())
        tree.write(path, encoding="utf-8", xml_declaration=True)

    def test_complete_contract_accepts_three_real_plans_and_hidden_pod(self):
        self.assertEqual(self.validate(), self.report)
        self.assertEqual(len(workflow.ARTIFACTS), 9)
        self.assertEqual(workflow.CASE_IDS, ("rocket-assembly", "basic-staging", "manual-staging-drag"))
        catalog = workflow.load_catalog(self.catalog)
        self.assertIsNone(catalog["pod-1"]["index"])
        self.assertEqual(catalog["fueltank-0"]["index"], 2)
        self.assertEqual(catalog["engine-0"]["index"], 3)

    def test_complete_fixture_also_matches_read_only_builtin_catalog(self):
        original = workflow.CATALOG_PATH.read_bytes()
        expected = write_workflow_evidence(self.output, self.started_ns + 1_000_000, workflow.CATALOG_PATH)
        self.assertEqual(workflow.validate_workflow(self.chapter, self.output, self.started_ns), expected)
        self.assertEqual(workflow.CATALOG_PATH.read_bytes(), original)

    def test_catalog_dimensions_and_explicit_points_drive_endpoint_validation(self):
        tree = ET.parse(self.catalog)
        for item in tree.getroot():
            height = float(item.get("height")) + 2
            item.set("height", str(height))
            for point in list(item)[0]:
                location = point.get("location")
                if location in ("TopCenter", "BottomCenter"):
                    point.attrib.clear()
                    point.set("x", "0")
                    point.set("y", str(height / 2 if location == "TopCenter" else -height / 2))
        tree.write(self.catalog, encoding="utf-8", xml_declaration=True)
        expected = write_workflow_evidence(self.output, self.started_ns + 1_000_000, self.catalog)
        self.assertEqual(self.validate(), expected)

    def test_small_real_endpoint_error_is_accepted_and_correlated_with_report(self):
        for name in workflow.ARTIFACTS[5:8]:
            self.change_xml(name, lambda root: root.findall("Parts/Part")[1].set("x", "1.250001"))
        self.change_report(lambda d: d["cases"][0]["metrics"].update(endpoint_max_error=1e-6))
        document = json.loads((self.output / "rocket-workflow.json").read_text(encoding="utf-8"))
        document["cases"][0]["metrics"]["placements"][0]["position"][0] = 1.250001
        (self.output / "rocket-workflow.json").write_text(json.dumps(document), encoding="utf-8")
        self.assertEqual(self.validate(), document)

    def test_branch_topology_and_exact_installation_points_are_required(self):
        for edge_index, field, value in ((8, "parentPart", "5"), (9, "childPart", "14"),
                                         (8, "parentAttachPoint", "4"), (9, "childAttachPoint", "3"),
                                         (14, "childAttachPoint", "2")):
            self.reset()
            self.change_xml("rocket-assembled.xml", lambda root: root.findall("Connections/Connection")[edge_index].set(field, value))
            with self.subTest(edge=edge_index, field=field), self.assertRaises(workflow.WorkflowError):
                workflow.validate_ship(self.output / "rocket-assembled.xml", workflow.load_catalog(self.catalog), [], [])

    def test_asymmetry_is_rejected_even_when_each_endpoint_error_is_tolerable(self):
        def skew(root):
            for part in root.findall("Parts/Part"):
                part_id = int(part.get("id"))
                if 10 <= part_id <= 15:
                    delta = 6e-6 if part_id <= 12 else -6e-6
                    part.set("y", str(float(part.get("y")) + delta))
        self.change_xml("rocket-assembled.xml", skew)
        with self.assertRaisesRegex(workflow.WorkflowError, "几何对称"):
            workflow.validate_ship(self.output / "rocket-assembled.xml", workflow.load_catalog(self.catalog), [], [])

    def test_nonzero_angle_mirroring_and_srb_without_own_fuel_are_rejected(self):
        for mutate in (lambda p: p.set("angle", "0.1"), lambda p: p.set("flippedX", "1"),
                       lambda p: p.set("flippedY", "1"), lambda p: p.remove(p.find("Tank")),
                       lambda p: p.find("Tank").set("fuel", "0")):
            self.reset()
            self.change_xml("rocket-assembled.xml", lambda root: mutate(root.findall("Parts/Part")[10]))
            with self.subTest(mutate=mutate), self.assertRaises(workflow.WorkflowError):
                workflow.validate_ship(self.output / "rocket-assembled.xml", workflow.load_catalog(self.catalog), [], [])

    def test_exact_cards_reject_same_multiset_in_wrong_groups_or_order(self):
        for case_index in (1, 2):
            for source, target in ((0, 1), (2, 3), (7, 6)):
                def regroup(document):
                    rows = document["cases"][case_index]["metrics"]["steps"]
                    rows[target].append(rows[source].pop())
                self.reject_report(regroup)
        self.reject_report(lambda d: d["cases"][1]["metrics"]["steps"][0].reverse())
        for name in ("rocket-basic-staging.xml", "rocket-manual-staging.xml"):
            self.reset()
            def regroup_xml(root):
                steps = root.findall("Parts/Part/Pod/Staging/Step")
                item = steps[0][1]
                steps[0].remove(item)
                steps[1].append(item)
            self.change_xml(name, regroup_xml)
            with self.assertRaisesRegex(workflow.WorkflowError, "分级步骤"):
                self.validate()

    def test_report_requires_completed_chapter_input_mode_and_exact_case_order(self):
        changes = [lambda d: d.update(completed=False), lambda d: d.update(completed=1),
                   lambda d: d.update(chapter="staging"), lambda d: d.update(input_mode="直接执行 Place"),
                   lambda d: d.update(cases=[]), lambda d: d["cases"].reverse(),
                   lambda d: d["cases"].append(copy.deepcopy(d["cases"][0])),
                   lambda d: d["cases"][0].update(status="failed"), lambda d: d["cases"][0].update(metrics=None)]
        for index, mutate in enumerate(changes):
            with self.subTest(index=index):
                self.reject_report(mutate)

    def test_every_count_history_and_boolean_metric_is_required_and_typed(self):
        for case_index, case in enumerate(self.report["cases"]):
            for key, value in case["metrics"].items():
                if type(value) not in (int, bool):
                    continue
                invalid = [None, not value, 1.0, "1"] if type(value) is bool else [None, value + 1, bool(value), float(value), -1]
                for bad in invalid:
                    with self.subTest(case=case["id"], key=key, value=bad):
                        self.reject_report(lambda d: d["cases"][case_index]["metrics"].update({key: bad}))
                with self.subTest(case=case["id"], missing=key):
                    self.reject_report(lambda d: d["cases"][case_index]["metrics"].pop(key))

    def test_endpoint_error_requires_real_finite_nonnegative_xml_measurement(self):
        for value in (None, True, -0.1, 1e-5, 0.2, float("nan"), float("inf"), "0", 5e-6):
            with self.subTest(value=value):
                self.reject_report(lambda d: d["cases"][0]["metrics"].update(endpoint_max_error=value))

    def test_staging_targets_preserve_order_empty_card_multiset_and_only_engine9_moved(self):
        for case_index, field, values in (
                (1, "steps", ([], [[3], [4], [6], [7], [9]], [[9], [7], [6], [4]], [[9.0], [7], [6], [4], [3]])),
                (2, "steps", ([[9], [7, 6], [4], [3]], [[9], [7, 6, 6], [], [4], [3]], [[11, 14], [9], [10, 13], [7], [6], [4], [3], [16]])),
                (2, "moved_ids", ([], [7], [6, 9], [9.0], [True])),
                (2, "activation_multiset", ([3, 4, 6, 9], [3, 4, 6, 6, 7, 9], [9, 7, 6, 4, 3], [3, 4, 6, 7, 8]))):
            for value in values:
                with self.subTest(case=case_index, field=field, value=value):
                    self.reject_report(lambda d: d["cases"][case_index]["metrics"].update({field: value}))

    def test_each_placement_records_real_catalog_hit_preview_and_matching_world_position(self):
        mutations = [("id", 3), ("id", 2.0), ("part_type", "pod-1"), ("catalog_index", 0),
                     ("catalog_index", True), ("preview_unchanged", False), ("normal_snap", 1),
                     ("catalog_hit", [0, 0, 0, 30]), ("catalog_hit", [0, 0, 30, -1]),
                     ("catalog_hit", [0, False, 30, 40]), ("catalog_hit", [0, 0, float("nan"), 4]),
                     ("position", [0, 0]), ("position", [1]), ("position", [False, 1]),
                     ("position", [10 ** 400, 0])]
        for field, value in mutations:
            with self.subTest(field=field, value=value):
                self.reject_report(lambda d: d["cases"][0]["metrics"]["placements"][0].update({field: value}))
        self.reject_report(lambda d: d["cases"][0]["metrics"]["placements"].pop())
        self.reject_report(lambda d: d["cases"][0]["metrics"]["placements"].append(copy.deepcopy(d["cases"][0]["metrics"]["placements"][0])))
        self.reject_report(lambda d: d["cases"][0]["metrics"]["placements"].__setitem__(0, None))

    def test_visual_evidence_requires_sixteen_ordered_unique_keys_and_entities(self):
        mutations = [lambda metrics: metrics.pop("visuals"), lambda metrics: metrics.update(visuals=None),
                     lambda metrics: metrics.update(visuals=[]), lambda metrics: metrics["visuals"].pop(),
                     lambda metrics: metrics["visuals"].append(copy.deepcopy(metrics["visuals"][0])),
                     lambda metrics: metrics["visuals"].reverse(),
                     lambda metrics: metrics["visuals"].__setitem__(0, None),
                     lambda metrics: metrics["visuals"][8].update(entity=metrics["visuals"][0]["entity"]),
                     lambda metrics: metrics["visuals"][8].update(id=1)]
        for index, mutate in enumerate(mutations):
            with self.subTest(index=index):
                self.reject_report(lambda d: mutate(d["cases"][0]["metrics"]))
        for field, value in (("id", True), ("id", 1.0), ("group", False), ("group", 1),
                             ("occurrence", False), ("occurrence", 1), ("part_type", "engine-0")):
            with self.subTest(field=field, value=value):
                self.reject_report(lambda d: d["cases"][0]["metrics"]["visuals"][0].update({field: value}))

    def test_every_visual_observation_field_is_required(self):
        for field in self.report["cases"][0]["metrics"]["visuals"][0]:
            with self.subTest(field=field):
                self.reject_report(lambda d: d["cases"][0]["metrics"]["visuals"][0].pop(field))

    def test_visual_paths_and_three_image_ids_are_independently_correlated(self):
        for field in ("expected_path", "image_path", "material_image_path"):
            for path in (None, "", "textures/parts/TankTiny.png", "../Pod.png"):
                with self.subTest(field=field, path=path):
                    self.reject_report(lambda d: d["cases"][0]["metrics"]["visuals"][0].update({field: path}))
        for field in ("entity", "expected_image", "image", "material", "material_image"):
            for value in (None, "", " ", False, 1, [], " padded "):
                with self.subTest(field=field, value=value):
                    self.reject_report(lambda d: d["cases"][0]["metrics"]["visuals"][0].update({field: value}))
        for field in ("expected_image", "image", "material_image"):
            with self.subTest(mismatch=field):
                self.reject_report(lambda d: d["cases"][0]["metrics"]["visuals"][0].update({field: "other-image"}))
        # 三个自报句柄彼此相同仍不足：同一路径须同句柄，不同贴图须不同句柄。
        self.reject_report(lambda d: d["cases"][0]["metrics"]["visuals"][5].update(
            expected_image="other-engine-image", image="other-engine-image", material_image="other-engine-image"))
        self.reject_report(lambda d: [row.update(expected_image="shared", image="shared", material_image="shared")
                                      for row in d["cases"][0]["metrics"]["visuals"]])

    def test_visual_loaded_state_and_dimensions_reject_false_integer_impostors(self):
        for field, values in (("image_loaded", (False, 1, "true", None)),
                              ("image_load_state", (None, "", "Some(Loading)", "Some(Failed(error))")),
                              ("image_size", (None, [32], [0, 64], [-1, 64], [True, 64],
                                              [32.0, 64], [32, False], [float("inf"), 64]))):
            for value in values:
                with self.subTest(field=field, value=value):
                    self.reject_report(lambda d: d["cases"][0]["metrics"]["visuals"][0].update({field: value}))

    def test_visual_local_and_global_coordinates_require_finite_xml_scaled_positions(self):
        for field in ("local_translation", "global_translation"):
            for value in (None, [75, 150], [76, 150, 0], [75, 151, 0], [False, 150, 0],
                          [75, float("nan"), 0], [75, 150, float("inf")], [10 ** 400, 150, 0]):
                with self.subTest(field=field, value=value):
                    self.reject_report(lambda d: d["cases"][0]["metrics"]["visuals"][0].update({field: value}))

    def test_visual_visibility_requires_true_view_and_nonhidden_known_state(self):
        for field, values in (("in_view", (False, 1, "true", None)),
                              ("visibility", ("Hidden", "unknown", "", False, None))):
            for value in values:
                with self.subTest(field=field, value=value):
                    self.reject_report(lambda d: d["cases"][0]["metrics"]["visuals"][0].update({field: value}))

    def test_visual_sprite_and_material_diagnostics_must_agree(self):
        changes = [("anchor", None), ("anchor", [0, False]), ("anchor", [float("nan"), 0]),
                   ("material_anchor", [0.5, 0]), ("material_flip", [True, False]),
                   ("sprite_flip", [0, 0]), ("sprite_flip", [False]), ("material_flip", None),
                   ("sprite_color", ""), ("sprite_color", 1), ("material_color", "other-color"),
                   ("sprite_custom_size", [0, 64]), ("material_custom_size", [32, 64]),
                   ("sprite_custom_size", [False, 64]), ("sprite_custom_size", [32]),
                   ("material_custom_size", [32, float("nan")])]
        for field, value in changes:
            with self.subTest(field=field, value=value):
                self.reject_report(lambda d: d["cases"][0]["metrics"]["visuals"][0].update({field: value}))

    def test_visual_diagnostics_allow_consistent_metadata_and_subpixel_tolerance_without_gpu_claim(self):
        document = copy.deepcopy(self.report)
        observed = document["cases"][0]["metrics"]["visuals"][0]
        observed.update(visibility="Visible", anchor=[-0.5, 0.25], material_anchor=[-0.5, 0.25],
                        sprite_custom_size=[32.0, 64.0], material_custom_size=[32.0, 64.0])
        observed["local_translation"][0] += 0.0005
        observed["global_translation"][1] -= 0.0005
        (self.output / "rocket-workflow.json").write_text(json.dumps(document), encoding="utf-8")
        self.assertEqual(self.validate(), document)

    def test_visual_positions_are_checked_against_xml_not_only_placement_metrics(self):
        for name in workflow.ARTIFACTS[5:8]:
            self.change_xml(name, lambda root: [part.set("x", str(float(part.get("x")) + 1))
                                                for part in root.findall("Parts/Part")])
        document = copy.deepcopy(self.report)
        for placement in document["cases"][0]["metrics"]["placements"]:
            placement["position"][0] += 1
        path = self.output / "rocket-workflow.json"
        path.write_text(json.dumps(document), encoding="utf-8")
        with self.assertRaisesRegex(workflow.WorkflowError, "translation.*XML"):
            self.validate()
        for observed in document["cases"][0]["metrics"]["visuals"]:
            observed["local_translation"][0] += 60
            observed["global_translation"][0] += 60
        path.write_text(json.dumps(document), encoding="utf-8")
        self.assertEqual(self.validate(), document)

    def test_visual_expected_paths_come_from_actual_catalog_sprite(self):
        tree = ET.parse(self.catalog)
        list(tree.getroot())[0].set("sprite", "OtherPod.png")
        tree.write(self.catalog, encoding="utf-8", xml_declaration=True)
        with self.assertRaisesRegex(workflow.WorkflowError, "expected_path"):
            self.validate()
        expected = write_workflow_evidence(self.output, self.started_ns + 1_000_000, self.catalog)
        self.assertEqual(self.validate(), expected)
        for value in (None, "", " "):
            if value is None:
                list(tree.getroot())[0].attrib.pop("sprite", None)
            else:
                list(tree.getroot())[0].set("sprite", value)
            tree.write(self.catalog, encoding="utf-8", xml_declaration=True)
            with self.subTest(sprite=value), self.assertRaises(workflow.WorkflowError):
                self.validate()

    def test_drag_kinds_events_and_frames_require_real_separate_press_hold_release(self):
        mutations = [lambda drags: drags.pop(), lambda drags: drags.reverse(),
                     lambda drags: drags[0].update(kind="click"), lambda drags: drags[0].update(events=[]),
                     lambda drags: drags[0]["events"].reverse(),
                     lambda drags: drags[0]["events"][1].update(event="click"),
                     lambda drags: drags[0]["events"][1].update(frame=100),
                     lambda drags: drags[0]["events"][1].update(frame=99),
                     lambda drags: drags[0]["events"][0].update(frame=0),
                     lambda drags: drags[0]["events"][0].update(frame=True),
                     lambda drags: drags[0]["events"][0].update(frame=100.0),
                     lambda drags: drags[1]["events"][0].update(frame=103),
                     lambda drags: drags[0]["events"].__setitem__(0, None),
                     lambda drags: drags.__setitem__(0, None)]
        for index, mutate in enumerate(mutations):
            with self.subTest(index=index):
                self.reject_report(lambda d: mutate(d["cases"][2]["metrics"]["drags"]))

    def test_drag_requires_distinct_finite_source_target_and_measured_threshold(self):
        for field, value in (("source", None), ("source", [1]), ("source", [False, 2]),
                             ("target", [float("inf"), 2]), ("target", [90.0, 200.0]),
                             ("threshold_distance", 0), ("threshold_distance", True),
                             ("threshold_distance", 13.9), ("threshold_distance", "14")):
            with self.subTest(field=field, value=value):
                self.reject_report(lambda d: d["cases"][2]["metrics"]["drags"][0].update({field: value}))

    def test_every_artifact_must_be_declared_once_in_exact_order(self):
        for names in ([], list(reversed(workflow.ARTIFACTS)), list(workflow.ARTIFACTS[1:]),
                      [*workflow.ARTIFACTS, workflow.ARTIFACTS[0]],
                      ["../" + workflow.ARTIFACTS[0], *workflow.ARTIFACTS[1:]]):
            with self.subTest(names=names), self.assertRaises(workflow.WorkflowError):
                workflow.validate_workflow({"id": "rocket-workflow", "artifacts": names}, self.output,
                                           self.started_ns, self.catalog)

    def test_all_nine_evidences_must_exist_be_nonempty_and_be_from_this_run(self):
        for name in workflow.ARTIFACTS:
            for mode in ("missing", "empty", "stale"):
                self.reset()
                path = self.output / name
                if mode == "missing":
                    path.unlink()
                elif mode == "empty":
                    path.write_bytes(b"")
                else:
                    os.utime(path, ns=(self.started_ns - 1, self.started_ns - 1))
                with self.subTest(name=name, mode=mode), self.assertRaises(workflow.WorkflowError):
                    self.validate()

    def test_unreadable_duplicate_and_nonfinite_json_cannot_pass(self):
        path = self.output / "rocket-workflow.json"
        for content in (b"{", b"[]", b"null", b"\xff", b'{"completed":true,"completed":false}',
                        b'{"unknown": NaN}', b'{"unknown": Infinity}'):
            path.write_bytes(content)
            with self.subTest(content=content), self.assertRaises(workflow.WorkflowError):
                self.validate()

    def test_all_three_xml_files_are_parsed_instead_of_trusting_metrics(self):
        for name in workflow.ARTIFACTS[5:8]:
            for content in ("<Ship", "<Ship/>", "<Other/>"):
                self.reset()
                (self.output / name).write_text(content, encoding="utf-8")
                with self.subTest(name=name, content=content), self.assertRaises(workflow.WorkflowError):
                    self.validate()

    def test_xml_requires_exact_main_parts_types_unique_ids_and_no_disconnected_group(self):
        changes = [lambda root: root.find("Parts").remove(root.find("Parts/Part")),
                   lambda root: root.find("Parts").append(copy.deepcopy(root.find("Parts/Part"))),
                   lambda root: root.find("Parts/Part").set("partType", "engine-0"),
                   lambda root: root.findall("Parts/Part")[1].set("id", "1"),
                   lambda root: root.findall("Parts/Part")[1].set("id", "10"),
                   lambda root: ET.SubElement(root, "DisconnectedPart"),
                   lambda root: ET.SubElement(root.find("Parts/Part"), "Pod"),
                   lambda root: ET.SubElement(root.findall("Parts/Part")[1], "Pod"),
                   lambda root: root.find("Parts/Part").remove(root.find("Parts/Part/Pod"))]
        for index, mutate in enumerate(changes):
            self.reset()
            self.change_xml("rocket-assembled.xml", mutate)
            with self.subTest(index=index), self.assertRaises(workflow.WorkflowError):
                self.validate()

    def test_xml_normal_connections_require_exact_tree_valid_indices_and_coincident_endpoints(self):
        changes = [lambda root: root.find("Connections").remove(root.find("Connections/Connection")),
                   lambda root: root.find("Connections").append(copy.deepcopy(root.find("Connections/Connection"))),
                   lambda root: setattr(root.find("Connections/Connection"), "tag", "DockConnection"),
                   lambda root: root.find("Connections/Connection").set("parentPart", "99"),
                   lambda root: root.find("Connections/Connection").set("childPart", "1"),
                   lambda root: root.find("Connections/Connection").set("childPart", "3"),
                   lambda root: root.find("Connections/Connection").set("parentAttachPoint", "0"),
                   lambda root: root.find("Connections/Connection").set("childAttachPoint", "99"),
                   lambda root: root.find("Connections/Connection").set("parentAttachPoint", "1"),
                   lambda root: root.find("Connections/Connection").set("childAttachPoint", "2"),
                   lambda root: root.find("Connections/Connection").attrib.pop("parentAttachPoint"),
                   lambda root: root.find("Connections/Connection").set("unknown", "1"),
                   lambda root: root.findall("Parts/Part")[2].set("x", "1.5")]
        for index, mutate in enumerate(changes):
            self.reset()
            self.change_xml("rocket-assembled.xml", mutate)
            with self.subTest(index=index), self.assertRaises(workflow.WorkflowError):
                self.validate()

    def test_xml_geometry_values_must_be_finite_and_flips_binary(self):
        for field, value in (("x", "NaN"), ("y", "inf"), ("angle", "-inf"), ("angle", "bad"),
                             ("flippedX", "true"), ("flippedY", "2"), ("flippedX", "0.0")):
            self.reset()
            self.change_xml("rocket-assembled.xml", lambda root: root.find("Parts/Part").set(field, value))
            with self.subTest(field=field, value=value), self.assertRaises(workflow.WorkflowError):
                self.validate()

    def test_xml_assembled_empty_basic_automatic_manual_empty_card_and_moved_flags(self):
        for name, mutate in (
                ("rocket-assembled.xml", lambda s: ET.SubElement(s, "Step")),
                ("rocket-basic-staging.xml", lambda s: s.set("currentStage", "1")),
                ("rocket-basic-staging.xml", lambda s: s.find("Step/Activate").set("moved", "1")),
                ("rocket-basic-staging.xml", lambda s: s.find("Step/Activate").set("Id", "6")),
                ("rocket-basic-staging.xml", lambda s: s.find("Step/Activate").set("Id", "9.0")),
                ("rocket-manual-staging.xml", lambda s: s.remove(list(s)[1])),
                ("rocket-manual-staging.xml", lambda s: s.find("Step/Activate[@Id='9']").set("moved", "0")),
                ("rocket-manual-staging.xml", lambda s: s.find("Step/Activate").set("moved", "1")),
                ("rocket-manual-staging.xml", lambda s: s.find("Step/Activate").set("moved", "2")),
                ("rocket-manual-staging.xml", lambda s: ET.SubElement(list(s)[1], "Activate", Id="6", moved="1")),
                ("rocket-manual-staging.xml", lambda s: list(s)[2].remove(list(list(s)[2])[1])),
                ("rocket-manual-staging.xml", lambda s: ET.SubElement(s, "Unexpected"))):
            self.reset()
            self.change_xml(name, lambda root: mutate(root.find("Parts/Part/Pod/Staging")))
            with self.subTest(name=name, mutate=mutate), self.assertRaises(workflow.WorkflowError):
                self.validate()

    def test_all_three_xml_geometries_and_connections_must_remain_identical(self):
        for name, mutate in (
                ("rocket-basic-staging.xml", lambda root: [part.set("x", "5.25") for part in root.findall("Parts/Part")]),
                ("rocket-manual-staging.xml", lambda root: root.find("Parts/Part/Pod").set("throttle", "0.5")),
                ("rocket-manual-staging.xml", lambda root: root.find("Parts/Part/Tank").set("fuel", "500")),
                ("rocket-manual-staging.xml", lambda root: root.find("Connections").append(root.find("Connections")[0]))):
            self.reset()
            self.change_xml(name, mutate)
            with self.subTest(name=name), self.assertRaises(workflow.WorkflowError):
                self.validate()

    def test_world_attach_applies_catalog_scale_mirrors_rotation_and_translation(self):
        part = {"x": 3.0, "y": -4.0, "angle": math.pi / 2, "flip_x": 1, "flip_y": 0}
        x, y = workflow.world_attach(part, (4.0, 2.0))
        self.assertAlmostEqual(x, 2.0)
        self.assertAlmostEqual(y, -6.0)
        part["flip_y"] = 1
        self.assertAlmostEqual(workflow.world_attach(part, (4.0, 2.0))[0], 4.0)

    def test_catalog_is_required_and_true_hidden_pod_cannot_be_replaced_by_visible_entry(self):
        for mutate in (lambda root: root.remove(list(root)[0]),
                       lambda root: list(root)[0].set("hidden", "false"),
                       lambda root: list(root)[0].set("type", "engine"),
                       lambda root: list(root)[0].set("height", "NaN"),
                       lambda root: list(root)[0].set("height", "0")):
            write_catalog(self.catalog)
            tree = ET.parse(self.catalog)
            mutate(tree.getroot())
            tree.write(self.catalog, encoding="utf-8", xml_declaration=True)
            with self.subTest(mutate=mutate), self.assertRaises(workflow.WorkflowError):
                self.validate()
        self.catalog.unlink()
        with self.assertRaises(workflow.WorkflowError):
            self.validate()


if __name__ == "__main__":
    unittest.main()
