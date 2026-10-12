"""启动真实窄窗，核对 Bevy、Win32 客户区与原始 PNG；不注入系统输入。"""
import argparse
import ctypes
from ctypes import wintypes
import json
from pathlib import Path
import struct
import subprocess
import sys
import time


SIZES = [(390, 844), (640, 360), (320, 640)]


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
    return (pid == owned_pid and visible and title.endswith("Difficult Rocket Editor")
            and client[0] > 0 and client[1] > 0)


def run_case(editor, assets, output, size, user, callback_type):
    width, height = size
    case = output / f"{width}x{height}"
    (case / "target").mkdir(parents=True)
    log_path = case / "editor.log"
    command = [str(editor), "--assets", str(assets), "--catalog", str(assets / "builtin/PartList.xml"),
               "--ship", str(assets / "ships/Test.xml"), "--smoke-test", "--smoke-window-size", f"{width}x{height}"]
    result = {"expected_size": list(size), "command": command, "client_observations": [],
              "log": str(log_path), "passed": False}
    try:
        with log_path.open("w", encoding="utf-8") as log:
            process = subprocess.Popen(command, cwd=case, stdout=log, stderr=subprocess.STDOUT,
                                       creationflags=subprocess.CREATE_NO_WINDOW)
            try:
                deadline = time.monotonic() + 45
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
                        client = wintypes.RECT()
                        if user.GetClientRect(handle, ctypes.byref(client)):
                            actual = [client.right - client.left, client.bottom - client.top]
                            observations = result["client_observations"]
                            if not observations or observations[-1] != actual:
                                observations.append(actual)
                    time.sleep(0.02)
                result["exit_code"] = process.wait(timeout=2)
                if result["exit_code"] != 0:
                    raise RuntimeError(f"编辑器退出码 {result['exit_code']}，查看 {log_path}")
            finally:
                if process.poll() is None:
                    process.terminate()
                    process.wait(timeout=5)
        screenshot = case / "target/editor-smoke.png"
        window = json.loads((case / "target/editor-smoke-window.json").read_text(encoding="utf-8"))
        result["bevy_window"] = window
        png = screenshot.read_bytes()
        dimensions = validate_dimensions(size, result["client_observations"], window, png)
        result.update(screenshot=str(screenshot), png_size=list(dimensions), bytes=len(png), passed=True)
    except Exception as error:
        result["error"] = str(error)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--editor", type=Path, required=True)
    parser.add_argument("--assets", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
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
    user.IsWindowVisible.argtypes = [wintypes.HWND]
    user.GetWindowTextW.argtypes = [wintypes.HWND, wintypes.LPWSTR, ctypes.c_int]
    user.GetClientRect.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.RECT)]
    user.SetProcessDpiAwarenessContext.argtypes = [ctypes.c_void_p]
    user.SetProcessDpiAwarenessContext(ctypes.c_void_p(-4))
    report = {"cases": [], "physical_touch_measured": False,
              "input_path": "初始 Bevy 窗口尺寸；仅只读测量 Win32 客户区，无外部改窗、键鼠或触摸注入"}
    for size in SIZES:
        report["cases"].append(run_case(editor, assets, output, size, user, callback_type))
        report["passed"] = all(case["passed"] for case in report["cases"])
        (output / "report.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        if not report["passed"]:
            break
    print(json.dumps(report, ensure_ascii=False, indent=2))
    return 0 if report["passed"] and len(report["cases"]) == len(SIZES) else 1


if __name__ == "__main__":
    raise SystemExit(main())
