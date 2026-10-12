"""生成 Windows x64 编辑器试用 ZIP，仅包含 release、必要资源和六艘范例船。"""
import argparse
from datetime import datetime, timezone
import hashlib
import io
import json
from pathlib import Path
import subprocess
import zipfile

ROOT = Path(__file__).resolve().parents[1]
PACKAGE_ROOT = "dr-editor-win64"
SAMPLES = ("Solar.xml", "R.xml", "Co13.xml", "Gzj.xml", "Test.xml", "Heronb.xml")
FONT_DIR = Path("fonts/HarmonyOS_Sans/HarmonyOS_Sans_SC")
SOURCE_URL = "https://github.com/shenjackyuanjie/Difficult-Rocket-RS"

START = r'''@echo off
setlocal
pushd "%~dp0"
"%~dp0dr-editor.exe" --catalog "%~dp0assets\builtin\PartList.xml" --assets "%~dp0assets" --ship "%~dp0assets\ships\Solar.xml" %*
set "result=%errorlevel%"
if not "%result%"=="0" (
    echo Editor exited with error %result%. Please keep the messages above.
    pause
)
popd
exit /b %result%
'''

README = """# Difficult Rocket Editor — Windows x64 试用包

## 开始试用

1. 把整个 ZIP 解压到可写目录，保留 dr-editor.exe 与 assets 的相对位置。
2. 双击 Start.cmd。会打开 Solar 范例；启动脚本会自动指定资源路径。
3. 按 F 适配整船。在左侧船体列表切换其它范例，右侧目录可拖出新部件。
4. 建议用 Ctrl+Shift+S 另存自己的作品，再继续编辑。

无需安装 Rust 或 Python。这是编辑器开发快照，当前展示 XML 船体编辑，尚不包含飞行模拟。
直接双击 dr-editor.exe 会使用开发仓库的默认资源路径，请使用 Start.cmd。

## 环境

- Windows 10/11 x64，支持 Vulkan 或 DirectX 12 的显卡及相应驱动。
- 若提示缺少 VCRUNTIME140.dll，安装 Microsoft Visual C++ 2015–2022 x64 运行库：
  https://aka.ms/vs/17/release/vc_redist.x64.exe
- 在控制台保留启动日志；遇到问题可把错误文字、显卡型号和 Windows 版本一起反馈。

## 常用操作

| 操作 | 按键 |
| --- | --- |
| 抓取移动部件 | 左键拖动；保留抓取偏移，松手提交 |
| 平移与缩放 | 默认左键拖空白平移，滚轮在鼠标处缩放 |
| 多选 | Shift+左键；默认中键拖动框选，可在右侧切换按钮 |
| 放置新部件 | 从右侧目录拖到画布；或选择目录部件后左键/P 放置 |
| 取消预览 | Esc 或右键 |
| 旋转、镜像 | R：90°；Q/E：±15°；Shift+Q/E：±1°；X/Y：镜像 |
| 撤销、重做 | Ctrl+Z / Ctrl+Y |
| 复制、剪切、粘贴 | Ctrl+C / Ctrl+X / Ctrl+V |
| 保存、另存为 | Ctrl+S / Ctrl+Shift+S |
| 适配整船、选区 | F / Shift+F；Home 复位视图 |
| 属性与分级、连接树/图 | F2 / F6 |
| 完整帮助 | F1 |

辅助模式显示吸附与碰撞预览；自由模式可手动点选连接端点。
新建连接对同一对部件只保留一条；加载范例时原有历史连接原样保留。

## 范例船

- Solar.xml：太阳能板、燃料箱与 RCS，默认打开的小型范例。
- R.xml：机头、降落伞、燃料箱与 RCS。
- Co13.xml：电池、燃料箱、离子推进等不同部件组合。
- Gzj.xml：太阳能、车轮与桁架组合。
- Test.xml：小型连接与分离器测试船，包含原版历史边界记录。
- Heronb.xml：799 部件大型范例，包含对接及复杂历史引用。

编辑副本位于本包 assets/ships，启动和浏览不会修改它们，保存时才写入所选文件。

## 来源与许可

- 项目源码：https://github.com/shenjackyuanjie/Difficult-Rocket-RS
- 部件资源与范例来自：https://github.com/shenjackyuanjie/Difficult-Rocket
- 本项目采用 MPL-2.0，全文见 LICENSE；source.zip 附本次运行程序的项目源码快照。
- 中文字体附自身许可：assets/fonts/HarmonyOS_Sans/HarmonyOS_Sans_SC/LICENSE.txt。
- 第三方 Rust 依赖的版本由源码快照中的 Cargo.lock 固定，其许可声明见对应依赖项目。
- build-manifest.json 记录版本、时间和包内文件 SHA256；外置 .sha256 文件可校验 ZIP 下载完整性。
"""


def sha256(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def source_snapshot(root, license_path):
    buffer = io.BytesIO()
    with zipfile.ZipFile(buffer, "w", zipfile.ZIP_DEFLATED, compresslevel=9) as archive:
        for path in (root / "Cargo.toml", root / "Cargo.lock"):
            archive.write(path, path.name)
        for path in sorted((root / "crates").rglob("*")):
            if path.is_file():
                archive.write(path, path.relative_to(root).as_posix())
        archive.write(license_path, "LICENSE")
    return buffer.getvalue()


def build_package(root, editor, assets, output, revision):
    license_path = assets.parent / "LICENSE"
    entries = [(editor, "dr-editor.exe"), (license_path, "LICENSE"),
               (assets / "builtin/PartList.xml", "assets/builtin/PartList.xml")]
    entries.extend((path, "assets/" + path.relative_to(assets).as_posix())
                   for path in sorted((assets / "textures/parts").glob("*.png")))
    entries.extend((assets / FONT_DIR / name, (Path("assets") / FONT_DIR / name).as_posix())
                   for name in ("HarmonyOS_Sans_SC_Regular.ttf", "LICENSE.txt"))
    entries.extend((assets / "ships" / name, "assets/ships/" + name) for name in SAMPLES)
    for path, _ in entries:
        if not path.is_file():
            raise FileNotFoundError(f"缺少打包文件：{path}")
    extras = {"Start.cmd": START.replace("\n", "\r\n").encode("ascii"),
              "README.md": README.encode("utf-8"),
              "source.zip": source_snapshot(root, license_path)}
    hashes = {name: sha256(path) for path, name in entries}
    hashes.update({name: hashlib.sha256(content).hexdigest() for name, content in extras.items()})
    manifest = {"platform": "windows-x86_64", "profile": "release", "revision": revision,
                "created_utc": datetime.now(timezone.utc).isoformat(), "source_url": SOURCE_URL,
                "samples": list(SAMPLES), "sha256": hashes}
    output.parent.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(output, "x", zipfile.ZIP_DEFLATED, compresslevel=9) as archive:
        for path, name in entries:
            archive.write(path, PACKAGE_ROOT + "/" + name)
        for name, content in extras.items():
            archive.writestr(PACKAGE_ROOT + "/" + name, content)
        archive.writestr(PACKAGE_ROOT + "/build-manifest.json",
                         json.dumps(manifest, ensure_ascii=False, indent=2).encode("utf-8"))
    output.with_suffix(output.suffix + ".sha256").write_text(
        f"{sha256(output)}  {output.name}\n", encoding="utf-8")
    return manifest


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--editor-bin", type=Path, default=ROOT / "target/release/dr-editor.exe")
    parser.add_argument("--assets", type=Path, default=ROOT.parent / "Difficult-Rocket/assets")
    parser.add_argument("--output", type=Path, required=True, help="新 ZIP 路径，不覆盖已有文件")
    args = parser.parse_args()
    revision = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
    build_package(ROOT, args.editor_bin.resolve(), args.assets.resolve(), args.output.resolve(), revision)
    print(json.dumps({"zip": str(args.output.resolve()), "bytes": args.output.stat().st_size,
                      "sha256": sha256(args.output), "revision": revision}, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
