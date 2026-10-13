"""为每次 GUI 自检建立内容相同、进程名唯一的 EXE 副本。

桌面工具（如 PersistentWindows）会按进程名沿用上一次的窗口尺寸，同一个 EXE 名会让
demo 运行与三个窄窗样例互相污染；这里按 uuid 复制出唯一进程名副本，核对源与副本的
SHA256 相同后才交给调用方启动。只使用标准库，不改写或删除源 EXE 与样本。
"""
import hashlib
import json
import shutil
import uuid
from pathlib import Path


SESSION_NAME = "editor-session.json"


class SessionError(RuntimeError):
    """副本无法建立或与源不一致，此时禁止启动。"""


def file_sha256(path):
    """流式读取摘要，避免为校验把整个编辑器读进内存。"""
    digest = hashlib.sha256()
    with Path(path).open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def copy_for_run(source, directory):
    """复制 source 到 directory，返回并落盘身份记录；不一致时禁止启动。

    返回四字段：source_path、source_sha256、copy_path、copy_sha256，
    同时写入 directory/editor-session.json 作为保留证据。
    """
    source = Path(source).resolve()
    if not source.is_file():
        raise SessionError(f"源可执行文件不存在，无法建立隔离副本：{source}")
    directory = Path(directory)
    directory.mkdir(parents=True, exist_ok=True)
    source_sha256 = file_sha256(source)
    copy_path = (directory / f"{source.stem}-{uuid.uuid4().hex}{source.suffix}").resolve()
    shutil.copyfile(source, copy_path)
    identity = {
        "source_path": str(source),
        "source_sha256": source_sha256,
        "copy_path": str(copy_path),
        "copy_sha256": file_sha256(copy_path),
    }
    (directory / SESSION_NAME).write_text(
        json.dumps(identity, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    if identity["copy_sha256"] != source_sha256:
        raise SessionError(f"副本与源 SHA256 不一致，禁止启动：{copy_path}")
    return identity
