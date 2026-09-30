"""Windows 原生文件对话框回归；仅向本脚本启动的窗口发送输入，不保存样本。"""
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
    root = Path(__file__).resolve().parents[1]
    sample = (root / "../Difficult-Rocket/assets/ships/Test.xml").resolve()
    original_hash = hashlib.sha256(sample.read_bytes()).digest()
    user = ctypes.WinDLL("user32", use_last_error=True)
    callback_type = ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.HWND, wintypes.LPARAM)
    user.EnumWindows.argtypes = [callback_type, wintypes.LPARAM]
    user.GetWindowThreadProcessId.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.DWORD)]
    user.GetWindowTextW.argtypes = [wintypes.HWND, wintypes.LPWSTR, ctypes.c_int]
    user.PostMessageW.argtypes = [wintypes.HWND, wintypes.UINT, wintypes.WPARAM, wintypes.LPARAM]
    user.IsWindow.argtypes = [wintypes.HWND]
    with (root / "target/native-dialogs.log").open("w", encoding="utf-8") as log:
        process = subprocess.Popen(
            [str(root / "target/debug/dr-editor.exe"), "--ship", str(sample), "--native-dialog-test"],
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
            post(hwnd, 0x0111, 2)  # WM_COMMAND / IDCANCEL
            wait_for(lambda: not user.IsWindow(hwnd), "对话框关闭")
            time.sleep(0.3)

        try:
            main_window = wait_for(lambda: find_title("Difficult Rocket Editor"), "编辑器启动")
            cancel(wait_for(lambda: find_title("打开 SR1 船体"), "原生打开对话框"))
            assert find_title("Test.xml") == main_window
            cancel(wait_for(lambda: find_title("保存 SR1 船体"), "原生另存为对话框"))
            assert find_title("Test.xml") == main_window
            cancel(wait_for(lambda: find_title("保存未保存的修改"), "未保存退出保护"))
            assert user.IsWindow(main_window)
            confirmation = wait_for(lambda: find_title("保存未保存的修改"), "再次退出提示")
            post(confirmation, 0x0111, 7)  # IDNO：放弃修改
            assert process.wait(timeout=10) == 0
            assert hashlib.sha256(sample.read_bytes()).digest() == original_hash
            print("原生对话框自测通过：打开取消、另存为取消、未保存退出取消和放弃退出；样本未改动。")
        finally:
            # 只清理由本脚本启动的进程。
            if process.poll() is None:
                process.terminate()
                process.wait(timeout=10)


if __name__ == "__main__":
    main()
