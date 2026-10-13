# 演示评审建议

此页只维护已采纳项和**尚未实施的回放候选**，不是本次新增需求或验收报告。实际运行统一见[当前进度](editor-progress.md)与[历史验收](validation-history.md)，命令见[验证指南](validation.md)。

## 基线与维护规则

brief 保持 10 章；detailed 为 19 章、45 例、95 个证据。若以后批准新增 case，同步 Rust [manifest](../crates/dr-editor/src/demo_cases/manifest.rs)/测试及 Python [驱动](../scripts/demo_editor.py)的契约，不仅改数字。已有覆盖不重复加需求，也不把内部输入回放当成系统键鼠或 input-to-photon 证据。

## 已采纳

- 连接提示与实际提交核对完整 Connection，反向换点拒绝读取真实错误状态；平移锚点保留既有容差。
- 零长路径使用真实中心挂点，走父中心→挂点→子中心的生产路径，验证有限/零弧长、方向采样 None、真实 Gizmos 顶点增量 0 和安全跳过；保留正常线正向对照与目录复原。[实现](../crates/dr-editor/src/demo_cases.rs)、[回归](../crates/dr-editor/src/demo_cases/tests.rs)。旧“实际画出零长线”指标已删除，不重加兼容项。
- detailed 驱动核对逐例通用 metrics、有限几何误差/严格 5%、实际手势计数及 physical=false，另验章节顺序、45 例与新鲜非空证据。[驱动](../scripts/demo_editor.py)、[回归](../scripts/test_demo_editor.py)。
- 分类演示复用真实组标题/全部展开及既有滚动寻址，不增加演示状态或延长超时。

## 优先候选

### ① 悬空引用下的新放置 ID

- 现状：next_part_id 取已占 ID 最大值 +1（[model.rs](../crates/dr-core/src/model.rs)），occupied_ids 覆盖连接上的悬空引用（[同文件](../crates/dr-core/src/model.rs)）；修复与粘贴路径同样预留既有 ID（[repair.rs](../crates/dr-core/src/edit/repair.rs)、[selection.rs](../crates/dr-core/src/edit/selection.rs)、[scoped.rs](../crates/dr-core/src/edit/scoped.rs)）；放置入口使用该 ID（[placement.rs](../crates/dr-editor/src/placement.rs)）。
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
- F6 现有 undo/redo 按钮直接显示新分支清 redo：指标 'new_branch_clears_old_redo' 已存在（[workflow.rs](../crates/dr-editor/src/demo_cases/workflow.rs)），用同模式 UI 已提供的常驻撤销/重做按钮呈现（[egui_panel.rs 的 directory_header](../crates/dr-editor/src/panels/egui_panel.rs)、[常驻按钮列](../crates/dr-editor/src/panels/egui_panel.rs)），不需要新增 HUD。
- 线设置窗口现在按可用区域自适应：宽屏保持 340 宽与 (800,100) 既有外观，窄窗与短横屏收缩到可用区域并改用正文滚动（[connection_lines.rs 的 window_layout 与 touch_spacing](../crates/dr-editor/src/connection_lines.rs)，紧凑布局测试自 [同文件](../crates/dr-editor/src/connection_lines.rs) 起，窄竖屏 320x640 见 [同文件](../crates/dr-editor/src/connection_lines.rs)）；新 case 仍需确认夹具落点不被窗口遮挡。

## 后续工具候选

- 单章/单例回放入口，便于只跑改动章。
- 相机感知取景 point，使夹具落点在视口内可预期。
- 两者都在现有驱动上小步扩展，本轮不扩架构。

## 单独非阻断观察

- 出窗分支按左键是否按下决定取消（[mouse_editor 的失焦与出窗段](../crates/dr-editor/src/canvas_input.rs)）：默认框选键是中键、平移键是左键（[view.rs 的按键映射](../crates/dr-editor/src/view.rs)、[默认值](../crates/dr-editor/src/view.rs)），中键按住出窗当帧即触发取消，框选随之被丢。若修，沿用 blocked 分支已有的 'drag.rectangle' 与当前框选按钮分流（[mouse_editor 的 blocked 段](../crates/dr-editor/src/canvas_input.rs)，框选键取自 [同文件](../crates/dr-editor/src/canvas_input.rs)），part/palette 保持 Left，失焦继续无条件取消。
- 仅建议，不替用户改语义；回归组合：出窗中键框选、出窗左键拖动、失焦拖动、UI 上释放。
