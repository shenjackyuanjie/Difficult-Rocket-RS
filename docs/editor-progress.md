# 编辑器当前进度与验收

完整复刻仍在进行，不把独立任务完成视为全项目完成。历史日期、旧测试计数与失败集中在[历史验收](validation-history.md)，操作见[操作指南](editor-usage.md)，复现命令见[验证指南](validation.md)。

## 当前能力

| 功能块 | 现行合同 |
| --- | --- |
| XML / 核心 | 严格读写船体和 PartList，保留静态/运行/分级字段；未知内容明确拒绝，同目录临时文件原子保存 |
| 事务 / 历史 | 放置/吸附/整组变换/删除/属性原子提交，失败/no-op 保留重做；256 步/64 MiB 快照预算 |
| 分组 / 重号 | PartKey 定位具体实例，跨组合并只重映射相关冲突及引用；歧义已有显式修复，不自动猜测 |
| 几何 / 连接 | 任意角度与镜像、真实 Shape/轮子、严格 5% 重叠和 0.35 吸附；吸附与自由几何是两个独立开关——关闭吸附不再对齐也不新建自动连接，自由模式跳过整体碰撞但新的自动连接仍严格校验占用、歧义、重复部件对与连接碰撞，关闭吸附不删除已有连接 |
| 输入 / 显示 | 连续抓取偏移、松手末段位移、同帧视觉同步；鼠标按下触发，触摸抬指一次，UI/模态隔离 |
| 界面 | 56px 图标底栏，Menu / Staging / Help 三枚原生入口与 egui 图标同高同 48px 间距排成一条连续工具栏；Parts / Ships / Tools 彼此互斥，但打开属性 / 分级不再收起它们，停靠分级可与右侧部件 / 工具或左侧船体抽屉共存；子节点跟随（默认开启）、吸附、自由模式是三个互不联动的图标开关；帮助窗口默认居中且可拖动，底部图例用真实渲染色 |
| 小屏 | 最小 320×320，单侧抽屉、44px 触控入口，按实际画布适配；物理触屏与平台发布另行验证 |
| 分级 | 从所选 Pod 的同组 Normal 连接树显式生成基础草稿；卡片与动作真实拖放，跨卡移动标记 moved；停靠分级并入主编辑——画布拖动 / 框选 / 放置与 F3/F4 不受它限制；Ctrl+Z / Ctrl+Y 先回退草稿本地历史、耗尽后回落文档历史；Apply 按字段三方合并，外部改同一字段才拒绝并保留草稿 |
| 演示 / 工具 | brief 11 章；detailed 23 章/51 例/114 项证据（新增触摸提交、千件拖动、窄窗三态）；界面与成片共用程序合成音效；火箭章升级为 14 件 / 13 条连接 / 四个物理推进级的双固体助推火箭（SRB 为叶子、自带固体燃料 Tank，顶部不挂罐），自动生成 8 张卡、手动六次真实拖放；另有分级多状态截图驱动（`--staging-shots`）输出 6 张状态图与 JSON 证据；视频工具支持 brief / detailed 两种成片 |

原生按钮使用 Bevy 0.20 的 ui_widgets / picking / Activate，无旧 Interaction 兼容层。依赖版本以 Cargo.lock 为准。DRRS 的具体 API 合同与差异见[核心覆盖核对](drrs-core-coverage.md)。

## 2026-10-18 部件网格步长：对齐原版 0.5

**原版实测**：用真实 Win32 输入驱动 SimpleRockets PC 1.6.11 编辑器，24 个有效样本、四档缩放（41/62/84/98 px/单位）下拖动位移始终是 **0.5 逻辑单位**；0.1 与 0.25 在拖动中从未出现，恰好半格向远离零方向进位，x / y 独立量化。证据在 `target/grid-probe/`（完整报告、65 张截图、37 份存档快照与可复用遥控脚本），该目录不入库。

实现取舍：

- `EDITOR_GRID_STEP = 0.5` 位于 dr-core 的 geometry，拖动、放置与整组平移都引用它；改实测值只改这一处。
- 量化的是**位移增量**而不是绝对坐标：已加载的 0.75、连接几何产生的 0.25 偏移不会被拉回网格，避免改写约 28% 的原版部件坐标。
- 连接吸附优先于网格；自由模式（本项目增强开关）保留连续位移；旋转、镜像与 15° / 1° 精调不受影响。
- 演示夹具随量化调整：31° 辅助拒绝样例改为向部件 1 平移半格（量化后真实重叠 20.9%），选择章的小幅拖动样例改用 0.26 / 0.74 跨过半格。

| 检查 | 本次实际结果 |
| --- | --- |
| Rust | `cargo test --workspace --release --locked`：**550 项**（core 119 / editor 431）通过 |
| Python | 全量 **203 项**通过 |
| 格式 / Release | fmt check、严格 clippy、workspace Release build 通过 |
| detailed 演示 | DX12 40ms，23 章全部 `passed`、退出 0；[报告](../target/demo-grid4-20261018/demo-report.json) |
| brief 演示 | DX12 40ms，11 章全部通过、退出 0；[报告](../target/demo-grid-brief-20261018/demo-report.json) |

未做：按新步长重录两版成片、Vulkan 复跑、系统键鼠/输入法专项。
## 2026-10-18 演示展示、音效与新章节：当前构建

本轮聚焦“看得见、听得到”的演示质量，不是新增游戏玩法。构建身份：Release 二进制 SHA-256 `aaa78ac83e31114e4617ac1209ec6dce036893ec27dde08f25e74f74d5a11f54`；演示与视频使用同一 EXE。

| 检查 | 本次实际结果 / 本地证据 |
| --- | --- |
| Rust | `cargo test --workspace --release --locked`：**548 项**（core 118 / editor 430）通过 |
| Python | 全量 **203 项**通过（`TEMP/TMP=target/test-temp`） |
| 格式 / Release | `cargo fmt --all --check`、严格 `clippy -D warnings`、workspace Release build 通过 |
| detailed 演示 | DX12 40ms，**23 章 / 51 例 / 114 个唯一非空产物**，全部章节 `passed`、退出 0；[报告](../target/demo-final-20261018/demo-report.json) |
| brief 演示 | DX12 40ms，**11 章**全部通过、退出 0；[报告](../target/demo-brief-final-20261018/demo-report.json) |
| 简略成片 | `dr-rs-progress-1080p.mp4` 363.8s（SHA-256 `b746bf8f…`），H.264 1080p + 48kHz 单声道 AAC，音效由真实回放事件驱动；[manifest](../target/video-brief-final-20261018/video-manifest.json) |
| 详细成片 | `dr-rs-progress-detailed-1080p.mp4` 661.7s，含 407 条真实动作音效；[manifest](../target/video-detailed-new-20261018/video-manifest.json) |
| 音效导出 | `--sfx-export` 产出 8 个 WAV（click/place/connect/reject/undo/redo/chapter/complete），与视频混音共用同一音源；[目录](../target/sfx-export) |

本轮改动：

- 展示层：顶部章节条（序号 / 章名 / 进度 / 章首说明）、指针柔光与点击涟漪、输入提示胶囊、窄窗让出画布、完成页汇总卡；非录制模式保留底部说明卡，录制模式由成片字幕承担解说。
- 成片：片头品牌页、底部字幕栏改为“青柠序号 + 短名 + 动作说明”，片尾列出能力清单与范围提示；章节数量文案由章节清单直接推导。
- 音效：八种程序化合成音效（48kHz 单声道 16-bit，峰值 -24 dBFS，双端淡入淡出，同帧同类去重），覆盖 UI 点击、放置、连接、拒绝、撤销重做与章节切换；无音频设备时静默降级。
- detailed 新增三章：`mobile-input`（触摸按下只预览、抬指一次提交；鼠标按下即提交）、`performance`（真实 Ophioglossum 样本选中 1000 部件拖动，记录真实帧耗时并核对撤销重做与 XML 往返）、`narrow-layout`（运行中切 640×360 / 390×844，单侧抽屉与 44px 入口不压船体，章末恢复尺寸）。
- 清理：dr-core 删除无引用 API（`Vec2d::rotate`、`find_snap`、`EditorState`、`drag_part`、`Ship::remove_part`、`Ship::disconnect_part`）及两个只测已删辅助的用例；dr-editor 删除一个完全重复的贴图路径测试与一个只测私有编号推算的测试。输入边沿、事务、XML、失败数据保护、5% 几何边界与分级断言全部保留。

未做：Vulkan detailed 复跑、三窄窗专项脚本、系统键鼠 / 输入法 / 物理触屏专项、重新打包便携 ZIP。录制模式下 `narrow-layout` 不改物理窗口（录制脚本要求客户区恒定 1920×1080），JSON 中如实标注 `presentation=true`、`resized=false`。
## 2026-10-16 最近冻结基线：正式修复后的 DX12 闭环

**这是新一轮用户要求之前的验收基线，不代表后续在改工作区或新增功能已经通过。** 上一轮三级火箭的数据、交互及 DX12 缺件 / 错图问题已在正式修复构建中闭环；完整复刻与下列新增要求继续进行。

构建基底 `b6db201db166387e465a9a1c8731b687841a4fbd`，EXE SHA-256 为 `08b3fcc6d791c64042db41f3652a0481641f1b4535f9b90302301e04a464f899`；[构建身份](../target/validation-ui-staging/build-identity-final-dx12.json)记录该次源码快照。

> 该段引用的 `demo-ui-staging-*` 与 `narrow-ui-staging-*` 产物已在 2026-10-16 的 `target/` 清理中删除（`validation-ui-staging/` 下的日志与构建身份保留），路径按历史引用保留，不补造产物。

| 检查 | 该冻结基线的实际结果 / 本地证据 |
| --- | --- |
| Rust | Release 全量 **487 项**（core 120 / editor 367）通过；[日志](../target/validation-ui-staging/rust-final-dx12.log) |
| Python | 全量 **189 项**通过；[日志](../target/validation-ui-staging/python-final-dx12.log) |
| 格式 / Release | fmt check、严格 Release Clippy `-D warnings`、workspace Release build 均通过；[Clippy](../target/validation-ui-staging/clippy-final-dx12.log)、[build](../target/validation-ui-staging/build-final-dx12.log) |
| DX12 完整详细演示 | 40ms 内部输入回放，**20 章 / 48 例 / 104 个唯一非空产物**，全部章节通过、完整退出 0；两份源船体 SHA256 未变；[报告](../target/demo-ui-staging-final-dx12-20261016/demo-report.json)、[日志](../target/validation-ui-staging/demo-final-dx12.log) |
| 火箭原图复验 | 装配、生成、手调原图均为九件连续火箭，#9 发动机贴图正确；[装配](../target/demo-ui-staging-final-dx12-20261016/editor-rocket-assembled.png)、[生成](../target/demo-ui-staging-final-dx12-20261016/editor-rocket-generated.png)、[手调](../target/demo-ui-staging-final-dx12-20261016/editor-rocket-manual.png) |
| 三窄窗分级 | 同 SHA 构建串行完成 **390×844、640×360、320×640**，三例均 `passed=true`、退出 0；每例仍为 **64 级 / 1024 动作**，末级末项真实可达，编辑、Apply、撤销重做 / XML、悬停恢复及窗口尺寸合同通过；[报告](../target/narrow-ui-staging-final-dx12-20261016/report.json) |
| 连续章 / 压力保留 | 显式生成 `[[9],[7],[6],[4],[3]]` 与真实跨帧手调 `[[9],[7,6],[],[4],[3]]`、仅 6 的 `moved=true` 合同不变；旧 19 章 / 45 例 / 95 产物及 64 / 1024 专项保留；[火箭报告](../target/demo-ui-staging-final-dx12-20261016/rocket-workflow.json) |

完整闭环见[正式修复历史](validation-history.md#2026-10-16-正式修复后的-dx12-验收闭环新增要求之前的基线)。以下诊断构建的失败、公开 / 私有 Render 边界和独立 Naga 反例仍按原身份保留，不用后续通过覆盖历史事实。

## 2026-10-16 新增界面与分级融合：双后端冻结基线

**这一节取代此前“新增要求进行中”的状态**：帮助窗口可拖动 / 居中 / 真实颜色图例、分级与左右抽屉及画布历史的融合、工具溢出与默认跟随、独立吸附开关、DX12 与 Vulkan 完整验证、简略与详细两份视频、sr1 多状态图均已交付并验收。

构建基底 `3332912`（其后仅文档提交，二进制未变），EXE SHA-256 `32a9b610459aeb296bf5af9245a66d746063c681e7802cf59e4e24462d8a42be`；DX12 与 Vulkan 用**同一个二进制**，只切换 `WGPU_BACKEND`；[构建身份](../target/validation-ui-fusion/build-identity-fusion-dx12.json)。

| 检查 | 本轮实际结果 / 本地证据 |
| --- | --- |
| Rust | Release 全量 **533 项**（core 120 / editor 413）通过；[日志](../target/validation-ui-fusion/rust-fusion-final.log) |
| Python | 全量 **203 项**通过；[日志](../target/validation-ui-fusion/python-final-pre-rocket.log) |
| 格式 / Release | fmt check、严格 Release Clippy `-D warnings`、workspace Release build 通过；[Clippy](../target/validation-ui-fusion/clippy-fusion-2.log)、[build](../target/validation-ui-fusion/build-fusion.log) |
| DX12 完整详细演示 | 40ms 内部输入回放，**20 章 / 48 例 / 104 个唯一非空产物**、退出 0；[报告](../target/demo-final-dx12-20261016/demo-report.json)、[日志](../target/validation-ui-fusion/final2-dx12.log) |
| Vulkan 完整详细演示 | 同 EXE 仅切后端，**20 章 / 104 产物**、退出 0、`backend: Vulkan`；[报告](../target/demo-final-vulkan-20261016/demo-report.json)、[日志](../target/validation-ui-fusion/final2-vulkan.log) |
| 贴图加载告警 | 四次窗口运行的日志中 `Multiple AssetLoaders` 均为 **0** 次 |
| 三窄窗分级 | **390×844、640×360、320×640** 三例 `passed=true`、退出 0，每例 64 级 / 1024 动作；[报告](../target/narrow-final2-dx12-20261016/report.json) |
| 分级多状态图 | 6 张状态图 + `staging-shots.json`（`complete=true`），含停靠分级与部件目录共存、停靠期间画布拖动、整卡插入线像素、外部删目标后的诊断；[目录](../target/staging-shots-final2-dx12-20261016) |
| 两版视频 | 简略 `dr-rs-progress-1080p.mp4`（365.3s）、详细 `dr-rs-progress-detailed-1080p.mp4`（616.1s），均通过成片校验；[简略](../target/video-final-brief-20261016/video-manifest.json)、[详细](../target/video-final-detailed-20261016/video-manifest.json) |
| 火箭章 | 14 件 / 13 连接 / 四个物理推进级的双固体助推火箭（SRB 为叶子、自带固体 Tank、顶部不挂罐）；自动 8 卡、手动六次真实拖放；[火箭报告](../target/demo-final-dx12-20261016/rocket-workflow.json) |

本轮窗口运行中修掉的三处真实回归（自由模式与吸附解耦后的旧样例、三方合并下的“同值收敛”样例、演示说明框多余文字）与两处环境干扰（窄窗前台所有权失败、视频录制被 Windows 任务切换浮层遮挡）记录在[历史验收](validation-history.md#2026-10-16-新增界面与分级融合双后端完整验收)。

仍未完成、继续保持未完成状态：系统键鼠 / 输入法 / 物理触屏等原生输入专项、完整复刻的其余缺口与原版对照清单。

## 2026-10-16 历史诊断构建：完整交互通过，DX12 画面未通过

以下为正式修复前的诊断快照，未通过结论仅属于该 SHA；后续闭环见上方冻结基线。

本节对应临时 RenderWorld 诊断构建；程序退出 0 与数据、交互合同通过分别记录，**不等于最终画面正确，也不表示本任务完成**。构建基底为 `37996ef`，Ships 抽屉入口修复已独立提交为 `b6db201`；源码快照及 EXE 身份见[构建身份](../target/validation-ui-staging/build-identity-render-probe.json)。

| 检查 | 本轮实际结果 / 本地证据 |
| --- | --- |
| Rust | Release 全量 **485 项**（core 120 / editor 365）通过；[日志](../target/validation-ui-staging/rust-render-probe.log) |
| Python | 全量 **189 项**通过；[日志](../target/validation-ui-staging/python-tests-final.log) |
| 格式 / Release | fmt check、严格 Release Clippy `-D warnings`、workspace Release build 均通过；[Clippy](../target/validation-ui-staging/clippy-render-probe.log)、[build](../target/validation-ui-staging/build-render-probe.log) |
| 完整详细演示 | 40ms 内部 UI / 画布输入回放，**20 章 / 48 例 / 104 个唯一非空产物**，完整退出 0；两份源船体 SHA256 未变；[报告](../target/demo-ui-staging-render-probe-20261016/demo-report.json)、[日志](../target/validation-ui-staging/demo-render-probe.log) |
| 连续火箭 / 旧压力 | 九件、八条 Normal 连接与三阶段真实输入、草稿、Apply / Cancel、撤销重做及 XML 合同通过；原 19 章 / 45 例 / 95 产物保留，原 **64 级 / 1024 动作**专项保留；[火箭报告](../target/demo-ui-staging-render-probe-20261016/rocket-workflow.json)、[分级报告](../target/demo-ui-staging-render-probe-20261016/editor-staging-performance.json) |
| RenderWorld 公开链路 | 一次稳定观测 `status=observed`，`invalid/pending` 为空；九件实例映射、phase membership 与 draw coverage 各 1，材质映射一致；Storage 的反射最小项尺寸 112 字节、10 项、缓冲区 1120 字节；[诊断表](../target/validation-ui-staging/rocket-render-table.json) |
| DX12 GPU 原图 | **未通过**：装配 / 手调原图仍仅见 5 张精灵，并有发动机错贴油箱图；可见位置呈 `2*i` 分布；[装配原图](../target/demo-ui-staging-render-probe-20261016/editor-rocket-assembled.png)、[手调原图](../target/demo-ui-staging-render-probe-20261016/editor-rocket-manual.png) |
| 同 EXE Vulkan 对照 | 仅切换 `WGPU_BACKEND=vulkan`，brief **11 章**完整退出 0，手调原图九件紧密完整、发动机 9 贴图正确；不是 Vulkan detailed 20 章验收；[报告](../target/demo-ui-staging-vulkan-diagnostic-20261016/demo-report.json)、[原图](../target/demo-ui-staging-vulkan-diagnostic-20261016/editor-rocket-manual.png)、[身份](../target/validation-ui-staging/build-identity-vulkan.json) |
| Naga 独立最小反例 | Naga 30.0.1 的 WGSL → HLSL 实测中，命名局部变量路径重复加入 `first_instance`，直接表达式路径只加入一次，严格断言通过；与 DX12 的 `2*i` 观测吻合，尚无生产修复复验；[named](../target/validation-ui-staging/naga-instance-probe/named.hlsl#L20-L21)、[direct](../target/validation-ui-staging/naga-instance-probe/direct.hlsl#L20)、[日志](../target/validation-ui-staging/naga-instance-probe/probe.log#L53) |

本轮 EXE SHA-256：`6b7d0ba761fd4490916260b7ad9c648a856f75c184da5e09cbd3bdf4b63b75b1`。默认材质分配器实测为 `NonBindless`；公开表能核对 source、prepared material、binding 身份和实际 phase / draw 范围，但不能读取私有的已上传 uniform 字节或最终绑定纹理描述符。source 重算不是 GPU 实际读值；后端对照与独立翻译反例是新的定位证据，生产 shader 尚未修复并完成 DX12 复验。probe 是临时诊断，未增加章、case 或产物合同，后续可能移除。

上一构建已串行通过三窄窗 **390×844、640×360、320×640** 的 64 / 1024 分级专项，具体身份见[后续实测记录](validation-history.md#窄窗换行与真实导航的后续实测)；不把该结果写成当前 probe 构建的重跑。当前构建的完整合同与公开 / 私有 Render 边界见[本轮历史记录](validation-history.md#2026-10-16-完整演示与-renderworld-诊断边界)。

## 2026-10-16 局部重构与回归（历史构建）

本节记录 UI 与自动分级改造前的独立重构验收，后续功能以各自构建记录为准。

- 同组唯一目标筛选由循环切换和下拉列表共用，保留源顺序、重号排除与跨组隔离；不合并两处已有的 `parse()` / `trim().parse()` 差异。
- 帮助、文件/属性与触摸入口共用未提交画布预览清理。触摸仍额外使旧光标位置失效，其他入口保留有效光标；剪贴板、预览设置、船体与历史合同不变。
- 新增四项回归；未增加依赖、缓存框架或数据格式。进度及连续焦点失败提醒统一到默认群 `inner`。

| 检查 | 本轮实际结果 / 本地证据 |
| --- | --- |
| Rust | `cargo test --workspace --release --locked`：387 项（core 104 / editor 283）通过；[日志](../target/validation-refactor-cleanup/rust-tests.log) |
| Python | 最终通知脚本状态下全量 151 项通过；[日志](../target/validation-refactor-cleanup/python-tests-inner.log) |
| 格式 / Release | fmt check、全目标 Release Clippy `-D warnings`、workspace Release build 均通过；[Clippy](../target/validation-refactor-cleanup/clippy.log)、[build](../target/validation-refactor-cleanup/build.log) |
| 完整窗口演示 | DX12 40ms，19 章全部通过，95 个唯一非空证据，退出 0；Test.xml / Heronb.xml 源哈希未变；[报告](../target/demo-refactor-cleanup-20261016/demo-report.json)、[日志](../target/validation-refactor-cleanup/demo.log) |
| 三窄窗分级 | 390×844、640×360、320×640 串行全部通过；各自真实滚动 64 级 / 1,024 动作，修改末项、应用、撤销重做与 XML 核对通过。Win32、Bevy/DPI、原始 PNG 尺寸一致，面板不遮挡船体，悬停/修改目标可见且移开恢复；[报告](../target/narrow-staging-refactor-cleanup-20261016/report.json) |
| 原图 / 身份 | 桌面与三窄窗修改状态原图均已查看，各 GUI 副本与源 EXE 的 SHA 一致；[桌面](../target/demo-refactor-cleanup-20261016/editor-staging-draft.png)、[390×844](../target/narrow-staging-refactor-cleanup-20261016/390x844/target/editor-staging-draft.png)、[640×360](../target/narrow-staging-refactor-cleanup-20261016/640x360/target/editor-staging-draft.png)、[320×640](../target/narrow-staging-refactor-cleanup-20261016/320x640/target/editor-staging-draft.png)、[构建身份](../target/validation-refactor-cleanup/build-identity.json) |

本节 EXE SHA-256：`d78a47f37f6c40e92676467e0df2ff73fac7388f3a1ea08320b0dd73c1d4236e`。构建身份记录了构建时的基线及待提交文件，两块代码随后独立提交，未在验收后修改。

本轮未重跑全库 XML 探针、450ms 观看档或仅启动窄窗，未执行物理触屏、系统键鼠/输入法专项，未录制视频或打包。以下旧构建记录保持原日期和计数。

## 2026-10-13 分级停靠、图标与悬停联动

按实际打开的 SR sandbox / Builder 分级侧栏与部件图标组织方式调整；复用现有属性草稿、Action/历史事务、目录贴图注册与等比绘制，不增加素材体系或平台工程。普通属性/重号修复仍用 Modal。README 保持 51 行；上一阶段结构/文档清理已移入[历史记录](validation-history.md#2026-10-13-结构与文档清理)，不混用旧构建验收。

- 宽屏/短横屏左侧停靠，宽屏最多 480px；窄竖屏底部停靠，两侧目录暂时收起，自动显示/适配船体，不用整屏遮罩。
- 固定部件图标/名称、移动、移除三列，长名截断，动作控件不换行、至少 44px 高；短横屏正文可完整容纳一行动作，应用/取消固定在正文外。
- 悬停级标题或动作图标/名称时，本组相应部件同步显示青柠填充、轮廓和 ID，移开恢复已选焦点；不改船体/选区/历史，不猜缺失或歧义引用。点击/修改继续保留目标联动。
- UI 卡片从第 1 级显示，XML 执行索引仍从 0 起。分级编辑期间隐藏会遮挡船底目标的演示横幅；驱动复用真实滚动正文，不猜固定位置。

### 该历史构建的实际验收

| 检查 | 实际结果 / 本地证据 |
| --- | --- |
| Rust | `cargo test --workspace --release --locked`：383 项（core 104 / editor 279）通过；本次进程 TEMP/TMP 使用工作区目录；[日志](../target/staging-ui-20261013/rust-current.log) |
| Python | 151 项通过；[日志](../target/staging-ui-20261013/python-hover-final.log) |
| 格式 / Release | fmt check、全目标 Release Clippy `-D warnings`、workspace Release build 均通过；[fmt](../target/staging-ui-20261013/fmt-current.log)、[Clippy](../target/staging-ui-20261013/clippy-current.log)、[build](../target/staging-ui-20261013/build-current.log) |
| 完整窗口演示 | DX12 40ms，19 章/45 例/95 个唯一非空证据，退出 0；[报告](../target/demo-staging-hover-final-20261013/demo-report.json)、[日志](../target/staging-ui-20261013/demo-hover-final.log) |
| 三窄窗分级 | 390×844、640×360、320×640 串行，均退出 0；各自真实滚动 64 级/1,024 动作，验证仅悬停目标可见及移开恢复，再修改末项/应用/撤销重做/XML；Win32、Bevy/DPI、原始 PNG 三重核对通过，三张草稿原图均已查看；[报告](../target/narrow-staging-hover-final-20261013/report.json) |
| 三窄窗启动 | 同三种尺寸串行，均退出 0，三重尺寸通过，原始 PNG 均已查看；仍不代表初始相机能完整取景；[报告](../target/narrow-staging-startup-final-20261013/report.json) |
| 布局 / 视觉 | 分级面板与船体矩形不重叠，悬停和修改目标完整处于船体区；桌面草稿原图已查看，图标/控制列单行、船底标记不再被横幅遮住；[几何与悬停报告](../target/demo-staging-hover-final-20261013/editor-staging-performance.json)、[修改状态截图](../target/demo-staging-hover-final-20261013/editor-staging-draft.png) |
| XML / 不变性 | 目录 27 类/793 字段往返退出 0；185 船体/125,856 部件往返，另 5 个既有异常明确拒绝，verify_ships 如实退出 1；191 份源 XML 哈希未变，7 个 GUI 副本与最终 EXE 的 SHA 一致；[目录](../target/staging-ui-20261013/catalog-final.log)、[船体](../target/staging-ui-20261013/ships-final.log)、[汇总](../target/staging-ui-20261013/summary.json) |

最终 EXE SHA-256：`883c080b6213b20a1466b77098ad3089e23b6efeadcaad1a8d7105d691414976`。系统 TEMP 拒绝访问、早期短横屏越界、旧 320×640 分级超时及横幅遮挡都如实留在[中间失败记录](validation-history.md#2026-10-13-分级界面中间验证与失败)；未放宽断言、未把失败称为焦点干扰。

本次未运行 450ms 观看档、Vulkan、物理触屏/移动真机、系统键鼠/文字/输入法/原生路径专项，未录制视频或重新打包，不推送。悬停由真实 egui 逐帧内部移动事件与绘制几何核对，截图为修改状态；不声称测量系统输入或物理显示延迟。

## 未完成与兼容边界

1. 新增帮助 / 抽屉 / 画布历史融合、工具行为、双后端完整验证、两份视频与 sr1 多状态图仍在进行中。上轮三级火箭 DX12 缺件 / 错图已有正式修复及原图闭环，不把该基线当作新要求已经完成。
2. 复杂连接点自动镜像的原版可执行行为尚无定论；flipX/flipY 元数据保留，但不能凭字段名猜测自动选择。非中心接触和混合交互继续按证据收敛。
3. 窄窗尺寸验收与初始相机整船取景分别核对；旧顶栏布局下的相机边界属于原构建记录，当前仍可用既有“适配视野”，不以尺寸正确推断初始取景已通过。
4. 真实大型船体目录耗时、不同驱动/设备和长时间混合交互仍需抽验。内部同帧 Transform 回归不是物理显示延迟测量。
5. Android / iOS / Web 宿主、资源/存储及系统文件选择未交付；物理触屏、软键盘、安全区域和第三方输入法需目标设备专项。按当前约定跳过输入法/系统文字专项。
6. Python 扩展兼容、旧 Console/任务栏等应用平台能力仍独立列项；不迁移原版未启用实验，也不照搬有数据丢失风险的 SaveStatus。
7. 新 XML 扩展必须先确认无损合同并加入对照回归；5 个已知异常输入明确拒绝，不为追求“全库退出 0”削弱检查。

原版行为对照以 Difficult-Rocket 的 SR1 Rust/Python 源码、PartList/船体库，以及已实际打开的 SimpleRockets-PC-1.6.11 sandbox/Builder 为依据。没有证据的差异保持未完成，不以 UI 截图或旧通过记录推断兼容。
