# SR1 PC 船体、树组与分级读取证据

## 范围与证据等级

实现入口为 [ship.rs](../../crates/dr-core/src/sr1/ship.rs) 与 [staging.rs](../../crates/dr-core/src/sr1/staging.rs)。本层不调用或修改既有严格 [io.rs](../../crates/dr-core/src/io.rs)、[model.rs](../../crates/dr-core/src/model.rs) 与编辑器默认合同。

本轮直接只读反汇编 [SimpleRockets.exe](D:/githubs/dr/SimpleRockets-PC-1.6.11/SimpleRockets.exe)，没有启动、注入或修改游戏。下列 `Ship::Load` 等是便于说明的逻辑名称，不是伪造的源码文件或行号。未用 unpack 的 C++/CLI wrapper 作为原生逻辑证据；本页结论不依赖 Android 符号。

- **A：PC 指令证据**。ImageBase `0x00400000`；本文地址均为该映像的绝对 VA，另列文件偏移。
- **B：随游戏提供的静态资源**。用于确认真实字段结构，并构造不依赖游戏安装路径的内联缩小 fixture；不等价于运行游戏验证。
- **S：dr 明确提供的可诊断边界**。在 PC 会空指针解引用或无限递归的输入上返回错误，或用可检查的空树表示；不宣称本体也会接受这些输入。

样本 SHA-256：`7ADF2D73D6F95F79333154A98CCA21AF734917981E0CCF94171E3846547DD0E9`。

只读工具：本机已有 `pefile`、`capstone`，通过 `uv run --no-project --no-cache --python 3.12 python -B` 读取 PE。临时反汇编输出位于工作树忽略目录 `target/sr1-research/sr1-ship-staging/`，不是构建依赖，也不进入提交。

## PC 地址与已实现语义

| 项目 | PC VA | 文件偏移 | 直接观察与实现 | 等级 |
| --- | --- | --- | --- | --- |
| Ship 读取 | `0x00454680` | `0x00053A80` | `version` 局部初值为 0；依次加载主 PartTree；`liftedOff`、`touchingGround` 的布尔初值均 false | A |
| Ship 写出 | `0x004547D0` | `0x00053BD0` | 新建 `Ship`，版本常量 1，两个布尔写成 `%d` 的 0/1，再输出主树 | A |
| PartTree 读取 | `0x0046FD20` | `0x0006F120` | 取 `Parts`、`Connections`，遍历其全部元素子节点；不是仅筛选 `Part` 或 `Connection` 标签 | A |
| parent/child 解析 | `0x00470040` | `0x0006F440` | 按部件输入次序正向扫描，匹配后继续扫描，所以取最后一个同 ID 实例；parent 分支优先于 child 分支 | A |
| 普通连接点解析 | `0x00470181` | `0x0006F581` | 双方连接点按 ID 查询，两个结果都非空才创建普通连接；缺失或解析失败的 ID 默认 0 | A |
| 无效连接点 | `0x00470487` | `0x0006F887` | 找不到连接点时打印 `TODO: REMOVE` 后跳过创建；dr 同样跳过该连接，不把两部分自动合并 | A |
| 对接引用 | `0x00470557` | `0x0006F957` | `dockPart` 查询在首个同 ID 部件处停止；与 parent/child 的末项规则不同 | A |
| 选择实际根 | `0x00470890` | `0x0006FC90` | 取 Parts 的第一个实例；若有父连接，沿父端祖先链寻找根 | A |
| 父链向上查找 | `0x00471E60` | `0x00071260` | 重复读取当前部件父连接的 parent Part，直到父连接为空 | A |
| 旧根分级迁移 | `0x004708B8` | `0x0006FCB8` | 仅外层版本恰等于 0 执行；读取当前容器的 `currentStage` 和 `Staging`，仅尝试挂到实际根自身的 Pod；不扫描任意首个 Pod | A |
| 不覆盖自身分级 | `0x0047C400` | `0x0007B800` | Pod 已有挂起的自身 Staging 节点时不接受旧根替换；没有时同时设置节点和索引初值 | A |
| Pod 二阶段分级加载 | `0x0047C430` | `0x0007B830` | 从 Pod 所属的 Part 开始枚举其子树，把该列表传入 Staging 加载；随后从 Staging 元素读取 `currentStage` | A |
| 全部部件次序 | `0x00471890` | `0x00070C90` | 先加入当前 Part，再对子节点按连接存储次序递归，得到前序遍历 | A |
| 子节点枚举 | `0x0046EA40` | `0x0006DE40` | 当前部件的连接列表中仅取 parent 为自身的 child；不走无向图搜索，不把孤立输入项纳入根树 | A |
| 树写出 | `0x00470D20` | `0x00070120` | Parts 按根前序写出，再写 Connections；根不可触达的输入项不会被写出 | A |
| 连接写出 | `0x004709C0` | `0x0006FDC0` | 普通连接写 parentAttachPoint/childAttachPoint；对接写 DockConnection/dockPart；两者均写 parentPart/childPart；每条边之后立即递归其子节点 | A |
| 默认活动 Pod | `0x00454381`、`0x004543AB` | `0x00053781`、`0x000537AB` | 根对象若为 Pod 优先采用；否则按根前序找到首个带 Pod 标记的部件后转换；不扫描编辑器分离组 | A |
| Staging 读取 | `0x004828D0` | `0x00081CD0` | 查找首个 Step、每步首个 Activate；随后两层均使用无名称过滤的元素兄弟遍历；Activate 的属性仅读取大写 `Id`，默认 0 | A |
| 分级过滤与 moved | `0x004829DA` | `0x00081DDA` | 仅当传入部件列表内存在该 ID 才加入动作，取首个匹配实例；内部 moved 字节直接赋 1，没有读取输入 `moved` | A |
| 空 Step | `0x00482AFA` | `0x00081EFA` | 无论本步骤加入几个动作，都会把步骤加入内部数组，所以全被过滤的步骤仍保留 | A |
| Staging 写出 | `0x00482B80` | `0x00081F80` | 按步骤和动作次序输出 Staging/Step/Activate，空步骤正常输出 | A |
| moved 写出 | `0x00482DFD` | `0x000821FD` | 从内部动作字节取 moved，写 0/1；不是硬编码始终写 1 | A |
| 分级索引写出 | `0x0047C5C8` | `0x0007B9C8` | 调用 Staging 保存取得元素后，把 Pod 的内部 currentStage 写在返回的 Staging 元素上；索引不在 Pod 元素上 | A |
| 分离组读取 | `0x0044EBBB` | `0x0004DFBB` | ShipEditor 主树加载完毕后读取 DisconnectedParts，按名称过滤 DisconnectedPart，逐组调用同一 PartTree 读取器并传入 Ship 版本 | A |
| 分离组写出 | `0x0044E37E` | `0x0004D77E` | 创建 DisconnectedParts 并插到 Ship 子节点首位；内部各组以原次序追加 DisconnectedPart，再独立保存该树 | A |

### 关键分支补充

1. 普通连接的标签判断是与字符串 `Connection` 做精确比较。不是该名称的元素进入对接分支；dr 也使用该分支规则，保存时统一输出 `DockConnection`。
2. 连接点是否存在通过共享 [catalog.rs](../../crates/dr-core/src/sr1/catalog.rs) 的 `PartType::has_attach_point` 判断。目录的连接点 ID 是从 1 开始按资源次序生成；不能把 XML 中人为添加的 `@id` 当成目录连接点身份。
3. 最后一个成功创建的入向连接成为该 child 的父连接指针，之前的连接仍在双方连接数组中。实现只在**父链选根**时采用最后入向连接，遍历时仍保留所有有效出向连接及重复路径。
4. 旧根迁移和默认活动 Pod 选择是两个不同过程。前者只允许实际根自身是 Pod，后者可回退到实际根子树的首个 Pod；不能混用“首 Pod”规则。
5. `currentStage` 没有范围裁剪。旧格式先用 Ship/DisconnectedPart 的同名属性作为初值；有自身 Staging 或旧根 Staging 中的有效同名属性时，以 Staging 自己的值优先。解析失败保留初值，数值转换复用共享 [scalars.rs](../../crates/dr-core/src/sr1/scalars.rs)。

## 资源依据与缩小样例

- [DefaultShip.xml](D:/githubs/dr/SimpleRockets-PC-1.6.11/DefaultShip.xml#L1-L18)：根无 version，存在根 currentStage/throttle/liftedOff；Parts 为 ID `1/9/10`，Connections 为 `1→9→10`，旧根 Staging 激活 `Id=10`。测试内联保留其全部数据属性，仅缩小空白。
- [EmptyShip.xml](D:/githubs/dr/SimpleRockets-PC-1.6.11/EmptyShip.xml#L1-L7)：同样缺 version，有一个 Pod、空 Connections 和空根 Staging。注意该资源**并不是零部件 Ship**。
- [PartList.xml](D:/githubs/dr/SimpleRockets-PC-1.6.11/PartList.xml)：fixture 的部件工厂与连接点次序来源。测试目录仅保留 Pod、Tank、Engine 所需字段，不冒充整份目录的无损复制。

真实样例测试 `default_ship_resource_migrates_and_saves_pod_staging_canonically` 确认：版本默认 0，前序 ID 为 `1/9/10`，旧根 Staging 迁入根 Pod，保存 version 1，根不再输出旧 Staging，Pod 内输出 `Staging/Step/Activate Id=10 moved=1`。生成的规范元素再次加载后规范保存保持一致。

## 公共 API 与模块边界

- `Ship::from_element(&Element, &Catalog)` / `from_xml(&str, &Catalog)`、`to_element()` / `to_xml()`：对应 Runtime 使用的 Ship 本体，只处理主树。显式编辑器入口之外，不隐式加载或保存 DisconnectedParts。
- `Ship::from_editor_element` / `from_editor_xml`、`to_editor_element` / `to_editor_xml`：在同一 Ship 模型上独立处理分离树组；保存时连空 DisconnectedParts 容器也放到首位。
- `PartTree::parts()` 保留输入项次序；`root()`、`root_index()`、`traversal()` 表示实际根及 PC 访问次序；`unreachable_parts()` 暴露孤立项，不伪造分离组。
- `Connection::parent/child` 与 `ConnectionKind::Dock::dock_part` 是该树局部的 `usize` 实例索引。写出时再取相应实例 ID，避免重复 ID 误绑定。主树和不同分离组之间不共享解析空间。
- `Ship::selected_pod_id()` 仅表示构造后的实际默认选择；`select_pod(id)` 是显式控制方法，不自动应用 Runtime 的请求字段；Runtime 的绑定行为由 [runtime.rs](../../crates/dr-core/src/sr1/runtime.rs) 单独处理。
- `PodState::staging: Option<Staging>` 由共享 [parts.rs](../../crates/dr-core/src/sr1/parts.rs) 持有；Staging/StageStep/StagedPart 的唯一 schema 属于本模块。`None` 表示输入没有自身 Staging，给旧根迁移保留判断信息；部件保存器仍可按 PC 无条件输出默认空 Staging。
- `Staging::from_element` 是第一阶段读取，先保存所有 ID，并把 moved 置 true；`PartTree` 完成连接后按每个 Pod 的**自身子树**调用 `retain_part_ids`。空 Step、重复动作、负 currentStage 都保留。

## 明确的实现边界

### 已实现且由 PC 支持

- 三项 Ship 默认值及固定版本写出；布尔与数值前缀读取复用共享转换。
- 按树局部 ID 查找连接、普通/对接差异、连接点过滤、重复 ID 的具体查找次序、父链选根和前序保存。
- 旧根分级的版本门槛、根自身 Pod 门槛、自身 Staging 优先和索引回退。
- Pod 自身子树内分级过滤、首个匹配 ID 规则、moved 重新初始化和真实内部状态写出、空 Step 保留。
- 编辑器分离组保持组边界、组顺序和局部 ID 空间，并在规范输出的首个子节点放置容器。

### 安全与表示边界（S）

- 缺少 Parts/Connections、缺少 parent/child/dock 引用，以及实际父链或被访问子树中的环，返回带具体上下文的 `Error::InvalidData`；不复现 PC 的空指针解引用、挂起或无限递归。
- 零部件且没有连接的树可表示为空树，`root()` 和 `selected_pod_id()` 返回 None。PC 后续 Ship 构造器会依赖非空根，本页不宣称其可安全运行；dr 保留可检查结果用于离线分析。
- 所有连接从构造后即为实例索引，拓扑不提供修改入口；`parts_mut()` 供状态修改。StagedPart 的公开引用为加载快照内的 i32 ID，不实现拓扑编辑或 ID 重编号后的分级指针自动迁移。
- 已读入但根不可触达的项仍可检查；规范保存按照 PC 访问集省略它们。这不是保存输入 XML 的逐字无损入口。未知字段诊断信息由部件层持有，不复制到本体规范保存中。

### 本轮未验证或未实现

- 对接引用目标为错误运行类时的 C++ 对象布局后果、游戏物理约束计算、附着点占用后的动态连接行为。读取器只验证引用存在，未伪造对接物理算法。
- Runtime 的后续选船、切换 Pod 和分级激活动作执行；本任务只提供读取后的数据语义及明确选择入口。
- 游戏窗口运行、渲染、编辑器 UI、真实数据库/网络/上游集成均不在本轮执行范围。
- 未通过运行 PC 本体做差分测试；A 级表示静态机器指令链路已核对，不表示本体动态对照已经完成。

## 离线测试与验证记录

新增测试位于上述两个实现文件的 `#[cfg(test)]` 模块，无需读取游戏安装目录、访问网络或数据库。覆盖：

- Ship 默认、固定保存版本、布尔/数值前缀、非 Part 名称的元素子项读取。
- 版本 0 与非 0 迁移差异、已有自身 Staging 不覆盖、实际根与首 Pod 区分、每 Pod 子树过滤。
- 连接点失效跳过、孤立项诊断、重复 ID 末项端点与首项 dockPart、普通/对接字段写出。
- 父链、深度优先 Parts/Connections 保存、多路径不去重、坏引用及环的错误结果。
- 分离组次序、同 ID 跨组不合并、拒绝跨组连接、分离组继承 Ship 版本、空容器首位。
- 真实 DefaultShip 缩小 fixture 的迁移和规范写出再加载。
- 分级 Id 大小写、缺失/解析失败默认 0、空步骤、重复动作、无名称后续兄弟、moved 读写不对称、currentStage 初值优先级。

本队友实际执行：

```text
rustfmt --edition 2024 --config skip_children=true crates/dr-core/src/sr1/ship.rs crates/dr-core/src/sr1/staging.rs
git diff --check -- crates/dr-core/src/sr1/ship.rs crates/dr-core/src/sr1/staging.rs
```

结果：上述格式化及空白检查通过；另外 `rustfmt --check` 通过，证据文档的 12 个文件链接全部确认存在。未执行 Cargo，以避免与 Lead 争用构建锁。新增 Rust 测试的执行结果由 Lead 统一验证后记录；本页不把“已编写”当成“已运行通过”。建议 Lead 顺序执行：

```text
cargo test -p dr-core sr1::staging::tests
cargo test -p dr-core sr1::ship::tests
```
