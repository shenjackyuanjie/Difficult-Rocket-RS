# Difficult-Rocket Rust 编辑器

独立的 Bevy 编辑器及可复用的 SR1 数据核心。当前仍在补齐功能，完整复刻尚未完成；差异与验证记录见 [编辑器进度](docs/editor-progress.md)。

从本仓库根目录运行，默认复用相邻 `Difficult-Rocket/assets` 中的部件目录、贴图和字体：

```powershell
cargo run -p dr-editor -- --ship ../Difficult-Rocket/assets/ships/Test.xml
```

省略 `--ship` 创建与原版一致、带默认驾驶舱的新船体；自定义目录没有驾驶舱类型时才创建空白文档。可用 `--catalog <PartList.xml>`、`--assets <资源目录>` 指定资源。加载失败会报错退出，不会用空白文档代替错误输入。

## 自动功能演示

在本仓库根目录运行下列脚本，会自动打开一次编辑器窗口，在同一窗口内顺序演示十个章节。底部显示章节说明，紫色圆环表示内部演示指针。正常观看模式不再给所有步骤相同停顿：默认 450ms 是节奏倍率基准，技术步骤约 80ms、导航 120ms、拖动节点 200ms，点击与按键结果约 650/700ms，章首至少 1.2s、章尾 0.9s。指针移动按距离分配 200–800ms，在目标之间逐帧线性移动，零件拖动与背景平移随真实画布输入连续更新；到达目标后才应用点击/按键边沿，egui 按钮短按释放，再等待下一动作。编辑器照常每帧更新，只有操作步骤按节拍推进；40ms 快速回归模式跳过移动动画。底部十章说明框保留，并随当前操作解释三键区别与配置效果；默认左键选择/拖动/空白平移、中键框选，真实点击侧栏切换后展示左键框选/中键平移，右键用于取消预览；后代跟随开关也通过真实控件点击展示正反例。演示中的实际鼠标三键、滚轮、按键与组合键会在画布上弹出提示，拖动时持续显示按住状态，停留 250ms 后用 650ms 线性淡出；提示只绘制、不截获点击，键盘持续按住不会每帧刷新淡出寿命；帮助打开时暂停章节横幅并将按键提示放到下方，避免遮住帮助内容。

```powershell
# 正常观看：默认 450ms 节奏基准，演示完成后保留窗口
uv run --no-project --python 3.12 python -X utf8 scripts/demo_editor.py

# 放慢演示，并在完成后自动退出
uv run --no-project --python 3.12 python -X utf8 scripts/demo_editor.py --step-ms 700 --exit-on-complete

# 快速 UI 回归：40ms 节拍，检查报告后自动退出
uv run --no-project --python 3.12 python -X utf8 scripts/demo_editor.py --fast

# 固定产物目录；不自动重新构建
uv run --no-project --python 3.12 python -X utf8 scripts/demo_editor.py --output target/demo-custom --no-build
```

默认程序为 `target/debug/dr-editor.exe`；缺失或相关源文件更新时脚本自动执行 `cargo build -p dr-editor`。可用 `--editor-bin <路径>` 指定独立程序（不会代为构建），用 `--timeout <秒数>` 调整默认 600 秒超时。输出目录默认是 `target/demo-<时间戳>`，重复演示请使用新目录，不能沿用已有报告。

| 章节 | 演示与断言 |
| --- | --- |
| 部件目录与放置 | 点击目录、旋转/镜像预览、碰撞拒绝、放置与撤销、分类筛选及恢复全部 |
| 连接点与吸附 | 紫色连接提示、长梁沿边吸附、多点连接、撤销及 XML 往返 |
| 旋转镜像、自由连接与连线设置 | 精调与非直角吸附、组合镜像、自由双点连/断；实际调整连线开关/颜色/粗细/线型/动态效果；树/连通/框选/后代整组变换及 XML 往返 |
| 选择与编辑 | 默认与切换后的三键对照、真实点击框选键与后代跟随配置、增选/框选、连续拖动、拖拽 R 旋转、重叠落点保留、拖入列表删除连接组件、复制粘贴、撤销重做及右键取消 |
| 视角与显示 | 整船/选区适配、普通模式调整窗口、调试显示与船体显隐、F1 帮助及输入隔离 |
| 属性与分级 | 实际属性控件、分级列表滚动、草稿修改、应用及撤销重做 |
| 连接树与连接图 | 实际树图控件、换父节点、环路拒绝、断边及撤销 |
| 原版重号船体修复 | Heronb 的歧义引用分配、修复应用、独立删除及撤销重做 |
| 船体目录与虚拟滚动 | 临时 1,000 船体目录、跳过坏 XML、滚动末项与点击打开 |
| 未保存确认 | 稳定的窗口内模态、实际点击取消保留文档、再次确认后放弃 |

部件目录与未保存确认使用原版 `Test.xml`，重号修复使用原版 `Heronb.xml`；其它章节使用新建船体或独立夹具。演示专用分级夹具有 8 级、48 个动作，便于观看；独立 `--staging-smoke-test` 仍保留 64 级、1,024 个动作的长列表验收。原版样本仅只读加载、在内存中编辑，**不保存覆盖原版**；启动脚本在成功与异常路径都核对两个源样本的 SHA256，拒绝将输出目录设为包含这些样本的目录或祖先。

每次成功输出完整的 `demo-report.json`、`demo-editor.log` 和逐章截图/XML 副本/专项 JSON 证据。独立的 `demo-timeline.jsonl` 记录十章开始/完成的 20 条 Unix 毫秒时间；`demo-actions.jsonl` 只记录实际重播后的鼠标按下、按键按下及非零滚轮事件，移动/松开不写入，同帧双输入渠道去重，用于音效同步。报告逐章记录通过状态和耗时，仅在十章完成后原子发布；脚本检查报告与声明的产物确实属于本次运行、非空且位于输出目录内，旧报告、缺失截图、提前退出或非零退出不算通过。耗时包含观看节拍和截图等待，**不是性能基准**。

演示进行中隔离原生鼠标、按键、文字和滚轮输入，误点窗口不会打断内部拖拽；演示自身的失焦取消步骤仍保留。普通编辑器不受此隔离影响；关闭窗口仍会中断演示，不能算作通过。

这是有断言的真实窗口演示，但采用内部 Bevy/egui 输入注入和虚拟指针，不移动系统鼠标、不发送系统键盘事件，也不冒充前台系统键鼠或输入法验收。原生打开/保存路径选择不纳入自动演示，仍由 `scripts/check_native_dialogs.py` 单独验收。观看时请不要同时操作鼠标键盘，也不要并行运行其它窗口自测：专项流程会先在 `target` 写临时证据，再归档到本次演示目录。默认完成后窗口可继续手动使用或关闭。

启动器自身的无 GUI 回归：

```powershell
uv run --no-project --python 3.12 python -B -m unittest discover -s scripts -p test_demo_editor.py -v
```

## 渲染无配音 1080p 进度视频

Windows 下使用 FFmpeg/ffprobe 与系统微软雅黑字体，将同一窗口中的十章真实操作录为 H.264 视频，并烧录中文字幕。默认静音；可用 `--sound-effects` 添加根据真实动作时间同步的低音量短音效，不含配音、音乐或外部版权资源。不是截图轮播；操作依然由内部 Bevy/egui 输入驱动，不冒充系统键鼠、输入法或原生路径选择验收。

```powershell
# 自动构建（必要时）、录制与渲染；使用新的独立目录
python -X utf8 scripts/render_demo_video.py --output target/video-progress

# 已构建程序可跳过构建
python -X utf8 scripts/render_demo_video.py --no-build --output target/video-progress-new

# 三键/按键/滚轮同步轻量音效；依然没有配音
python -X utf8 scripts/render_demo_video.py --no-build --sound-effects --output target/video-progress-sound

# 已成功录完后，只重试渲染；已有最终成片不会被覆盖
python -X utf8 scripts/render_demo_video.py --render-only target/video-progress-new

# 工具自身回归，不打开 GUI
python -B -m unittest discover -s scripts -p test_render_demo_video.py -v
```

录制专用 `--demo-presentation` 保持原生客户端 1920×1080、无边框、scale factor 1；视图/拓扑的普通专项仍测试 960×640。成片为 **1920×1080、30fps、H.264、yuv420p**，默认无音轨，音效模式为 48kHz 单声道 AAC，含 4s 片头与 5s 进度总结。原始编辑器画面等比缩放到上方 1706×960，保留编辑器底部的十章说明展示框，画面下方另留 120px 字幕带；视频章名与字幕不覆盖展示框或编辑控件。

脚本只定位与清理它创建的 PID/HWND；启用 DPI awareness，检查客户区与外框双尺寸及稳态，再使用 `gdigrab desktop` 的输入端坐标/尺寸参数，仅捕获该窗口的 1920×1080 物理客户区，不先录制整个桌面再裁剪。它不是离屏捕获，编辑器会临时置顶，录制中持续检查所有权、几何、最小化状态和九点遮挡采样；九点采样不是绝对无遮挡保证，请保持桌面可见、不要同时运行其它 GUI 自测。

十章报告与原版样本 SHA256 保护全部通过后才结束录制；依据 `demo-timeline.jsonl` 与 FFmpeg 实际 Unix 起点裁掉章间初始化等待，不加速真实操作。烧字幕前先对原始视频的每个保留章节检测实质像素变化，排除底部进度说明区域及微小编码噪声，冻结章节会被拒绝；低分辨率动态检测不能代替人工逐章观看。音效模式将实际 `demo-actions.jsonl` 事件映射到同一剪辑时间线，用标准库确定性合成 75ms 短音色并限制混音峰值，不为指针移动或松开添加噪声。最终 `dr-rs-progress-1080p.mp4` 在 ffprobe 规格和时长检查通过后才发布；`progress.srt`/`progress.ass` 是可复用字幕，`video-manifest.json` 记录命令、章节时间、动态检查、音效映射、哈希与规格，失败保留原始视频和日志以便排查。

## 手动操作

| 操作 | 按键 |
| --- | --- |
| 选择、移动并吸附部件 | 左键点击、拖动 |
| 增减选择、框选、全选 | Shift+左键；默认中键拖动框选，侧栏可切换为左键；Shift 框选追加；Ctrl+A |
| 复制、剪切、粘贴 | Ctrl+C/X/V；预览中 R/X/Y 变换、左键/P 提交、Esc/右键取消 |
| 从目录拖出新部件 | 列表任一行左键按住拖到画布，松手放置并吸附连接 |
| 拖拽中旋转、镜像、重叠落下 | 按住左键时按 R、Q/E、Shift+Q/E 或 X/Y；姿态与位移一次撤销，Esc/右键取消；拖拽中不复制 |
| 取消拖动预览 | Esc、右键；窗口失焦时自动取消；旋转与位移预览一并撤销 |
| 切换待放置部件、放置 | 当前分类中 Tab / Shift+Tab 切换；左键点击画布或 P 放置 |
| 删除、旋转、镜像 | Delete；R 为 90°，Q/E 为 ±15°，Shift+Q/E 为 ±1°；X / Y 镜像 |
| 撤销、重做 | Ctrl+Z、Ctrl+Y |
| 连接树 / 连接图编辑 | F6 或右侧“连接树 / 连接图”；Esc 关闭 |
| 属性与分级编辑 | F2 或顶部“属性 / 分级”；未选部件时打开驾驶舱 |
| 新建、打开 | Ctrl+N、Ctrl+O；支持单个 XML 文件拖入 |
| 保存、另存为 | Ctrl+S、Ctrl+Shift+S；首次保存选择目标文件 |
| 鼠标位置缩放、平移、视图复位 | 滚轮；默认左键拖动空白平移（左键框选模式下改用中键）；Home |
| 适配整船、适配选区 | F、Shift+F；视图居中到两侧目录之间 |
| 调试显示、船体显隐 | F3 显示坐标、原点和选中轮廓/连接面；F4 隐藏或恢复贴图 |
| 子节点跟随 | 右侧“↳ 子节点跟随”开关，默认关闭；开启后拖父节点带所有子孙，不带上级父节点 |
| 自由连接 | 右侧“自由模式”；依次点击两个部件的连接点或连接边建立连接，重复选择同一对端点可断开；Esc/右键取消选点 |
| 拖入列表删除 | 拖现有部件到右侧部件列表松手，删除整个相连分量；Ctrl+Z 一次撤销 |
| 操作帮助 | F1 或顶部“? 帮助”；F1 / Esc 关闭 |
| 截图 | F12，输出到当前目录 |

视图适配包含旋转后的贴图与真实实体轮廓；窗口最小尺寸为 960×640，调整窗口后可再按 F 适配。调试和显隐只影响显示，不改文档或撤销历史；属性草稿中不触发这些快捷键。

### 任意角度连接与自由模式

树/图中的“子树”“连通”选择、中键框选和 Shift 多选共用同一组选区：R、Q/E、Shift+Q/E 与 X/Y 都围绕选区中心同步变换位置与姿态，而不是逐个自转。开启“子节点跟随”后，抓住父节点拖拽期间的旋转/镜像也会同步作用于全部后代；内部连接保持，未选部件不移动。

默认辅助模式中，连接点和连接边随任意角度及镜像变换；不再要求两条连接面的法线完全相反。连接点/边的最近距离不超过 **0.35 Ship 单位** 时参与吸附，且两零件的实际实体相交面积必须 **严格小于较小零件实体面积的 5%**。恰好 5% 不允许；多个 Shape 取并集，传感器不计入，圆形按解析面积计算。对接类型、固定点/共享 group 的占用和重号引用安全规则仍保留。

右侧提供选区 `−15° / +15°` 按钮与当前角度显示。`Q/E`、`Shift+Q/E` 可在单选、多选、目录放置、粘贴预览和拖拽期间使用；多选围绕选区中心变换，拖拽绕抓取部件原点进行世界轴镜像和组合旋转。已有非直角角度不会被 R 覆盖成直角。

**自由模式**关闭自动吸附、碰撞判定及移动/旋转/镜像产生的自动断连。所有连接点/边始终以紫色显示，已选端点为金色；先点父端，再点子端，连接和断开均可一次撤销。距离、重叠、方向与占用不限制手动连接，但必须是两个不同且可唯一引用的部件，以及有效普通点或合法插头/端口对接点。放置和粘贴也不自动连边，可以重叠，目录数量及禁止旋转的限制仍保留。模式切换、文件操作、模态、Esc/右键及失焦清理未完成选点；模式本身不写入 XML、不新增撤销记录，切换回辅助模式不会主动改写已经建立的自由连接。

常驻 HUD 仅显示部件/选中数量、保存状态、质量与当前错误，不再铺满快捷键说明；完整操作表改为按需打开的帮助窗口。顶部和侧栏优先采用“符号 + 短标签”，排序/增删等按钮通过悬停补充解释，保存、放弃、应用、取消等关键操作保留明确文字。

文件操作也可使用顶部工具栏。新建、打开和退出前在主窗口内显示与属性面板统一的暗色 egui 模态，提供“保存后继续”“放弃修改并继续”“取消”；Esc 等价于取消，点击遮罩不会放弃修改。确认期间隔离画布、侧栏和快捷键，重复关闭不会覆盖待确认操作。保存失败或首次保存路径取消会返回原确认并保留当前文档。打开/另存为的路径选择仍使用系统文件选择器，但绑定编辑器 owner 并在工作线程等待，主窗口继续更新。保存先写入同目录临时文件再替换目标，不直接截断原文件。无法解释的 XML 根节点、扩展字段和异常内容会报错，避免静默丢失数据后覆盖保存。

两个边栏使用 egui 内置 Panel 和 ScrollArea。部件列表分为固定图片列和左对齐名称列，长名称截断；缩略图按 PNG 真实宽高等比缩放，不挤成方形。左侧船体列表支持虚拟滚动、刷新和切换目录，只列出可解析的 XML；大目录只绘制可见行，打开文件保留滚动位置。右侧部件目录支持分类、贴图、名称、说明及数量上限；选择部件后进入连续放置预览，R/X/Y 调整预览方向，Esc/右键或“取消放置”返回选择模式。预览吸附时为绿色，发生碰撞或达到数量上限时为红色；放置与连接共用一次撤销。侧栏的点击和滚轮不会编辑或缩放画布，属性模态打开时两个边栏禁用。 从部件列表任一行按住拖到画布，会出现与未连接部件相同的 `100/255` 半透明虚影；松手原子放置并吸附连接，碰撞或达到数量限制时拒绝。拖回侧栏松手、Esc/右键、失焦或文件操作取消；原有单击选择后连续放置保持不变。

编辑历史默认最多 256 步、快照容量预算 64 MiB，超出时移除最旧记录；若单步本身超过预算，仍保留最近一步。每步只保存一份快照，撤销/重做交换快照；预算不包含当前文档、保存点、渲染及分配器开销。

单击部件保留选中；实际拖动或目录拖放结束会取消选中高亮，左键点击画布空白也可取消（Shift 保留选区）。拖动连续跟随指针和抓取偏移，不再量化为半格，靠近有效连接面时仍会吸附。零件拖拽与空白平移使用当前帧相机位置/缩放，不等待上一帧的变换缓存；平移松手那帧的末段位移也会应用。编辑器关闭跨帧并行渲染，交换链最大排队帧数请求设为 1，同时保留 AutoVsync。该设置减少软件侧的旧帧排队，不保证驱动遵守提示，也不等价于实测输入到屏幕只有一帧。

多选后拖动任一已选部件可整体移动，R、Q/E、Shift+Q/E、X/Y 围绕所选部件中心整体旋转或镜像，Delete 一次删除。内部连接保持，辅助模式移动或变换时断开与未选部件的连接；自由模式保留所有已有连接；吸附和碰撞按完整选择验证。拖拽中 R、Q/E、Shift+Q/E 与 X/Y 可组合旋转和镜像，预览、贴图、连接提示共用同一姿态；旋转和位移在松手时合并成一次撤销，取消不改文档。辅助模式最终落点重叠超限时不构造新连接，落下后保留位置、断开外部连接，选区内部连接保持。开启右侧“↳ 子节点跟随”后，沿组内父→子连接递归加入所有后代（含孙节点），不加入上级父节点；跟随范围在开始拖拽时确定，默认关闭。拖到右侧部件列表松手时，按无向连接扩展并删除整个相连分量，包括父、子、孙和对接连接器，不受跟随开关影响；独立分量保留，一次撤销恢复全部部件和连接。列表显示红色删除提示，其他侧栏释放或 Esc / 右键只取消；引用重号有歧义时拒绝删除，不猜归属。复制保留内部连接、分组和内部的分级引用，粘贴分配新编号，并检查数量限制。剪贴板在切换文件时保留；预览和取消不改变文档，每次提交可一次撤销。

命中与碰撞支持目录中的凸多边形及多个 Shape 的组合、圆形车轮和默认矩形，随部件旋转与镜像。传感器不作为实体轮廓；相邻边界接触允许，辅助模式中实体重叠达到较小零件面积的 5% 时阻止新部件放置、粘贴及非拖拽的旋转和镜像，拖动时显示红色；现有部件拖拽仍可落在超限重叠位置，但不新建连接，不回退原位置。自由模式不进行该几何判定。目录的 `ignoreEditorIntersections` 仍允许着陆架等指定部件重叠。自定义 Shape 的非有限坐标、退化、自交或凹多边形会明确报错；凹轮廓需拆分成多个凸 Shape。

吸附区分固定中心点与 `Top/Bottom/Left/Right`、`*Side` 连接面；连接面允许沿边滑动，同边不同位置可以分别连接。辅助模式中固定点和共享 `group` 独占，吸附跳过已占用、类型不兼容或重叠超限的候选，不限制非直角方向，拖动时忽略即将断开的旧连接。拖动/放置时灰紫色标记表示源部件连接点或连接面，淡紫色表示附近可提交的连接候选，亮紫色表示已贴合的吸附点；标记绘制在贴图前方，圆圈保持屏幕大小。多选只高亮通过整体校验的实际吸附连接，避免显示单件可连但整体碰撞的假候选。辅助模式的新建连接在原子提交时再次检查编号、最近距离、重叠及占用；自由模式只检查端点与连接点类型。机械连接不因燃料类型不同而禁止。显式标记 `dock="true"` 的自定义连接点只允许插头与端口配对，原版 XML 中的历史对接记录仍原样保留。

属性面板使用 egui 内置 `Modal`、`TextEdit`、`ScrollArea` 和复选框，支持激活状态、燃料，以及驾驶舱中的船体名称、油门和分级。可增删、排序分级步骤，按 ID、下拉列表或上一/下一部件选择激活目标，增删动作并编辑移动标记。点击字段定位光标，Ctrl+A 全选后可替换原值；文本光标、选区、剪贴板和中文输入法由 egui 管理。点击“应用”提交整份草稿并支持一次撤销；“取消”或 Esc 放弃草稿，点击遮罩保留草稿，输入法事件同帧的 Esc 不关闭草稿。草稿打开时不处理画布快捷键，关闭窗口或拖入文件也不会丢弃草稿；需先应用或取消再继续文件操作。燃料不能为负或超过目录容量，油门范围为 0～1，分级目标必须存在且不能在同一级重复。

编辑按所属组和具体实例定位，断开组复用同一个 ID 时可分别选择、拖动、删除和编辑属性。删除及分级目标只作用于本组；跨组吸附会合并相关组，并同步重编号冲突的部件、连接与分级引用，支持一次撤销恢复原数据。打开和保存本身不重编号。同组重复 ID 的实例仍分别显示和选择；已有连接或分级若无法判定归属，相关拓扑操作会报错，不自动猜测。

绿色连接线经过部件中心及实际连接面，完全贴合时也可见。未连到主驾驶舱的部件以 `100/255` 透明度淡化，包括主组内的孤立部件；无驾驶舱时使用主组首实例作为参考根。选择或碰撞着色不取消淡化，连接及撤销/重做会更新显示；歧义重复编号的连接不猜测归属。

选中同组重复编号的部件后，F2 属性中提供“修复引用归属”。界面展示每个实例的类型、位置和预分配编号，逐条列出普通连接、对接插头及分级动作；点击各引用按钮选择归属实例，全部指定后才能应用。没有引用的重复实例可直接分配独立编号。取消不改文档，应用后可一次撤销；修复不改变位置、运行状态或其他组。已有属性草稿需先应用或取消，再进入修复。

```powershell
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p dr-core --example verify_ships -- ../Difficult-Rocket/assets/ships
cargo run -p dr-core --example verify_scoped_edits -- ../Difficult-Rocket/assets/ships
cargo run -p dr-core --example verify_catalog_edits -- ../Difficult-Rocket/assets/builtin/PartList.xml
cargo run -p dr-core --example verify_history -- ../Difficult-Rocket/assets/ships/Ophioglossum.xml
cargo run -p dr-editor -- --ship ../Difficult-Rocket/assets/ships/Test.xml --smoke-test
cargo run -p dr-editor -- --ship ../Difficult-Rocket/assets/ships/Test.xml --panel-smoke-test
cargo run -p dr-editor -- --ship ../Difficult-Rocket/assets/ships/Test.xml --properties-smoke-test
cargo run -p dr-editor -- --egui-smoke-test
cargo run -p dr-editor -- --ship ../Difficult-Rocket/assets/ships/Test.xml --native-ime-test
cargo run -p dr-editor -- --connection-smoke-test
cargo run -p dr-editor -- --transforms-smoke-test
cargo run -p dr-editor -- --scoped-smoke-test
cargo run -p dr-editor -- --selection-smoke-test
cargo run -p dr-editor -- --view-smoke-test
cargo run -p dr-editor -- --interaction-smoke-test
cargo run -p dr-editor -- --native-dialog-test
cargo run -p dr-editor -- --browser-smoke-test
cargo run -p dr-editor -- --staging-smoke-test
cargo run -p dr-editor -- --ship ../Difficult-Rocket/assets/ships/Heronb.xml --repair-smoke-test
cargo run -p dr-editor -- --ship ../Difficult-Rocket/assets/ships/Ophioglossum.xml --performance-test
cargo run -p dr-editor -- --ship ../Difficult-Rocket/assets/ships/Ophioglossum.xml --performance-test --performance-selection-count 100
python -X utf8 scripts/check_native_dialogs.py
```

`--smoke-test` 启动真实窗口，在约 5 秒后截图并自动退出，图片位于 `target/editor-smoke.png`。它验证启动和渲染，不代替交互测试。最后一条是 Windows 原生路径选择测试：使用构建好的程序，检查打开/另存为窗口属于编辑器、取消保留文档及选择期间主线程持续更新，不保存样本，也不要求抢占桌面焦点；可用 `--editor-bin <路径>` 指定独立程序。未保存确认不再是原生消息框，改由下面的主窗口模态专项验收。

`--panel-smoke-test` 在真实窗口中点击实际 egui 部件按钮并发送滚轮，画布流程通过 Bevy 输入验证预览旋转/镜像、碰撞拒绝及红色预览、放置、撤销重做、取消及侧栏输入隔离；不写回样本，输出 `target/editor-placement-preview.png`、`target/editor-collision-preview.png` 和 `target/editor-panels-smoke.png` 后退出。

`--properties-smoke-test` 与 `--egui-smoke-test` 使用同一 egui 自测：点击实际 TextEdit、Ctrl+A 中文替换、注入 egui IME 预编辑/提交事件、草稿快捷键隔离、点击分级移动标记和应用、一次撤销/重做及 XML 文件往返。自测在内存构造独立船体，输出 `target/editor-egui-draft.png`、`target/editor-egui-smoke.png` 和 `target/egui-smoke.xml`，不改写源样本。遮罩、取消、输入法取消同帧 Esc 和最小窗口非法值另有 egui 控件测试；此输入注入不能代替系统输入法验收。

`--native-ime-test` 仅适用于 Windows：在窗口创建线程通过已加载的简体中文 IMM 输入法生成真实预编辑/提交消息，经 Winit/Bevy/bevy_egui 进入 TextEdit，检查候选窗定位、草稿隔离、应用、撤销重做及 XML 往返。输出 `target/editor-native-ime.png` 和 `target/native-ime-smoke.xml`；此专项不代表所有第三方输入法或候选列表操作。

`--connection-smoke-test` 从默认新建船体开始，用原版长梁和两个分离器验证沿边吸附、同边多点连接、原子撤销/重做及保存往返，并捕获前景连接提示预览，输出 `target/editor-connection-hints.png`、`target/editor-connections-smoke.png` 和 `target/connections-smoke.xml`；运行时不要传 `--ship`。

`--transforms-smoke-test` 使用原版部件目录、独立内存夹具及实际 egui 控件/画布输入，演示并断言精细旋转按钮、15°/1° 快捷键、非直角吸附与 5% 超限断连、122° 双轴镜像组合拖拽与渲染姿态、自由模式远距离手动连/断、重叠放置不自动连接、多选整体旋转镜像、原子撤销重做及 XML 往返。同时验证可调连接线的真实控件与渲染配置、树中实际选择子树/连通分量、真实框选和后代跟随的整组精细旋转/镜像。输出十二张阶段截图和 `target/transforms-smoke.xml/json`；新增内容同时作为完整 demo 的第三章，不写回原版样本，不需要渲染视频。它采用内部输入注入，不冒充系统键鼠/输入法验收。

`--scoped-smoke-test` 使用原版分离器构造三个复用 ID 的组，通过真实窗口的鼠标及快捷键输入验证选择、拖动预览、删除、跨组吸附、撤销重做及 XML 保存往返，输出 `target/editor-scoped-smoke.png` 和 `target/scoped-smoke.xml`。`verify_scoped_edits` 在真实重复 ID 样本中每组抽取一个实例，并额外覆盖同组重复实例，检查属性编辑、撤销重做和往返；原版库目前覆盖 34 个样本、282 个实例，另 5 个异常输入使程序如实返回非零状态，不修改样本。

`--repair-smoke-test` 使用真实 `Heronb.xml` 的发动机/油箱重复编号，通过实际 UI 按钮分配三条连接及一条分级引用，验证未分配拒绝、草稿隔离、一次撤销重做、修复后的独立删除和 XML 往返，输出 `target/editor-repair-draft.png`、`target/editor-repair-smoke.png` 及 `target/repair-smoke.xml`，不写回源样本。

`--selection-smoke-test` 验证 Shift 多选及取消选择、中键/左键切换与真实轮廓框选、小幅连续跟随、松手高亮清理、空白取消选择、视角平移、整体拖动与内部连接、复制粘贴的红色碰撞/绿色吸附预览、预览旋转取消、全选删除、一次撤销重做、侧栏释放与失焦取消，以及 XML 往返。追加验证按住拖拽时 R 单次旋转及渲染姿态、取消不修改文档、重叠落点与连接裁剪、后代跟随开启/关闭、拖入真实右侧列表删除整个相连分量、非删除 UI 保护与一次撤销重做。输出 `target/editor-selection-preview.png`、`target/editor-selection-smoke.png`、`target/editor-drag-rotation.png` 和 `target/selection-smoke.xml`，截图捕获完成后才提交预览。

`--view-smoke-test` 验证整船与选区适配、窗口缩小到 960×640 后重新适配、F3 调试标签和 F4 船体显隐、F1 打开帮助与 Esc 关闭、帮助期间 R/F4 不穿透，以及文档和历史不变；输出 `target/editor-view-smoke.png` 和 `target/editor-help.png`。

`--interaction-smoke-test` 在真实渲染窗口中逐帧注入非中心抓取、亚像素和反向运动，以及两种平移按钮、三种缩放和松手末段位移，在当帧 Transform 传播后检查 167 次渲染坐标，并确认跨帧并行渲染已禁用、交换链排队提示为 1。输出 `target/editor-interaction-latency.json` 和 `target/editor-interaction-smoke.png`；这是应用同帧链路回归，**不是物理显示延迟测量**。

`--native-dialog-test` 保留旧命令名，现用于主窗口未保存模态：输出 `target/editor-unsaved-modal.png`，通过实际 egui 指针按下/释放点击“取消”，验证保留未保存文档，再次确认后实际点击“放弃修改并继续”退出；不会打开原生路径选择或写回样本。保存选项、Esc 与遮罩行为另由 headless 控件回归覆盖。

`--browser-smoke-test` 在 `target` 内生成 1,000 个小船体文件及 1 个异常 XML，验收排序、过滤、虚拟列表只绘制可见行、滚动到底、实际点击末项和保留滚动位置，输出 `target/editor-browser-smoke.png/json`。`--staging-smoke-test` 验收 64 级共 1,024 个动作的末项草稿编辑、原子应用、撤销重做和 XML 往返，输出 `target/editor-staging-draft.png`、`target/editor-staging-smoke.png` 与 `target/staging-smoke.xml`。

`--performance-test` 在真实窗口中适配船体视图并持续拖动 90 帧，确认部件实体保持不变、取消后船体数据未改动，输出 `target/editor-performance.json` 和 `target/editor-performance.png`。可加 `--performance-selection-count N` 测试从拖动锚点向外选择最近的 N 个部件，超过总数时全选；报告包含实际选择数。帧耗时包含实际吸附、碰撞和渲染流程，不是纯绘制基准；结果受硬件、驱动和构建配置影响。

船体批量校验同时对照模型和输入 XML 的原始元素、属性；遇到异常输入会报告并继续检查剩余文件，最后以失败状态退出，不修改样本。当前原版库有 185 个正常船体和 5 个被拒绝的异常输入，详见进度文档。


### Windows 前台系统键鼠回归

```powershell
cargo build -p dr-editor
uv run --no-project --python 3.12 python -X utf8 scripts/check_native_input.py
```

该脚本只操作自己启动的编辑器和失焦辅助窗口，验证真正的系统前台句柄，再通过系统鼠标和 `SendInput` 运行多选拖动、目录虚影、空白放置/吸附连接、原子撤销重做及侧栏/Esc/真实失焦取消。应用侧只观察，不伪造焦点或直接注入 Bevy/egui 输入；虚影截图捕获完成后才松手，并检查 XML 往返和源样本未改动。输入法/系统文字专项按当前要求跳过，报告明确记为未运行。

窗口驱动可用 `--editor-bin <编辑器程序路径>` 指定独立构建产物，避免覆盖正在运行的默认程序；例如 `cargo rustc -p dr-editor --bin dr-editor -- -o target/drag-editor.exe` 后以 `--editor-bin target/drag-editor.exe` 验收，不强制关闭已有编辑器窗口。

置前/失焦干扰最多自动重试三次，连续失败时尝试通过本机 noticer 的 `sr1` 房间提醒；程序断言和数据错误直接失败，不作为焦点失败重试。测试时会短暂置前窗口，不永久置顶、不向其他应用发送快捷键。产物位于 `target/foreground-*`，最近成功路径记录在 `target/foreground-last.txt`。可用 `--window-case keys` 运行真实系统快捷键验收，使用 `--window-case panels|connections|transforms|scoped|selection|view|topology|interaction|unsaved` 在受控前台运行已有注入式窗口回归，或 `--window-case performance --selection-count 1000` 测量拖动；前台受控不意味着这些旧回归已改为系统输入。

前台驱动自身的无副作用回归：`uv run --no-project --python 3.12 python -X utf8 -m unittest discover -s scripts -p test_native_input_driver.py`。


## 可调画布连接线

点击右侧“连接线设置…”即可调整显示开关、任意颜色与透明度、1–12px 粗细、实线/虚线/点线、虚线间距、静态/呼吸/流动效果、效果速度和父→子方向箭头；可一键恢复默认，Esc 关闭。

设置只影响当前会话的画布显示，不改连接、XML 或撤销历史，切换船体仍保留；重启恢复默认。隐藏连线不会隐藏自由模式的可点击端点/边。设置窗口打开时隔离画布编辑快捷键；自由拖动单个端点时，保留的连线会实时跟随预览。

## 连接树与连接图

F6 或右侧入口打开同一份船体的连接编辑窗口，两个视图共用文档和撤销历史：

1. **连接树**展示确定性的生成森林。点击节点可选择部件，“选择子树”后可一次删除整支并撤销；折叠只影响显示。环、多父关系不会被剪掉，额外边计数仍可在图视图中编辑。
2. **连接图**显示有向连接，支持点击节点或连线、选择连通分量、添加普通/对接连接、精确断开一条边。对接插头标记为父/子之一；历史文件中独立的第三引用也会保留显示，跨组编号不会混用。歧义或悬空边单独列出，不猜测端点。
3. 先选节点并点击“选中设为父/子”，或在下拉列表选择端点；选择双方的连接点后，树模式“设置父节点”原子替换唯一父边，图模式“添加连接”允许合法环。树模式拒绝将节点接到自身/后代，也不擅自清除多父边。
4. **辅助模式遵守邻近距离、实体重叠小于 5%、类型与占用规则**；距离过远时先在物理画布移动部件。自由模式跳过几何和占用判定，但不跳过树模式的环路/多父保护。拖动图节点只改变图的排版，不会移动船体或新增撤销记录。
5. 使用窗口内的撤销/重做按钮恢复操作。失败保留文档及重做；窗口打开期间隔离物理画布的编辑快捷键，并取消尚未提交的放置/拖动预览。

## 可复用核心接口

- `catalog_from_xml` / `load_catalog` 完整读取静态规格，`catalog_to_xml` / `save_catalog` 输出目录 XML；目录安全保存和船体保存一样使用同目录临时文件替换。可选物理字段保留未声明与显式零值的区别。
- `PartCatalog.types` 可以直接插入、删除、重排或重命名；`get` 始终查询当前列表，同 ID 类型按当前首项处理，不保留易失效的旁路索引。
- `Ship::mass(catalog, ShipScope::Main)` 只计算主组，`ShipScope::All` 计算主组和全部断开组；旧 `total_mass` 保持 all，HUD 同时显示 main/all。
- `Ship.name/description` 是船体元数据，不是驾驶舱名称。`EditorCommand::SetMetadata` 可原子修改；非空值通过可选 XML 根属性保存，空值不添加属性，纳入撤销和历史内存预算。
- `part_bounds` / `ship_bounds` 返回 Ship 单位的物理轮廓范围；`image_corners` / `image_bounds` 返回像素范围，原图尺寸由调用方提供，保留奇数像素锚点和镜像。没有部件时范围为 `None`，核心不依赖 Bevy 或 GPU。
- `Topology::from_ship` 提供精确实例连接图，`forest` 提供不丢边的生成森林，`reachable` 与 `subtree` 支持图分量和树分支选择；拓扑修改使用 `ConnectParts`、`Reparent`、`RemoveConnection` 等原子命令。


### 新增核心和拓扑回归

```powershell
cargo run --locked -p dr-core --example verify_catalog -- ../Difficult-Rocket/assets/builtin/PartList.xml
uv run --no-project --python 3.12 python -X utf8 scripts/check_native_input.py --window-case topology
```

目录校验逐项比较原始 XML 元素/属性，不修改原文件。拓扑窗口专项经过实际 egui 控件，覆盖树选择/删除/换父、拒绝树环、图环和断边、撤销重做、最小窗口与 XML 往返；它属于**系统前台受控的注入式回归**，不是系统键鼠专项。截图及数据输出为 `target/editor-topology-{tree,graph,smoke}.png` 与 `target/topology-smoke.xml/json`。

## ICU4X 中文/日文告警

当前 Bevy 0.19.1 / Parley 0.9 的轻量分段器未加载完整 CJK 字典，动态中文 HUD 可能反复输出 No segmentation model for complex script: Chinese/Japanese（旧版消息为 No segmentation model for language: ja）。这是已知上游限制，参见 [Bevy #24094](https://github.com/bevyengine/bevy/issues/24094)。

本项目与相邻 bevy-pvz 使用相同 workaround：显式启用 icu_provider 的 logging 特性，将原本直接写入 stderr 的诊断接入 Bevy 日志；默认仅对 icu_provider 设为 error，避免已知 warn 刷屏，其他模块日志和 ICU 真正的 error 保留。仍使用原有回退分段，不声称已经启用完整 CJK 词典，也不修改字体或输入法。RUST_LOG 仍遵守 Bevy 的标准覆盖行为；需要排查时可显式打开 icu_provider=warn。

窗口驱动在正常退出和产物校验后检查本次 editor.log，发现任一版本的缺失模型告警即失败，并在成功报告中记录 icu_segmentation_warnings_absent。依赖升级后若官方提供兼容的复杂文字分段支持，再评估启用并移除该 workaround。
