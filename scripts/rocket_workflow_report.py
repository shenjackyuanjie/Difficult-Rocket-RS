"""16 部件对称分支火箭连续演示的 JSON/XML 证据合同；只读取本次产物与真实部件目录。"""
import json
import math
from pathlib import Path
import re
import xml.etree.ElementTree as ET


CHAPTER_ID = "rocket-workflow"
CASE_IDS = ("rocket-assembly", "basic-staging", "manual-staging-drag")
ARTIFACTS = (
    "editor-rocket-command-pod.png", "editor-rocket-placement-preview.png",
    "editor-rocket-assembled.png", "editor-rocket-generated.png", "editor-rocket-manual.png",
    "rocket-assembled.xml", "rocket-basic-staging.xml", "rocket-manual-staging.xml",
    "rocket-workflow.json",
)
PART_TYPES = ("pod-1", "fueltank-0", "engine-0", "detacher-1", "fueltank-0",
              "engine-0", "detacher-1", "fueltank-0", "engine-0", "detacher-2",
              "engine-4", "fueltank-0", "detacher-2", "engine-4", "fueltank-0", "parachute-1")
# 深度8的 engine9 与 detacher10/13 同层，按 Engine → Detacher 稳定排序。
AUTO_STEPS = [[11, 14], [9], [10, 13], [7], [6], [4], [3], [16]]
MANUAL_STEPS = [[11, 14], [], [10, 13, 9], [7], [6], [4], [3], [16]]
TARGET_IDS = [3, 4, 6, 7, 9, 10, 11, 13, 14, 16]
CONNECTIONS = tuple((i, i + 1, "BottomCenter", "TopCenter") for i in range(1, 9)) + (
    (8, 10, "LeftSide", "RightCenter"), (10, 11, "LeftCenter", "RightSide"),
    (11, 12, "TopCenter", "BottomCenter"), (8, 13, "RightSide", "LeftCenter"),
    (13, 14, "RightCenter", "LeftSide"), (14, 15, "TopCenter", "BottomCenter"),
    (1, 16, "TopCenter", "BottomCenter"),
)
INPUT_MODE = "真实目录命中 / 生产画布鼠标 / egui 分帧拖放"
CATALOG_PATH = Path(__file__).resolve().parents[2] / "Difficult-Rocket/assets/builtin/PartList.xml"
ENDPOINT_TOLERANCE = 1e-5
VISUAL_TOLERANCE = 1e-3


class WorkflowError(ValueError):
    """工作流证据与真实对称分支火箭合同不符。"""


def require(condition, message):
    if not condition:
        raise WorkflowError(message)


def exact(actual, expected):
    """区分 JSON 的布尔、整数、浮点数，避免 True 冒充 1。"""
    if type(actual) is not type(expected):
        return False
    if isinstance(expected, list):
        return len(actual) == len(expected) and all(exact(a, b) for a, b in zip(actual, expected))
    return actual == expected


def fields(document, expected, context):
    require(isinstance(document, dict), f"{context} 必须为对象")
    for name, value in expected.items():
        require(exact(document.get(name), value), f"{context}.{name} 不符合合同")


def finite(value):
    if type(value) not in (int, float):
        return False
    try:
        return math.isfinite(value)
    except OverflowError:
        return False


def vector(value, length, context):
    require(isinstance(value, list) and len(value) == length and all(finite(x) for x in value),
            f"{context} 必须包含 {length} 个有限坐标")
    return value


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        require(key not in result, f"JSON 字段重复：{key}")
        result[key] = value
    return result


def invalid_constant(value):
    raise WorkflowError(f"JSON 包含非有限常量：{value}")


def read_json(path):
    try:
        return json.loads(path.read_text(encoding="utf-8"), object_pairs_hook=unique_object,
                          parse_constant=invalid_constant)
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise WorkflowError(f"工作流 JSON 不可读：{path.name}") from error


def read_xml(path):
    try:
        root = ET.parse(path).getroot()
    except (OSError, ET.ParseError) as error:
        raise WorkflowError(f"工作流 XML 不可读：{path.name}") from error
    for element in root.iter():
        element.tag = element.tag.rsplit("}", 1)[-1]
    return root


def xml_number(element, name, context):
    raw = element.get(name)
    try:
        value = float(raw)
    except (ValueError, TypeError) as error:
        raise WorkflowError(f"{context}.{name} 必须是有限数值") from error
    require(math.isfinite(value), f"{context}.{name} 必须是有限数值")
    return value


def xml_int(element, name, context):
    raw = element.get(name)
    require(isinstance(raw, str) and re.fullmatch(r"-?[0-9]+", raw) is not None,
            f"{context}.{name} 必须是整数")
    return int(raw)


def only(parent, tag, context):
    found = parent.findall(tag)
    require(len(found) == 1, f"{context} 必须包含唯一 {tag}")
    return found[0]


def load_catalog(path):
    root = read_xml(path)
    require(root.tag == "PartTypes", "真实目录根节点必须为 PartTypes")
    definitions = root.findall("PartType")
    visible = [item.get("id") for item in definitions if item.get("hidden", "false") != "true"]
    result = {}
    for part_type, kind in (("pod-1", "pod"), ("fueltank-0", "tank"),
                            ("engine-0", "engine"), ("detacher-1", "detacher"),
                            ("detacher-2", "detacher"), ("engine-4", "engine"),
                            ("parachute-1", "parachute")):
        matches = [item for item in definitions if item.get("id") == part_type]
        require(len(matches) == 1, f"真实目录缺少唯一部件：{part_type}")
        item = matches[0]
        require(item.get("type") == kind, f"真实目录部件类型不符：{part_type}")
        hidden = item.get("hidden", "false") == "true"
        require(hidden == (part_type == "pod-1"), f"真实目录隐藏状态不符：{part_type}")
        width = xml_number(item, "width", part_type)
        height = xml_number(item, "height", part_type)
        require(width > 0 and height > 0, f"真实目录尺寸必须为正：{part_type}")
        locations = {
            "Top": (0, height / 2), "TopSide": (0, height / 2), "TopCenter": (0, height / 2),
            "Bottom": (0, -height / 2), "BottomSide": (0, -height / 2), "BottomCenter": (0, -height / 2),
            "Left": (-width / 2, 0), "LeftSide": (-width / 2, 0), "LeftCenter": (-width / 2, 0),
            "Right": (width / 2, 0), "RightSide": (width / 2, 0), "RightCenter": (width / 2, 0),
        }
        points = []
        for point in only(item, "AttachPoints", part_type):
            require(point.tag == "AttachPoint", f"真实目录连接点格式不符：{part_type}")
            location = point.get("location", "")
            if location:
                require(location in locations, f"真实目录连接点位置未知：{part_type}/{location}")
                points.append(locations[location])
            else:
                points.append(tuple(xml_number(point, name, part_type) if point.get(name) is not None else 0
                                    for name in ("x", "y")))
        require(points, f"真实目录连接点为空：{part_type}")
        sprite = item.get("sprite")
        require(isinstance(sprite, str) and bool(sprite.strip()), f"真实目录贴图不能为空：{part_type}")
        result[part_type] = {"points": points, "index": None if hidden else visible.index(part_type),
                             "sprite": sprite, "width": width, "height": height}
    return result


def world_attach(part, point):
    # dr_core::part_world_attach：目录中心坐标先 /2，再镜像、旋转及平移。
    x, y = point[0] / 2, point[1] / 2
    if part["flip_x"]:
        x = -x
    if part["flip_y"]:
        y = -y
    cosine, sine = math.cos(part["angle"]), math.sin(part["angle"])
    return part["x"] + x * cosine - y * sine, part["y"] + x * sine + y * cosine


def signature(element):
    """除 Pod 分级计划外，三份 XML 的船体、部件及连接均完整一致。"""
    return (element.tag, tuple(sorted(element.attrib.items())), (element.text or "").strip(),
            tuple(signature(child) for child in element if child.tag != "Staging"))


def validate_plan(root, pod, expected, moved_ids, context):
    staging = only(pod, "Staging", context)
    require(len(list(root.iter("Staging"))) == 1, f"{context} 分级计划必须只属于唯一 Pod")
    require(xml_int(staging, "currentStage", context) == 0, f"{context} currentStage 必须为 0")
    require(all(step.tag == "Step" for step in staging), f"{context} 分级计划包含未知节点")
    rows, moved = [], []
    for step in staging:
        require(not step.attrib and all(item.tag == "Activate" for item in step),
                f"{context} Step 必须只包含 Activate")
        row = []
        for activation in step:
            require(set(activation.attrib) == {"Id", "moved"} and len(activation) == 0,
                    f"{context} Activate 字段不符")
            part_id = xml_int(activation, "Id", context)
            flag = xml_int(activation, "moved", context)
            require(flag in (0, 1), f"{context} moved 必须为 0/1")
            row.append(part_id)
            if flag:
                moved.append(part_id)
        rows.append(row)
    require(rows == expected, f"{context} 分级步骤与目标集合不符（必须保留空卡）")
    require(sorted(moved) == moved_ids, f"{context} moved 目标不符")
    return rows


def validate_ship(path, catalog, steps, moved_ids):
    context = path.name
    root = read_xml(path)
    require(root.tag == "Ship", f"{context} 根节点必须为 Ship")
    container = only(root, "Parts", context)
    elements = list(container)
    require(len(elements) == 16 and all(item.tag == "Part" for item in elements)
            and len(list(root.iter("Part"))) == 16, f"{context} 必须恰有 16 个主船体部件")
    require(not list(root.iter("DisconnectedPart")), f"{context} 不能含断开组")
    parts, pod = {}, None
    for element in elements:
        part_id = xml_int(element, "id", context)
        require(part_id in range(1, 17) and part_id not in parts, f"{context} 部件 ID 必须唯一且为 1..16")
        part_type = element.get("partType")
        require(part_type == PART_TYPES[part_id - 1], f"{context} 部件 {part_id} 类型与对称分支顺序不符")
        values = {name: xml_number(element, name, context) for name in ("x", "y", "angle")}
        flip_x = xml_int(element, "flippedX", context)
        flip_y = xml_int(element, "flippedY", context)
        require(flip_x in (0, 1) and flip_y in (0, 1), f"{context} 镜像值必须为 0/1")
        require(values["angle"] == 0 and flip_x == 0 and flip_y == 0,
                f"{context} 所有部件必须 angle0 且无镜像")
        if part_type == "engine-4":
            tank = only(element, "Tank", context)
            require(xml_number(tank, "fuel", context) > 0, f"{context} SRB 必须自带固体燃料 Tank")
        parts[part_id] = dict(values, part_type=part_type, flip_x=flip_x, flip_y=flip_y)
        require(len(element.findall("Pod")) == (1 if part_id == 1 else 0),
                f"{context} 只有部件 1 可以含 Pod")
        if part_id == 1:
            pod = element.find("Pod")
    require(len(list(root.iter("Pod"))) == 1, f"{context} 必须只有一个 Command Pod")
    validate_plan(root, pod, steps, moved_ids, context)
    connections = list(only(root, "Connections", context))
    require(len(connections) == 15 and all(edge.tag == "Connection" for edge in connections)
            and len(list(root.iter("Connection"))) == 15 and not list(root.iter("DockConnection")),
            f"{context} 必须恰有 15 条 Normal 连接")
    edges, maximum_error = set(), 0.0
    for edge in connections:
        require(set(edge.attrib) == {"parentPart", "childPart", "parentAttachPoint", "childAttachPoint"}
                and len(edge) == 0, f"{context} Normal 连接字段不符")
        parent, child = (xml_int(edge, name, context) for name in ("parentPart", "childPart"))
        require(parent in parts and child in parts and (parent, child) not in edges,
                f"{context} 连接引用无效或重复")
        edges.add((parent, child))
        expected = next((item for item in CONNECTIONS if item[:2] == (parent, child)), None)
        require(expected is not None, f"{context} 分支连接方向不符")
        endpoints = []
        for part_id, name, location in ((parent, "parentAttachPoint", expected[2]),
                                        (child, "childAttachPoint", expected[3])):
            index = xml_int(edge, name, context)
            points = catalog[parts[part_id]["part_type"]]["points"]
            require(1 <= index <= len(points), f"{context} 连接点索引越界")
            definition = catalog[parts[part_id]["part_type"]]
            w, h = definition["width"], definition["height"]
            desired = {"TopCenter": (0, h / 2), "BottomCenter": (0, -h / 2),
                       "LeftSide": (-w / 2, 0), "LeftCenter": (-w / 2, 0),
                       "RightSide": (w / 2, 0), "RightCenter": (w / 2, 0)}[location]
            require(points[index - 1] == desired, f"{context} 分支安装点不符：{parent}→{child}")
            endpoints.append(world_attach(parts[part_id], points[index - 1]))
        error = math.dist(*endpoints)
        require(math.isfinite(error) and error < ENDPOINT_TOLERANCE,
                f"{context} Normal 连接端点不一致：{parent}→{child}")
        maximum_error = max(maximum_error, error)
    require(edges == {item[:2] for item in CONNECTIONS}, f"{context} 必须为完整十五边分支树")
    axis = parts[8]["x"]
    for left, right in ((10, 13), (11, 14), (12, 15)):
        require(abs(parts[left]["x"] + parts[right]["x"] - 2 * axis) < ENDPOINT_TOLERANCE
                and abs(parts[left]["y"] - parts[right]["y"]) < ENDPOINT_TOLERANCE
                and parts[left]["x"] < axis < parts[right]["x"],
                f"{context} 左右分支必须几何对称")
    return {"signature": signature(root), "parts": parts, "endpoint_error": maximum_error}


def validate_drags(drags):
    require(isinstance(drags, list) and len(drags) == 6, "manual-staging-drag.drags 必须记录六次真实拖放")
    last_frame = 0
    for drag, kind in zip(drags, ("Activation", "CardDown", "CardUp", "ActivationBack", "Activation", "Activation")):
        fields(drag, {"kind": kind}, "manual-staging-drag.drags")
        source = vector(drag.get("source"), 2, "拖放 source")
        target = vector(drag.get("target"), 2, "拖放 target")
        require(math.dist(source, target) > 0, "拖放源与落点必须不同")
        threshold = drag.get("threshold_distance")
        require(finite(threshold) and threshold == 14, "拖放必须记录真实 14px 越阈值移动")
        events = drag.get("events")
        require(isinstance(events, list) and len(events) == 4, "拖放必须记录四个独立帧事件")
        for event, name in zip(events, ("press", "held_threshold", "held_target", "release")):
            fields(event, {"event": name}, "拖放事件")
            frame = event.get("frame")
            require(type(frame) is int and frame > last_frame,
                    "拖放帧必须为全程严格递增正整数，同帧 click 不算 drag")
            last_frame = frame


def validate_visuals(assembly, parts, catalog):
    """核对主 World 身份与派生材质证据；GPU 纹理绑定、遮挡及排序另由截图诊断。"""
    fields(assembly, {"visual_entities": 16, "visual_stable_frames": 2}, "rocket-assembly")
    visuals = assembly.get("visuals")
    require(isinstance(visuals, list) and len(visuals) == 16,
            "rocket-assembly.visuals 必须记录十六个真实视觉实体")
    entities, path_images, image_paths = set(), {}, {}
    for part_id, observed in enumerate(visuals, 1):
        context = f"rocket-assembly.visuals[{part_id - 1}]"
        part = parts[part_id]
        expected_path = "textures/parts/" + catalog[part["part_type"]]["sprite"]
        fields(observed, {"id": part_id, "group": 0, "occurrence": 0,
                          "part_type": part["part_type"], "expected_path": expected_path,
                          "image_path": expected_path, "material_image_path": expected_path,
                          "image_loaded": True, "image_load_state": "Some(Loaded)",
                          "in_view": True}, context)
        for name in ("entity", "expected_image", "image", "material", "material_image",
                     "sprite_color", "material_color"):
            text = observed.get(name)
            require(isinstance(text, str) and bool(text.strip()) and text == text.strip(),
                    f"{context}.{name} 必须为非空标识或观测字符串")
        entity = observed["entity"]
        require(entity not in entities, f"{context}.entity 重复，十六个部件必须对应十六个唯一实体")
        entities.add(entity)
        image = observed["image"]
        require(observed["expected_image"] == image == observed["material_image"],
                f"{context} 目录、Sprite 与派生材质的三个 image AssetId 必须一致")
        require(path_images.setdefault(expected_path, image) == image
                and image_paths.setdefault(image, expected_path) == expected_path,
                f"{context} 相同目录路径必须对应同一 AssetId，不同图片不能共用 AssetId")
        size = observed.get("image_size")
        require(isinstance(size, list) and len(size) == 2
                and all(type(value) is int and value > 0 for value in size),
                f"{context}.image_size 必须为两个正整数，布尔值不代表像素尺寸")
        expected_xy = (part["x"] * 60, part["y"] * 60)
        for name in ("local_translation", "global_translation"):
            position = vector(observed.get(name), 3, f"{context}.{name}")
            require(math.dist(position[:2], expected_xy) <= VISUAL_TOLERANCE,
                    f"{context}.{name} 的 x/y 与真实 XML 部件坐标乘 60 不符")
        require(observed.get("visibility") in ("Visible", "Inherited"),
                f"{context}.visibility 必须为 Visible 或 Inherited")
        anchor = vector(observed.get("anchor"), 2, f"{context}.anchor")
        material_anchor = vector(observed.get("material_anchor"), 2, f"{context}.material_anchor")
        require(anchor == material_anchor, f"{context} Sprite 与派生材质的 anchor 不符")
        for name in ("sprite_flip", "material_flip"):
            flips = observed.get(name)
            require(isinstance(flips, list) and len(flips) == 2
                    and all(type(value) is bool for value in flips),
                    f"{context}.{name} 必须为两个布尔镜像值")
            require(flips == [bool(part["flip_x"]), bool(part["flip_y"])],
                    f"{context}.{name} 必须与真实 XML 镜像状态一致")
        for name in ("sprite_custom_size", "material_custom_size"):
            require(name in observed, f"{context}.{name} 缺失")
            if observed[name] is not None:
                dimensions = vector(observed[name], 2, f"{context}.{name}")
                require(all(value > 0 for value in dimensions), f"{context}.{name} 必须为正尺寸")
        require(observed["sprite_custom_size"] == observed["material_custom_size"],
                f"{context} Sprite 与派生材质的 custom_size 不符")
        require(observed["sprite_color"] == observed["material_color"],
                f"{context} Sprite 与派生材质的 color 不符")


def validate_workflow(chapter, output, started_ns, catalog_path=None):
    """验证新章全部证据；返回已验证工作流 JSON，供调用方保留报告。"""
    fields(chapter, {"id": CHAPTER_ID, "artifacts": list(ARTIFACTS)}, "火箭章节")
    output = Path(output).resolve()
    for name in ARTIFACTS:
        path = (output / name).resolve()
        require(path.is_relative_to(output) and path.is_file(), f"工作流证据缺失或路径逃逸：{name}")
        stat = path.stat()
        require(stat.st_size > 0 and stat.st_mtime_ns >= started_ns, f"工作流证据为空或陈旧：{name}")
    report = read_json(output / ARTIFACTS[-1])
    fields(report, {"chapter": CHAPTER_ID, "completed": True, "input_mode": INPUT_MODE}, "火箭工作流")
    cases = report.get("cases")
    require(isinstance(cases, list) and len(cases) == len(CASE_IDS), "火箭工作流必须完整包含三个样例")
    metrics = []
    for case, case_id in zip(cases, CASE_IDS):
        fields(case, {"id": case_id, "status": "passed"}, "火箭工作流样例")
        require(isinstance(case.get("metrics"), dict), f"{case_id}.metrics 必须为对象")
        metrics.append(case["metrics"])
    assembly, basic, manual = metrics
    fields(assembly, {"start_parts": 1, "start_connections": 0, "start_staging_steps": 0,
                      "parts": 16, "normal_connections": 15, "engines": 5, "detachers": 4,
                      "physical_stages": 4, "placement_history_steps": 15, "xml_roundtrip": True}, CASE_IDS[0])
    endpoint_error = assembly.get("endpoint_max_error")
    require(finite(endpoint_error) and 0 <= endpoint_error < ENDPOINT_TOLERANCE,
            "rocket-assembly.endpoint_max_error 必须为小于 1e-5 的有限非负数")
    fields(basic, {"steps": AUTO_STEPS, "current_stage": 0, "all_moved_false": True,
                   "draft_only_before_apply": True, "apply_history_delta": 1,
                   "undo_exact": True, "redo_exact": True, "xml_roundtrip": True}, CASE_IDS[1])
    fields(manual, {"steps": MANUAL_STEPS, "moved_ids": [9], "activation_multiset": TARGET_IDS,
                    "draft_only_before_apply": True, "card_reordered": True, "card_restored": True,
                    "cancel_unchanged": True, "apply_history_delta": 1, "undo_exact": True,
                    "redo_exact": True, "xml_roundtrip": True}, CASE_IDS[2])
    validate_drags(manual.get("drags"))
    catalog = load_catalog(Path(catalog_path) if catalog_path is not None else CATALOG_PATH)
    ships = [validate_ship(output / name, catalog, steps, moved)
             for name, steps, moved in zip(ARTIFACTS[5:8], ([], AUTO_STEPS, MANUAL_STEPS), ([], [], [9]))]
    require(all(ship["signature"] == ships[0]["signature"] for ship in ships[1:]),
            "三份火箭 XML 除分级计划外必须具有完全相同的几何、部件属性与连接")
    require(math.isclose(endpoint_error, ships[0]["endpoint_error"], abs_tol=1e-8, rel_tol=1e-4),
            "rocket-assembly.endpoint_max_error 与真实 XML 连接端点误差不符")
    validate_visuals(assembly, ships[0]["parts"], catalog)
    placements = assembly.get("placements")
    require(isinstance(placements, list) and len(placements) == 15, "rocket-assembly.placements 必须记录十五次真实放置")
    for part_id, placement in enumerate(placements, 2):
        part_type = PART_TYPES[part_id - 1]
        fields(placement, {"id": part_id, "part_type": part_type, "catalog_index": catalog[part_type]["index"],
                           "preview_unchanged": True, "normal_snap": True}, "rocket-assembly.placements")
        rect = vector(placement.get("catalog_hit"), 4, "真实目录命中矩形")
        require(rect[0] < rect[2] and rect[1] < rect[3], "真实目录命中矩形必须具有正面积")
        position = vector(placement.get("position"), 2, "真实画布放置位置")
        part = ships[0]["parts"][part_id]
        require(math.dist(position, (part["x"], part["y"])) < ENDPOINT_TOLERANCE,
                f"rocket-assembly.placements 部件 {part_id} 位置与 XML 不符")
    return report
