"""Windows 原生路径选择回归：检查窗口 owner、取消和主线程持续更新，不保存样本。"""
import argparse
import json
import ctypes
from ctypes import wintypes
import hashlib
from pathlib import Path
import subprocess
import sys
import time


def main():
    if sys.platform != "win32":
        raise SystemExit("此自测仅适用于 Windows")
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--editor-bin", type=Path)
    options = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    artifact = root / "target/native-file-dialogs.json"
    started = time.time_ns()
    sample = (root / "../Difficult-Rocket/assets/ships/Test.xml").resolve()
    original_hash = hashlib.sha256(sample.read_bytes()).digest()
    user = ctypes.WinDLL("user32", use_last_error=True)
    callback_type = ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.HWND, wintypes.LPARAM)
    user.EnumWindows.argtypes = [callback_type, wintypes.LPARAM]
    user.GetWindowThreadProcessId.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.DWORD)]
    user.GetWindowTextW.argtypes = [wintypes.HWND, wintypes.LPWSTR, ctypes.c_int]
    user.PostMessageW.argtypes = [wintypes.HWND, wintypes.UINT, wintypes.WPARAM, wintypes.LPARAM]
    user.IsWindow.argtypes = [wintypes.HWND]
    user.GetWindow.argtypes = [wintypes.HWND, wintypes.UINT]
    user.GetWindow.restype = wintypes.HWND
    with (root / "target/native-dialogs.log").open("w", encoding="utf-8") as log:
        process = subprocess.Popen(
            [str((options.editor_bin or root / "target/debug/dr-editor.exe").resolve()), "--ship", str(sample), "--native-file-dialog-test"],
            cwd=root, stdout=log, stderr=log, creationflags=subprocess.CREATE_NO_WINDOW,
        )

        def windows():
            found = []

            @callback_type
            def collect(hwnd, _):
                pid = wintypes.DWORD()
                user.GetWindowThreadProcessId(hwnd, ctypes.byref(pid))
                if pid.value == process.pid:
                    title = ctypes.create_unicode_buffer(512)
                    user.GetWindowTextW(hwnd, title, len(title))
                    found.append((hwnd, title.value))
                return True

            user.EnumWindows(collect, 0)
            return found

        def wait_for(predicate, description, timeout=20):
            deadline = time.monotonic() + timeout
            while time.monotonic() < deadline:
                value = predicate()
                if value:
                    return value
                if process.poll() is not None:
                    raise AssertionError(f"进程提前退出：{description}，退出码 {process.returncode}")
                time.sleep(0.1)
            raise AssertionError(f"等待超时：{description}；当前窗口：{windows()}")

        def find_title(text):
            return next((hwnd for hwnd, title in windows() if text in title), None)

        def post(hwnd, message, wparam=0, lparam=0):
            if not user.PostMessageW(hwnd, message, wparam, lparam):
                raise ctypes.WinError(ctypes.get_last_error())

        def cancel(hwnd):
            assert user.GetWindow(hwnd, 4) == main_window, "路径选择窗口未绑定编辑器 owner"
            time.sleep(0.6)  # 主 Update 在选择窗口等待期间必须继续运行。
            post(hwnd, 0x0111, 2)  # WM_COMMAND / IDCANCEL
            wait_for(lambda: not user.IsWindow(hwnd), "对话框关闭")
            time.sleep(0.3)

        try:
            main_window = wait_for(lambda: find_title("Difficult Rocket Editor"), "编辑器启动")
            cancel(wait_for(lambda: find_title("打开 SR1 船体"), "原生打开对话框"))
            assert find_title("Test.xml") == main_window
            cancel(wait_for(lambda: find_title("保存 SR1 船体"), "原生另存为对话框"))
            # 第二次取消后应用已完成内部文档断言并自动退出，不再要求窗口存活。
            assert process.wait(timeout=10) == 0
            assert artifact.stat().st_mtime_ns >= started, "缺少本次非阻塞选择验收报告"
            result = json.loads(artifact.read_text(encoding="utf-8"))
            assert result["document_preserved"]
            assert result["open_frames"] >= 3 and result["save_as_frames"] >= 3, "选择窗口期间主线程未更新"
            assert hashlib.sha256(sample.read_bytes()).digest() == original_hash
            print("原生路径选择自测通过：打开/另存为取消、编辑器 owner、主线程持续更新；样本未改动。", result)
        finally:
            # 只清理由本脚本启动的进程。
            if process.poll() is None:
                process.terminate()
                process.wait(timeout=10)


if __name__ == "__main__":
    main()
