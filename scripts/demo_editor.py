"""单窗口简略/详细演示驱动：内部输入注入展示，不代表系统键鼠或前台验收。

默认完成后保留编辑器；--fast 固定 40ms 且自动退出，仅供 UI 回归。
只读取原版样本并校验 SHA256，不保存或恢复覆盖任何原版文件。
"""
import argparse
from datetime import datetime
import hashlib
import json
import math
from pathlib import Path, PureWindowsPath
import subprocess
import sys
import time


ROOT = Path(__file__).resolve().parents[1]
CHAPTER_IDS = (
    "panels", "connections", "transforms", "selection", "view", "staging",
    "topology", "repair", "browser", "unsaved",
)
DETAILED_CHAPTER_IDS = (
    "panels", "placement-cases", "connections", "geometry-cases", "transforms",
    "free-cases", "group-cases", "line-cases", "selection", "view", "staging",
    "history-cases", "topology", "topology-cases", "repair", "browser", "unsaved", "file-cases",
)
DETAILED_CASE_IDS = {
    "placement-cases": ("palette-cancel", "quantity-limit", "disabled-rotation"),
    "geometry-cases": ("overlap-below", "overlap-exact", "overlap-above", "overlap-rotated-mirrors",
                       "overlap-relative-45", "overlap-contained"),
    "free-cases": ("manual-same", "manual-self", "manual-escape", "manual-blank", "manual-stale",
                   "manual-type-mismatch", "manual-reverse-unlink"),
    "group-cases": ("selection-empty", "selection-single", "selection-disconnected", "mirrors-involution",
                    "transform-order", "descendants-no-ancestor"),
    "line-cases": ("line-width-min", "line-width-max", "line-reset", "line-zero-length", "line-hidden-endpoint"),
    "history-cases": ("history-empty", "history-batch-rollback", "history-new-branch", "fuel-nan", "fuel-negative",
                      "fuel-over-capacity", "properties-stale"),
    "topology-cases": ("topology-self", "topology-tree-cycle", "topology-graph-cycle", "topology-cross-group"),
    "file-cases": ("file-invalid-open", "file-save-failure", "file-modal-cancel"),
}


def chapter_ids(mode):
    if mode == "brief":
        return CHAPTER_IDS
    if mode == "detailed":
        return DETAILED_CHAPTER_IDS
    raise DemoError(f"未知演示模式：{mode!r}")


REPORT_NAME = "demo-report.json"


class DemoError(RuntimeError):
    """演示未完成或证据不符合契约。"""


def positive_int(value):
    try:
        number = int(value)
    except ValueError as error:
        raise argparse.ArgumentTypeError("必须是正整数") from error
    if number <= 0:
        raise argparse.ArgumentTypeError("必须是正整数")
    return number


def positive_seconds(value):
    try:
        number = float(value)
    except ValueError as error:
        raise argparse.ArgumentTypeError("必须是有限正数") from error
    if not math.isfinite(number) or number <= 0:
        raise argparse.ArgumentTypeError("必须是有限正数")
    return number


def parse_args(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--editor-bin", type=Path, help="编辑器路径，默认 target/debug/dr-editor.exe")
    parser.add_argument("--no-build", action="store_true", help="不自动构建默认编辑器")
    parser.add_argument("--mode", choices=("brief", "detailed"), default="brief",
                        help="brief 简略（原有节奏）；detailed 详细（含边界样例），默认 brief")
    parser.add_argument("--step-ms", type=positive_int, default=450, help="每动作间隔毫秒，默认 450")
    parser.add_argument("--fast", action="store_true", help="40ms UI 回归模式，强制自动退出")
    parser.add_argument("--exit-on-complete", action="store_true", help="演示完成后自动关闭编辑器")
    parser.add_argument("--output", type=Path, help="输出目录，默认 target/demo-时间戳")
    parser.add_argument("--timeout", type=positive_seconds,
                        help="启动后的超时秒数；简略默认 600，详细默认不限时")
    options = parser.parse_args(argv)
    if options.timeout is None and options.mode == "brief":
        options.timeout = 600
    if options.fast:
        options.step_ms = 40
        options.exit_on_complete = True
    return options


def sample_hashes(root):
    samples = root.parent / "Difficult-Rocket/assets/ships"
    return {
        (samples / name).resolve(): hashlib.sha256((samples / name).read_bytes()).hexdigest()
        for name in ("Test.xml", "Heronb.xml")
    }


def check_samples(original):
    changed = []
    for path, digest in original.items():
        try:
            current = hashlib.sha256(path.read_bytes()).hexdigest()
        except OSError:
            current = None
        if current != digest:
            changed.append(str(path))
    if changed:
        raise DemoError("原版样本被修改或丢失（不会覆盖恢复）：" + "、".join(changed))


def needs_build(root, binary):
    if not binary.is_file():
        return True
    built_ns = binary.stat().st_mtime_ns
    inputs = [root / name for name in ("Cargo.toml", "Cargo.lock", "rust-toolchain", "rust-toolchain.toml")]
    for folder in ("crates", "vendor", ".cargo"):
        inputs.extend(path for path in (root / folder).rglob("*") if path.is_file())
    return any(path.is_file() and path.stat().st_mtime_ns > built_ns for path in inputs)


def prepare_binary(options, root):
    default_binary = (root / "target/debug/dr-editor.exe").resolve()
    binary = (options.editor_bin or default_binary).resolve()
    if binary == default_binary and not options.no_build and needs_build(root, binary):
        print("默认编辑器缺失或源文件更新，正在 cargo build -p dr-editor……", flush=True)
        subprocess.run(["cargo", "build", "-p", "dr-editor"], cwd=root, check=True)
    if not binary.is_file():
        raise DemoError(f"编辑器不存在：{binary}；请先构建或指定 --editor-bin")
    return binary


def validate_report(report, output, started_ns, expected_mode=None):
    if not isinstance(report, dict) or report.get("completed") is not True:
        raise DemoError("报告未声明 completed: true")
    mode = report.get("mode", "brief")  # 兼容既有十章视频报告；详细报告必须显式标识。
    expected = chapter_ids(mode)
    if expected_mode is not None and mode != expected_mode:
        raise DemoError(f"报告模式 {mode!r} 与请求的 {expected_mode!r} 不符")
    chapters = report.get("chapters")
    if not isinstance(chapters, list) or len(chapters) != len(expected):
        raise DemoError(f"{mode} 报告必须包含完整 {len(expected)} 章")
    seen = set()
    for index, chapter in enumerate(chapters):
        if not isinstance(chapter, dict):
            raise DemoError("章节必须为对象")
        chapter_id = chapter.get("id")
        if not isinstance(chapter_id, str) or chapter_id not in expected or chapter_id in seen:
            raise DemoError(f"章节 ID 未知或重复：{chapter_id!r}")
        if chapter_id != expected[index]:
            raise DemoError(f"章节顺序不符：预期 {expected[index]}，实际 {chapter_id}")
        seen.add(chapter_id)
        title = chapter.get("title")
        if not isinstance(title, str) or not title.strip():
            raise DemoError(f"章节 {chapter_id} 缺少标题")
        if chapter.get("status") != "passed":
            raise DemoError(f"章节 {chapter_id} 未通过")
        elapsed = chapter.get("elapsed_seconds")
        if (type(elapsed) not in (int, float) or not math.isfinite(elapsed) or elapsed < 0):
            raise DemoError(f"章节 {chapter_id} 耗时无效")
        artifacts = chapter.get("artifacts")
        if not isinstance(artifacts, list):
            raise DemoError(f"章节 {chapter_id} 缺少 artifacts 列表")
        for name in artifacts:
            if not isinstance(name, str) or not name.strip():
                raise DemoError(f"章节 {chapter_id} 的 artifact 文件名无效")
            relative = Path(name.replace("\\", "/"))
            windows_path = PureWindowsPath(name)
            if relative.is_absolute() or windows_path.root or windows_path.drive or ".." in relative.parts:
                raise DemoError(f"artifact 必须是输出目录内相对路径：{name}")
            artifact = (output / relative).resolve()
            if not artifact.is_relative_to(output.resolve()):
                raise DemoError(f"artifact 路径逃逸输出目录：{name}")
            if not artifact.is_file():
                raise DemoError(f"声明的 artifact 不存在（不得虚报截图）：{name}")
            stat = artifact.stat()
            if stat.st_size == 0 or stat.st_mtime_ns < started_ns:
                raise DemoError(f"artifact 为空或不是本次新产物：{name}")
        if mode == "detailed" and chapter_id in DETAILED_CASE_IDS:
            validate_cases(chapter, output)
    return report


def validate_cases(chapter, output):
    """详细模式不能用十章简略报告或空边界清单冒充通过。"""
    chapter_id = chapter["id"]
    filename = chapter_id + ".json"
    if filename not in chapter["artifacts"]:
        raise DemoError(f"详细章节 {chapter_id} 缺少独立样例报告")
    try:
        report = json.loads((output / filename).read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise DemoError(f"详细样例报告不可读：{chapter_id}") from error
    if not isinstance(report, dict) or report.get("completed") is not True or report.get("chapter") != chapter_id:
        raise DemoError(f"详细样例报告身份或完成状态不符：{chapter_id}")
    expected = DETAILED_CASE_IDS[chapter_id]
    cases = report.get("cases")
    if not isinstance(cases, list) or len(cases) != len(expected):
        raise DemoError(f"详细章节 {chapter_id} 必须包含完整 {len(expected)} 个样例")
    for case_id, case in zip(expected, cases):
        if not isinstance(case, dict) or case.get("id") != case_id or case.get("status") != "passed":
            raise DemoError(f"详细样例未通过、重复或顺序不符：{case_id}")
        if any(not isinstance(case.get(key), str) or not case[key].strip() for key in ("title", "expected", "input_path")):
            raise DemoError(f"详细样例缺少说明或真实输入路径：{case_id}")
        screenshot = "case-" + case_id + ".png"
        if case.get("screenshot") != screenshot or screenshot not in chapter["artifacts"]:
            raise DemoError(f"详细样例缺少声明的独立截图：{case_id}")
        if not isinstance(case.get("metrics"), dict):
            raise DemoError(f"详细样例缺少实际验收数据：{case_id}")


def read_completed_report(output, started_ns, expected_mode=None):
    path = output / REPORT_NAME
    if not path.is_file() or path.stat().st_mtime_ns < started_ns:
        return None
    try:
        report = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError:
        return None  # 允许非原子写入的短暂中间态；超时或提前退出仍失败。
    if isinstance(report, dict) and report.get("completed") is False:
        return None
    return validate_report(report, output, started_ns, expected_mode)


def stop_owned_process(process):
    """仅清理本脚本创建的 Popen，不枚举或结束其它编辑器。"""
    if process is not None and process.poll() is None:
        process.terminate()
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait(timeout=5)


def wait_for_demo(process, output, started_ns, timeout, exit_on_complete, expected_mode=None):
    deadline = time.monotonic() + timeout if timeout is not None else None
    while True:
        code = process.poll()
        if code is not None and code != 0:
            raise DemoError(f"编辑器异常退出，退出码 {code}")
        report = read_completed_report(output, started_ns, expected_mode)
        if report is not None:
            if exit_on_complete:
                remaining = deadline - time.monotonic() if deadline is not None else None
                if remaining is not None and remaining <= 0:
                    raise DemoError("演示报告完成，但等待自动退出超时")
                try:
                    code = process.wait(timeout=remaining)
                except subprocess.TimeoutExpired as error:
                    raise DemoError("演示报告完成，但等待自动退出超时") from error
            else:
                code = process.poll()
            if code is not None and code != 0:
                raise DemoError(f"编辑器异常退出，退出码 {code}")
            return report
        if code is not None:
            raise DemoError(f"编辑器提前退出，退出码 {code}，缺少本次完整报告（可能手动关闭）")
        if deadline is not None and time.monotonic() >= deadline:
            raise DemoError(f"演示等待超时（{timeout:g} 秒），缺少本次完整报告")
        time.sleep(0.1)


def run_demo(options, root=ROOT):
    root = root.resolve()
    original = sample_hashes(root)
    process = None
    succeeded = False
    try:
        binary = prepare_binary(options, root)
        default_output = root / "target" / ("demo-" + datetime.now().strftime("%Y%m%d-%H%M%S-%f"))
        output = (options.output or default_output).resolve()
        if any(path.is_relative_to(output) for path in original):
            raise DemoError("输出目录不得包含受保护的原版样本，请改用独立演示目录")
        output.mkdir(parents=True, exist_ok=True)
        command = [str(binary), "--demo-showcase", str(output), "--demo-mode", options.mode,
                   "--demo-step-ms", str(options.step_ms)]
        if options.exit_on_complete:
            command.append("--demo-exit-on-complete")
        label = "详细" if options.mode == "detailed" else "简略"
        count = len(chapter_ids(options.mode))
        print(f"启动单窗口{label}演示（{count} 章）：动作间隔 {options.step_ms}ms；输出 {output}", flush=True)
        print("仅内部输入注入，无系统键鼠操作或前台抢占。", flush=True)
        with (output / "demo-editor.log").open("w", encoding="utf-8") as log:
            started_ns = time.time_ns()
            process = subprocess.Popen(command, cwd=root, stdout=log, stderr=log)
            report = wait_for_demo(process, output, started_ns, options.timeout,
                                   options.exit_on_complete, options.mode)
        check_samples(original)
        for chapter in report["chapters"]:
            print(f"[通过] {chapter['id']} · {chapter['title']}：{chapter['elapsed_seconds']:.2f} 秒，产物 {len(chapter['artifacts'])} 个")
        print(f"{label}模式 {count} 章演示完成，Test.xml / Heronb.xml SHA256 未改变。报告：{output / REPORT_NAME}")
        if not options.exit_on_complete and process.poll() is None:
            print("主窗口保留展示结果；脚本不会终止该进程。")
        succeeded = True
        return report
    finally:
        if not succeeded:
            stop_owned_process(process)
        try:
            check_samples(original)
        except DemoError:
            if succeeded:
                stop_owned_process(process)
            raise


def main(argv=None):
    options = parse_args(argv)
    try:
        run_demo(options)
    except (DemoError, OSError, subprocess.SubprocessError) as error:
        print(f"演示失败：{error}", file=sys.stderr)
        return 1
    except KeyboardInterrupt:
        print("演示已中断，仅清理本脚本启动的进程。", file=sys.stderr)
        return 130
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
