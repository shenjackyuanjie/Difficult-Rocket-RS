# 演示评审建议（第二轮只读二审汇总）

本文件汇总四路历史只读二审的结论，作为后续演示改动与验收的输入。「驱动契约」已实施并提交；零长路径修正已作为 `37489c0` 提交，并由触控适配阶段（Bevy 0.19）的完整 DX12 两档回放验收。三个依赖升级已独立提交；Bevy 0.20.0 / bevy_egui 0.43.1、serde_json 1.0.154 与统一风格的最终程序通过 373 项 Rust、148 项 Python、严格 Release Clippy / build、完整详细 DX12 40ms / 450ms 两档及三个 DX12 窄窗。失败过程与验证范围见[最终集成记录](./editor-progress.md#2026-10-13-最终集成验收)。本轮不再增加 corner 样例或委派审查；其余新增 corner 均为提案、尚未实现，已有覆盖不重复提需求。

部分历史 `target` 证据已不在当前工作区，旧链接仅定位当时记录；最终 2026-10-13 证据已独立核对存在，不补造历史产物。

## 口径与基线

- 既有规模保持不变：brief 10 章不动；detailed 保持 19 章、45 例、95 证据（现行记录见 [editor-progress.md](./editor-progress.md#L194)）。
- 每追加一个现有章内的 case：cases +1、evidence +1，章数仍为 19；同步 [manifest.rs](../crates/dr-editor/src/demo_cases/manifest.rs#L73)、[demo_cases/tests.rs](../crates/dr-editor/src/demo_cases/tests.rs#L74)、[demo/tests/configuration.rs](../crates/dr-editor/src/demo/tests/configuration.rs#L3-L6)、[demo_editor.py](../scripts/demo_editor.py#L22-L41) 与 [README.md](../README.md#L30)，并在 [editor-progress.md](./editor-progress.md#L186) 记录当次验证。
- 演示证据以内部 Bevy/egui 输入注入与回放为准（见 [selection_smoke.rs](../crates/dr-editor/src/selection_smoke.rs#L48)），不声称 OS 键鼠或 input-to-photon 结论；'physical_display_latency_measured' 保持 false（[gestures.rs](../crates/dr-editor/src/demo_cases/gestures.rs#L431)）。
- 历史阶段验收（触控适配 / Bevy 0.19，已冻结）：351 项 Rust 测试通过（core 101 / editor 250）、127 项 Python 回归通过、严格 Release Clippy 与 Release build 通过；纯鼠标与触控失焦仲裁两项回归通过。该阶段的完整 DX12 快速 40ms 与观看 450ms 两档均通过，分别见[快速报告](../target/demo-mobile-fast-20261012-canvas-final/demo-report.json)、[观看报告](../target/demo-mobile-watch-20261012-canvas-final/demo-report.json)。三种真实窄窗[较早完整记录](../target/mobile-narrow-20261012-owned-window/report.json)保留；最新复验 Vulkan 仅首例通过、第二例尺寸变化后超时，DX12 首例尺寸变化后超时，未计为全量通过，详见[适配验收边界](./mobile-support.md#L77-L111)。
- 阶段边界：XML 已独立提交 `9e47977`，其后 354 项 Rust（core 104 / editor 250）属 XML 加旧 Bevy 0.19 阶段。Bevy 0.20.0 / bevy_egui 0.43.1 本阶段为 366 项 Rust（core 104 / editor 262）和 148 项 Python，通过严格 Release Clippy / build、详细 DX12 40ms 与三尺寸窄窗；没有沿用旧阶段的通过结论。原生按钮已直接迁至 [ui_widgets / picking / Activate](../crates/dr-editor/src/native_buttons.rs)，不再有自持 Button 标记、弃用别名或旧 `Interaction` 兼容层；`smoke_window_size` 使用全限定 `anyhow::Context`。
- 最终复验另行记录：`980abe0` 复用 `UiTestInput` 的完整短点击，删除重复释放状态，普通长按 / 拖动 / 触摸语义不变；没有靠延长点击阈值或放宽断言通过。最终 373 项 Rust（core 104 / editor 269）与 148 项 Python、同一 Release 程序的两档详细回放和三个窄窗通过，不拿基础迁移的 366 项记录代替。

## 已采纳

1. hint 与提交做完整 connection 精确核对（现为整条连接比较，见 [gestures.rs](../crates/dr-editor/src/demo_cases/gestures.rs#L304-L308)，布尔指标见 [同段 metrics](../crates/dr-editor/src/demo_cases/gestures.rs#L327-L332) 的 'hinted_connection_matches_commit'）。
2. 反向换点使用同一口径与真实错误状态：拒绝时读 'document.status' 而不是构造文案（[gestures.rs](../crates/dr-editor/src/demo_cases/gestures.rs#L378-L391)、[workflow.rs](../crates/dr-editor/src/demo_cases/workflow.rs#L72)）。
3. pan 锚点保留余量：平移后锚点与释放帧 delta 沿用既有容差口径（[interaction_tests.rs](../crates/dr-editor/src/interaction_tests.rs#L143-L197) 是现有基线）。

## 零长路径：历史问题与已验收修正

历史问题（修正前；旧 '[Vec2::ZERO; 4]' 调用与旧指标所在代码已移除，不再对现行代码给行锚）：'ZeroLength' 场景用固定零长点集调用绘制后无条件置位，metrics 据此声称 'zero_length_path_actually_drawn: true'；而 draw_path 折叠重复点后会提前返回，实际没有提交任何顶点，指标与实际绘制不符。

已实现修正（unit 回归与触控适配阶段（Bevy 0.19）的完整 DX12 两档窗口回放均已通过）：

- 夹具改用真实中心挂点：复制 'fuselage-1' 为临时目录项 'demo-zero-length'，挂点 location 清空并置于 (0,0)，两个部件同为中心（[interactions.rs 的 ZeroLength 夹具](../crates/dr-editor/src/demo_cases/interactions.rs#L88-L109)）。
- 路径按生产渲染口径构造父中心→父挂点→子挂点→子中心四点（[demo_cases.rs 的 connection_path](../crates/dr-editor/src/demo_cases.rs#L422-L440)）。
- 绘制检查按真实提交量判定（[demo_cases.rs 的 draw_degenerate](../crates/dr-editor/src/demo_cases.rs#L442-L487)）：断言路径有限、总弧长恰为 0、along 在 0/0.5/1 三个采样位置均返回 None，并以 draw_path 调用前后的实际 Gizmos 顶点增量为 queued_vertex_count，断言其为 0；指标改为 'degenerate_path_safely_skipped'、'direction_sample_count'、'queued_vertex_count' 等，旧指标已移除（[demo_cases/tests.rs](../crates/dr-editor/src/demo_cases/tests.rs#L223-L228) 断言其不存在）。
- 同一 unit 含正常线正向对照与目录复原：先画一条正常线确认确实提交顶点，再验证退化路径零增量（[demo_cases/tests.rs](../crates/dr-editor/src/demo_cases/tests.rs#L205-L214)）；专用夹具的挂点取自真实中心，随后一例复位目录与原目录项（[demo_cases/tests.rs](../crates/dr-editor/src/demo_cases/tests.rs#L174-L190)）。
- 底层安全路径保持不变：along 在总弧长 < 1e-4 时返回 None（[connection_lines.rs 的 along](../crates/dr-editor/src/connection_lines.rs#L107-L124)），draw_path 折叠重复点后 'points.len() < 2' 提前返回（[connection_lines.rs 的 draw_path](../crates/dr-editor/src/connection_lines.rs#L127-L147)）。

实际验收（触控适配 / Bevy 0.19 历史阶段）：[快速逐例证据](../target/demo-mobile-fast-20261012-canvas-final/line-cases.json)与[450ms 逐例证据](../target/demo-mobile-watch-20261012-canvas-final/line-cases.json)的 `line-zero-length` 均为 passed：`path_point_count=4`、`path_length=0.0`、`direction_sample_count=0`、`queued_vertex_count=0`、`degenerate_path_safely_skipped=true`，流动与箭头均开启，dirty 为 false、undo/redo 均为 0。该阶段两个完整报告均为 19 章 / 45 例 / 95 证据，不用早期部分 probe 替代全量结果；0.20 / serde_json 1.0.154 最终构建的[40ms 连线证据](../target/demo-closeout-fast-20261013/line-cases.json)与[450ms 连线证据](../target/demo-click-packet-watch-20261013/line-cases.json)同样完整通过，零长指标与文档 / 历史不变的口径相同。

## 优先候选

### ① 悬空引用下的新放置 ID

- 现状：next_part_id 取已占 ID 最大值 +1（[model.rs](../crates/dr-core/src/model.rs#L572-L573)），occupied_ids 覆盖连接上的悬空引用（[同文件](../crates/dr-core/src/model.rs#L577-L599)）；修复与粘贴路径同样预留既有 ID（[repair.rs](../crates/dr-core/src/edit/repair.rs#L45-L63)、[selection.rs](../crates/dr-core/src/edit/selection.rs#L513-L531)、[scoped.rs](../crates/dr-core/src/edit/scoped.rs#L177-L201)）；放置入口使用该 ID（[placement.rs](../crates/dr-editor/src/placement.rs#L153)）。
- 缺口：已有 unit 覆盖（[model_tests.rs](../crates/dr-core/src/model_tests.rs#L77-L119)、[repair_tests.rs](../crates/dr-core/src/edit/repair_tests.rs#L131-L139)），缺可见 demo。
- 回放内容：id1 + 缺失 id2 的连接下，预览新 ID 为 3，提交实际 ID 也为 3；历史引用原样保留，undo/redo 与 XML 往返一致。
- 独立断言：预览 ID == 提交 ID == 3；悬空引用在提交、撤销、XML 后逐字段相同。
- 成本：约 1 个 case。

### ② 滚轮缩放与光标锚定

- 现状：滚轮按行/像素换算 delta，scale 钳制 0.1–100，并用光标相对屏幕中心的偏移校正平移锚点（[camera_controls 的滚轮段](../crates/dr-editor/src/main.rs)）；触控缩放按两指中点保持世界锚点（[camera_controls 的触控段](../crates/dr-editor/src/main.rs)）；已有各缩放档平移像素校验与光标取当前缩放（[interaction_tests.rs](../crates/dr-editor/src/interaction_tests.rs#L143-L224)）、键盘与注入输入延迟覆盖（[input_latency_tests.rs](../crates/dr-editor/src/input_latency_tests.rs#L221-L260) 的滚轮与窗口尺寸例）。
- 缺口：滚轮缩放没有 demo 回放，拖动中缩放不漂移也未被断言。
- 回放内容：连续滚轮放大缩小，光标下的世界点不动；到 0.1 与 100 边界不再变化；按住平移按钮的同一动作序列中滚轮不引入额外漂移。
- 独立断言：缩放前后光标世界点误差 < 1e-4；clamp 后 scale 恰为 0.1 与 100；拖动 + 滚轮后平移误差 < 0.002 世界像素。
- 成本：约 1 个 case。

### ③ 已占候选跳过、次近空闲候选

- 现状：提示按 0.75 距离列举候选并过滤，每对部件只保留最近的一个合法候选（[attachment_hints.rs 的 nearby](../crates/dr-editor/src/attachment_hints.rs#L16-L72)）；吸附阈值仍是 0.35（[connections.rs](../crates/dr-core/src/connections.rs#L7)）。候选章已断言 'geometric_candidates'、'displayed_pairs: 1' 与提交一致（[gestures.rs](../crates/dr-editor/src/demo_cases/gestures.rs#L327-L332)），unit 覆盖见 [panels/tests.rs](../crates/dr-editor/src/panels/tests.rs#L296) 与 [attachment_hints/tests.rs](../crates/dr-editor/src/attachment_hints/tests.rs#L30)。
- 缺口：「最近候选已占用 → 跳过 → 次近空闲候选可提示」缺少可见回放。
- 独立断言：已占候选不出现在提示集合；演示提示的连接与提交连接完全相同。
- 成本：约 1 个 case。

### ④ 吸附阈值精确边界

- 现状：常量 'CONNECTION_DISTANCE = 0.35'（[connections.rs](../crates/dr-core/src/connections.rs#L7)），测试用 0.35 与 0.350001 边界（[selection/tests.rs](../crates/dr-editor/src/selection/tests.rs#L295-L298)）。
- 回放内容：用精确本地 cursor 参数避开像素量化，分别在 0.35 与 0.3505 落点提交。
- 独立断言：0.35 吸附成功且连接点与提示一致；0.3505 不吸附但仍成功放置独立部件（无连接、无提示），历史 +1、dirty 变真，撤销后回到初始。
- 成本：约 1 个 case。

### ⑤ UI 遮挡期间冻结与取消回位

- 现状：'UiPointer.blocked' 时鼠标编辑提前返回，并按释放取消拖动/框选、清理放置预览（[mouse_editor 的 blocked 段](../crates/dr-editor/src/main.rs)），相机控制同时清空拖动并丢弃滚轮（[camera_controls 的 blocked 段](../crates/dr-editor/src/main.rs)）；Esc/右键取消与失焦、出窗取消已实现（[mouse_editor 的取消段](../crates/dr-editor/src/main.rs)），拖到目录上松手删除另有独立分支（[同文件](../crates/dr-editor/src/main.rs)）；UI 归属由模态阻塞与面板打开两处置位（[egui_ui.rs](../crates/dr-editor/src/egui_ui.rs#L85-L95)、[同文件](../crates/dr-editor/src/egui_ui.rs#L112-L123)）。相机侧 blocked 行为已有覆盖（[interaction_tests.rs](../crates/dr-editor/src/interaction_tests.rs#L227-L268)），UI 上释放取消与失焦取消已由 [selection_smoke.rs](../crates/dr-editor/src/selection_smoke.rs#L333-L343) 与 [Escape 取消段](../crates/dr-editor/src/selection_smoke.rs#L501-L506) 覆盖，不重复提。
- 缺口：缺「遮挡期间部件 Transform 逐帧冻结、解除遮挡后抓取偏移保持、右键取消拖动/框选当帧视觉回位」的可见回放；相关实现与 unit 已在位，不属于新增实现项。
- 独立断言：blocked 期间部件 Transform 逐帧不变；恢复后拖动位置含原抓取偏移；右键当帧预览消失且文档与 dirty 不变。
- 成本：1–2 个 case（先拖动，后框选）。

### ⑥ 剪贴板失败与上限

- 现状：粘贴走统一 apply 路径（[edit.rs](../crates/dr-core/src/edit.rs#L311-L313)）；数量上限检查在 [selection.rs 的 paste](../crates/dr-core/src/edit/selection.rs#L501-L511)；同组重复 ID 复制拒绝在 capture（[同文件](../crates/dr-core/src/edit/selection.rs#L431-L435) 报 'AmbiguousReference'）。
- 缺口：超上限粘贴与重复 ID 拒绝无回放，预览、文档、历史、报错四者一致性未被断言。
- 独立断言：超限时提交被拒且粘贴预览保留（可继续定位或用 Esc 取消），status 为真实错误，undo/redo 长度不变；重复 ID 复制被拒且剪贴板保留旧内容。
- 成本：约 1–2 个 case。

### ⑦ Dock 独立第三引用换父拒绝

- 现状：换父走 'topology::reparent'（[topology.rs](../crates/dr-core/src/edit/topology.rs#L18-L70)），独立第三引用的 Dock 换父拒绝在 [同文件第三引用分支](../crates/dr-core/src/edit/topology.rs#L62-L66)，connect 路径的 Dock 占用拒绝在 [connections.rs](../crates/dr-core/src/connections.rs#L445-L450)；树换父拒绝环、多父歧义与单次历史项已有 unit（[topology/tests.rs](../crates/dr-core/src/topology/tests.rs#L130)、[多父歧义](../crates/dr-core/src/topology/tests.rs#L259)、[单历史项](../crates/dr-core/src/topology/tests.rs#L174)）。
- 缺口：Dock 作为独立第三引用时换父被拒绝缺少 demo，「精确断边一次 undo」与 XML 需同场景可见。
- 独立断言：换父被拒后 dock 引用原样且 undo 长度不变；一次断边只产生一个 undo 步，undo 后 XML 与初始一致。
- 成本：约 1 个 case。

## 展示层改进

- 对照轮廓展示 E→X 与 X→E 差异及 XX/YY 复位：现有用例保留前后姿态供断言，尚未绘制对照轮廓（[manifest.rs](../crates/dr-editor/src/demo_cases/manifest.rs#L238-L251) 的 mirrors-involution 与 transform-order、[interactions.rs](../crates/dr-editor/src/demo_cases/interactions.rs#L346-L390) 的步骤、候选轮廓取色见 [demo_cases.rs 的 draw](../crates/dr-editor/src/demo_cases.rs#L382-L398)），建议同屏标注方向差异与镜像复位键。
- demo-only 灰色候选对照解释 4→1：明确灰色几何候选不是生产合法提示，避免被误读为多提示；生产提示仍走 [attachment_hints.rs 的 nearby](../crates/dr-editor/src/attachment_hints.rs#L16-L72)，注释约定见 [demo_cases.rs](../crates/dr-editor/src/demo_cases.rs#L382)。
- F6 现有 undo/redo 按钮直接显示新分支清 redo：指标 'new_branch_clears_old_redo' 已存在（[workflow.rs](../crates/dr-editor/src/demo_cases/workflow.rs#L318)），用同模式 UI 已提供的常驻撤销/重做按钮呈现（[egui_panel.rs 的 directory_header](../crates/dr-editor/src/panels/egui_panel.rs#L153-L155)、[常驻按钮列](../crates/dr-editor/src/panels/egui_panel.rs#L194-L206)），不需要新增 HUD。
- 线设置窗口现在按可用区域自适应：宽屏保持 340 宽与 (800,100) 既有外观，窄窗与短横屏收缩到可用区域并改用正文滚动（[connection_lines.rs 的 window_layout 与 touch_spacing](../crates/dr-editor/src/connection_lines.rs#L167-L199)，紧凑布局测试自 [同文件](../crates/dr-editor/src/connection_lines.rs#L600) 起，窄竖屏 320x640 见 [同文件](../crates/dr-editor/src/connection_lines.rs#L734)）；新 case 仍需确认夹具落点不被窗口遮挡。

## 驱动契约（已实施并提交）

- 已实施：detailed 报告逐例校验四类契约——通用字段非空且类型正确（part_count / undo_len / redo_len / dirty，见 [通用字段校验](../scripts/demo_editor.py#L224-L232)）、interaction 误差与计数在有限范围内且 physical=false（[手势校验](../scripts/demo_editor.py#L239-L252)）、几何值 finite 且 accepted 与严格 5% 判定一致（[几何校验](../scripts/demo_editor.py#L255-L265)）、metrics 为对象（[样例校验入口](../scripts/demo_editor.py#L226)），并由 [逐例接线](../scripts/demo_editor.py#L292) 调用；[validate_report](../scripts/demo_editor.py#L146-L196) 与 [validate_cases](../scripts/demo_editor.py#L268-L292) 另行校验章节顺序、状态、耗时、新鲜非空产物与逐例证据。通用 metrics 字段由运行层写入（[demo_cases.rs](../crates/dr-editor/src/demo_cases.rs#L373-L376)）。
- 驱动契约已作为 `fa97971` 提交；该历史节点全套 Python 为 120 项。触控适配阶段（Bevy 0.19）收尾重新执行全套 Python 127 项并通过，其中 [test_demo_editor.py](../scripts/test_demo_editor.py#L60) 含 43 项演示专项测试；新增 7 项为窄窗工具回归。见[本次 Python 日志](../target/validation-mobile-python-closeout.log)。
- 阶段口径：触控适配阶段（Bevy 0.19）的 Rust 验证为 351 项（core 101 / editor 250），该阶段的完整 DX12 快节奏与 450ms 回放均已通过（选择章及按实际 UI 画布核对的视图章同时通过，纯鼠标失焦尾锁已修）；XML 独立提交 `9e47977` 之后为 354 项（core 104 / editor 250），仍属旧 Bevy 0.19 阶段。0.20 基础迁移与之后的 UI 风格、serde_json、短点击精简分别属于不同构建，不混用测试计数；最终两档及窄窗通过结论见[最终集成记录](./editor-progress.md#2026-10-13-最终集成验收)。
- 除上述已落地项外，本文件其余 corner 仍为提案，尚未实现。

## 后续新工具（非本轮）

- 单章/单例回放入口，便于只跑改动章。
- 相机感知取景 point，使夹具落点在视口内可预期。
- 两者都在现有驱动上小步扩展，本轮不扩架构。

## 单独非阻断观察

- 出窗分支按左键是否按下决定取消（[mouse_editor 的失焦与出窗段](../crates/dr-editor/src/main.rs)）：默认框选键是中键、平移键是左键（[view.rs 的按键映射](../crates/dr-editor/src/view.rs#L10-L24)、[默认值](../crates/dr-editor/src/view.rs#L39)），中键按住出窗当帧即触发取消，框选随之被丢。若修，沿用 blocked 分支已有的 'drag.rectangle' 与当前框选按钮分流（[mouse_editor 的 blocked 段](../crates/dr-editor/src/main.rs)，框选键取自 [同文件](../crates/dr-editor/src/main.rs)），part/palette 保持 Left，失焦继续无条件取消。
- 仅建议，不替用户改语义；回归组合：出窗中键框选、出窗左键拖动、失焦拖动、UI 上释放。
