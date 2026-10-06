"""Windows 前台窗口验收：仅操作自己启动的编辑器和失焦辅助窗口。

默认使用 SendInput/系统鼠标验证完整输入链；--window-case 可在受控前台运行
已有注入式窗口回归和性能测试。不会向其他进程发快捷键，也不永久置顶窗口。
"""
import argparse
import ctypes
from ctypes import wintypes
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import time


class MOUSEINPUT(ctypes.Structure):
    _fields_ = [("dx", wintypes.LONG), ("dy", wintypes.LONG),
                ("mouseData", wintypes.DWORD), ("dwFlags", wintypes.DWORD),
                ("time", wintypes.DWORD), ("dwExtraInfo", ctypes.c_size_t)]


class KEYBDINPUT(ctypes.Structure):
    _fields_ = [("wVk", wintypes.WORD), ("wScan", wintypes.WORD),
                ("dwFlags", wintypes.DWORD), ("time", wintypes.DWORD),
                ("dwExtraInfo", ctypes.c_size_t)]


class HARDWAREINPUT(ctypes.Structure):
    _fields_ = [("uMsg", wintypes.DWORD), ("wParamL", wintypes.WORD), ("wParamH", wintypes.WORD)]


class INPUTDATA(ctypes.Union):
    _fields_ = [("mi", MOUSEINPUT), ("ki", KEYBDINPUT), ("hi", HARDWAREINPUT)]


class INPUT(ctypes.Structure):
    _anonymous_ = ("data",)
    _fields_ = [("type", wintypes.DWORD), ("data", INPUTDATA)]


WINDOW_ARTIFACTS = {
    "panels": "editor-panels-smoke.png", "browser": "editor-browser-smoke.png",
    "egui": "editor-egui-smoke.png", "staging": "editor-staging-smoke.png",
    "repair": "editor-repair-smoke.png", "native-ime": "editor-native-ime.png",
    "connections": "editor-connections-smoke.png", "scoped": "editor-scoped-smoke.png",
    "selection": "editor-selection-smoke.png", "view": "editor-view-smoke.png",
    "topology": "editor-topology-smoke.png", "performance": "editor-performance.json",
    "unsaved": "editor-unsaved-modal.png",
}


def assert_no_segmentation_warnings(log_path):
    """窗口成功退出后仍检查已知 ICU4X 告警，防止日志刷屏悄悄回归。"""
    text = log_path.read_text(encoding="utf-8", errors="replace")
    assert "No segmentation model" not in text, f"ICU4X 缺失模型告警再次出现：{log_path}"


def completed_window_case(root, case, started_ns, exit_code):
    """只接受正常退出和本次新生成的最终产物，不把提前关闭当作验收成功。"""
    if exit_code != 0 or case not in WINDOW_ARTIFACTS:
        return False
    try:
        return (root / "target" / WINDOW_ARTIFACTS[case]).stat().st_mtime_ns >= started_ns
    except OSError:
        return False


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--window-case", choices=["native", "performance", "panels", "browser", "egui", "staging", "repair", "native-ime", "connections", "scoped", "selection", "view", "topology", "keys", "unsaved"], default="native")
    parser.add_argument("--selection-count", type=int, default=1)
    parser.add_argument("--editor-bin", type=Path, help="指定独立编辑器产物，避免覆盖正在运行的默认程序")
    options = parser.parse_args()
    if sys.platform != "win32":
        raise SystemExit("此自测仅适用于 Windows")
    root = Path(__file__).resolve().parents[1]
    run = Path(tempfile.mkdtemp(prefix="foreground-", dir=root / "target"))
    user = ctypes.WinDLL("user32", use_last_error=True)
    callback_type = ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.HWND, wintypes.LPARAM)
    signatures = {
        "EnumWindows": ([callback_type, wintypes.LPARAM], wintypes.BOOL),
        "GetWindowThreadProcessId": ([wintypes.HWND, ctypes.POINTER(wintypes.DWORD)], wintypes.DWORD),
        "GetWindowTextW": ([wintypes.HWND, wintypes.LPWSTR, ctypes.c_int], ctypes.c_int),
        "GetForegroundWindow": ([], wintypes.HWND),
        "SetForegroundWindow": ([wintypes.HWND], wintypes.BOOL),
        "IsWindow": ([wintypes.HWND], wintypes.BOOL),
        "IsWindowVisible": ([wintypes.HWND], wintypes.BOOL),
        "IsIconic": ([wintypes.HWND], wintypes.BOOL),
        "ShowWindow": ([wintypes.HWND, ctypes.c_int], wintypes.BOOL),
        "GetWindowRect": ([wintypes.HWND, ctypes.POINTER(wintypes.RECT)], wintypes.BOOL),
        "WindowFromPoint": ([wintypes.POINT], wintypes.HWND),
        "GetAncestor": ([wintypes.HWND, wintypes.UINT], wintypes.HWND),
        "GetDpiForWindow": ([wintypes.HWND], wintypes.UINT),
        "SetWindowPos": ([wintypes.HWND, wintypes.HWND, ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_int, wintypes.UINT], wintypes.BOOL),
        "ClientToScreen": ([wintypes.HWND, ctypes.POINTER(wintypes.POINT)], wintypes.BOOL),
        "SetCursorPos": ([ctypes.c_int, ctypes.c_int], wintypes.BOOL),
        "GetAsyncKeyState": ([ctypes.c_int], ctypes.c_short),
        "SendInput": ([wintypes.UINT, ctypes.POINTER(INPUT), ctypes.c_int], wintypes.UINT),
        "CreateWindowExW": ([wintypes.DWORD, wintypes.LPCWSTR, wintypes.LPCWSTR, wintypes.DWORD, ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_int, wintypes.HWND, wintypes.HMENU, wintypes.HINSTANCE, wintypes.LPVOID], wintypes.HWND),
        "DestroyWindow": ([wintypes.HWND], wintypes.BOOL),
        "UpdateWindow": ([wintypes.HWND], wintypes.BOOL),
    }
    for name, (args, result) in signatures.items():
        function = getattr(user, name)
        function.argtypes, function.restype = args, result
    user.SetProcessDpiAwarenessContext.argtypes = [wintypes.HANDLE]
    user.SetProcessDpiAwarenessContext(ctypes.c_void_p(-4))
    samples = [root / "../Difficult-Rocket/assets/ships/Test.xml", root / "../Difficult-Rocket/assets/ships/Heronb.xml"]
    hashes = [hashlib.sha256(path.read_bytes()).digest() for path in samples]
    args = [str(options.editor_bin.resolve() if options.editor_bin else root / "target/debug/dr-editor.exe")]
    if options.window_case in ["native", "keys"]:
        args += ["--native-input-test" if options.window_case == "native" else "--native-keys-test", str(run)]
    elif options.window_case == "performance":
        args += ["--ship", str(root / "../Difficult-Rocket/assets/ships/Ophioglossum.xml"), "--performance-test", "--performance-selection-count", str(options.selection_count)]
    else:
        flag = {"panels": "panel-smoke-test", "connections": "connection-smoke-test", "native-ime": "native-ime-test", "unsaved": "native-dialog-test"}.get(options.window_case, options.window_case + "-smoke-test")
        if options.window_case in ["panels", "native-ime", "repair", "unsaved"]:
            args += ["--ship", str(samples[1 if options.window_case == "repair" else 0])]
        args += ["--" + flag]
    held = set()
    mouse_held = False
    helper = None
    foreground_checks = 0
    events = []
    started_ns = time.time_ns()
    with (run / "editor.log").open("w", encoding="utf-8") as log:
        process = subprocess.Popen(args, cwd=root, stdout=log, stderr=log, creationflags=subprocess.CREATE_NO_WINDOW)

        def find_window():
            found = []
            @callback_type
            def collect(hwnd, _):
                pid = wintypes.DWORD()
                user.GetWindowThreadProcessId(hwnd, ctypes.byref(pid))
                if pid.value == process.pid and user.IsWindowVisible(hwnd):
                    title = ctypes.create_unicode_buffer(256)
                    user.GetWindowTextW(hwnd, title, len(title))
                    if "Difficult Rocket Editor" in title.value:
                        found.append(hwnd)
                return True
            user.EnumWindows(collect, 0)
            return found[0] if found else None

        def wait_for(predicate, reason, timeout=20):
            end = time.monotonic() + timeout
            while time.monotonic() < end:
                value = predicate()
                if value:
                    return value
                if process.poll() is not None:
                    if process.returncode != 0:
                        raise AssertionError(f"编辑器运行失败：退出码 {process.returncode}；日志 {run}")
                    raise AssertionError(f"编辑器提前退出：{reason}，退出码 {process.returncode}；日志 {run}")
                time.sleep(0.02)
            raise AssertionError(f"等待超时：{reason}；日志 {run}")

        def activate(hwnd):
            user.ShowWindow(hwnd, 9 if user.IsIconic(hwnd) else 5)
            user.SetForegroundWindow(hwnd)
            if user.GetForegroundWindow() != hwnd:
                # Windows 可能拒绝后台线程直接激活。短暂抬高测试窗口，只点击
                # 经 WindowFromPoint 确认归属的标题栏，不能用 Alt 等按键打扰其他应用。
                flags = 0x13  # SWP_NOSIZE | SWP_NOMOVE | SWP_NOACTIVATE
                try:
                    if not user.SetWindowPos(hwnd, ctypes.c_void_p(-1), 0, 0, 0, 0, flags):
                        raise ctypes.WinError(ctypes.get_last_error())
                    rect = wintypes.RECT()
                    if not user.GetWindowRect(hwnd, ctypes.byref(rect)):
                        raise ctypes.WinError(ctypes.get_last_error())
                    scale = user.GetDpiForWindow(hwnd) / 96
                    point = wintypes.POINT(rect.left + round(120 * scale), rect.top + round(15 * scale))
                    if user.GetAncestor(user.WindowFromPoint(point), 2) != hwnd:
                        raise AssertionError("激活点击点不属于测试窗口，拒绝发鼠标输入")
                    if not user.SetCursorPos(point.x, point.y):
                        raise ctypes.WinError(ctypes.get_last_error())
                    batch = (INPUT * 2)(INPUT(type=0, mi=MOUSEINPUT(dwFlags=2)), INPUT(type=0, mi=MOUSEINPUT(dwFlags=4)))
                    if user.SendInput(2, batch, ctypes.sizeof(INPUT)) != 2:
                        raise ctypes.WinError(ctypes.get_last_error())
                    wait_for(lambda: user.GetForegroundWindow() == hwnd, "系统拒绝将测试窗口置前", timeout=3)
                finally:
                    user.SetWindowPos(hwnd, ctypes.c_void_p(-2), 0, 0, 0, 0, flags)
            wait_for(lambda: user.GetForegroundWindow() == hwnd, "系统前台确认", timeout=3)

        def check_front(hwnd):
            nonlocal foreground_checks
            if process.poll() is not None:
                raise AssertionError(f"编辑器运行失败：退出码 {process.returncode}；日志 {run}")
            if not user.IsWindow(hwnd):
                code = process.wait(timeout=3)
                raise AssertionError(f"编辑器窗口已销毁：退出码 {code}；日志 {run}")
            if user.GetForegroundWindow() != hwnd:
                # 窗口崩溃/销毁会同时失焦，先检查退出状态，不能把程序断言当焦点重试。
                time.sleep(0.1)
                if process.poll() is not None:
                    raise AssertionError(f"编辑器运行失败：退出码 {process.returncode}；日志 {run}")
                raise AssertionError("测试窗口已失去真实前台，停止发输入，不能把内部 focused 当作系统焦点")
            if user.IsIconic(hwnd):
                raise AssertionError("测试窗口被最小化")
            foreground_checks += 1

        def send(inputs, hwnd):
            check_front(hwnd)
            batch = (INPUT * len(inputs))(*inputs)
            if user.SendInput(len(batch), batch, ctypes.sizeof(INPUT)) != len(batch):
                raise ctypes.WinError(ctypes.get_last_error())

        def key(vk, down, hwnd):
            # Delete/Home 属于扩展键；直接发送虚拟键时可能被解释成小键盘键，
            # 因此明确发送 E0 扫描码，让 Bevy 收到稳定的物理键事件。
            extended_scan = {0x24: 0x47, 0x2E: 0x53}.get(vk)
            if extended_scan is not None:
                flags = 0x0008 | 0x0001 | (0 if down else 0x0002)
                keyboard = KEYBDINPUT(wVk=0, wScan=extended_scan, dwFlags=flags)
            else:
                keyboard = KEYBDINPUT(wVk=vk, dwFlags=0 if down else 0x0002)
            send([INPUT(type=1, ki=keyboard)], hwnd)
            if down:
                held.add(vk)
            else:
                held.discard(vk)

        def chord(vk, hwnd, control=False, shift=False):
            if control:
                key(0x11, True, hwnd)
            if shift:
                key(0x10, True, hwnd)
            key(vk, True, hwnd)
            time.sleep(0.09)
            key(vk, False, hwnd)
            if shift:
                key(0x10, False, hwnd)
            if control:
                key(0x11, False, hwnd)
            time.sleep(0.12)

        def move(hwnd, x, y):
            check_front(hwnd)
            point = wintypes.POINT(round(x), round(y))
            if not user.ClientToScreen(hwnd, ctypes.byref(point)) or not user.SetCursorPos(point.x, point.y):
                raise ctypes.WinError(ctypes.get_last_error())
            time.sleep(0.08)

        def button(hwnd, down):
            nonlocal mouse_held
            send([INPUT(type=0, mi=MOUSEINPUT(dwFlags=2 if down else 4))], hwnd)
            mouse_held = down
            time.sleep(0.1)


        try:
            hwnd = wait_for(find_window, "编辑器窗口创建")
            activate(hwnd)
            print(f"系统前台窗口已确认，开始 {options.window_case} 验收。", flush=True)
            for modifier in [0x10, 0x11, 0x12, 0x5B, 0x5C]:
                if user.GetAsyncKeyState(modifier) & 0x8000:
                    raise AssertionError("检测到真实修饰键被按住，不能安全执行自动键鼠测试")
            if options.window_case not in ["native", "keys"]:
                end = time.monotonic() + 120
                while process.poll() is None:
                    if not user.IsWindow(hwnd):
                        process.wait(timeout=10)
                        break
                    try:
                        check_front(hwnd)
                    except AssertionError:
                        if completed_window_case(root, options.window_case, started_ns, process.poll()):
                            break
                        raise
                    if time.monotonic() > end:
                        raise AssertionError("前台窗口回归超时")
                    time.sleep(0.02)
                assert completed_window_case(root, options.window_case, started_ns, process.returncode), "窗口提前退出或缺少本次最终验收产物"
            else:
                for step in range(100 if options.window_case == "keys" else 21):
                    request = run / f"step-{step}.json"
                    def ready():
                        if options.window_case == "keys" and (run / "report.json").exists():
                            return True
                        check_front(helper if options.window_case == "native" and step == 18 else hwnd)
                        return request.exists()
                    wait_for(ready, f"输入步骤 {step}")
                    if options.window_case == "keys" and (run / "report.json").exists():
                        break
                    data = json.loads(request.read_text(encoding="utf-8"))
                    action = data["action"]
                    if action not in ["blur", "refocus"]:
                        check_front(hwnd)
                    if action in ["click", "shift_click", "drag", "drag_hold"]:
                        if action == "shift_click":
                            key(0x10, True, hwnd)
                        move(hwnd, data["x0"], data["y0"])
                        button(hwnd, True)
                        if action in ["drag", "drag_hold"]:
                            for fraction in [0.25, 0.5, 0.75, 1.0]:
                                move(hwnd, data["x0"] + (data["x1"] - data["x0"]) * fraction, data["y0"] + (data["y1"] - data["y0"]) * fraction)
                        if action != "drag_hold":
                            button(hwnd, False)
                        if action == "shift_click":
                            key(0x10, False, hwnd)
                    elif action == "move":
                        move(hwnd, data["x0"], data["y0"])
                    elif action.startswith("key:"):
                        _, modifiers, name = action.split(":")
                        if modifiers not in ["none", "ctrl", "shift", "ctrlshift"]:
                            raise AssertionError(f"未知修饰键：{modifiers}")
                        codes = {"ESC": 0x1B, "DELETE": 0x2E, "TAB": 0x09, "HOME": 0x24,
                                 "F2": 0x71, "F3": 0x72, "F4": 0x73, "F6": 0x75}
                        vk = ord(name) if len(name) == 1 and "A" <= name <= "Z" else codes[name]
                        chord(vk, hwnd, control="ctrl" in modifiers, shift="shift" in modifiers)
                    elif action in ["undo", "redo"]:
                        chord(0x5A if action == "undo" else 0x59, hwnd, control=True)
                    elif action == "release":
                        button(hwnd, False)
                    elif action == "sidebar_release":
                        move(hwnd, data["x0"], data["y0"])
                        button(hwnd, False)
                    elif action == "escape":
                        chord(0x1B, hwnd)
                        button(hwnd, False)
                    elif action == "blur":
                        check_front(hwnd)
                        helper = user.CreateWindowExW(0, "STATIC", "编辑器失焦自测辅助窗口", 0x00CF0000, 40, 40, 280, 130, None, None, None, None)
                        if not helper:
                            raise ctypes.WinError(ctypes.get_last_error())
                        activate(helper)
                        user.UpdateWindow(helper)
                        check_front(helper)
                        if mouse_held:
                            button(helper, False)
                        time.sleep(0.2)
                    elif action == "refocus":
                        check_front(helper)
                        activate(hwnd)
                        user.DestroyWindow(helper)
                        helper = None
                    else:
                        raise AssertionError(f"未知自测动作：{action}")
                    (run / f"ack-{step}").write_text("系统输入已发送\n", encoding="utf-8")
                    events.append({"step": step, "action": action, "foreground_verified": True})
                    print(f"步骤 {step}：{action}，系统前台已验证。", flush=True)
                wait_for(lambda: (run / "report.json").exists(), "文档与 XML 最终验收")
                assert process.wait(timeout=10) == 0
            assert process.returncode == 0, f"编辑器失败：{process.returncode}；日志 {run}"
            assert_no_segmentation_warnings(run / "editor.log")
            for sample, before in zip(samples, hashes):
                assert hashlib.sha256(sample.read_bytes()).digest() == before, "源样本被修改"
            if options.window_case == "performance":
                report = json.loads((root / "target/editor-performance.json").read_text(encoding="utf-8"))
                (run / "performance.json").write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
                print(json.dumps(report, ensure_ascii=False), flush=True)
            result = {"case": options.window_case, "system_foreground_checks": foreground_checks, "real_system_input": options.window_case in ["native", "keys"], "ime_tested": options.window_case == "native-ime", "events": events, "source_samples_unchanged": True, "icu_segmentation_warnings_absent": True}
            (run / "driver.json").write_text(json.dumps(result, ensure_ascii=False, indent=2), encoding="utf-8")
            (root / "target/foreground-last.txt").write_text(str(run), encoding="utf-8")
            print(f"前台验收通过：{foreground_checks} 次系统前台校验；产物 {run}", flush=True)
        finally:
            # 失败时只释放本脚本按下的按键/按钮，避免系统遗留按下状态。
            releases = [INPUT(type=1, ki=KEYBDINPUT(wVk=vk, dwFlags=2)) for vk in held]
            if mouse_held:
                releases.append(INPUT(type=0, mi=MOUSEINPUT(dwFlags=4)))
            if releases:
                batch = (INPUT * len(releases))(*releases)
                user.SendInput(len(batch), batch, ctypes.sizeof(INPUT))
            if helper:
                user.DestroyWindow(helper)
            if process.poll() is None:
                process.terminate()
                process.wait(timeout=10)


if __name__ == "__main__":
    # 桌面输入会暂时阻止窗口置前，只重试明确的焦点失败；断言/数据错误直接失败。
    for attempt in range(1, 4):
        try:
            main()
            break
        except AssertionError as error:
            focus_failure = any(reason in str(error) for reason in [
                "系统拒绝将测试窗口置前", "系统前台确认", "已失去真实前台",
                "测试窗口被最小化", "激活点击点不属于测试窗口", "检测到真实修饰键被按住",
            ])
            if focus_failure and attempt == 3:
                skill = Path.home() / ".agents/skills/noticer-progress/scripts/report_progress.py"
                if skill.is_file():
                    try:
                        subprocess.run(["uv", "run", "--no-project", "--python", "3.12", str(skill),
                            "--state", "progress", "--room", "sr1", "--task", "DR 编辑器前台测试焦点提醒",
                            "--message", "前台窗口测试连续三次受到焦点或真实键鼠干扰，当前专项未通过。测试短暂运行时请尽量暂停桌面键鼠输入；Agent 会继续其他工作并按需重新验证，不把失败当通过。"],
                            check=True, creationflags=subprocess.CREATE_NO_WINDOW)
                    except (OSError, subprocess.CalledProcessError) as notify_error:
                        print(f"noticer 提醒失败：{type(notify_error).__name__}", file=sys.stderr)
                else:
                    print("noticer 脚本不可用，未发送焦点提醒。", file=sys.stderr)
            if not focus_failure or attempt == 3:
                raise
            print(f"焦点暂时不可用，自动重试 {attempt + 1}/3：{error}", flush=True)
            time.sleep(1)
