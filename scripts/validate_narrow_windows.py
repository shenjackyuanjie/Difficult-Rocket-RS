"""启动真实窄窗，核对 Bevy、Win32 客户区与原始 PNG；不注入系统输入。

每个样例都把编辑器复制成唯一进程名副本再启动：桌面工具会按进程名沿用上一次窗口
尺寸，三个样例必须各用自己的进程名，副本与身份记录一并保留为证据。
"""
import argparse
import ctypes
from ctypes import wintypes
import editor_session
from demo_editor import validate_detailed_staging
import json
from pathlib import Path
import struct
import subprocess
import sys
import time


SIZES = [(390, 844), (640, 360), (320, 640)]
TITLE_SUFFIX = "Difficult Rocket Editor"
HANDLE_GONE = "HWND 已失效"
EXIT_CONFIRM_SECONDS = 0.5


def validate_dimensions(size, clients, window, png):
    expected = list(size)
    if not clients or list(clients[-1]) != expected:
        raise ValueError(f"Win32 客户区尺寸不符：{clients}，预期 {expected}")
    for key in ("requested_size", "logical_size", "physical_size"):
        if window.get(key) != expected:
            raise ValueError(f"Bevy {key} 不符：{window.get(key)}，预期 {expected}")
    if window.get("scale_factor") != 1.0 or window.get("stable_size_frames", 0) < 2:
        raise ValueError(f"截图前窗口尺寸或 DPI 尚未稳定：{window}")
    if len(png) < 24 or png[:8] != b"\x89PNG\r\n\x1a\n" or png[12:16] != b"IHDR":
        raise ValueError("原始截图不是带 IHDR 的 PNG")
    actual = struct.unpack(">II", png[16:24])
    if actual != tuple(size):
        raise ValueError(f"原始 PNG 尺寸 {actual}，预期 {size}")
    return actual


def is_editor_window(pid, owned_pid, visible, title, client):
    return (pid == owned_pid and visible and title.endswith(TITLE_SUFFIX)
            and client[0] > 0 and client[1] > 0)


def sample_owned_window(user, handle, owned_pid, elapsed):
    """采样前复核句柄仍属于本次 Popen 的编辑器窗口，再只读客户区。"""
    if not user.IsWindow(handle):
        return None, HANDLE_GONE
    pid = wintypes.DWORD()
    user.GetWindowThreadProcessId(handle, ctypes.byref(pid))
    title = ctypes.create_unicode_buffer(256)
    user.GetWindowTextW(handle, title, len(title))
    if pid.value != owned_pid:
        return None, f"PID 已变为 {pid.value}，本次进程为 {owned_pid}"
    if not title.value.endswith(TITLE_SUFFIX):
        return None, f"标题已变为 {title.value!r}"
    client = wintypes.RECT()
    if not user.GetClientRect(handle, ctypes.byref(client)):
        return None, "无法读取客户区"
    return {"elapsed": round(elapsed, 3), "pid": pid.value, "hwnd": int(handle),
            "title": title.value, "visible": bool(user.IsWindowVisible(handle)),
            "client": [client.right - client.left, client.bottom - client.top]}, None


def confirm_exit(process, timeout=EXIT_CONFIRM_SECONDS):
    """有界确认：窗口先销毁而进程正在正常退出时，不把句柄失效记成所有权丢失。"""
    if process.poll() is not None:
        return True
    try:
        process.wait(timeout=timeout)
    except subprocess.TimeoutExpired:
        return False
    return True


def read_bevy_window(case):
    """尽力读回编辑器写下的窗口 JSON，让失败时的报告也保留编辑器侧观测。"""
    try:
        return json.loads((case / "target/editor-smoke-window.json").read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return None


def record_sample(result, sample):
    """尺寸变化时同步写入兼容观测与详细样本。"""
    observations = result.setdefault("client_observations", [])
    samples = result.setdefault("client_samples", [])
    if observations and observations[-1] == sample["client"]:
        return False
    observations.append(sample["client"])
    samples.append(sample)
    return True


def ownership_lost(result, detail, handle, elapsed):
    """所有权丢失即失败：保留已采样观测，并记录定位所需字段。"""
    samples = result.setdefault("client_samples", [])
    lost = {"elapsed": round(elapsed, 3), "hwnd": int(handle), "detail": detail,
            "last_sample": samples[-1] if samples else None}
    result["ownership_lost"] = lost
    raise RuntimeError(
        f"编辑器窗口所有权丢失：{detail}（本次采集 {lost['elapsed']}s, hwnd={lost['hwnd']}）"
    )


def run_case(editor, assets, output, size, user, callback_type, staging=False):
    width, height = size
    case = output / f"{width}x{height}"
    (case / "target").mkdir(parents=True, exist_ok=True)
    log_path = case / "editor.log"
    result = {"expected_size": list(size), "command": [], "identity": None, "client_observations": [],
              "client_samples": [], "log": str(log_path), "passed": False}
    try:
        identity = editor_session.copy_for_run(editor, case)
        command = [identity["copy_path"], "--assets", str(assets), "--catalog", str(assets / "builtin/PartList.xml")]
        command += ["--staging-smoke-test"] if staging else ["--ship", str(assets / "ships/Test.xml")]
        command += ["--smoke-test", "--smoke-window-size", f"{width}x{height}"]
        result["command"] = command
        result["identity"] = identity
        with log_path.open("w", encoding="utf-8") as log:
            process = subprocess.Popen(command, cwd=case, stdout=log, stderr=subprocess.STDOUT,
                                       creationflags=subprocess.CREATE_NO_WINDOW)
            started = time.monotonic()
            try:
                deadline = started + (190 if staging else 45)
                handle = None
                while time.monotonic() < deadline and process.poll() is None:
                    if handle is None:
                        matches = []

                        @callback_type
                        def callback(hwnd, _):
                            pid = wintypes.DWORD()
                            user.GetWindowThreadProcessId(hwnd, ctypes.byref(pid))
                            title = ctypes.create_unicode_buffer(256)
                            user.GetWindowTextW(hwnd, title, len(title))
                            client = wintypes.RECT()
                            if user.GetClientRect(hwnd, ctypes.byref(client)):
                                dimensions = [client.right - client.left, client.bottom - client.top]
                                if is_editor_window(pid.value, process.pid, user.IsWindowVisible(hwnd),
                                                    title.value, dimensions):
                                    matches.append(hwnd)
                            return True

                        user.EnumWindows(callback, 0)
                        if matches:
                            handle = matches[0]
                    if handle is not None:
                        elapsed = time.monotonic() - started
                        sample, error = sample_owned_window(user, handle, process.pid, elapsed)
                        if error is not None:
                            if process.poll() is not None:
                                break
                            if error == HANDLE_GONE and confirm_exit(process):
                                break
                            ownership_lost(result, error, handle, elapsed)
                        record_sample(result, sample)
                    time.sleep(0.02)
                result["exit_code"] = process.wait(timeout=2)
                if result["exit_code"] != 0:
                    raise RuntimeError(f"编辑器退出码 {result['exit_code']}，查看 {log_path}")
            finally:
                if process.poll() is None:
                    process.terminate()
                    process.wait(timeout=5)
        screenshot = case / ("target/editor-staging-draft.png" if staging else "target/editor-smoke.png")
        window = json.loads((case / "target/editor-smoke-window.json").read_text(encoding="utf-8"))
        result["bevy_window"] = window
        png = screenshot.read_bytes()
        dimensions = validate_dimensions(size, result["client_observations"], window, png)
        if staging:
            validate_detailed_staging({"artifacts": ["editor-staging-performance.json"]}, case / "target")
            result["staging"] = json.loads((case / "target/editor-staging-performance.json").read_text(encoding="utf-8"))
        result.update(screenshot=str(screenshot), png_size=list(dimensions), bytes=len(png), passed=True)
    except Exception as error:
        result["error"] = str(error)
        window = read_bevy_window(case)
        if window is not None:
            result["bevy_window"] = window
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--editor", type=Path, required=True)
    parser.add_argument("--assets", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--staging", action="store_true", help="同一窄窗执行 64 级/1024 动作、目标可见与历史/XML 验收")
    args = parser.parse_args()
    if sys.platform != "win32":
        parser.error("Win32 客户区交叉验收需要在 Windows 运行")
    editor, assets, output = (path.resolve() for path in (args.editor, args.assets, args.output))
    if not editor.is_file() or not (assets / "builtin/PartList.xml").is_file():
        parser.error("编辑器或部件目录不存在")
    output.mkdir(parents=True, exist_ok=False)
    user = ctypes.WinDLL("user32", use_last_error=True)
    callback_type = ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.HWND, wintypes.LPARAM)
    user.EnumWindows.argtypes = [callback_type, wintypes.LPARAM]
    user.GetWindowThreadProcessId.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.DWORD)]
    user.IsWindow.argtypes = [wintypes.HWND]
    user.IsWindow.restype = wintypes.BOOL
    user.IsWindowVisible.argtypes = [wintypes.HWND]
    user.GetWindowTextW.argtypes = [wintypes.HWND, wintypes.LPWSTR, ctypes.c_int]
    user.GetClientRect.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.RECT)]
    user.SetProcessDpiAwarenessContext.argtypes = [ctypes.c_void_p]
    user.SetProcessDpiAwarenessContext(ctypes.c_void_p(-4))
    report = {"cases": [], "physical_touch_measured": False,
              "input_path": "初始 Bevy 窗口尺寸；仅只读测量 Win32 客户区，无外部改窗、键鼠或触摸注入",
              "staging_mode": args.staging}
    for size in SIZES:
        report["cases"].append(run_case(editor, assets, output, size, user, callback_type, args.staging))
        report["passed"] = all(case["passed"] for case in report["cases"])
        (output / "report.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        if not report["passed"]:
            break
    print(json.dumps(report, ensure_ascii=False, indent=2))
    return 0 if report["passed"] and len(report["cases"]) == len(SIZES) else 1


if __name__ == "__main__":
    raise SystemExit(main())
