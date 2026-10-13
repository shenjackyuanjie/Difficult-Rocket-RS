# 开发与验收指南

[返回 README](../README.md) · [当前实际结果](editor-progress.md) · [历史记录](validation-history.md)

以下命令是复现入口，**不是已运行通过的声明**。从仓库根目录执行；每次 GUI 验收使用新输出目录、同一已构建程序，串行运行，勿覆盖旧报告或原版资源。

## 无窗口与 Release 检查

```powershell
cargo fmt --all --check
cargo test --workspace --release --locked
cargo clippy --workspace --all-targets --release --locked -- -D warnings
cargo build --workspace --release --locked
uv run --no-project --python 3.12 python -B -X utf8 -m unittest discover -s scripts -p 'test_*.py' -v
```

Windows 若系统 TEMP 写入受限，可为本次进程将 `TEMP` / `TMP` 设为已创建的 `target/test-temp`。Rust 测试按模块邻近存放；`test_support` 仅在测试编译，共享内存文档/目录及真实系统夹具。不要为了删测试数量删掉输入边沿、事务、XML 或失败数据保护的独立断言。

## XML 与核心探针

```powershell
cargo run -p dr-core --example verify_catalog --release --locked -- ../Difficult-Rocket/assets/builtin/PartList.xml
cargo run -p dr-core --example verify_ships --release --locked -- ../Difficult-Rocket/assets/ships
cargo run -p dr-core --example verify_scoped_edits --release --locked -- ../Difficult-Rocket/assets/ships
cargo run -p dr-core --example verify_catalog_edits --release --locked -- ../Difficult-Rocket/assets/builtin/PartList.xml
cargo run -p dr-core --example verify_surface_edits --release --locked -- ../Difficult-Rocket/assets/builtin/PartList.xml
cargo run -p dr-core --example verify_history --release --locked -- ../Difficult-Rocket/assets/ships/Ophioglossum.xml
```

探针只读加载、在内存编辑/撤销/XML 往返，不覆盖样本。`verify_catalog` / `verify_ships` 同时核对模型和原始元素/属性；船体批量校验遇异常继续检查，其后返回失败。原版库历史为 185 个正常船体、125,856 部件及 5 个明确拒绝的异常输入，不能把预期的退出码 1 写成全库命令成功。目录组合/历史探针的有限枚举不代表所有连续角度、位置或完整用户路径。

## 自动窗口演示

```powershell
$env:WGPU_BACKEND = 'dx12' # 仅本次进程；普通编辑器默认后端不改

# 完整详细回归：19 章、45 例、95 个证据，40ms 节拍，完成自动退出
uv run --no-project --python 3.12 python -B -X utf8 scripts/demo_editor.py --editor-bin target/release/dr-editor.exe --mode detailed --fast --output target/demo-detailed-new

# 详细观看：450ms 节奏基准，完成自动退出
uv run --no-project --python 3.12 python -B -X utf8 scripts/demo_editor.py --editor-bin target/release/dr-editor.exe --mode detailed --step-ms 450 --exit-on-complete --output target/demo-watch-new

# 简略十章；默认 450ms，完成后保留窗口
uv run --no-project --python 3.12 python -B -X utf8 scripts/demo_editor.py --editor-bin target/release/dr-editor.exe --mode brief --output target/demo-brief-new
```

默认程序为 `target/debug/dr-editor.exe`，缺失或相关源码更新时自动 `cargo build -p dr-editor`；显式 `--editor-bin` 不代构建，`--no-build` 禁止自动构建。`--step-ms` 改节拍，`--fast` 强制 40ms/退出但不改变内容模式；简略默认超时 600s，详细默认不限时，`--timeout <秒>` 可显式限制。直接运行对应 `--demo-showcase <新目录> --demo-mode brief|detailed`。

正常观看按技术/导航/拖动/点击分配停顿、抵达目标才发输入边沿，逐帧移动紫色指针；快速回归跳过移动动画。`UiTestInput` 的短点击按下/抬起成对交给同一次 egui pass，避免慢帧变长按，不重复释放；长按/拖动仍分帧。章节底部说明和动作提示不拦截画布，分级停靠期间隐藏章节横幅，避免挡住船底目标。

| 基础十章 | 内容 |
| --- | --- |
| 部件目录与放置 | 真实部件行、预览变换/碰撞/放置、组标题收起及全部展开 |
| 连接点与吸附 | 沿边与同边多点连接、撤销与 XML |
| 旋转/镜像/自由连接 | 精调、自由连断、线设置、整组变换 |
| 选择与编辑 | 三键配置、框选/后代跟随、连续拖动、复制粘贴与连接分量删除 |
| 视角与显示 | F/Shift+F、调试/显隐、F1 与输入隔离 |
| 属性与分级 | 真实控件、滚动末项、草稿应用及历史 |
| 连接树与图 | 换父、环路保护、断边 |
| 原版重号修复 | Heronb 真实歧义分配及独立删除 |
| 船体目录 | 临时千船体目录、坏 XML、虚拟滚动与末项打开 |
| 未保存确认 | 主窗口模态取消、隔离及放弃 |

详细模式穿插放置（3）、精确重叠（6）、自由连接（7）、整组变换（6）、连线（5）、交互回放（4）、历史/属性（7）、拓扑（4）、文件（3）九组，共 45 例。每例核对内存文档/历史和真实 metrics，不宣称穷举。严格 5% 的几何例用精确事务参数；对接类型夹具显式设置 `dock=true`，不改原资源。详细分级为 64 级/1,024 动作，简略为 8 级/48 动作；驱动核对真实规模及末项修改。详细分级报告另记录实际停靠/船体/目标矩形与组/ID，检查面板不覆盖船体、修改目标完整可见，不能只用布尔字段宣称通过。

- 成功报告 `demo-report.json` 在全模式完成后才原子发布；模式、章节顺序、状态、耗时、路径及新鲜非空产物均须通过。
- `demo-timeline.jsonl` 记录章首/章尾时间；`demo-actions.jsonl` 仅记录实际重播后的按下/滚轮，供音效同步，不记录移动/松开。
- 详细模式另有九份 `*-cases.json` 与 45 张 `case-*.png`；缺例/缺图、提前关闭、非零退出或简略冒充详细均失败。
- Test.xml / Heronb.xml 只读，成功/异常路径都核对源哈希；输出不得包含源样本或其祖先目录。
- 演示隔离原生鼠标/键盘/文字/滚轮，使用内部 Bevy/egui 注入，不移动系统指针，不等于系统键鼠、输入法或 input-to-photon 延迟实测。

演示与窄窗工具复用 `editor_session.py`，从唯一进程名、同 SHA-256 的 EXE 副本启动，保留 `editor-session.json`。专项临时证据会先写到工作目录 `target` 再归档，**不要并行运行 GUI 自测**。只清理本工具拥有的 PID/HWND，不关闭其他编辑器或桌面工具。

## 窄窗与启动截图

```powershell
uv run --no-project --python 3.12 python -B -X utf8 scripts/validate_narrow_windows.py --editor target/release/dr-editor.exe --assets ../Difficult-Rocket/assets --output target/narrow-new
# 同一工具与三种尺寸，另验真实分级正文、末项修改及船上目标联动
uv run --no-project --python 3.12 python -B -X utf8 scripts/validate_narrow_windows.py --editor target/release/dr-editor.exe --assets ../Difficult-Rocket/assets --staging --output target/narrow-staging-new
cargo run --release --locked -p dr-editor -- --ship ../Difficult-Rocket/assets/ships/Test.xml --smoke-test --smoke-window-size 390x844
```

窄窗串行检查 390×844、640×360、320×640，核对 Win32 客户区、Bevy 逻辑/物理尺寸与 DPI、原始 PNG。仅只读观测，不改系统窗口几何。启动截图约 5s 后输出 `target/editor-smoke.png` / `editor-smoke-window.json` 并退出；它证明启动/尺寸，不代替完整侧栏内容、初始自动整船取景、交互或物理设备验收。

`--staging` 复用窗口身份/尺寸观测与既有 `staging_smoke`，在同一窄窗内滚动 64 级/1,024 动作至末项、只移动指针验证悬停高亮及移开恢复，再修改草稿、应用、撤销重做及 XML 往返；核对 `editor-staging-performance.json` 的真实布局/悬停/目标几何，截图使用 `editor-staging-draft.png`。驱动取本帧实际滚动正文中心，不从按钮位置猜固定偏移。组合 `--smoke-test --staging-smoke-test --smoke-window-size` 时由分级流程截图退出，不被启动自测的 5s 退出截断。仍是内部注入、非物理触摸，动作行不换行及图标的视觉质量须另看原图。

## 单项窗口与系统输入

直接运行 Release 编辑器时，可选择以下既有入口；输出在当前工作目录 `target`。完整演示已经包含多数专项，不必为同一改动重复跑所有窗口。

| 标志 | 夹具 / 检查 |
| --- | --- |
| `--panel-smoke-test` | 加 `--ship .../Test.xml`；部件目录/碰撞/侧栏隔离 |
| `--connection-smoke-test` | 不传 ship，默认驾驶舱；长梁同边多点 |
| `--transforms-smoke-test` | 独立内存夹具；精调、自由连接、整组与线设置 |
| `--selection-smoke-test` / `--scoped-smoke-test` | 多选/剪贴板、分组实例与历史/XML |
| `--topology-smoke-test` / `--view-smoke-test` | 树图与视图；普通专项含 960×640 检查 |
| `--properties-smoke-test` / `--egui-smoke-test` | 同一属性自测，含 TextEdit/IME 内部注入，不代表系统输入法 |
| `--staging-smoke-test` / `--browser-smoke-test` | 64 级/1,024 动作；临时 1,000 文件目录 |
| `--repair-smoke-test` | 加 `--ship .../Heronb.xml`；真实重号引用修复 |
| `--interaction-smoke-test` | 内部回放的同帧视觉跟随，不是物理显示延迟 |
| `--native-dialog-test` | 窗口内未保存模态；不是原生路径对话框 |
| `--performance-test --performance-selection-count 1000` | 加 `--ship .../Ophioglossum.xml`；真实拖动统计 |
| `--native-ime-test` | Windows IMM 实际预编辑/提交专项，按需单独安排 |

```powershell
# 绑定 owner 的真实打开/另存为对话框取消；不保存源样本
uv run --no-project --python 3.12 python -B -X utf8 scripts/check_native_dialogs.py --editor-bin target/release/dr-editor.exe

# 真实前台系统鼠标 / SendInput；keys 为系统快捷键专项
uv run --no-project --python 3.12 python -B -X utf8 scripts/check_native_input.py --editor-bin target/release/dr-editor.exe
uv run --no-project --python 3.12 python -B -X utf8 scripts/check_native_input.py --editor-bin target/release/dr-editor.exe --window-case keys
```

前台驱动只操作自己启动的编辑器及失焦辅助窗口，核对真实前台，置前/失焦最多自动重试三次，之后按项目默认向 `sr1` 提醒；程序崩溃/数据断言立即失败，不冒充焦点失败。其他 `--window-case` 是前台受控的内部注入式专项，并未变成系统输入。当前约定跳过输入法/系统文字专项，确需运行时另行安排。

## 录制无配音 1080p 视频

需要 Windows、FFmpeg/ffprobe 与系统微软雅黑。复用简略十章及时间线，不是截图轮播；默认静音，`--sound-effects` 可合成真实按下/滚轮的轻量音效，不含配音或音乐。

```powershell
uv run --no-project --python 3.12 python -B -X utf8 scripts/render_demo_video.py --no-build --output target/video-new
uv run --no-project --python 3.12 python -B -X utf8 scripts/render_demo_video.py --no-build --sound-effects --output target/video-sound-new
uv run --no-project --python 3.12 python -B -X utf8 scripts/render_demo_video.py --render-only target/video-new
```

录制专用窗口为无边框、scale factor 1 的 1920×1080。脚本仅置顶自建窗口（不激活、不改位置/大小），使用 `gdigrab desktop` 输入端坐标裁剪该物理客户区；不是后台捕获，须保持可见、勿最小化/移动/遮挡。持续所有权、几何与九点遮挡检查不能保证绝对无遮挡。

原始片逐章检查实质像素变化，排除底部说明及微小编码噪声；低分辨率检测不代替人工观看。根据时间线去初始化间隙、不加速操作。最终输出 1920×1080 / 30fps / H.264 / yuv420p，独立 120px 字幕带，4s 片头/5s 总结；可选 48kHz 单声道 AAC。只有报告、源哈希、动态/规格/时长校验通过才发布 MP4，已有成片不覆盖；失败保留原始视频和日志，字幕及 `video-manifest.json` 可用于排查。

## 日志边界

`icu_provider` 显式启用 logging，由 Bevy 默认过滤已知缺失 CJK 分词模型的 warn；真正 error 和其他模块日志保留。仍是原分段回退，不声称完整 CJK 词典或输入法修复。`RUST_LOG` 可覆盖，排查时启用 `icu_provider=warn`。前台驱动在成功退出后仍检查该告警，不将日志刷屏悄悄视作通过。
