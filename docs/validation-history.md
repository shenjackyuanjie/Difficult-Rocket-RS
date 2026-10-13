# 历史验收记录

本文件按阶段冻结历史事实，**不是当前构建的验收结论**。旧界面、依赖、测试计数、窗口尺寸及“最新/本轮”等说法仅在对应日期和程序内成立。当前状态见[编辑器进度](editor-progress.md)，复现方法见[验证指南](validation.md)。

`target/` 证据不随 Git 发布，部分旧日志/报告/ZIP 已不在工作区；保留历史路径，不补造产物，也不把较早通过覆盖后续失败。

## 对照来源

- `Difficult-Rocket/mods/dr_game/sr1_ship.py`：船体文件选择、贴图与断开组渲染、镜像旋转、视图平移缩放、文件拖入、保存、截图和调试显示。
- `Difficult-Rocket/mods/dr_game/Difficult_Rocket_rs/src/src/sr1_parse/raw/ship.rs`：SR1 XML 结构、默认值、部件运行状态、分级与两类连接。
- `Difficult-Rocket/mods/dr_game/Difficult_Rocket_rs/src/src/python/editor.rs`：编辑区域命中形状。
- `Difficult-Rocket/assets/builtin/PartList.xml`、`assets/ships`：真实目录及船体样本。
- `SimpleRockets-PC-1.6.11`：通过用户提供的既有 `gui-auto/gui-auto.ps1` 实际打开 sandbox / Builder，观察控件风格、部件组顺序、成员及独立折叠行为；不只根据静态资源推断。

## 2026-10-05 中间检查点（历史）

- XML 输出根节点为 `Ship`，断开组使用 `DisconnectedParts/DisconnectedPart` 层级。保留 Pod、Staging、Tank/Engine 区分、降落伞、着陆架运行字段和两类连接。严格检查根节点、尾随内容及未知字段；遇到无法无损解释的内容时明确拒绝。
- 批量编辑原子执行；拖动与吸附一次撤销。失败和无变化命令不破坏重做栈；保存点按文档内容比较。
- 历史每步只保存一份可交换快照，撤销重做不克隆整船；默认上限 256 步、64 MiB，按前后容量较大的一端预留预算，超出时淘汰最旧记录，至少保留最近一步。容量包含部件、组、连接、字符串、分级数组等 owned 分配，不含分配器和渲染开销；失败/no-op 不清理重做或提前淘汰历史。
- 删除只清理被选实例所在组的连接、对接及分级激活引用。跨组连接合并相关组；拒绝本组反向重复连接和缺失部件引用。
- 选择、拖动、碰撞、吸附、变换、删除及属性草稿使用 `PartKey`（组、ID、同组出现次数），避免误操作其他同 ID 实例。连接占用与分级目标只在本组解析。旧的裸 ID 编辑命令遇到多个实例时明确报错。结构变化及撤销重做清除旧选择，避免快照位置复用后误选另一组。
- 跨组合并只重编号被合并组与目标组发生冲突的 ID，同步更新普通连接、对接与 Pod 分级引用；悬空引用也保留独立命名空间，不因合并意外指向新部件。无关组保持原数据，失败原子回滚，一次撤销恢复原分组及编号；批量命令完成后才压缩被清空的组。
- 同组重复 ID 的独立属性可编辑，未被引用的实例可分别移动和删除。已有连接/分级无法判定归属时，相关拓扑操作明确报错，连接线也不猜测端点；读取和保存不自动修复。
- 属性面板提供重复编号修复：展示实例类型、位置、新编号及相邻部件信息，对普通连接两端、对接插头和分级动作逐字段显式指定实例。所有引用分配完后才能原子应用；取消不变，修复一次撤销，其他字段及无关组保留。预分配编号避开现有部件和悬空引用；快照过期或编号被占用时拒绝应用。已有属性草稿须先处理，不会因进入修复丢失。
- 已补齐多选操作核心 `DeleteSelection`、`TransformSelection` 和 `ShipFragment/Paste`：整体平移、旋转和世界轴镜像保持内部相对位置及连接，只断开选择边界上的连接；预览与提交共用碰撞检查。复制保留分组、内部连接和内部激活动作，过滤指向未复制部件的引用；粘贴避开已有编号及悬空引用，同步重映射两类连接和分级，并检查数量限制、碰撞及禁止旋转标记。原子撤销和重复实例删除顺序有回归覆盖。已接入 Shift 多选、按真实 Shape 框选（Shift 追加）、Ctrl+A、整体拖动/旋转/镜像/删除及 Ctrl+C/X/V。粘贴提供实体预览、红色碰撞/绿色吸附、旋转镜像与取消，提交后选中新实例。文件操作取消预览但保留剪贴板，输入草稿继续隔离画布快捷键；整体拖动在侧栏释放或失焦时取消。
- 连接点保留 location、flipX/flipY、group、order 等元数据。固定点与沿边连接面分别计算；边允许多个独立接触位置，固定点和共享组检查占用。方向随旋转与镜像变化，显式 docking 点只匹配插头与端口；燃料属性不作为机械连接限制。
- 吸附会依距离及目录优先级查找可用候选，跳过占用、不兼容和碰撞候选，拖动时忽略将断开的旧连接。编辑器提交走带目录校验的原子命令，再次检查连接点范围、接触和占用；导入的历史连接不被清理或强制修正。连接线使用实际接触位置，不把边上连接画到部件中心。
- 部件尺寸采用 PartList 的 30 像素单位，实例位置采用 Ship 的 60 像素单位。矩形命中和碰撞支持任意旋转；连接点编号按 SR1 从 1 开始。
- 保留目录中全部 Shape 的顶点及 sensor 属性；命中和碰撞支持凸多边形、组合形状、圆形车轮与默认矩形，并应用任意旋转和双轴镜像。组合轮廓保留凹口，传感器不参与实体碰撞；自定义非法或非凸 Shape 明确拒绝，凹轮廓使用多个凸 Shape 表达。
- 放置、移动、旋转和镜像统一拒绝实体内部重叠，边界接触允许；预览及拖动以红色显示碰撞。失败保留原船体、连接及撤销/重做记录；尊重 `ignoreEditorIntersections`。远距离部件先做包围圆排除，避免不必要的多边形计算。
- 放置、选择、拖动预览、取消、删除、旋转、镜像、保存及撤销重做。拖动预览不直接修改文档。
- 原版贴图、中文字体、HUD、连接线、缩放、平移、复位和截图。F / Shift+F 将整船或选区适配到两侧面板和顶部提示之外，边界同时包含旋转贴图与实体 Shape。窗口最小 960×640；F3 显示鼠标/视图坐标、原点连线和选中部件轮廓/连接面，F4 切换船体贴图显示，不改文档及历史；属性草稿隔离视图快捷键。
- 部件显示按所属组、ID 和同组出现次数缓存，文档变化只增删相应实体；拖动更新相关部件，取消后恢复。绘制层级随文档顺序更新，与反向命中次序一致。连接线在文档变化时按组索引并缓存，不在每帧逐条扫描整个船体。
- 顶部文件工具栏、原生打开/另存为对话框、Ctrl+N/O/S/Shift+S、XML 拖入与标题未保存标记。新建、打开和关闭窗口统一提供保存/放弃/取消；失败和取消保留文档及撤销栈。
- 文件保存使用同目录临时文件、同步后替换，失败时保留原文件；“先保存再重新打开同一文件”会读取最新内容。
- 常驻船体列表支持排序、滚动、切换目录、刷新和当前文件标记，过滤异常 XML 并提示数量；切换目录失败时保留原列表。
- 部件目录显示分类、贴图、名称、说明和数量上限，排除隐藏部件；鼠标选择与 Tab/Shift+Tab 共用模板。放置预览支持旋转、镜像、连续放置、吸附与取消；数量超限时显示红色并拒绝放置。
- 新部件按目录规格初始化 Tank/Engine 燃料种类和 Pod 分级状态。新建船体按原版 `none_ship.xml` 初始化隐藏驾驶舱及其位置，避免新文档无法添加驾驶舱。预览与实际提交共用吸附逻辑，放置和连接一次撤销；侧栏输入与画布隔离。
- F2 属性面板支持激活状态、燃料、驾驶舱名称与油门；分级步骤增删排序、目标部件选择、激活动作增删及移动标记编辑。草稿应用使用原子批量命令，一次撤销；取消、非法数字、过量燃料、缺失或重复分级引用不修改文档。未修改的分级状态及其他运行字段保持原样。
- 属性文本框支持中文 IME 消息、全选替换和按字符边界移动/删除；草稿打开时隔离画布与文件快捷键。关闭窗口或文件拖入要求先应用或取消，保留未提交草稿。模态滚轮不滚动后方侧栏。

验证记录：

- `cargo test --workspace`：123 项通过（63 项核心、60 项编辑器，当前 egui 界面）。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- `cargo build -p dr-editor`：通过。
- 真实 Vulkan 窗口加载 `Test.xml` 并截图，已人工检查贴图与中文显示。尚未覆盖所有鼠标与快捷键组合。
- `python -X utf8 scripts/check_native_dialogs.py`：真实原生打开取消、另存为取消、未保存退出取消、放弃后退出通过；确认样本文件哈希未变。此测试从应用文件消息入口触发，不依赖 Windows 桌面焦点；工具栏按下、拖入、快捷键保存和关闭事件另有 Bevy 集成测试。
- `--panel-smoke-test`：真实窗口中通过 Bevy 输入与 UI 状态验证目录选择、预览实体及旋转/镜像、碰撞拒绝及红色预览、合法放置、一次撤销/重做、Esc 取消、侧栏点击隔离、滚轮只滚动目录；三张阶段截图已检查。不等同于全量人工鼠标交互验收。
- `--properties-smoke-test`：真实窗口中验证中文 IME 提交消息、草稿不改文档、快捷键隔离、分级激活动作及移动标记、应用、一次撤销/重做、Esc 取消和 XML 文件保存往返；输出草稿/完成截图及独立测试 XML，不写回源样本。系统输入法候选窗与所有输入组合仍需整体验收。
- `--connection-smoke-test`：真实原版目录与新建驾驶舱，在长梁下方两个不同位置分别吸附分离器，验证同边多连接、保留横向偏移、一次撤销/重做及 XML 文件往返，截图输出到 `target/editor-connections-smoke.png`。
- `--scoped-smoke-test`：真实窗口中用原版分离器构造三个同 ID 分组，经鼠标与快捷键输入验证精确选择/着色/拖动预览、组内移动、删除、跨组吸附重编号、一次撤销重做与 XML 文件往返，截图 `target/editor-scoped-smoke.png` 已检查。目录、属性及连接窗口自测均回归通过。
- `--repair-smoke-test`：真实 `Heronb.xml` 中重复编号 `1200404` 的发动机与油箱，通过实际 UI 按钮分配三条连接及一条 Pod 分级引用。未分配拒绝、草稿不改原船体、原子应用与撤销重做、修复后独立删除油箱且保留发动机引用、XML 保存往返均通过；源文件哈希未变。草稿与完成截图已检查，正常属性窗口自测回归通过。修复保留历史连接参数，不据此声称历史连接的物理几何全部合法。
- `--selection-smoke-test`：真实窗口输入验证 Shift 多选/取消选择、框选、整体拖动预览不改文档、内部连接保持、复制与粘贴碰撞颜色、旋转预览取消、合法吸附粘贴、一次撤销重做、全选删除、侧栏释放/失焦取消及 XML 往返。预览与完成截图已检查，截图捕获后再提交以确保拍到绿色预览；目录、属性、分组、连接窗口自测均回归通过。
- `--view-smoke-test`：真实窗口验证 F 整船适配、实际调整到 960×640 后再次适配、Shift+F 选区适配、F3 坐标标签、F4 显隐及数据/历史不变。`target/editor-view-smoke.png` 已检查，侧栏、HUD 和调试标签可读，旋转轮廓与贴图一致；多选、属性、目录交互回归通过。
- `verify_history`：真实 Ophioglossum 的 10,679 部件连续 400 次属性编辑，保留 18 步、预留 66,300,930 字节（约 63.2 MiB），编辑耗时 796.452 ms；18 次撤销及重做共 0.010 ms，文档及 XML 往返一致。预算淘汰、单步超预算、嵌套分级容量、失败/no-op 保留重做和零克隆交换有测试覆盖；多选、属性、真实重号修复窗口回归通过。
- `verify_catalog_edits`：真实 27 类部件的 432 个旋转/镜像姿态 XML 往返，729 种有序部件对，双方 16 种姿态按连接面中心对齐，共 337,984 次兼容连接提交与撤销重做通过，729 对均覆盖连接后 XML 往返。此枚举覆盖中心对齐的姿态组合，不等同于沿边无限接触位置及所有用户输入序列；非中心位置另有连接面测试与窗口验收。
- 多选窗口自测曾受外部焦点/鼠标消息干扰而取消拖动；已持续维持注入状态，同时保留专门的失焦取消断言，复测通过。
- `--browser-smoke-test`：生成 1,000 个小船体文件及 1 个异常 XML，扫描 143.481 ms；排序、全部行渲染、滚动到底、打开末项、保留滚动位置和异常文件过滤通过。`target/editor-browser-smoke.png/json` 已检查；该数据不能代表 1,000 个万部件船体的解析耗时。
- `--staging-smoke-test`：64 级、1,024 个激活动作，真实 UI 滚动至末项并切换移动标记；草稿隔离、原子应用、一次撤销重做及 XML 往返通过。`target/editor-staging-draft.png` 已检查，最后一项移动标记及应用/取消按钮可读。
- 本轮末尾再次回归 Windows 原生打开取消、另存为取消、退出保护取消与放弃退出，源样本哈希保持不变。
- 截图检查发现并修复滚动列表文本宽度为零导致名称不显示的问题；真实交互自测发现并修复取消预览重置滚动位置的问题。
- 样本调查发现 33 个 XML 存在跨组重复 ID，另有 `Heronb.xml` 主组内重复 ID，合计 34 个样本有重复 ID。`verify_scoped_edits` 对每组抽取一个重复 ID 实例，并覆盖额外的同组重复实例：34 个真实样本、282 个实例的属性编辑、撤销重做及 XML 往返通过，全部在内存执行。此项不等同于所有样本的全部拓扑编辑验收；5 个异常输入仍明确拒绝，程序返回非零状态。
- `--performance-test`：Windows、RTX 5060 Ti、Vulkan、1440×900、开发构建（应用 opt-level=1、依赖 opt-level=3）、自动 VSync 下，真实拖动 90 更新帧（89 个计时间隔），包含鼠标编辑、吸附、碰撞及渲染。三个样本均零实体重建，取消后文档数据不变，截图已检查；这些结果不代表其他机器或所有交互的性能。

  | 样本 | 部件数 | 平均 ms | 中位 ms | P95 ms | 最大 ms |
  |---|---:|---:|---:|---:|---:|
  | Ophioglossum.xml | 10679 | 23.646 | 23.353 | 25.322 | 35.598 |
  | 003.xml | 2304 | 16.957 | 16.869 | 18.264 | 25.554 |
  | Heronb.xml | 799 | 16.692 | 15.614 | 27.913 | 31.673 |

  报告与截图保留在 `target/editor-performance-{样本名}.json/png`。

  接入分组编辑后，同配置对 `Ophioglossum.xml` 回归：平均 24.560 ms、中位 24.357 ms、P95 25.722 ms、最大 38.659 ms，89 个帧间隔、零实体重建、取消后原文档不变。产物为 `target/editor-performance-scoped-Ophioglossum.json/png`。
  接入多选 UI 后，同配置单选回归得到平均 33.045 ms、中位 32.741 ms、P95 35.838 ms、最大 45.976 ms，零实体重建、取消后文档不变，产物 `target/editor-performance-selection-Ophioglossum.json/png`。后续加入连接面范围剔除后，单选测得平均 27.078 ms、P95 31.909 ms；开发构建及系统负载仍有波动。

  新增可指定选择数量的性能自测；定位到整体吸附为大量不可用落点反复克隆整船。缓存连接面/碰撞范围，先做不克隆文档的实际 Shape 碰撞预筛选，再对可用候选运行完整原子命令校验；同一碰撞落点不重复校验。100 部件的平均耗时从 582.193 ms 降至 48.545 ms。连接面超出贴图边界、旋转镜像、圆形及碰撞豁免有回归覆盖，不能用贴图尺寸裁掉合法连接候选。

  | Ophioglossum 多选数 | 平均 ms | 中位 ms | P95 ms | 最大 ms |
  |---:|---:|---:|---:|---:|
  | 100 | 48.545 | 48.423 | 53.413 | 65.579 |
  | 1000 | 79.170 | 83.160 | 92.519 | 106.127 |
  | 10679（全选） | 62.189 | 61.527 | 70.160 | 83.873 |

  均为 89 个帧间隔、零实体重建、取消后文档未变；产物 `target/editor-performance-multi-{100-final,1000,all-final}.json/png`。此规模仍未达到 60 FPS；不同选择形状、粘贴、保存和历史操作尚需各自验收，不能将单一拖动基准等同于全路径性能。
- Bevy 0.19 的 Parley 使用 `new_for_non_complex_scripts`，运行时仍报告 ICU4X 中文分词模型缺失；截图中的中文字形正常。关闭自动换行未消除该日志，后续需检查上游分词配置，不应把它当作字体加载失败。
- 190 个样本中 185 个、125,856 个部件完成模型及**原始 XML 元素、属性**对照往返，包括着陆架字段；字符串原样比较，数字允许等值格式化。
- 5 个输入被拒绝：`Alliance.xml` 标签错配；`Amect-MAIIa TEST.xml` 的 `currentStage` 为空；`AmectVII.xml` 和 `Ss.xml` 使用非标准连接节点；`X Type 2.xml` 根节点后存在非法文本。异常输入不自动修复，也不允许静默删掉内容后保存。批量校验如实返回非零状态。
- 主分支原有 3 个提交信息已改中文，文件树与作者信息保留；旧历史备份引用为 `refs/backup/before-chinese-messages`。没有远程仓库或推送操作。

## 2026-10-04 补充对照与验收

- 修正实际贴图、放置和粘贴预览：原版按 PNG 原始像素绘制，而非目录物理尺寸。27 类中 20 类二者不同；已对齐 floor 半尺寸锚点、奇数像素镜像偏移和断开组 100/255 透明度。着陆架、对接器与 960×640 视图截图已检查；物理 Shape 仍独立于图片边界。异步加载保留实体复用。
- 多选删除改为按组/原始 occurrence 扫描，原子预检所有歧义引用，不再因删除顺序隐式消除歧义。12,000 部件中删除 9,000 个、跨组引用清理和撤销有回归覆盖。
- `verify_surface_edits`：158,720 次合法非中心接触提交与撤销重做，其中 79,360 次共同旋转 0.37 弧度；405 种可连接部件对 XML 往返，两个真实长梁同边双落点与占用拒绝样本通过。仅抽取沿段 0.2/0.8，不等同于穷尽连续几何。
- 混合流程回归补齐跨组复制/粘贴→属性/分级→另存为→跨保存点撤销重做→取消/重开，以及新建前保存失败保留剪切剪贴板和重做分支。此项文件对话框使用 mock，不冒充原生窗口验收。
- 原生 Windows IMM 自测已通过：同线程系统预编辑/提交经 Winit/Bevy 到达属性草稿，候选窗位置参数与真实字段匹配，应用/撤销/重做/XML 往返通过。另修复预编辑取消误关草稿、空文本/控制字符误删全选名称。此证据不代表第三方输入法全部候选列表交互。
- 分组、多选、目录预览、视图的真实窗口回归通过。输入注入自测持续维持焦点/坐标，避免桌面消息在释放帧取消测试拖动；真实失焦取消仍由独立阶段断言。
- 已接入 bevy_egui 0.42 / Bevy 0.19：属性、分级和重复编号修复使用内置 Modal、TextEdit、ScrollArea、ComboBox 和复选框；复用原有事务和 XML，删除手写文本光标/选区/IME 编辑路径。验收依据见下一节。

## 2026-10-04 双边栏与拓扑显示

- 两个目录边栏已迁移 egui 内置 Panel 和 ScrollArea：部件分类、贴图按钮、数量上限、说明、取消放置以及船体当前项、目录切换/刷新、异常文件提示均保留。千文件列表使用 `show_rows` 只绘制可见行，不再创建千个 Bevy UI 实体。滚动位置跨打开文件保留，切换目录或分类重置；按 Tab 更换部件时将当前项滚入视区。
- 边栏按钮通过消息进入原有操作系统，不另建文档编辑通路。属性模态打开时两个边栏禁用；画布点击/滚轮按实际 egui 边栏矩形隔离，按钮焦点不吞掉画布快捷键。部件数量按文档变化缓存；HUD 文本相同时不重复写入，避免鼠标每帧变化引发不必要的文字布局。
- 主驾驶舱可达的部件保持正常透明度；其余部件（包括主组内孤立部件和断开组）以 `100/255` alpha 淡化，无驾驶舱时以主组首实例为参考根。同组重复编号引用无法唯一解释时不猜连接归属；普通连接及对接拓扑有测试覆盖。选择/碰撞颜色保留淡化，连接提交和撤销/重做更新透明度且不重建现有实体。
- 连接线绘制中心→实际连接面→实际连接面→中心，接触点重合时仍显示两侧线段，避免原先零长度检查把合法贴合连接全部隐藏。沿边接触位置不改为固定连接面中心；跨拖动边界的旧连接在预览时隐藏，内部连接跟随整体平移。
- `cargo test --workspace`：123 项（63 核心、60 编辑器）通过；`cargo clippy --workspace --all-targets -- -D warnings`、构建和格式检查通过。新增实际 egui 控件测试覆盖 Ctrl+A 中文/emoji 替换、按字符移动和 Backspace/Delete、输入法取消同帧 Escape、点击遮罩保留草稿、显式取消、最小窗口非法值及原子应用/撤销/XML 往返。
- 最终串行窗口回归：目录预览、千文件浏览、egui 属性、64 级/1,024 动作分级、真实 Heronb 重号修复、原生 IMM、连接、分组、多选和 960×640 视图全部通过。目录、千文件末项、修复草稿、复杂分级末项、贴合连接及小窗口截图已人工检查。修复和属性自测等待截图捕获后才点击应用；滚动惯性结束后才点击控件，首个布局测量 pass 不当作可交互 UI。
- 原生 IMM 通过窗口创建线程生成系统预编辑和提交，经 Winit/Bevy/bevy_egui 到达 TextEdit，检查候选窗定位及 XML 往返。bevy_egui 直接更新 Winit，专项改读 `EguiContextImeState`，不再等候未回写的 Bevy Window 旧 IME 字段。`target/native-ime-state.txt` 为 `complete`，截图及 XML 已产生；不据此宣称所有第三方输入法通过。
- Windows 原生文件对话框脚本再次通过：打开取消、另存为取消、退出保护取消/放弃，脚本确认 Test.xml 哈希未变；自测输出均写入 target，不保存回源样本。
- 同一万部件样本（Ophioglossum，10,679 部件）、同机 Vulkan/VSync/开发配置，89 个帧间隔、零实体重建、取消不改文档。最新平均/P95：单选 `16.120/17.472 ms`，100 部件 `30.621/35.031 ms`，1,000 部件 `58.176/69.036 ms`。单选记录见 `target/egui-performance.log`，多选产物为 `target/editor-performance-egui-{100,1000}.json/png`；系统负载会波动，大量多选仍未达到 60 FPS，不能将单选结果推广到全路径。
- 功能提交：`f0ce773`（属性/分级），`befe626`（双边栏），`cade7c0`（连接线与孤立部件淡化）。独立串行执行，无 subagent、无额外 worktree、无远程推送。

## 2026-10-05 部件缩略图比例修复

- 修复右侧目录把纹理来源尺寸写成 `40×40`、进而将非方形贴图挤成方形的问题。现在从已加载的 Bevy Image 读取 PNG 原始像素宽高，保持比例缩放至 `40×40` 范围；未解码时暂用目录宽高比例，图片加载后自动使用真实尺寸。
- 原版 `DetacherVertical.png` 为 `129×30`，`DetacherRadial.png` 为 `30×100`；预览恢复横条、竖条形状。按钮改用 egui 图片/文字组合，避免图片被额外限制成默认文字高度，列表仍保持 52 点行高和原有点击行为。
- 新增两项无头 egui 绘制测试，直接检查纹理绘制矩形，覆盖原版分离器尺寸、横长/竖长/方形/奇数像素贴图和未加载的目录比例回退；像素对齐允许不足一像素的量化误差。
- 当前工作区 `cargo test --workspace` 129 项（66 核心、63 编辑器）通过，Clippy、构建及格式检查通过。目录窗口回归在系统确认的前台运行，记录 120 次前台句柄校验；选择、碰撞预览、放置、撤销重做和侧栏输入隔离通过，已查看新的 `target/editor-placement-preview.png`，确认分离器及长燃料箱未拉伸。
- 本次目录窗口输入仍为 Bevy/egui 注入式回归，前台校验不把它变成真实系统键鼠测试。新的系统键鼠专项正在补验，不能用本次结果代替。用户已反馈实测 egui 输入法正常，记作实机反馈，不扩大为所有输入法候选列表验收。

## 2026-10-05 部件列表双列布局

- 参考用户给出的列表，将图片与名称分列：图片列固定为 56 点，缩略图居中并保持真实比例，右侧名称从统一位置左对齐；两列之间有分隔线，长名称截断并保留说明悬停，整行仍由内置 Button 处理选中/点击，行高保持 52 点。
- 新增布局与交互绘制测试：横条、竖条、方形和不同长度名称的文字起点一致，图片中心固定，长名称不越界；点击图片与文字都触发同一行。当前工作区 131 项测试、Clippy、构建通过。
- 目录注入式窗口回归在系统前台通过 117 次句柄校验，已查看新截图确认双列、左对齐与比例。注入驱动现在逐帧保存自己的测试光标，避免桌面 CursorMoved 覆盖释放阶段的坐标，不以此冒充真实焦点。
- 按用户要求暂跳过输入法及系统文字输入专项；已保留用户实机正常的反馈，需要进一步专项验证时通过 noticer 联系。前台失败自动重试最多三次，连续失败时提醒，不把程序断言错误当作焦点失败吞掉。

## 2026-10-05 从部件列表拖拽放置

- 图片列或名称列所在整行均支持按住拖出部件；进入画布后显示虚影，保持 PNG 原比例、镜像/旋转及现有吸附/碰撞规则。目录拖拽虚影与未连接部件共用 `100/255` alpha，吸附显示绿色、无效落点显示红色，透明度不因颜色提示改变。
- 松手只提交一次放置及连接，合为一个原子历史步骤；P 提前提交也结束虚影，之后松手不重复放置。原有单击目录后连续放置仍保留，Tab 切换回普通放置模式。
- 在侧栏释放、Esc/右键、失焦、窗口外释放、文件操作或属性模态取消拖拽；碰撞、数量上限及无效位置不改文档、不丢重做。统一清理拖拽模式，避免取消后部件粘住鼠标。
- 新增拖动手势、虚影透明度、放置/连接原子撤销、重复结束、碰撞/上限拒绝、侧栏/失焦/Esc/文件取消及 P 提前提交的测试。最终工作区 140 项测试（66 核心、74 编辑器）通过；Clippy、构建、格式及差异检查通过。
- 新的 Windows 系统输入回归由外部脚本只对自建编辑器/失焦辅助窗口使用系统鼠标及 SendInput，应用侧仅观察，不覆写 focused、ButtonInput 或 egui 原始输入。21 步覆盖多选、从名称区域拖出虚影、空白放置、吸附连接、撤销重做及侧栏/Esc/真实失焦取消；虚影截图完成后才放开鼠标，并核验 XML 往返及源样本未改动。报告明确 `ime_tested=false`，按用户要求未运行输入法专项。
- 前台句柄在每次输入及等待阶段校验；仅焦点干扰自动重试最多三次，连续失败使用 noticer 提醒。先核验进程退出状态，程序断言/窗口崩溃不再误归为失焦重试。产物记录在 `target/foreground-*` 和 `target/foreground-last.txt`。

## 2026-10-05 前台收尾验收与多选性能

- 最新真实系统键鼠回归 21 步通过：外部 261 次、应用侧 401 次系统前台句柄检查；目录虚影的自由/吸附截图及提交后连接线均已查看。当前产物为 `target/foreground-s6n1bg3p`，原生输入法专项未运行。
- 目录、连接、分组、多选、最小窗口视图五项注入式回归均在确认的系统前台串行通过，外部检查分别为 134、131、136、153、137 次；与真实系统输入回归分开记账。原版仓库工作区保持不变。
- 同一 10,679 部件船体、89 帧采样的受控前台拖动，平均/P95：单选 `16.035/17.427 ms`，100 部件 `23.509/24.633 ms`，1,000 部件 `41.384/44.591 ms`，全选 `41.100/43.294 ms`；四项均零实体重建，取消不改文档。报告为 `target/foreground-performance-{1,100,1000,10679}.json`。
- 早先基线未统一校验前台状态，不能用它与上述结果作严格性能 A/B。大量多选仍未达到 60 FPS，完整复刻目标和其他语义差异仍不能据此宣称完成。
- 前台驱动另有六项无桌面/无通知副作用的 Python 回归，覆盖三次焦点失败仅提醒一次、程序失败不重试、第三次恢复不提醒、所有窗口测试名称对应真实 CLI 标志，防止复数连接名称误映射后进入普通窗口。


## 2026-10-05 连接树、连接图与快捷键抽验

- `dr-core` 已按组内 `PartKey` 构建有向连接图和确定性生成森林，保留环、多父额外边、孤立节点、悬空/歧义引用及历史独立第三对接引用；普通对接记录中的 `dock` 父/子别名不会再制造伪自环。精确断边与树换父均进入原子历史，树模式拒绝自身/后代、多父歧义和无效几何，图模式允许合法环。
- 编辑器新增 F6 连接窗口：树视图支持节点/子树选择、整支删除和设置父节点；图视图支持拖动排版、分量选择、普通/对接连接和精确断边。窗口与画布共享选择和撤销历史，打开时隔离画布快捷键并取消未提交放置。
- `target/editor-topology-{tree,graph,smoke}.png` 已查看；真实 egui 控件窗口回归通过树删除、换父、树环拒绝、图环、分量、断边、撤销重做、最小窗口和 XML 往返。当前全工作区测试为核心 86 项、编辑器 81 项，Clippy 全目标零警告。
- 新增真实系统快捷键验收。Windows `Delete/Home` 使用 E0 扫描码；本次实际运行已通过 Delete 并继续到 P 放置步骤，随后连续三次失去真实前台，已按约定向 `sr1` 提醒，**不能记为完整通过**。输入法和系统文字输入仍按用户要求跳过；驱动自身 6 项无桌面副作用回归通过。

## 2026-10-09 任意角度连接、自由模式与十章演示首轮验收

- 连接点/边与实体共用任意角度及镜像姿态；辅助连接取消直角/反向法线限定，改用最近接触点距离与真实实体相交面积（相交 / 较小实体面积严格小于 5%）。多 Shape 先取并集，传感器不计入，圆使用解析交集；新增旋转、镜像、大坐标下 4.9% / 5% / 5.1% 边界回归，保守处理裁剪量化误差。
- 新增自由模式双点连接/精确断连；移动、旋转、镜像保持原连接，不吸附、不自动连断，放置和粘贴允许重叠；仍保护引用、对接类型与编号归属。精调支持 Q/E ±15°、Shift+Q/E ±1°，R 保留非直角偏移，X/Y 支持选区及拖拽组合镜像；撤销重做与 XML 往返均有回归。
- 实际窗口暴露了拖拽轮廓已镜像而 Sprite/奇数像素锚点仍使用原姿态的问题，已修复并补当帧渲染及 Esc 恢复回归。自由放置虚影也绘制随旋转镜像的端点/边。
- `check_native_input.py --window-case transforms` 通过 164 次真实前台校验，检查 16° 连接（实体重叠约 2.5833%）、122° 双镜像拖拽、约 4.233 Ship 单位远距离手动连/断、完全重叠放置、多选及撤销重做。产物 `target/foreground-ya9i4dp4/driver.json`；这是内部输入窗口回归，不冒充系统键鼠或输入法验收。
- 首次完整 demo 被用户误点打断，在选区拖拽阶段断言失败，未记为通过。随后隔离内部回放与原生鼠标/按键/文字/滚轮，保持演示自身的失焦测试，新增误点/按住输入不会重复触发的回归。
- 本次 `demo_editor.py --fast --no-build` 的十章全部通过，产物位于 `target/demo-rotation-free-20261009-002524`，原版 Test.xml / Heronb.xml SHA256 未改变。实际运行 `cargo test --workspace --locked` 267 项（97 核心、170 编辑器）、全目标 Clippy 和 107 项 Python 脚本回归通过；TEMP/TMP 使用 `target/test-temp`。视频工具仅更新十章兼容与无 GUI 测试，**未渲染视频**。
- 首轮收尾时，用户后来追加的连接线可调设置及树/框选来源的成组旋转镜像验收仍在单独功能块推进；上述历史结果不代替以下追加需求的验收。

## 2026-10-09 可调连接线与整组变换追加验收

- 右侧新增“连接线设置…”窗口：显示开关、RGBA 颜色、1–12 px 粗细、实线/虚线/点线、静止/呼吸/流动效果、间距、速度、方向箭头及恢复默认。设置独立于船体、XML 和撤销历史；使用独立 Gizmo 配置，不改变自由模式连接点提示的样式，隐藏连线仍可手动连接。
- 树中子树、连通分量及真实框选共用刚性选区变换，围绕共同中心旋转或镜像；后代跟随拖拽覆盖全部子孙。精调与双轴组合镜像同时更新各部件位置、角度、贴图、连接点和预览连线，内部连接及未选部件保持，提交一次历史并支持撤销重做。新增测试同时覆盖辅助/自由模式的树、连通与框选来源。
- 扩展 `--transforms-smoke-test` 至完整 112 阶段，实际点击连线配置、树中子树/连通选择、框选及后代跟随开关，输出十二张截图与 XML/JSON。前台驱动改以最终 JSON 判定完成，避免中途截图掩盖后续失败；本次 `check_native_input.py --window-case transforms` 通过 258 次系统前台校验，记录为 `target/foreground-1zwas9yi/driver.json`，`real_system_input=false`、`ime_tested=false`。
- 窗口验收发现树窗口首次打开尚未布局、以及小窗口框选末点落入侧栏的演示夹具问题，分别等待实际 UI 布局、收紧夹具到可用画布后重新通过；未削弱选中数量、刚性姿态或文档断言，也未把程序断言错误当作焦点失败重试。
- 本次完整十章 demo 在快速模式和正常 `--step-ms 450 --exit-on-complete --no-build` 节奏均正常退出且全部通过，分别位于 `target/demo-lines-groups-20261009-004818`、`target/demo-lines-groups-watch-20261009-005107`。两份报告均有 40 个非空产物、20 条章节时间线和 204 条动作记录，第三章有 14 个产物；另行审计最终 JSON，树/连通/框选/全部后代变换、连线真实控件、历史不变与 XML 往返断言全部为真。原版 Test.xml / Heronb.xml SHA256 未变，已查看新增连线及整组变换截图。
- 当前工作区实际执行 `cargo test --workspace --locked`：273 项通过（97 核心、176 编辑器）；`cargo clippy --workspace --all-targets --locked -- -D warnings`、`cargo fmt --all -- --check`、`git diff --check` 通过。`uv run --no-project --python 3.12 python -B -X utf8 -m unittest discover -s scripts -p 'test_*.py'`：107 项通过；Rust 测试 TEMP/TMP 使用 `target/test-temp`。以上是追加需求的本次验证，首轮及 2026-10-05 的数量仅为历史记录。
- 本轮编辑增强及追加演示要求已验收，未渲染视频；演示使用内部 UI/画布输入，不冒充系统键鼠验收，输入法和系统文字输入专项按要求跳过。这不代表下列全项目复刻缺口已经消除。

## 2026-10-09 简略与详细演示扩展

- 默认 `--mode brief` 保持原有十章与节奏；`--mode detailed` 穿插八章、四十一个新增边界样例，不以旧专项的固定时长截断。正常节奏每例通过后另留阅读时间，`--fast` 只缩短节拍，不减内容。
- 新增样例按放置、精确几何、自由连接、整组变换、连接线、历史/属性、拓扑、文件工作流分类，数量为 3/6/7/6/5/7/4/3。每例显示编号、预期、真实输入或精确事务路径和断言结果，并保存独立截图及实际数据；不宣称穷举连续角度和位置。
- 详细分级实际使用 64 级、1,024 个动作，滚到最后一级最后一个动作修改草稿、应用、撤销重做及 XML 往返；驱动另核验规模和末项索引。简略仍使用 8 级、48 动作夹具，不改变原观看节奏。
- 开发回放暴露的样例推进状态未清理、夹具错误和颜色末位浮点差异均单独核实并补回归，失败回放不计为通过。原版插头的普通安装点语义未改；类型错配使用临时显式 `dock=true` 夹具并恢复目录。树自连接按真实按钮点击后拒绝验证，不假称按钮禁用。
- 演示入口从 2,139 行拆至 168 行，实现模块最长 281 行；原内嵌与 726 行节拍测试按职责拆分，保留原 27 项演示测试，最长测试文件 445 行。边界样例也分模块维护，目前最长 495 行，避免继续堆到一个文件。
- 人工查看正常回放截图发现，非法属性样例的详细说明会挡住模态底部错误和取消按钮。现按本帧真实应用控件的位置把详细说明移到模态外侧；首次尚无控件时等待布局。新增真实属性控件的避让、错误文字可见、取消仍可点击及输入/焦点不变回归，普通章节保留原底部居中样式。
- 本轮最终实际执行 `cargo test --workspace --locked`：283 项通过（97 核心、186 编辑器）；Python 脚本全套 `unittest`：114 项通过。全目标 Clippy 零警告，格式、差异检查及重新构建通过；Rust 测试 TEMP/TMP 使用 `target/test-temp`。
- 两种模式的正常/快速回放均完整通过，退出码均为 0，源样本 SHA256 未改变；另只读核验模式、顺序、全部新鲜非空产物、逐例证据及时间线。简略为 10 章/40 个产物/20 条时间线/204 条动作，详细为 18 章/41 个新增样例/90 个产物/36 条时间线/364 条动作；详细分级确为 64/1024，末项 63/15，耗时含观看停顿而非性能基准。
- 最新详细正常与快速目录为 `target/demo-detailed-watch-20261009-110517`、`target/demo-detailed-final-fast-20261009-110330`，均在说明避让修复后重跑；已查看正常节奏下的几何边界、整组变换、连线重置、长分级、非法草稿和文件取消截图，错误及按钮完整可见。简略正常为 `target/demo-brief-watch-20261009-105128`（说明布局微调前，默认模式），微调后简略快速再通过于 `target/demo-brief-fast-20261009-111358`；普通说明位置另有回归保护。
- 用户本轮执行 `cargo clean`，旧 `target` 产物已清理；上述早期路径仅是历史记录，本轮使用重新构建的程序和新目录验收。仍是内部 Bevy/egui 回放，不冒充系统键鼠；输入法及系统文字输入专项按要求跳过，未渲染视频，也不代表下列全项目缺口消除。

## 2026-10-12 拖拽性能、连接唯一性与交互演示

- 空闲和背景平移不再制造文档或相机投影的伪变更，快捷键按实际按下边沿处理；部件与相机的释放帧保留最后一段位移。连通分量计算改为分组编号计数和邻接遍历，并按文档版本与拖拽成员缓存侧栏删除提示，避免每个预览帧重复扫描全船。
- 同一对部件的新连接按无向端点去重，方向、连接点和 Normal/Dock 类型均不能绕过；失败在写回文档前拒绝，不污染历史。新放置编号同时避开连接和分级的悬空引用，候选提示与提交保持一致；历史 XML 不因载入被自动清理。
- 10,679 部件、单件拖动 89 帧的同规模窗口对照，平均应用帧耗时由 386.853 ms 降到 30.231 ms，中位数由 384.073 ms 降到 24.645 ms，P95 由 415.559 ms 降到 50.177 ms，重建实体均为 0。证据为[优化前帧耗时](../target/foreground-6r0hd5md/performance.json)与[优化后帧耗时](../target/foreground-yrtb_aau/performance.json)；这是应用帧耗时，不是 input-to-photon 测量。
- 实际完成交互、连接及选择窗口专项；交互专项通过 291 次前台校验。额外系统键鼠驱动三次受到失焦干扰，按约定停止自动重试，未把它记录为完整通过；输入法和系统文字输入专项仍按要求跳过。
- 新增详细模式的四个交互样例：非中心/反向/亚像素连续拖动、两种按键配置与两档缩放的背景平移、多连接面只显示最近合法一对、换点及反向换点重复连接拒绝。视觉期望来自实际本帧光标与抓取锚点，渲染实值来自传播后的 GlobalTransform；连接提示另精确核对实际提交的连接点，保留生产错误状态。原点反向点击已有边仍是主动断开，不能混称为拒绝。
- 本轮交互二审完成时实际执行 `cargo test --workspace --release`：303 项通过（101 核心、202 编辑器）；当时的全目标 release Clippy 零警告、release 构建、格式检查及差异检查通过，Python 脚本全套 unittest 实际运行 114 项通过。这是后续零长路径与触控适配之前的验收基线，新增改动另行验收；Rust 测试 TEMP/TMP 使用 `target/test-temp`。
- 最新完整详细快速回放退出码为 0，19 章、45 个新增边界样例全部通过，95 个唯一证据与 38 条章节时间线齐全；原版 [Test.xml](../../Difficult-Rocket/assets/ships/Test.xml) / [Heronb.xml](../../Difficult-Rocket/assets/ships/Heronb.xml) SHA256 未改变。[快速演示报告](../target/demo-interaction-fast-20261012/demo-report.json)与[交互逐例证据](../target/demo-interaction-fast-20261012/interaction-cases.json)记录拖动 19 次、平移 18 次同帧校验，释放帧分别为 1/2，最大世界像素误差为 0.000067408/0。随后正常 450 ms 详细回放也退出码为 0，[观看演示报告](../target/demo-interaction-watch-20261012/demo-report.json)与[观看交互逐例证据](../target/demo-interaction-watch-20261012/interaction-cases.json)记录拖动 118 次、平移 149 次同帧校验，释放帧仍为 1/2，最大误差仍为 0.000067408/0。两次均为零长路径及触控适配之前的旧构建记录，新改动以新目录回放为准。
- 用户要求的修改已按核心、性能、提示和演示四个功能块分别提交，未推送。DeepSeek 二审建议按实际价值取舍，后续演示证据完善与验收记录继续独立提交；本轮仍使用内部 Bevy/egui 回放，不冒充系统键鼠或物理显示延迟测量，未渲染视频，也不代表以下全项目缺口消除。

## 2026-10-12 Windows 小型试用包

- 已生成并独立验收 [Windows x64 试用 ZIP](../target/dist/dr-editor-win64-20261012-4ef8aa7.zip)，压缩后 28.46 MiB，包含 release 程序、必需贴图、中文字体、程序与字体许可、源码快照和 Solar / R / Co13 / Gzj / Test / Heronb 六艘范例船。该包使用触控适配前的构建，后续源码修改不覆盖已交付 ZIP。
- 实际核验 ZIP CRC、清单内 50 个文件的 SHA-256，并解压到仓库外形态的中文与空格目录；从不同当前目录调用启动脚本，免 Rust / Python 的编辑器冒烟退出码为 0。[独立启动截图](../target/portable-validation-20261012/中文%20空格目录/dr-editor-win64/target/editor-smoke.png)已人工查看。ZIP 的 SHA-256 为 `a7623769d4a523ce15480f8af079ed1cbb0c2868473a60725ef5e5908a93d045`，附[校验文件](../target/dist/dr-editor-win64-20261012-4ef8aa7.zip.sha256)。
- 打包脚本与回归已作为 `13550b7` 提交；逐例指标严格类型与有限数值校验已作为 `fa97971` 提交。加入这些改动后，全量 Python unittest 实际运行 120 项并通过，其中演示驱动 43 项、打包脚本 3 项；这些结果不代替后续 Rust / 窗口验收。

## 2026-10-12 统一触控与小屏适配收尾

- 鼠标、键盘和 Bevy TouchInput 在同一编辑模式使用；独立触摸指针锁定 UI / 画布起点归属，单指拖动 / 空白平移，放置及粘贴抬指提交。第二指取消未提交编辑并切入中点导航，全部抬指解锁，第三指封锁当前手势。兼容鼠标尾锁只来自触摸接管，纯鼠标失焦后的新 Shift+Left 边沿正常消费。
- 文件顶栏与常用编辑按钮换行，最小窗口 320×320；窄屏默认关单侧目录抽屉，主要按钮可触达。帮助、连接编辑、线设置和未保存确认限宽 / 滚动；适配视野使用真实剩余画布，窗口视图自测也按真实画布与 DPI 核对位置、比例及贴图角点。操作与平台边界见[适配说明](./mobile-support.md)。
- 最终集成工作树实际执行 workspace Release 测试 351 项（core 101 / editor 250）、格式检查、严格全目标 Release Clippy、Release build，均通过；收尾再次执行 Python 全套 127 项并通过。对应日志：[Rust](../target/validation-mobile-workspace-canvas-corrected.log)、[Clippy](../target/validation-mobile-clippy-canvas-corrected.log)、[构建](../target/validation-mobile-build-canvas-corrected.log)、[Python](../target/validation-mobile-python-closeout.log)。
- 最新构建在进程级 `WGPU_BACKEND=dx12` 下，详细快速 40ms 与观看 450ms 两档均完整通过，仍为 19 章 / 45 例 / 95 证据，源样本 SHA256 不变。分别见[快速报告](../target/demo-mobile-fast-20261012-canvas-final/demo-report.json)、[观看报告](../target/demo-mobile-watch-20261012-canvas-final/demo-report.json)。真实零长四点路径在两档均为安全跳过：长度 0、方向样本 0、提交顶点 0；[快速零长证据](../target/demo-mobile-fast-20261012-canvas-final/line-cases.json)、[观看零长证据](../target/demo-mobile-watch-20261012-canvas-final/line-cases.json)。
- [窄窗验收工具](../scripts/validate_narrow_windows.py#L38-L98) 从初始 Bevy 尺寸启动自建进程，仅只读测量 Win32 客户区，并交叉检查 Bevy JSON 与原始 PNG。较早构建[三尺寸完整报告](../target/mobile-narrow-20261012-owned-window/report.json)已通过；最新补充复验[Vulkan 报告](../target/mobile-narrow-20261012-vulkan-final/report.json)中 390×844 通过、640×360 运行中变为 390×844 后截图超时；[DX12 报告](../target/mobile-narrow-20261012-canvas-final/report.json)中首例变为 1440×900 后超时。最新三尺寸全量未通过，不用历史结果替代；尺寸变化来源和默认 Vulkan 详细回放时序作为后续独立核查项，本轮按用户要求收尾、不再扩大诊断。
- 修改按独立块提交：零长路径验证 `37489c0`、统一触控 / 小屏布局 `c19426a`、初始窄窗与三重验收 `534a785`；最终文档另作一个提交。未推送、未重写历史；[二审建议](./demo-review-suggestions.md)保留七个未实施 corner 提案，基线规模不扩张。
- 本轮未制作 Android / iOS 安装包，未测物理触屏、移动真机或输入到显示的物理延迟；输入法专项按约定跳过。既有[Windows ZIP](../target/dist/dr-editor-win64-20261012-4ef8aa7.zip)保持适配前快照，不覆盖、不宣称包含本轮改动。

## 2026-10-13 依赖升级与原生按钮迁移

- quick-xml 已独立提交 `9e47977`：0.39.4 → 0.42.0，使用新版字符串 QName / 属性归一化接口；字符引用与属性回车、换行、制表符往返、根外文本拒绝及默认 128 层限制均有回归。该提交记录中的 354 项测试属于 XML / Bevy 0.19 阶段，不作本次 0.20 验收。
- Bevy 0.19.1 → 0.20.0，bevy_egui 0.42.0 → 0.43.1；显式启用 `bevy_ui_widgets` 与 `ui_picking`。所有原生按钮使用 `ui_widgets::Button` / `ActivateOnPress`，`Hovered` / `Pressed` 只负责状态反馈，`Activate` 桥接为 Update 中有序消费的消息。删除旧 `Interaction`、自定义 Button 标记和弃用别名，不保留老版本兼容层。
- 鼠标只允许主键驱动原生按钮；中键、右键在 picking 输入生成之后过滤，移动和滚动保留，按住及抬起不重复触发。触摸 picking 关闭，真实触点仍由 `GestureInput` 锁定起点、抬指一次提交；取消、双指接管与兼容鼠标尾随不补发旧激活。新增真实 WindowEvent → 官方 picking → widget 回归及文件、帮助、属性消费者回归。
- 窗口驱动改动已另作 `c37c036`：每次运行保留唯一进程名、字节一致的 EXE 副本与 SHA-256 身份记录；窄窗持续核对本次 PID / HWND / 标题，正常退出的窗口先销毁不误报所有权丢失，真实所有权变化仍立即失败。编辑器同时记录尺寸变化和截图请求状态，不把超时、崩溃或数据断言归为焦点干扰。
- 本阶段实际执行 `cargo test --workspace --release --locked`：366 项通过（core 104 / editor 262）；`cargo fmt --all --check`、严格全目标 Release Clippy 与 `cargo build --workspace --release --locked` 通过；全量 Python unittest 148 项通过。Rust 首轮默认临时目录拒绝访问，69 项通过 / 35 项失败；使用 `TEMP/TMP=target/test-temp` 后全量复验通过，未改测试断言。[首轮失败](../target/validation-bevy020-rust-20261013.log)、[复验](../target/validation-bevy020-rust-temp-20261013.log)、[Clippy](../target/validation-bevy020-clippy-20261013.log)、[构建](../target/validation-bevy020-build-20261013.log)、[Python](../target/validation-bevy020-python-20261013.log)。
- 本阶段串行运行 DX12 详细 40ms 回放，19 章 / 45 例 / 95 证据全部通过，退出码 0；[报告](../target/demo-bevy020-fast-20261013/demo-report.json)及[零长 / 连线证据](../target/demo-bevy020-fast-20261013/line-cases.json)保留。同一 Release 程序的 390×844、640×360、320×640 窄窗全部通过 Bevy / Win32 客户区 / PNG 三重核对，退出码均为 0，见[报告](../target/narrow-bevy020-dx12-20261013/report.json)。程序 SHA-256 为 `8be2ffc057979d5e870e5546034246413e0c1fe4675b6e9864d70959f1fd6366`；本阶段尚未运行 450ms 观看回放，不拿 0.19 的历史结果替代。
- 真实 XML 再验：`verify_catalog` 27 类部件 / 793 个元素与属性通过；`verify_ships` 185 个船体 / 125,856 个部件往返通过，仍拒绝 Alliance、Amect-MAIIa TEST、AmectVII、Ss、X Type 2 五个既有异常输入，实际退出码 1，不标为零错误通过。目录和 190 个船体文件共 191 份 XML 的 SHA-256 前后相同，见[目录日志](../target/validation-bevy020-catalog-20261013.log)、[船体日志](../target/validation-bevy020-ships-20261013.log)、[哈希记录](../target/validation-bevy020-xml-20261013.json)。
- 阶段边界：serde_json 此时仍锁定 1.0.151，1.0.154 将单独升级提交；用户追加的原生 / egui 风格统一另作功能块，最终构建仍需重新执行观看与窗口验收。本轮不新增平台打包工程、不推送、不重写历史；物理触屏、系统键鼠、输入法和系统文字输入未测。

## 2026-10-13 原生与 egui 轻量风格统一

- 按追加要求统一深色背景、按钮状态色、16px 按钮字号、6px 圆角和间距；保留窄屏 44px 触控目标及原输入 / 事务。配色和反馈复用 `native_buttons`，egui 配置沿用 `egui_ui`，不新增主题模块或工程。文件、帮助、属性入口共用文字构造和状态反馈，删除属性按钮未使用的选中参数及被覆盖的旧尺寸配置。
- 原生顶栏背景复用文件栏容器，egui 只透明保留该区域；生产与回归共用保留区函数，新增不透明遮盖回归。已实际查看[修复后的目录截图](../target/demo-theme-fast-fixed-20261013/editor-panels-smoke.png)及拓扑截图，原生文件 / 属性 / 帮助入口恢复，不用无头通过代替视觉检查。
- 正式主题暴露的窄窗问题分别修正：属性目标选择单独成行并截断长名称；连线窗口按可用高度初始化，回归使用真实滚动视口而非外框中心判断完整可见；船体虚拟行复用 egui 按钮的文字 / 边距 / 描边尺寸，演示复用 `ScrollAreaOutput` 实测最大偏移，不再估算 32px 行距。覆盖 12.5 / 16 / 22px 字号的千行末项可达回归。
- 自连接窗口断言失败已无头复现并定位到首次测量阶段过早发布拓扑命中矩形：只注册 `response.enabled()` 且非 sizing pass 的控件，保留真实点击、拒绝错误及历史断言。Painter 说明层重写的对照实验没有修复该问题，已撤回，不保留无证据的额外改动。
- 本阶段在 serde_json 1.0.151 下实际执行 371 项 workspace Release 测试（core 104 / editor 267）、格式检查、严格全目标 Release Clippy 及 Release build，均通过：[测试](../target/validation-theme-rust-complete-20261013.log)、[Clippy](../target/validation-theme-clippy-complete-20261013.log)、[构建](../target/validation-theme-build-complete-20261013.log)。五项新增回归属于真实执行，不是窗口验收替代品。
- 窗口失败如实保留：首轮 `demo-theme-fast-20261013` 原生顶栏被 egui 背景盖住，停在第 17 章，编辑器退出 0 但无完整报告、驱动退出 1；第二轮 `demo-theme-fast-fixed-20261013` 的极大滚动偏移方案仍未完成第 17 章，主动终止已核实路径的本次副本，编辑器退出 `4294967295`、驱动退出 1；第三轮 `demo-theme-measured-fast-20261013` 在第 15 章自连接断言失败，编辑器退出 101、驱动退出 1。全部未计为通过、未归类为焦点失败；开发中的 API 编译错误、布局失败和对照日志也保留在 `target/validation-theme-*`。最新修正后的完整窗口回放待最终 serde_json 集成构建重新执行。

## 2026-10-13 最终集成验收

- 三个依赖块独立提交：quick-xml 0.42.0 为 `9e47977`，Bevy 0.20.0 / bevy_egui 0.43.1 为 `3e3b8f2`，serde_json 1.0.154 为 `9ef6734`。风格统一单独提交 `d84524b`，窗口进程隔离为 `c37c036`，短点击复用与精简为 `980abe0`；未推送、未重写历史、未新增平台打包工程。
- 原生文件 / 属性 / 帮助入口和 egui 共用现有模块中的字体、配色、圆角与状态反馈，不新增主题框架。已实际查看最终[目录截图](../target/demo-closeout-fast-20261013/editor-panels-smoke.png)、[拓扑截图](../target/demo-click-packet-watch-20261013/editor-topology-smoke.png)及三个窄窗原始 PNG：顶栏正常显示，触控入口保留 44px 高度。窄窗默认相机没有自动适配整船，320×640 / 640×360 初始截图中船体被顶栏遮挡或裁切；需使用已有“适配视野”入口，不能把尺寸 / 入口验收写成初始整船取景通过。
- 正式 450ms 首轮在第 5 章 `lines.open` 断言失败，编辑器退出 101、驱动退出 1，见[失败日志](../target/demo-final-watch-20261013/demo-editor.log)。随后两轮临时诊断在目录点击处失败，同为 101 / 1；[带帧间隔诊断](../target/demo-watch-trace2-detailed-20261013/demo-editor.log)显示同一真实控件矩形保持稳定，但按下 / 抬起相隔约 1.02s，未判为点击。未将这些断言失败归为焦点干扰，也不把诊断构建算作最终通过程序。
- 复用 `UiTestInput::click_rect` 同帧交付完整短按事件对，属性 Action 点击也调用该入口；动画仍在抵达控件后交付。删除驱动 `release` 与演示 `click_release` 两套重复释放状态，保留等待、真实分帧长按 / 拖动、单次动作记录与触摸语义，不修改 egui 长按阈值、不伪造时间、不保留临时诊断代码。新增短点击契约测试[修正前失败](../target/validation-click-packet-before-20261013.log)，修正后覆盖 16ms / 1s / 10s 帧间隔、真实目录行、两档动画抵达和不重复点击；真实 1s 长按仍不得当作短点击。

最终实际验证使用同一 Release 程序，SHA-256 为 `608dc9d9688cf00a243f15597d22e90d21f49e44acba378d288059237384f594`；每份窗口身份记录均核对源 / 副本一致，见[集成摘要](../target/validation-closeout-summary-20261013.json)。下表不沿用前述阶段的通过记录：

| 检查 | 本次结果与证据 |
| --- | --- |
| Rust | `TEMP/TMP=target/test-temp` 下执行 `cargo test --workspace --release --locked`，373 项通过（core 104 / editor 269）；[日志](../target/validation-click-packet-rust-20261013.log) |
| Python | `uv run --no-project --python 3.12 python -B -X utf8 -m unittest discover -s scripts -p 'test_*.py' -v`，148 项通过；[日志](../target/validation-click-packet-python-20261013.log) |
| fmt / Release | `cargo fmt --all --check`、`cargo clippy --workspace --all-targets --release --locked -- -D warnings`、`cargo build --workspace --release --locked` 均通过；[Clippy](../target/validation-click-packet-clippy-20261013.log)、[构建](../target/validation-click-packet-build-20261013.log) |
| DX12 详细观看 450ms | 串行单窗口完成 19 章 / 45 例 / 95 证据，编辑器与驱动退出码均为 0；[报告](../target/demo-click-packet-watch-20261013/demo-report.json)、[驱动日志](../target/validation-click-packet-watch-20261013.log) |
| DX12 详细快速 40ms | 观看档结束后再启动，规模相同，编辑器与驱动退出码均为 0；[报告](../target/demo-closeout-fast-20261013/demo-report.json)、[驱动日志](../target/validation-closeout-fast-20261013.log) |
| DX12 窄窗 | 随后串行运行 390×844、640×360、320×640，三例均通过 Win32 客户区 / Bevy 尺寸与 scale factor 1 / 原始 PNG 三重核对，退出码均为 0；[报告](../target/narrow-closeout-dx12-20261013/report.json) |
| XML | `verify_catalog` 27 类 / 793 字段通过，退出 0；`verify_ships` 185 船体 / 125,856 部件通过，另 5 个既有异常输入仍拒绝，实际退出 1；191 份源 XML 前后 SHA-256 一致；[目录](../target/validation-closeout-catalog-20261013.log)、[船体](../target/validation-closeout-ships-20261013.log)、[哈希](../target/validation-closeout-xml-20261013.json) |

- 两档详细回放都保持 Test.xml / Heronb.xml 源哈希不变；64 级 / 1,024 动作的末项编辑、应用、撤销重做及 XML 往返均通过。两份 `line-cases.json` 的真实零长路径均为四点、长度 0、方向采样 0、提交顶点 0、安全跳过，流动 / 箭头开启且文档 / 历史不变。
- 观看回放前段仍观察到约 1s 的慢帧，后段恢复较快更新；用户协助保持窗口可见，未自动抢占前台。耗时包含观看节拍、滚轮与截图等待，不是性能或物理输入延迟测量；未确诊慢帧来源，也不宣称呈现策略已经解决所有后端时序问题。运行日志仍有 `DockingConnector.PNG` 的多 AssetLoader 警告，本轮未改资源加载管线。
- 本次窗口结果限定 DX12；默认 Vulkan 未复验。物理触屏、Android / iOS 真机、系统键鼠、原生路径选择、输入法与系统文字输入专项未执行；不以内部输入或窄窗截图替代。旧 Windows ZIP 仅保留适配前的历史记录，当前工作区未找到该归档，本次未重新打包、不宣称旧包包含改动。全历史证据链接首次检查[失败日志](../target/validation-closeout-doc-links-first-20261013.log)保留；缺失的是旧本地证据，不抹去历史执行记录或伪造旧产物。当前依赖升级目标已完成，项目整体复刻的下列缺口仍继续保留。

## 2026-10-13 原版 sandbox 风格与分组验收

- 本次实际从原版主菜单进入 sandbox → Smolar System → Builder → 部件目录，滚动观察四组并点击折叠。原版为中性灰黑底、青柠标题 / 图标、方正连续行和固定图片列；Gizmos / Fuel Tanks / Engines / Satellite 分别为 8 / 4 / 6 / 7 项，两个隐藏类型不显示。原版观察截图保留在本地 `target/sr1-sandbox-reference-20261013`，有效分组记录为 `10-parts-top.png`、`11-parts-middle.png`、`12-parts-fuel-collapsed.png`、`13-parts-bottom.png`、`14-satellite-tail.png`；一次旧窗口几何截图无效，不作证据。桌面不可见时两次恢复未成功，已通知 `sr1` 并等待，用户恢复后继续；观察结束正常关闭本任务启动的原版游戏，不和编辑器窗口并行。
- 原生入口与 egui 继续共享 `native_buttons` / `egui_ui`，改为灰黑控件、青柠状态描边和 2px 圆角，按钮字号仍为 16px；部件行改为 64px，图片列 64px、等比缩略图 48px，名称左对齐、长名称截断。复用原有反馈、图片加载、拖放、滚动与原生 `ui_widgets` / picking / `Activate`，未引入主题框架或新资产。保留当前编辑器布局，不为复刻原版底栏另加一套界面。
- 移除旧“常规 / Satellite”独占筛选、旧分类消息及 `UiState` 分类镜像字段；`Palette` 唯一维护收起的组，复用 egui `CollapsingHeader` 标题与裁剪动画，关闭动画中的旧行不可交互。四组按原版顺序显示，可独立收起并全部展开；目录与 Tab / Shift+Tab 使用同一可见集合，组内顺序与原可见索引不变。显式 XML `category` 优先（包括 Satellite 的油罐），未知自定义组保留原名并追加；默认分类只在视图推断，不写回目录或船体。实际目录成员核对见[分组记录](../target/validation-sandbox-categories-20261013.json)。
- 新增六项回归：默认分类 / 显式元数据优先、分组顺序与隐藏项、独立折叠与历史不变、主题视觉契约、自定义长组名限宽 / 44px 高度、真实标题点击与全部展开共享 `Palette`。已有图片比例、鼠标按下、触摸抬指一次及长按不重复等断言保留；不增加详细演示 corner 或独立自动化框架。
- 首轮 Rust 为 core 104 通过、editor 262 通过 / 7 失败，原因是测试仍写死旧配色与 40px 图 / 52px 行规格；更新为共享角色色及新尺寸，行为断言不变，[失败日志](../target/validation-sandbox-first-rust-20261013.log)保留。新增测试首次比较 `PartCatalog` 因没有 `PartialEq` 编译失败，改为比较已有 `types`，不为测试改核心 API，[编译日志](../target/validation-sandbox-tests-compile-20261013.log)保留；随后一项标题限宽测试错误假设 `run_ui` 有 16px 根边距，274 / 1 的[失败日志](../target/validation-sandbox-rust-final-20261013.log)保留，改正测试视口宽度而不放宽边界断言。严格 Clippy 首轮报按钮 Query 类型过长，提取本模块局部类型别名，不增加 suppress，[日志](../target/validation-sandbox-clippy-first-failed-20261013.log)保留。
- 首次详细快速回放在滚轮验证之后等待“当前可见的分组标题”，标题已经滚出视口，夹具停滞。仅终止已核对路径的本次 EXE 副本，编辑器退出 `4294967295`、驱动退出 1，[终止原因](../target/demo-sandbox-fast-20261013/agent-abort.json)与[日志](../target/validation-sandbox-fast-20261013.log)保留，不记为焦点失败。修正为指定目录末组并复用既有 `click_panel` 滚动寻址，进入视口后真实点击；未延长超时、放宽文档 / 历史断言或新增驱动状态。

以下为修正后的最新 Release 程序实际结果，SHA-256 为 `b8b8497d2bb61c80160161e40cf97ed283de66bc3870c084f62bca8387e0af35`；各窗口先核对源 / 唯一副本一致，所有窗口串行运行，不拿上节旧程序的两档记录代替。本次[汇总](../target/validation-sandbox-summary-20261013.json)独立核对报告、逐例状态及非空产物：

| 检查 | 本次结果与证据 |
| --- | --- |
| Rust | `TEMP/TMP=target/test-temp` 下执行 `cargo test --workspace --release --locked`，379 项通过（core 104 / editor 275）；[日志](../target/validation-sandbox-rust-complete-20261013.log) |
| Python | `uv run --no-project --python 3.12 python -B -X utf8 -m unittest discover -s scripts -p 'test_*.py' -v`，148 项通过；[日志](../target/validation-sandbox-python-20261013.log) |
| fmt / Release | `cargo fmt --all --check`、`cargo clippy --workspace --all-targets --release --locked -- -D warnings`、`cargo build --workspace --release --locked` 均通过；[fmt](../target/validation-sandbox-fmt-20261013.log)、[Clippy](../target/validation-sandbox-clippy-passed-20261013.log)、[build](../target/validation-sandbox-build-20261013.log) |
| DX12 详细快速 40ms | 19 章 / 45 例 / 95 个唯一非空证据，编辑器与驱动退出码均为 0，Test.xml / Heronb.xml 源哈希不变；[报告](../target/demo-sandbox-fast-final-20261013/demo-report.json)、[日志](../target/validation-sandbox-fast-final-20261013.log) |
| DX12 窄窗 | 随后串行运行 390×844、640×360、320×640，三例均通过 Win32 客户区 / Bevy 窗口尺寸及 scale factor 1 / 原始 PNG 三重核对，退出码均为 0；[报告](../target/narrow-sandbox-dx12-20261013/report.json) |
| XML | `verify_catalog` 27 类 / 793 字段往返通过，退出 0；`verify_ships` 185 船体 / 125,856 部件往返通过，另 5 个既有异常输入仍拒绝，实际退出 1；191 份源 XML 前后哈希不变；[目录](../target/validation-sandbox-catalog-20261013.log)、[船体](../target/validation-sandbox-ships-20261013.log)、[哈希](../target/validation-sandbox-xml-20261013.json) |

- 已实际查看当前[目录](../target/demo-sandbox-fast-final-20261013/editor-panels-smoke.png)、[连接编辑窗口](../target/demo-sandbox-fast-final-20261013/editor-topology-smoke.png)、三个窄窗原始 PNG；又串行用同一程序运行 1440×900 普通 `--smoke-test`，退出 0，并查看[无演示遮罩预览](../target/sandbox-preview-20261013/target/editor-smoke.png)。原生顶栏与 egui 共用同一灰黑 / 青柠风格，顶栏入口和窄屏触控尺寸保留。320×640 / 640×360 初始船体仍被顶部 UI 裁切，需既有“适配视野”；本次没有宣称默认相机自动取景通过，也未扩大范围改变相机。
- 本次只复跑 DX12 40ms，**450ms 观看档未复跑**，上节通过记录属于风格改动前程序；默认 Vulkan、物理触屏、移动真机、系统键鼠、原生路径选择、输入法与系统文字输入专项未测。没有修改依赖版本、原版游戏文件或 XML 数据格式，不新增平台打包工程、不重新打包、不推送、不重写历史；本次风格 / 分类完成不代表全项目复刻缺口消除。


## 2026-10-12 移动适配专项记录（Bevy 0.19）

| 检查 | 当前记录 |
| --- | --- |
| 只读审查 | 已完成：TouchInput / 鼠标仲裁、窄屏 UI、平台构建边界；V41f high 子代理参与独立审查与最小 UI 实现 |
| 差异检查 | `git diff --check` 通过，只有 Windows LF / CRLF 转换提示 |
| Rust / fmt / Clippy / build | 最终 351 项 Rust 测试通过（core 101 / editor 250）；格式检查、严格全目标 Release Clippy 和 Release build 通过。日志：[测试](../target/validation-mobile-workspace-canvas-corrected.log)、[Clippy](../target/validation-mobile-clippy-canvas-corrected.log)、[构建](../target/validation-mobile-build-canvas-corrected.log) |
| Python | 收尾重新执行全套 127 项并通过，含演示驱动 43 项、窄窗工具 7 项契约回归；[运行日志](../target/validation-mobile-python-closeout.log) |
| 详细演示 | 最新构建在进程级 `WGPU_BACKEND=dx12` 下，快速 40ms 与观看 450ms 两档均完整通过：19 章、45 例、95 个证据；[快速报告](../target/demo-mobile-fast-20261012-canvas-final/demo-report.json)、[观看报告](../target/demo-mobile-watch-20261012-canvas-final/demo-report.json)。原版样本哈希未改变 |
| 真实窄窗：较早记录 | 390×844、640×360、320×640 三种原始截图曾通过 Win32 客户区、Bevy 窗口尺寸与 PNG 像素三重核对；[已通过报告](../target/mobile-narrow-20261012-owned-window/report.json)。这是较早构建的记录，和最新复验分开使用 |
| 真实窄窗：最新复验 | [Vulkan 报告](../target/mobile-narrow-20261012-vulkan-final/report.json)：390×844 通过，640×360 运行中变成 390×844 后截图超时，320×640 未运行；[DX12 报告](../target/mobile-narrow-20261012-canvas-final/report.json)：390×844 运行中变成 1440×900 后超时，后两例未运行。最新三尺寸全量复验未通过，尺寸变化来源保留为后续核查项 |
| 物理触屏与 Android / iOS 真机 | 本轮未执行，不以内部消息或桌面窄窗回归替代真机结果 |

### 当时的复现入口与未完成核查

[窄窗验收工具](../scripts/validate_narrow_windows.py#L101-L131) 从初始 Bevy 窗口尺寸启动，仅只读测量其自建进程的 Win32 客户区，并校验原始 PNG；不改变系统窗口尺寸，不注入鼠标或触摸。输出目录要求全新，失败报告保留，不用历史截图替换。

```powershell
# 最新详细回放的已通过条件；只影响本次进程，不更改普通编辑器默认后端
$env:WGPU_BACKEND = 'dx12'
uv run --no-project --python 3.12 python -B -X utf8 scripts/demo_editor.py --editor-bin target/release/dr-editor.exe --mode detailed --fast --output target/demo-closeout-fast-new
uv run --no-project --python 3.12 python -B -X utf8 scripts/demo_editor.py --editor-bin target/release/dr-editor.exe --mode detailed --step-ms 450 --exit-on-complete --output target/demo-closeout-watch-new

# 窄窗对照；此命令为复现入口，最新三例并未全部通过
$env:WGPU_BACKEND = 'vulkan'
uv run --no-project --python 3.12 python -B -X utf8 scripts/validate_narrow_windows.py --editor target/release/dr-editor.exe --assets ../Difficult-Rocket/assets --output target/mobile-narrow-closeout-new
```

普通编辑保留默认更新策略与 `AutoVsync`；自动演示及启动冒烟使用持续更新与 `AutoNoVsync`。自动策略本身不代表交换链最终采用 Immediate，也不保证解决后端时序问题。此前默认 Vulkan 的详细回放曾出现约 1 秒按下区间，超过 egui 默认 0.8 秒短点击规则；同一真实行的 45ms / 1s 对照回归确认短点击与长按语义正确，因此没有调大点击时限或放宽放置断言。最新两档详细 DX12 回放通过，但默认 Vulkan 的回放间隔来源仍未定位；最新窄窗尺寸变化在两个后端均有记录，不归结为已确诊的单一图形后端问题。

本轮按用户要求收尾，不再扩大诊断。后续窗口稳定性专项需定位尺寸变化来源，再用同一构建重新跑满三个尺寸；已有消息回归、布局回归与较早截图仍按各自验证层次使用。

## 2026-10-13 结构与文档清理

本次清理不新增功能或平台工程、不重写历史、不推送；代码独立提交 `41caff4`。README 从 339 行精简到 51 行，仅保留定位、启动、功能和导航；详细操作/验证各有单一入口，历史状态冻结，不再在首页重复累加测试计数。清除旧 Bevy 0.19、“最小 960×640”、旧独占分类和旧窄窗失败冒充当前状态等矛盾描述。

### 代码与测试整理

- main.rs 从 1,274 行缩为 231 行，只保留资源/插件/系统接线；不改调度顺序。
- 文档/保存点归 document，窗口配置与截图归 window，画布输入归 canvas_input；拖拽、游标、相机与 HUD 归现有 selection、placement、view、render，不增加另一套管理层。
- 共享文档/目录/鼠标夹具归仅测试编译的 test_support；目录直接用内存 catalog_from_xml，不反复创建临时目录文件；删除旧 movement_for 测试包装。
- 根测试归各自模块；目录 UI 大文件按行、布局、手势、分组分开。视频字幕同步四组折叠，不再描述旧独占筛选。
- 删除 7 个测试入口：4 个 egui 依赖自身/旧分帧短点击诊断、1 个缺同步指针的纯 Touch 依赖行为、1 个静态帮助文案检查、1 个重复插值夹具。插值首/末帧、延迟按下及完成状态并回真实调度测试，默认帮助关闭并回实际打开/隔离测试；不减少现行输入、XML 与原子事务的独立断言。

### 本次实际验收

| 检查 | 本次实际结果 / 证据 |
| --- | --- |
| Rust | `cargo test --workspace --release --locked`：372 项（core 104 / editor 268）通过；[日志](../target/cleanup-20261013/rust-final.log) |
| Python | `uv run --no-project --python 3.12 python -B -X utf8 -m unittest discover -s scripts -p 'test_*.py' -v`：148 项通过；[日志](../target/cleanup-20261013/python-passed.log) |
| 格式 / Release | fmt check、全目标 Release Clippy `-D warnings`、workspace Release build 均通过；[fmt](../target/cleanup-20261013/fmt.log)、[Clippy](../target/cleanup-20261013/clippy.log)、[build](../target/cleanup-20261013/build.log) |
| 详细窗口演示 | 同一 Release 程序 DX12 40ms，19 章/45 例/95 个唯一非空证据通过，退出 0；[报告](../target/demo-cleanup-fast-20261013/demo-report.json)、[驱动日志](../target/cleanup-20261013/demo.log) |
| 串行窄窗 | 390×844、640×360、320×640 三重尺寸校验、退出 0；原始 PNG 均已查看；[报告](../target/narrow-cleanup-dx12-20261013/report.json) |
| XML | 27 类/793 字段目录往返退出 0；185 船体/125,856 部件往返、5 个既有异常明确拒绝，verify_ships 如实退出 1；[目录](../target/cleanup-20261013/catalog.log)、[船体](../target/cleanup-20261013/ships.log) |
| 不变性 / 整理审计 | 191 份源 XML 哈希未变，4 份窗口副本与源程序 SHA 一致；[汇总](../target/cleanup-20261013/summary.json)、[迁移正文/调度核对](../target/cleanup-20261013/migration-check.json)、[测试删除核对](../target/cleanup-20261013/test-audit.json) |

本次程序 SHA-256：`b169b9e94806b86ab70aaa83a46cb8ef833cce0e3f8aadc223621e7e384805d8`。本次未运行 450ms 观看档、Vulkan、物理触屏/移动真机、系统键鼠/文字/输入法/原生路径专项，也未录制视频、重新打包。此前 379/373 项和观看结果均属于其他构建，不代替本次。

### 失败与验证边界

模块迁移首次 fmt 因自动提取误加可见性到 anyhow 宏报错；首次 Rust 编译暴露原入口隐式 imports（58 项缺失），第二次编译暴露 HUD 系统签名的私有类型。均以显式所有者导入及适当模块内可见性修正，未压制警告、未放宽断言。[首次编译](../target/cleanup-20261013/refactor-compile-first.log)、[第二次编译](../target/cleanup-20261013/refactor-compile-second.log)、[迁移后回归](../target/cleanup-20261013/rust-first.log)、[测试归位后回归](../target/cleanup-20261013/rust-final.log)。

Python 两轮首次回归均为 147 通过/1 失败：视频字幕已同步组折叠，测试仍要求旧“分类筛选/恢复全部”文字。同步两条断言至现行“独立收起/全部展开”，其余真实三键与显示说明不变，全套 148 项复跑通过。[首轮](../target/cleanup-20261013/python.log)、[第二轮](../target/cleanup-20261013/python-final.log)。文档活动链接及章节已核对；历史 30 个缺失 target 链接明确保留，不伪造日志。记录勘误：`7de95b8` 提交正文将链接数误写为 229，实际报告为 215；这里只更正记录，不重写历史提交。

## 2026-10-13 分级界面中间验证与失败

以下均为中间构建，不能代替最终悬停联动版本的验收。

- 初次停靠回归暴露 640×360 下 ScrollArea 默认最小高度将面板撑出窗口；设 `min_scrolled_height(0)` 并压缩短窗辅助标题，保留真实几何断言。[越界复现](../target/staging-ui-20261013/dock-bounds-failed.log)。首次 Clippy 的参数类型复杂度及无用更新语法也直接修正，不加警告抑制。[首次 Clippy](../target/staging-ui-20261013/clippy-first.log)。
- 补图标时首次编译漏改一个 show 调用；首轮新布局测试把空 Button 的空文字 Shape 当实际标签，修正为核对非空文字，真实列宽/比例/单行断言保留。[编译](../target/staging-ui-20261013/rust-icons-first.log)、[测试](../target/staging-ui-20261013/rust-icons-second.log)。
- 整套 Rust 首次为 core 69 通过/35 失败，均因系统 TEMP 拒绝访问；仅为本次进程设置工作区 TEMP/TMP 后全套复跑，不放宽数据断言。[失败](../target/staging-ui-20261013/rust-icons-final.log)、[复跑](../target/staging-ui-20261013/rust-icons-passed.log)。
- 停靠版及图标版分别完成 DX12 40ms 详细 19 章/45 例/95 证据，退出 0，但不是最后构建。[停靠版](../target/demo-staging-docked-fast-20261013/demo-report.json)、[图标版](../target/demo-staging-icons-fast-20261013/demo-report.json)。人工查看图标版截图发现底部演示横幅遮住船底目标；随后在分级停靠期间隐藏横幅，不把“已绘制/矩形在画布内”冒充无遮挡。
- 第一次真实窄窗分级：390×844、640×360 通过，320×640 到 180s 仍未完成，应用明确退出 101；不是焦点失败，未冒充通过或自动重复置前。[完整失败报告](../target/narrow-staging-dx12-20261013/report.json)、[320×640 日志](../target/narrow-staging-dx12-20261013/320x640/editor.log)。随后滚动驱动改取实际正文中心，短横屏压缩标题；最终复跑结果见当前进度。
- 新窗口策略无头测试最初在 crate 目录生成窗口 JSON，已改为不创建窗口、不写证据；真实尺寸留给串行窗口专项，移除本次测试生成的无用文件。
