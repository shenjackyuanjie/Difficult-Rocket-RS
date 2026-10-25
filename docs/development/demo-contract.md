# 编辑器演示报告合同

[项目首页](../../README.md) · [操作指南](../guides/editor.md) · [验证指南](../validation/README.md)

本页只维护已实现的章节、用例与报告要求，不是运行验收报告；未排期的演示愿望清单不再单独维护。实际结果见[当前状态](../../README.md#当前状态)，复现入口见[验证指南](../validation/README.md)。

## 基线与维护规则

当前 brief 为 **11 章**；detailed 为 **23 章、54 例、114 个章节产物**。54 例由 51 个边界用例与独立 `rocket-workflow` 的 3 个连续用例组成，不把边界用例数当成总数。当前合同由旧基线逐项叠加，原用例及压力数字保持不变：

| 范围 | brief 章数 | detailed 章数 | detailed 用例 | detailed 章节产物 |
| --- | --- | --- | --- | --- |
| 原有基线 | 10 | 19 | 45 | 95 |
| 连续四物理推进级火箭新增 | 1 | 1 | 3 | 9 |
| 触摸、千件拖动、窄窗新增 | 0 | 3 | 6 | 10 |
| 当前报告合同 | 11 | 23 | 54（51 边界 + 3 连续火箭） | 114 |

原 45 个边界用例仍由 Rust [manifest](../../crates/dr-editor/src/demo_cases/manifest.rs)维护；触摸 2 例、千件拖动 1 例、窄窗 3 例将边界用例扩展为 51 例，连续火箭另有 3 例由独立工作流报告承载。统计口径由[驱动合同回归](../../scripts/test_demo_editor.py)核对；原 `staging` 的 **64 级、1024 动作**压力章继续保留，新的八卡入门示例不替代它。后续新增 case 时需同步 Rust 实现/测试与 Python [驱动](../../scripts/demo_editor.py)的契约，不仅改数字。已有覆盖不重复加需求，也不把内部输入回放当成系统键鼠或 input-to-photon 证据。

## 已采纳

- 连接提示与实际提交核对完整 Connection，反向换点拒绝读取真实错误状态；平移锚点保留既有容差。
- 零长路径使用真实中心挂点，走父中心→挂点→子中心的生产路径，验证有限/零弧长、方向采样 None、真实 Gizmos 顶点增量 0 和安全跳过；保留正常线正向对照与目录复原。[实现](../../crates/dr-editor/src/demo_cases.rs)、[回归](../../crates/dr-editor/src/demo_cases/tests.rs)。旧“实际画出零长线”指标已删除，不重加兼容项。
- detailed 驱动继续核对原 45 例的通用 metrics、有限几何误差/严格 5%、实际手势计数及 physical=false；加上触摸、千件拖动与窄窗 6 例后为 51 个边界用例，另有工作流 3 例，总计 54 例，并核对章节顺序及新鲜非空证据。[驱动](../../scripts/demo_editor.py)、[回归](../../scripts/test_demo_editor.py)、[工作流严格验收](../../scripts/rocket_workflow_report.py)。
- 分类演示复用真实组标题/全部展开及既有滚动寻址，不增加演示状态或延长超时。

## 连续四物理推进级火箭章合同

章 ID 为 `rocket-workflow`，标题为“三级芯级 + 双固体助推器：真实搭建、自动分级与拖放调级”。brief 顺序为 `connections → rocket-workflow → transforms`；detailed 顺序为 `connections → rocket-workflow → geometry-cases`。章内三阶段连续使用同一文档，不在阶段间重新加载船体。章定义见[章节清单](../../crates/dr-editor/src/demo/chapters.rs)，实现见[rocket_workflow.rs](../../crates/dr-editor/src/demo/rocket_workflow.rs)。

本节记录实现与报告合同，**不表示当前构建的完整 GUI 已通过**。实际通过范围按当前状态及对应日期记录确认，不复用旧截图或旧报告替代当前构建证据，也不在此页追加验收历史。

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

三份 XML 必须保持**相同的 14 部件几何、SRB 固体 Tank 状态与 13 条 Normal 树边的实际端点**，仅分级计划按 assembled（空）、basic（八卡、全 false）、manual（八卡含空卡、仅 9 为 true）变化。严格验收同时核对指标、目标多重集、拖动事件与帧序、XML 和证据新鲜度；缺失、空文件、陈旧或相互不符的证据都应失败。[Python 验收](../../scripts/rocket_workflow_report.py)、[负向回归](../../scripts/test_rocket_workflow_report.py)、[Rust 工作流回归](../../crates/dr-editor/src/demo/tests/rocket_workflow.rs)。

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
