# 演示评审建议

此页维护已采纳项、已实施的报告合同和**尚未实施的回放候选**，不是运行验收报告。新版底栏、抽屉、显式生成及真实拖放的操作见[使用指南](editor-usage.md#底部图标栏与抽屉)。实际运行统一见[当前进度](editor-progress.md)与[历史验收](validation-history.md)，命令见[验证指南](validation.md)。

## 基线与维护规则

当前 brief 为 **11 章**；detailed 为 **23 章、54 例、114 个章节产物**。54 例由 51 个边界用例与独立 `rocket-workflow` 的 3 个连续用例组成，不把边界用例数当成总数。当前合同由旧基线逐项叠加，原用例及压力数字保持不变：

| 范围 | brief 章数 | detailed 章数 | detailed 用例 | detailed 章节产物 |
| --- | --- | --- | --- | --- |
| 原有基线 | 10 | 19 | 45 | 95 |
| 连续四物理推进级火箭新增 | 1 | 1 | 3 | 9 |
| 触摸、千件拖动、窄窗新增 | 0 | 3 | 6 | 10 |
| 当前报告合同 | 11 | 23 | 54（51 边界 + 3 连续火箭） | 114 |

原 45 个边界用例仍由 Rust [manifest](../crates/dr-editor/src/demo_cases/manifest.rs)维护；触摸 2 例、千件拖动 1 例、窄窗 3 例将边界用例扩展为 51 例，连续火箭另有 3 例由独立工作流报告承载。统计口径由[驱动合同回归](../scripts/test_demo_editor.py)核对；原 `staging` 的 **64 级、1024 动作**压力章继续保留，新的八卡入门示例不替代它。后续新增 case 时需同步 Rust 实现/测试与 Python [驱动](../scripts/demo_editor.py)的契约，不仅改数字。已有覆盖不重复加需求，也不把内部输入回放当成系统键鼠或 input-to-photon 证据。

## 已采纳

- 连接提示与实际提交核对完整 Connection，反向换点拒绝读取真实错误状态；平移锚点保留既有容差。
- 零长路径使用真实中心挂点，走父中心→挂点→子中心的生产路径，验证有限/零弧长、方向采样 None、真实 Gizmos 顶点增量 0 和安全跳过；保留正常线正向对照与目录复原。[实现](../crates/dr-editor/src/demo_cases.rs)、[回归](../crates/dr-editor/src/demo_cases/tests.rs)。旧“实际画出零长线”指标已删除，不重加兼容项。
- detailed 驱动继续核对原 45 例的通用 metrics、有限几何误差/严格 5%、实际手势计数及 physical=false；加上触摸、千件拖动与窄窗 6 例后为 51 个边界用例，另有工作流 3 例，总计 54 例，并核对章节顺序及新鲜非空证据。[驱动](../scripts/demo_editor.py)、[回归](../scripts/test_demo_editor.py)、[工作流严格验收](../scripts/rocket_workflow_report.py)。
- 分类演示复用真实组标题/全部展开及既有滚动寻址，不增加演示状态或延长超时。

## 连续四物理推进级火箭章合同

章 ID 为 `rocket-workflow`，标题为“三级芯级 + 双固体助推器：真实搭建、自动分级与拖放调级”。brief 顺序为 `connections → rocket-workflow → transforms`；detailed 顺序为 `connections → rocket-workflow → geometry-cases`。章内三阶段连续使用同一文档，不在阶段间重新加载船体。章定义见[章节清单](../crates/dr-editor/src/demo/chapters.rs)，实现见[rocket_workflow.rs](../crates/dr-editor/src/demo/rocket_workflow.rs)。

本节记录实现与报告合同，**不表示新版完整 GUI 已通过**。新版入口的全章回放仍需统一重跑；不得复用旧截图或旧报告替代本次证据，也不在此页追加验收历史。

### 三阶段与独立用例

| 用例 ID | 连续操作 | 必须成立的结果 |
| --- | --- | --- |
| `rocket-assembly` | 从新文档已有的 Command Pod 开始；逐次真实命中 Parts 目录选件、移动光标查看预览，再由生产画布鼠标放置并吸附 | 起点为 ID 1 的隐藏 `pod-1`、0 连接、空分级；预览不改文档。终点为 14 部件、13 条 Normal 树边、5 台发动机、4 个分离器和 1 个返回降落伞，连接端点一致；13 次放置产生 13 步历史 |
| `basic-staging` | 显式点击 Generate，检查草稿，再 Apply、Undo/Redo 与 XML 往返 | 生成草稿为 `[[11,13],[9],[10,12],[7],[6],[4],[3],[14]]`，执行索引 0、全部 `moved=false`；生成不写文档，Apply 仅新增一步历史 |
| `manual-staging-drag` | 跨帧拖发动机 9 到双分离器 10/12 所在卡尾、移回原卡、再移出；整卡下移再回置；Cancel 一次后重新打开、再次操作并 Apply 一次，再 Undo/Redo 与 XML 往返 | 手动计划为 `[[11,13],[],[10,12,9],[7],[6],[4],[3],[14]]`，保留空卡，仅 9 为 `moved=true`；取消后文档仍为自动计划且历史不变，最终 Apply 仅再新增一步历史 |

三级芯级的实际串联顺序为 `pod-1 → fueltank-0 → engine-0 → detacher-1 → fueltank-0 → engine-0 → detacher-1 → fueltank-0 → engine-0`，ID 依次为 1～9。两侧各仅由侧挂分离器连接 SRB 固体发动机：左侧为 ID 10 分离器 → ID 11 SRB，右侧为 ID 12 分离器 → ID 13 SRB；顶部返回降落伞为 ID 14。SRB 是 Normal 树的叶子，自带固体 Tank，无需额外挂罐，顶部也不连接油箱。对称双固体助推器共同构成一个物理推进级，与三级芯级合计四个物理推进级；它们的八张动作卡不是八个物理推进级。驾驶舱已由新文档提供，不从可见目录寻找隐藏 Pod，也不直接插入部件或调用放置命令冒充鼠标拼装。放置位置由目录连接点和当前相机世界投屏动态计算，连接端点误差须小于 `1e-5`。

基础分级按 Pod 同组 Normal 无向树深度从深到浅、同深度同种类归组；这里只生成八张草稿卡，手动操作不会重新生成。搭建阶段为空计划；生成后，自动到手动调级全过程的动作目标多重集始终为 `[3,4,6,7,9,10,11,12,13,14]`，不丢失、不复制目标。

### 真实拖放与数据断言

动作拖源使用真实裁剪后的 `SelectActivation` 矩形，卡片拖源/目标使用 `SelectStage` 标题矩形，不依赖固定屏幕坐标。动作落标题追加卡尾；整卡落标题下半部后插、上半部前插，最终索引按移除源后的列表计算。报告中的六次拖动依次为动作 9 从卡 1 跨到卡 2、整卡 0 下移到卡 1、整卡回置、动作 9 移回卡 1、再次跨到卡 2，以及取消后再次将动作 9 跨卡移出；每次必须记录四个独立帧：

`press → held_threshold → held_target → release`

必须包含真实按下、保持按住并越过阈值、保持按住移到目标、释放；帧号与事件顺序一致，不用同帧 click 代替 drag。仅跨卡动作 9 变为 `moved=true`，整卡重排保持动作和标记；预览、生成草稿、未释放的拖动及 Cancel 都不能改文档或历史。

三份 XML 必须保持**相同的 14 部件几何、SRB 固体 Tank 状态与 13 条 Normal 树边的实际端点**，仅分级计划按 assembled（空）、basic（八卡、全 false）、manual（八卡含空卡、仅 9 为 true）变化。严格验收同时核对指标、目标多重集、拖动事件与帧序、XML 和证据新鲜度；缺失、空文件、陈旧或相互不符的证据都应失败。[Python 验收](../scripts/rocket_workflow_report.py)、[负向回归](../scripts/test_rocket_workflow_report.py)、[Rust 工作流回归](../crates/dr-editor/src/demo/tests/rocket_workflow.rs)。

### 九个新增章节产物

五张 PNG、三份 XML、一份 JSON，文件名固定如下；这是待运行生成的产物合同，不是当前目录已有证据的声明：

```text
editor-rocket-command-pod.png
editor-rocket-placement-preview.png
editor-rocket-assembled.png
editor-rocket-generated.png
editor-rocket-manual.png
rocket-assembled.xml
rocket-basic-staging.xml
rocket-manual-staging.xml
rocket-workflow.json
```

这九项与原有 95 项构成加入连续火箭后的中间基线（20 章、104 项产物）；再计入触摸、千件拖动与窄窗三章的 10 项产物，当前 detailed 合计 23 章、114 项产物。XML/JSON 用于逐字段核验，截图用于观察连续过程，二者不能相互替代。

## 优先候选

### ① 悬空引用下的新放置 ID

- 现状：next_part_id 通常取已占 ID 最大值 +1，触及 i64 上界时从低位寻找空正数（[model.rs](../crates/dr-core/src/model.rs)、[编号分配助手](../crates/dr-core/src/part_id.rs)），occupied_ids 覆盖连接上的悬空引用（[同文件](../crates/dr-core/src/model.rs)）；修复与粘贴路径同样预留既有 ID（[repair.rs](../crates/dr-core/src/edit/repair.rs)、[selection.rs](../crates/dr-core/src/edit/selection.rs)、[scoped.rs](../crates/dr-core/src/edit/scoped.rs)）；放置入口使用该 ID（[placement.rs](../crates/dr-editor/src/placement.rs)）。
- 缺口：已有 unit 覆盖（[model_tests.rs](../crates/dr-core/src/model_tests.rs)、[repair_tests.rs](../crates/dr-core/src/edit/repair_tests.rs)），缺可见 demo。
- 回放内容：id1 + 缺失 id2 的连接下，预览新 ID 为 3，提交实际 ID 也为 3；历史引用原样保留，undo/redo 与 XML 往返一致。
- 独立断言：预览 ID == 提交 ID == 3；悬空引用在提交、撤销、XML 后逐字段相同。
- 成本：约 1 个 case。

### ② 滚轮缩放与光标锚定

- 现状：滚轮按行/像素换算 delta，scale 钳制 0.1–100，并用光标相对屏幕中心的偏移校正平移锚点（[camera_controls 的滚轮段](../crates/dr-editor/src/view.rs)）；触控缩放按两指中点保持世界锚点（[camera_controls 的触控段](../crates/dr-editor/src/view.rs)）；已有各缩放档平移像素校验与光标取当前缩放（[interaction_tests.rs](../crates/dr-editor/src/canvas_input/interaction_tests.rs)）、键盘与注入输入延迟覆盖（[input_latency_tests.rs](../crates/dr-editor/src/canvas_input/input_latency_tests.rs) 的滚轮与窗口尺寸例）。
- 缺口：滚轮缩放没有 demo 回放，拖动中缩放不漂移也未被断言。
- 回放内容：连续滚轮放大缩小，光标下的世界点不动；到 0.1 与 100 边界不再变化；按住平移按钮的同一动作序列中滚轮不引入额外漂移。
- 独立断言：缩放前后光标世界点误差 < 1e-4；clamp 后 scale 恰为 0.1 与 100；拖动 + 滚轮后平移误差 < 0.002 世界像素。
- 成本：约 1 个 case。

### ③ 已占候选跳过、次近空闲候选

- 现状：提示按 0.75 距离列举候选并过滤，每对部件只保留最近的一个合法候选（[attachment_hints.rs 的 nearby](../crates/dr-editor/src/attachment_hints.rs)）；吸附阈值仍是 0.35（[connections.rs](../crates/dr-core/src/connections.rs)）。候选章已断言 'geometric_candidates'、'displayed_pairs: 1' 与提交一致（[gestures.rs](../crates/dr-editor/src/demo_cases/gestures.rs)），unit 覆盖见 [panels/tests.rs](../crates/dr-editor/src/panels/tests.rs) 与 [attachment_hints/tests.rs](../crates/dr-editor/src/attachment_hints/tests.rs)。
- 缺口：「最近候选已占用 → 跳过 → 次近空闲候选可提示」缺少可见回放。
- 独立断言：已占候选不出现在提示集合；演示提示的连接与提交连接完全相同。
- 成本：约 1 个 case。

### ④ 吸附阈值精确边界

- 现状：常量 'CONNECTION_DISTANCE = 0.35'（[connections.rs](../crates/dr-core/src/connections.rs)），测试用 0.35 与 0.350001 边界（[selection/tests.rs](../crates/dr-editor/src/selection/tests.rs)）。
- 回放内容：用精确本地 cursor 参数避开像素量化，分别在 0.35 与 0.3505 落点提交。
- 独立断言：0.35 吸附成功且连接点与提示一致；0.3505 不吸附但仍成功放置独立部件（无连接、无提示），历史 +1、dirty 变真，撤销后回到初始。
- 成本：约 1 个 case。

### ⑤ UI 遮挡期间冻结与取消回位

- 现状：'UiPointer.blocked' 时鼠标编辑提前返回，并按释放取消拖动/框选、清理放置预览（[mouse_editor 的 blocked 段](../crates/dr-editor/src/canvas_input.rs)），相机控制同时清空拖动并丢弃滚轮（[camera_controls 的 blocked 段](../crates/dr-editor/src/view.rs)）；Esc/右键取消与失焦、出窗取消已实现（[mouse_editor 的取消段](../crates/dr-editor/src/canvas_input.rs)），拖到目录上松手删除另有独立分支（[同文件](../crates/dr-editor/src/canvas_input.rs)）；UI 归属由模态阻塞与面板打开两处置位（[egui_ui.rs](../crates/dr-editor/src/egui_ui.rs)、[同文件](../crates/dr-editor/src/egui_ui.rs)）。相机侧 blocked 行为已有覆盖（[interaction_tests.rs](../crates/dr-editor/src/canvas_input/interaction_tests.rs)），UI 上释放取消与失焦取消已由 [selection_smoke.rs](../crates/dr-editor/src/selection_smoke.rs) 与 [Escape 取消段](../crates/dr-editor/src/selection_smoke.rs) 覆盖，不重复提。
- 缺口：缺「遮挡期间部件 Transform 逐帧冻结、解除遮挡后抓取偏移保持、右键取消拖动/框选当帧视觉回位」的可见回放；相关实现与 unit 已在位，不属于新增实现项。
- 独立断言：blocked 期间部件 Transform 逐帧不变；恢复后拖动位置含原抓取偏移；右键当帧预览消失且文档与 dirty 不变。
- 成本：1–2 个 case（先拖动，后框选）。

### ⑥ 剪贴板失败与上限

- 现状：粘贴走统一 apply 路径（[edit.rs](../crates/dr-core/src/edit.rs)）；数量上限检查在 [selection.rs 的 paste](../crates/dr-core/src/edit/selection.rs)；同组重复 ID 复制拒绝在 capture（[同文件](../crates/dr-core/src/edit/selection.rs) 报 'AmbiguousReference'）。
- 缺口：超上限粘贴与重复 ID 拒绝无回放，预览、文档、历史、报错四者一致性未被断言。
- 独立断言：超限时提交被拒且粘贴预览保留（可继续定位或用 Esc 取消），status 为真实错误，undo/redo 长度不变；重复 ID 复制被拒且剪贴板保留旧内容。
- 成本：约 1–2 个 case。

### ⑦ Dock 独立第三引用换父拒绝

- 现状：换父走 'topology::reparent'（[topology.rs](../crates/dr-core/src/edit/topology.rs)），独立第三引用的 Dock 换父拒绝在 [同文件第三引用分支](../crates/dr-core/src/edit/topology.rs)，connect 路径的 Dock 占用拒绝在 [connections.rs](../crates/dr-core/src/connections.rs)；树换父拒绝环、多父歧义与单次历史项已有 unit（[topology/tests.rs](../crates/dr-core/src/topology/tests.rs)、[多父歧义](../crates/dr-core/src/topology/tests.rs)、[单历史项](../crates/dr-core/src/topology/tests.rs)）。
- 缺口：Dock 作为独立第三引用时换父被拒绝缺少 demo，「精确断边一次 undo」与 XML 需同场景可见。
- 独立断言：换父被拒后 dock 引用原样且 undo 长度不变；一次断边只产生一个 undo 步，undo 后 XML 与初始一致。
- 成本：约 1 个 case。

## 展示层改进

- 对照轮廓展示 E→X 与 X→E 差异及 XX/YY 复位：现有用例保留前后姿态供断言，尚未绘制对照轮廓（[manifest.rs](../crates/dr-editor/src/demo_cases/manifest.rs) 的 mirrors-involution 与 transform-order、[interactions.rs](../crates/dr-editor/src/demo_cases/interactions.rs) 的步骤、候选轮廓取色见 [demo_cases.rs 的 draw](../crates/dr-editor/src/demo_cases.rs)），建议同屏标注方向差异与镜像复位键。
- demo-only 灰色候选对照解释 4→1：明确灰色几何候选不是生产合法提示，避免被误读为多提示；生产提示仍走 [attachment_hints.rs 的 nearby](../crates/dr-editor/src/attachment_hints.rs)，注释约定见 [demo_cases.rs](../crates/dr-editor/src/demo_cases.rs)。
- F6 现有 undo/redo 按钮直接显示新分支清 redo：指标 'new_branch_clears_old_redo' 已存在（[workflow.rs](../crates/dr-editor/src/demo_cases/workflow.rs)），用底栏或 Tools 已提供的撤销/重做入口呈现（[底栏与抽屉实现](../crates/dr-editor/src/panels/egui_panel.rs)），不需要新增 HUD。
- 线设置窗口现在按可用区域自适应：宽屏保持 340 宽与 (800,100) 既有外观，窄窗与短横屏收缩到可用区域并改用正文滚动（[connection_lines.rs 的 window_layout 与 touch_spacing](../crates/dr-editor/src/connection_lines.rs)，紧凑布局测试自 [同文件](../crates/dr-editor/src/connection_lines.rs) 起，窄竖屏 320x640 见 [同文件](../crates/dr-editor/src/connection_lines.rs)）；新 case 仍需确认夹具落点不被窗口遮挡。

## 后续工具候选

- 单章/单例回放入口，便于只跑改动章。
- 相机感知取景 point，使夹具落点在视口内可预期。
- 两者都在现有驱动上小步扩展，本轮不扩架构。

## 单独非阻断观察

- 出窗分支按左键是否按下决定取消（[mouse_editor 的失焦与出窗段](../crates/dr-editor/src/canvas_input.rs)）：默认框选键是中键、平移键是左键（[view.rs 的按键映射](../crates/dr-editor/src/view.rs)、[默认值](../crates/dr-editor/src/view.rs)），中键按住出窗当帧即触发取消，框选随之被丢。若修，沿用 blocked 分支已有的 'drag.rectangle' 与当前框选按钮分流（[mouse_editor 的 blocked 段](../crates/dr-editor/src/canvas_input.rs)，框选键取自 [同文件](../crates/dr-editor/src/canvas_input.rs)），part/palette 保持 Left，失焦继续无条件取消。
- 仅建议，不替用户改语义；回归组合：出窗中键框选、出窗左键拖动、失焦拖动、UI 上释放。
