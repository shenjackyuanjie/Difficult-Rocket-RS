# Difficult-Rocket Rust 编辑器

独立的 Bevy 编辑器及可复用的 SR1 数据核心。当前仍在补齐功能，完整复刻尚未完成；差异与验证记录见 [编辑器进度](docs/editor-progress.md)。

从本仓库根目录运行，默认复用相邻 `Difficult-Rocket/assets` 中的部件目录、贴图和字体：

```powershell
cargo run -p dr-editor -- --ship ../Difficult-Rocket/assets/ships/Test.xml
```

省略 `--ship` 创建与原版一致、带默认驾驶舱的新船体；自定义目录没有驾驶舱类型时才创建空白文档。可用 `--catalog <PartList.xml>`、`--assets <资源目录>` 指定资源。加载失败会报错退出，不会用空白文档代替错误输入。

| 操作 | 按键 |
| --- | --- |
| 选择、移动并吸附部件 | 左键点击、拖动 |
| 取消拖动预览 | Esc、右键；窗口失焦时自动取消 |
| 切换待放置部件、放置 | 当前分类中 Tab / Shift+Tab 切换；左键点击画布或 P 放置 |
| 删除、旋转、镜像 | Delete、R、X / Y |
| 撤销、重做 | Ctrl+Z、Ctrl+Y |
| 属性与分级编辑 | F2 或顶部“属性 / 分级”；未选部件时打开驾驶舱 |
| 新建、打开 | Ctrl+N、Ctrl+O；支持单个 XML 文件拖入 |
| 保存、另存为 | Ctrl+S、Ctrl+Shift+S；首次保存选择目标文件 |
| 鼠标位置缩放、平移、视图复位 | 滚轮、中键拖动、Home |
| 截图 | F12，输出到当前目录 |

文件操作也可使用顶部工具栏。新建、打开和退出前会提示保存、放弃或取消；保存或打开失败时保留当前文档。保存先写入同目录临时文件再替换目标，不直接截断原文件。无法解释的 XML 根节点、扩展字段和异常内容会报错，避免静默丢失数据后覆盖保存。

左侧船体列表支持滚动、刷新和切换目录，只列出可解析的 XML。右侧部件目录支持分类、贴图、名称、说明及数量上限；选择部件后进入连续放置预览，R/X/Y 调整预览方向，Esc/右键或“取消放置”返回选择模式。预览吸附时为绿色，发生碰撞或达到数量上限时为红色；放置与连接共用一次撤销。侧栏的点击和滚轮不会编辑或缩放画布。

命中与碰撞支持目录中的凸多边形及多个 Shape 的组合、圆形车轮和默认矩形，随部件旋转与镜像。传感器不作为实体轮廓；相邻边界接触允许，实体内部重叠会阻止放置、移动、旋转和镜像，拖动时显示红色。目录的 `ignoreEditorIntersections` 仍允许着陆架等指定部件重叠。自定义 Shape 的非有限坐标、退化、自交或凹多边形会明确报错；凹轮廓需拆分成多个凸 Shape。

吸附区分固定中心点与 `Top/Bottom/Left/Right`、`*Side` 连接面；连接面允许沿边滑动，同边不同位置可以分别连接。固定点和共享 `group` 独占，吸附会跳过已占用、方向不兼容或产生碰撞的候选，拖动时忽略即将断开的旧连接。新建连接在原子提交时再次检查编号、实际接触及占用；机械连接不因燃料类型不同而禁止。显式标记 `dock="true"` 的自定义连接点只允许插头与端口配对，原版 XML 中的历史对接记录仍原样保留。

属性面板支持激活状态、燃料，以及驾驶舱中的船体名称、油门和分级。可增删、排序分级步骤，按 ID 或上一/下一部件选择激活目标，增删动作并编辑移动标记。点击字段后输入会替换原值；支持中文输入法、Ctrl+A、方向键、Home/End、Backspace/Delete。点击“应用”提交整份草稿并支持一次撤销；“取消”或 Esc 放弃草稿。草稿打开时不处理画布快捷键，关闭窗口或拖入文件也不会丢弃草稿；需先应用或取消再继续文件操作。燃料不能为负或超过目录容量，油门范围为 0～1，分级目标必须存在且不能在同一级重复。

编辑按所属组和具体实例定位，断开组复用同一个 ID 时可分别选择、拖动、删除和编辑属性。删除及分级目标只作用于本组；跨组吸附会合并相关组，并同步重编号冲突的部件、连接与分级引用，支持一次撤销恢复原数据。打开和保存本身不重编号。同组重复 ID 的实例仍分别显示和选择；已有连接或分级若无法判定归属，相关拓扑操作会报错，不自动猜测。明确分配这些歧义引用的修复入口尚待补齐。

```powershell
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p dr-core --example verify_ships -- ../Difficult-Rocket/assets/ships
cargo run -p dr-core --example verify_scoped_edits -- ../Difficult-Rocket/assets/ships
cargo run -p dr-editor -- --ship ../Difficult-Rocket/assets/ships/Test.xml --smoke-test
cargo run -p dr-editor -- --ship ../Difficult-Rocket/assets/ships/Test.xml --panel-smoke-test
cargo run -p dr-editor -- --ship ../Difficult-Rocket/assets/ships/Test.xml --properties-smoke-test
cargo run -p dr-editor -- --connection-smoke-test
cargo run -p dr-editor -- --scoped-smoke-test
cargo run -p dr-editor -- --ship ../Difficult-Rocket/assets/ships/Ophioglossum.xml --performance-test
python -X utf8 scripts/check_native_dialogs.py
```

`--smoke-test` 启动真实窗口，在约 5 秒后截图并自动退出，图片位于 `target/editor-smoke.png`。它验证启动和渲染，不代替交互测试。最后一条是 Windows 原生对话框测试：使用构建好的程序，自动验证取消和退出保护，不保存样本，也不要求抢占桌面焦点。

`--panel-smoke-test` 在真实窗口中通过 Bevy 输入与 UI 状态验证预览旋转/镜像、碰撞拒绝及红色预览、放置、撤销重做、取消及侧栏输入隔离；不依赖系统桌面焦点、不写回样本，输出 `target/editor-placement-preview.png`、`target/editor-collision-preview.png` 和 `target/editor-panels-smoke.png` 后退出。

`--properties-smoke-test` 验证中文输入消息、草稿输入隔离、分级激活动作、应用、一次撤销/重做、取消及 XML 文件往返。输出 `target/editor-properties-draft.png`、`target/editor-properties-smoke.png` 和 `target/properties-smoke.xml`，不改写源样本。输入法测试注入 Bevy 的 IME 消息，尚未覆盖各系统输入法的候选窗口。

`--connection-smoke-test` 从默认新建船体开始，用原版长梁和两个分离器验证沿边吸附、同边多点连接、原子撤销/重做及保存往返，输出 `target/editor-connections-smoke.png` 和 `target/connections-smoke.xml`；运行时不要传 `--ship`。

`--scoped-smoke-test` 使用原版分离器构造三个复用 ID 的组，通过真实窗口的鼠标及快捷键输入验证选择、拖动预览、删除、跨组吸附、撤销重做及 XML 保存往返，输出 `target/editor-scoped-smoke.png` 和 `target/scoped-smoke.xml`。`verify_scoped_edits` 在真实重复 ID 样本中每组抽取一个实例，并额外覆盖同组重复实例，检查属性编辑、撤销重做和往返；原版库目前覆盖 34 个样本、282 个实例，另 5 个异常输入使程序如实返回非零状态，不修改样本。

`--performance-test` 在真实窗口中适配船体视图并持续拖动 90 帧，确认部件实体保持不变、取消后船体数据未改动，输出 `target/editor-performance.json` 和 `target/editor-performance.png`。帧耗时包含实际吸附、碰撞和渲染流程，不是纯绘制基准；结果受硬件、驱动和构建配置影响。

船体批量校验同时对照模型和输入 XML 的原始元素、属性；遇到异常输入会报告并继续检查剩余文件，最后以失败状态退出，不修改样本。当前原版库有 185 个正常船体和 5 个被拒绝的异常输入，详见进度文档。
