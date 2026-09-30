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
| 新建、打开 | Ctrl+N、Ctrl+O；支持单个 XML 文件拖入 |
| 保存、另存为 | Ctrl+S、Ctrl+Shift+S；首次保存选择目标文件 |
| 鼠标位置缩放、平移、视图复位 | 滚轮、中键拖动、Home |
| 截图 | F12，输出到当前目录 |

文件操作也可使用顶部工具栏。新建、打开和退出前会提示保存、放弃或取消；保存或打开失败时保留当前文档。保存先写入同目录临时文件再替换目标，不直接截断原文件。无法解释的 XML 根节点、扩展字段和异常内容会报错，避免静默丢失数据后覆盖保存。

左侧船体列表支持滚动、刷新和切换目录，只列出可解析的 XML。右侧部件目录支持分类、贴图、名称、说明及数量上限；选择部件后进入连续放置预览，R/X/Y 调整预览方向，Esc/右键或“取消放置”返回选择模式。预览吸附时为绿色，达到数量上限时为红色；放置与连接共用一次撤销。侧栏的点击和滚轮不会编辑或缩放画布。

```powershell
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p dr-core --example verify_ships -- ../Difficult-Rocket/assets/ships
cargo run -p dr-editor -- --ship ../Difficult-Rocket/assets/ships/Test.xml --smoke-test
cargo run -p dr-editor -- --ship ../Difficult-Rocket/assets/ships/Test.xml --panel-smoke-test
python -X utf8 scripts/check_native_dialogs.py
```

`--smoke-test` 启动真实窗口，在约 5 秒后截图并自动退出，图片位于 `target/editor-smoke.png`。它验证启动和渲染，不代替交互测试。最后一条是 Windows 原生对话框测试：使用构建好的程序，自动验证取消和退出保护，不保存样本，也不要求抢占桌面焦点。

`--panel-smoke-test` 在真实窗口中通过 Bevy 输入与 UI 状态验证预览旋转/镜像、放置、撤销重做、取消及侧栏输入隔离；不依赖系统桌面焦点、不写回样本，输出 `target/editor-placement-preview.png` 和 `target/editor-panels-smoke.png` 后退出。

船体批量校验同时对照模型和输入 XML 的原始元素、属性；遇到异常输入会报告并继续检查剩余文件，最后以失败状态退出，不修改样本。当前原版库有 185 个正常船体和 5 个被拒绝的异常输入，详见进度文档。
