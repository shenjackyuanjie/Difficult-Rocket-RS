# 编辑器复刻进度与验收依据

目标保持为完整复刻 Difficult-Rocket 编辑器相关逻辑并补全功能。以下是 2026-10-01 的中间检查点，不能视为完成验收。

## 对照来源

- `Difficult-Rocket/mods/dr_game/sr1_ship.py`：船体文件选择、贴图与断开组渲染、镜像旋转、视图平移缩放、文件拖入、保存、截图和调试显示。
- `Difficult-Rocket/mods/dr_game/Difficult_Rocket_rs/src/src/sr1_parse/raw/ship.rs`：SR1 XML 结构、默认值、部件运行状态、分级与两类连接。
- `Difficult-Rocket/mods/dr_game/Difficult_Rocket_rs/src/src/python/editor.rs`：编辑区域命中形状。
- `Difficult-Rocket/assets/builtin/PartList.xml`、`assets/ships`：真实目录及船体样本。

## 当前已实现并验证

- XML 输出根节点为 `Ship`，断开组使用 `DisconnectedParts/DisconnectedPart` 层级。保留 Pod、Staging、Tank/Engine 区分、降落伞运行字段和两类连接。
- 批量编辑原子执行；拖动与吸附一次撤销。失败和无变化命令不破坏重做栈；保存点按文档内容比较。
- 删除同时清理主船体、断开组、对接引用及分级激活引用。跨组连接合并相关组；拒绝反向重复连接和缺失部件引用。
- 部件尺寸采用 PartList 的 30 像素单位，实例位置采用 Ship 的 60 像素单位。矩形命中和碰撞支持任意旋转；连接点编号按 SR1 从 1 开始。
- 放置、选择、拖动预览、取消、删除、旋转、镜像、保存及撤销重做。拖动预览不直接修改文档。
- 原版贴图、中文字体、HUD、连接线、缩放、平移、复位和截图。

验证记录：

- `cargo test --workspace`：20 项通过（15 项核心、5 项编辑器）。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- `cargo build -p dr-editor`：通过。
- 真实 Vulkan 窗口加载 `Test.xml` 并截图，已人工检查贴图与中文显示。尚未覆盖所有鼠标与快捷键组合。
- Bevy 0.19 的 Parley 使用 `new_for_non_complex_scripts`，运行时仍报告 ICU4X 中文分词模型缺失；截图中的中文字形正常。关闭自动换行未消除该日志，后续需检查上游分词配置，不应把它当作字体加载失败。
- 190 个样本中 186 个、125,860 个部件完成**当前模型字段**的读写往返。此项不证明未建模字段无损保留。
- 4 个输入被拒绝：`Alliance.xml` 标签错配；`Amect-MAIIa TEST.xml` 的 `currentStage` 为空；`AmectVII.xml` 和 `Ss.xml` 使用非标准连接节点。批量校验如实返回非零状态。
- 主分支原有 3 个提交信息已改中文，文件树与作者信息保留；旧历史备份引用为 `refs/backup/before-chinese-messages`。没有远程仓库或推送操作。

## 后续必须完成的工作

1. 文件工作流：打开、另存为、新建、原版文件列表/拖入、未保存提示、失败时保留原文档、可靠替换保存、退出保护。
2. 图形化部件目录和放置预览：分类/选择/说明、快捷键与 UI 状态一致；目前仅能用 Tab/P。
3. 编辑完整性：碰撞规则接入操作，连接点占用与类型兼容、吸附预览、复杂 Shape 命中、多选/复制等编辑能力及端到端交互测试。
4. 属性与分级编辑：名称、燃料、激活状态、分级动作的编辑及撤销重做。
5. XML 兼容性审计：当前模型未涵盖所有原始扩展字段，例如 `Test.xml` 中的 `lower/raise/length/legAngle`；完整复刻不得以模型往返测试代替输入字段覆盖检查。对异常样本明确界定兼容或拒绝策略。
6. 原版显示对齐：复杂部件与连接点语义、调试显示、断开组完整渲染验收、大船体渐进渲染或性能处理、视图适配。当前未做大船体窗口性能验收。
7. 全量验收：逐项比较原版行为、补齐必要回归测试与真实交互验证，所有功能块使用中文提交并继续通过 noticer 上报。上述缺口消除前，保持目标进行中。
