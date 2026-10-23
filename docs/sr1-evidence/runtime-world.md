# SR1 PC 运行态、星系与挑战数据证据

## 目标与实现入口

本轮新增独立本体层的 [runtime.rs](../../crates/dr-core/src/sr1/runtime.rs) 与 [solar.rs](../../crates/dr-core/src/sr1/solar.rs)，不修改现有严格入口 [io.rs](../../crates/dr-core/src/io.rs)、[model.rs](../../crates/dr-core/src/model.rs) 的默认合同，不接 UI，不实现物理引擎。

- Runtime：根属性默认、节点读取、星体名称绑定、有效轨道初值覆盖、显式保存选择、带实际星体半径的快照序列化。
- SolarSystem：真实资源树、轨道/大气/地形/范围/层读取，PC 的 f64→f32、角度端点排序、颜色解析及静态质量计算。
- ChallengeState：由调用方显式指定挑战类，区分大小写不同的字段；实现本次核实的五组保存数据。
- 嵌套 Ship 使用 [ship.rs](../../crates/dr-core/src/sr1/ship.rs) 的 Runtime 入口，不使用 Editor 专有的分离组入口；目录与数值分别复用 [catalog.rs](../../crates/dr-core/src/sr1/catalog.rs)、[scalars.rs](../../crates/dr-core/src/sr1/scalars.rs)，XML 复用 [xml.rs](../../crates/dr-core/src/sr1/xml.rs)。

## 证据来源和等级

**A：PC 原生指令直接证实。R：真实资源直接证实。B：基于 A/R 的本层显式建模或错误边界。U：未验证，未补猜测算法。** A 不等于已经运行游戏验证；本轮没有运行、修改游戏，也没有读私密配置、连接数据库或上游服务。

主要二进制：[PC 原生可执行文件](D:/githubs/dr/SimpleRockets-PC-1.6.11/SimpleRockets.exe)，不是 unpack 的 C++/CLI 空 wrapper。只读读取 PE，ImageBase 为 0x400000；SHA-256：

7adf2d73d6f95f79333154a98cca21af734917981e0ccf94171e3846547dd0e9

本表的“文件偏移”由 PE 节映射得到，不把 VA 当文件偏移；函数职责名称是分析标识，不是伪造的源码位置。反汇编通过本机已有 pefile/capstone，使用以下无项目、无缓存的 Python 入口；临时分析仅写入工作区的忽略目录。

~~~powershell
uv run --no-project --no-cache --python 3.12 python -B target/sr1-research/sr1-runtime-world/pc_readonly.py d 451110 451838
uv run --no-project --no-cache --python 3.12 python -B target/sr1-research/sr1-runtime-world/pc_readonly.py d 484650 4852d0
~~~

## Runtime 读取与处理

| PC VA | 文件偏移 | 直接观察与实现 | 等级 |
| --- | --- | --- | --- |
| 0x451110、0x45118A | 0x050510、0x05058A | 初始化先读 solarSystem；缺失/空字符串时用调用方传入的默认资源名 | A |
| 0x4512EF..0x45131A | 0x0506EF..0x05071A | 布尔临时值先清零，再读 firstStageActivated；缺失/失败保持 false | A |
| 0x451330..0x45136B | 0x050730..0x05076B | time 临时值先置 double 0，按 %lf 读取并存 double | A |
| 0x451372..0x4513B4 | 0x050772..0x0507B4 | shipId、podId 是选择请求；原生调用参数为 -1 时才由 XML 替换，不应直接当成当前选择 | A |
| 0x4514F0..0x451519 | 0x0508F0..0x050919 | PlanetNode 根据 name 查已有星体；找不到时跳过，不创建“缺省新星体” | A |
| 0x4515CC、0x4515D6..0x451600 | 0x0509CC、0x0509D6..0x050A00 | 先调用 ShipNode 读取，再读 planet、查名并绑定父星体；随后追加到该星体 children | A |
| 0x4837A0..0x48381E | 0x082BA0..0x082C1E | 名称查询先自身再按子节点原序递归，命中立即返回；重复名称不是后项覆盖前项 | A |

读取模型保留 ship_id=-1、pod_id=-1 请求。调用方通过 solar_system_or(default) 选择默认资源来源；本层不读文件。缺失 Nodes、缺失嵌套 Ship，以及绑定到未知星体等无效结构返回 InvalidData，而不复现 PC 空指针访问（B）。未知 Runtime 子节点与 PC 一样跳过。

### ShipNode：输入半径不是恢复来源

| PC VA | 文件偏移 | 读取或保存差异 | 等级 |
| --- | --- | --- | --- |
| 0x476810..0x476855 | 0x075C10..0x075C55 | 首先加载嵌套 Ship | A |
| 0x476898..0x4768CC | 0x075C98..0x075CCC | id 默认 0，按 %d 读取 | A |
| 0x4768D7..0x4769D3 | 0x075CD7..0x075DD3 | 仅依次读取 x/y/vx/vy，每项 double 初值 0，按 %lf 读取；没有读取 planetRadius | A |
| 0x476A30、0x476B04..0x476B4A | 0x075E30、0x075F04..0x075F4A | 保存 planetRadius 从已绑定的父星体数据取 double，再以 %f 输出，不取原 XML 属性 | A |

实现中的 source_planet_radius: Option<String> 只是输入旁证。绑定与保存均忽略它；保存必须传入实际 SolarSystem，查名失败报错，不能用 0 或输入半径凑出恢复状态。位置与速度保持 f64，不提前转为 f32。

### PlanetNode：有效补丁，不是无条件默认零

0x4870AD / 0x0864AD 查找 trueAnomaly；不存在直接跳至 0x487182。存在时按 %lf 读取，在 0x4870D6 / 0x0864D6 比较 sscanf 返回值是否为 1，失败仍直接退出；**只在成功时重建轨道对象并更新轨道初值状态**（A）。保存 0x487007 / 0x086407 取当前轨道对象的值，而不是重读 XML（A）。

[bind_solar_system](../../crates/dr-core/src/sr1/runtime.rs) 先使用资源 Orbit.v，再按 Runtime 节点顺序应用成功的 trueAnomaly；缺失/失败为 None，不覆盖资源值或先前有效补丁。未知名称跳过；重复有效补丁最后一次生效（B，由读取顺序与成功覆盖分支导出）。返回 WorldBindings，保留 Planet 与 ShipNode 的实际引用，不计算轨道位置、速度或时间推进。

## 保存选择、树顺序与快照边界

- 0x450D90 / 0x050190 保存 Runtime，写出 time、firstStageActivated、solarSystem、shipId。
- 0x450ECA..0x450F0B / 0x0502CA..0x05030B：当前所选 Ship 的 Pod 指针非空才写 podId；空指针不写该属性（A）。实现用显式 SaveSelection { ship_id, pod_id: Option<i32> }，不拿载入请求冒充当前指针。
- 0x450AF0 / 0x04FEF0：先输出星体，再按 children 原序递归。资源子星体先建，载入 ShipNode 后追加，所以快照输出按“星体→各资源子树→附着该星体的 ShipNode”生成（A/B）。未在 Runtime XML 列出的资源星体仍有资源初值。
- 原生 Save 在 0x450BE5..0x450C3C / 0x04FFE5..0x05003C 还要求 **Node+0x100 为真、根相关对象+0x1CB 为假、Node+0x184 为真、PartTree 前序项数大于 1**（A）。已实现 ShipSaveFlags::eligible(root_traversal_count)，保留偏移命名，不赋予臆测的业务含义；没有猜测 Default。8 组 flag × 0/1/2 项计数的 24 个断言覆盖完整真值表。
- 三处 flag 的完整生命周期与业务含义未核实（U）；XML 不含足够信息恢复内存标志。调用方可用上述纯 helper 先筛选待保存 ShipNode，再调用序列化。
- **Runtime::to_element/to_xml(&SolarSystem, SaveSelection) 是调用方筛选后的快照序列化，不是完整游戏对象 Save 的替代品。** 调用方提供实际选择、待保存 ShipNode 集合和当前挑战数值；本层不猜存活、销毁或物理对象状态。
- solarSystem 来源名为空时保存返回 InvalidData。默认资源调用方解析资源名后填回 runtime.solar_system；不能用 SolarSystem 的展示名称冒充资源来源名。

## 挑战字段：读取默认、大小写与输出来源

挑战 Runtime 的类由关卡工厂选择，根没有可靠的类标签。本层通过 ChallengeKind 显式选择，不根据恰好出现的属性猜类，也不将不同挑战数据合并。

| 本层显式类型 | 读取 VA / 文件偏移 | 保存 VA / 文件偏移 | 行为 |
| --- | --- | --- | --- |
| MoonTouchDown | 0x458075 / 0x057475 | 0x457FA9 / 0x0573A9 | MoonTouchDown 默认 false，布尔读取；写 0/1 |
| MoonTouchDownFuelUsed | 0x45792F / 0x056D2F，并调用上行读取 | 0x457858 / 0x056C58 | 大写 FuelUsed 默认 double 0；保留 MoonTouchDown 并额外写 FuelUsed |
| MaxSpeed | 0x459583 / 0x058983 | 0x45949F / 0x05889F | maxSpeed 默认 double 0；保存值来自当前状态对象，不是位置差估算 |
| FuelUsed | 0x459E33 / 0x059233；0x45A613 / 0x059A13 | 0x459D5A / 0x05915A；0x45A53A / 0x05993A | 小写 fuelUsed，默认 double 0；两类 Runtime 共用同一规则，保存时遥测对象为空则省略 |
| LowFlyer | 0x456AE7..0x456BF4 / 0x055EE7..0x055FF4 | 0x4568E2..0x456A47 / 0x055CE2..0x055E47 | angleTraveled、lastAngle、lastAngleDelta 默认 double 0；recordedAngle、goodJob 默认 false；五项均恢复和写出 |

以上分支均为 A。数值读取使用 %lf / double 保存，输出均为 %f 的六位格式。**大小写敏感：FuelUsed 不等于 fuelUsed。** LowFlyer 还在 0x456BFA / 0x055FFA 清零一个不保存的内部 byte；职责未确认，未给它编造业务字段或评判公式（U）。

ChallengeState 写出调用方当前快照：不计算得分、着陆判定、耗油或最高速度。maxSpeed 与小写 fuelUsed 的游戏 Save 从活跃状态/遥测对象取值，读取的累计值只是恢复初值，不代表已重建游戏对象。小写燃料快照的 Some(value) 写数值，None 显式表示保存时遥测对象不存在；读取缺失/失败得到 Some(0)，表达“恢复初值为 0”，不证明真实遥测对象已经构造。

## 真实 SmolarSystem 资源与静态归一

资源：[SmolarSystem.xml](D:/githubs/dr/SimpleRockets-PC-1.6.11/SmolarSystem.xml)。根是 SolarSystem，不是把文件名误写成 schema 名称。只读统计为 **16 个 Planet、15 个 Orbit、8 个 Atmosphere、5 个 Range**，每个 Planet 都有 Terrain（R）。根 Sun；Smearth→Smoon、Smupiter→四颗 Jr 卫星、Smaturn→Titan Jr 均为嵌套 Children，而非扁平同级。

真实恢复例：[LevelSmearthLanding.xml:1–13](D:/githubs/dr/SimpleRockets-PC-1.6.11/LevelSmearthLanding.xml#L1-L13) 的 ShipNode 带 planetRadius，而星体先由 solarSystem 资源构造；该 XML 缺 shipId/podId，不能把“保存时输出”当成“读取时必需”。[LevelSimpleDocking.xml](D:/githubs/dr/SimpleRockets-PC-1.6.11/LevelSimpleDocking.xml) 有 shipId 但可缺 podId。[solar.rs 的缩小 fixture](../../crates/dr-core/src/sr1/solar.rs) 保留真实 Sun/Smearth/Smoon 层级、Orbit、海洋山脉与大气数据，测试不依赖游戏安装路径。

| PC VA | 文件偏移 | 已实现的数据处理 | 等级 |
| --- | --- | --- | --- |
| 0x4852D0、0x48533C..0x485385 | 0x0846D0、0x08473C..0x084785 | 取 SolarSystem，再取首个根 Planet；读取展示 name/description | A |
| 0x484650、0x4841DE | 0x083A50、0x0835DE | 递归 Planet 数据；构造 launchEnabled 初值 true，坏值不覆盖 | A |
| 0x484825..0x48488F | 0x083C25..0x083C8F | gravity、radius double 默认 0 | A |
| 0x4848BA..0x4848D6；常量 0x535260 | 0x083CBA..0x083CD6；0x134060 | 质量 = radius × radius × gravity / 6.67384e-11，保持 double 运算顺序，不擅自 clamp | A |
| 0x4848ED..0x484A28 | 0x083CED..0x083E28 | Orbit 可选；w/e/a/v 默认 double 0，prograde 默认 false | A |
| 0x484A4B..0x484B17 | 0x083E4B..0x083F17 | Atmosphere 可选；graphicType 默认 int 0，height/surfacePressure 默认 double 0 | A |
| 0x484B21、0x48525C..0x4852BF | 0x083F21、0x08465C..0x0846BF | Terrain 必需，缺失原生抛“Planet is missing terrain”；本层返回 InvalidData | A/B |
| 0x484B38..0x484B7C | 0x083F38..0x083F7C | noise 先 double 读取，再转 f32，不能套成所有字段都先 f32 | A |
| 0x484C4C..0x484C6C；常量 0x535330 | 0x08404C..0x08406C；0x134130 | waterDensity 保 double；严格大于 0.1 才 has_water=true | A |
| 0x484C92..0x484D1E | 0x084092..0x08411E | maxHeight/minHeight 转 f32；base_height=maxHeight>0?0:f32(maxHeight)；water_level=minHeight>0?0:minHeight，后者仍 double | A |
| 0x484DB8..0x484DE1 | 0x0841B8..0x0841E1 | Range 两角端点按原比较分支排序；高度 f64→f32，角端点仍 double | A |
| 0x484F2C..0x485078 | 0x08432C..0x084478 | Layer 的 depth/waveLength/waveHeight 都为 double，另读 topColor/bottomColor | A |
| 0x485193..0x48521A | 0x084593..0x08461A | 子 Planet 接父星体名称/质量参数递归；同级 Planet 继续按名称筛选 | A |
| 0x484480..0x4845C5；0x415A80..0x415B64 | 0x083880..0x0839C5；0x014E80..0x014F64 | 颜色逗号分割跳过空项；默认不透明黑；至少三项取 RGB、恰好四项取 alpha；atoi 结果存 byte，不做饱和裁剪 | A |

Ranges/Layers 从首个命名项开始后使用无名称过滤的后续元素遍历，本层保留此读取差异；Children 的 Planet 迭代则继续按名称过滤。颜色整数极端溢出遵循公共标量层的已验证边界，不将额外输入规则声称为整个 CRT atoi 的完整复刻。

## API 使用

~~~rust
use dr_core::sr1::{catalog::Catalog, runtime::{Runtime, SaveSelection}, solar::SolarSystem};

let catalog = Catalog::from_xml(part_types_xml)?;
let mut runtime = Runtime::from_xml(runtime_xml, &catalog)?;
let resource_name = runtime.solar_system_or("SmolarSystem.xml").to_owned();
// 由调用方从自己的资源来源读取文本；本层不会按该字符串访问磁盘。
let system = SolarSystem::from_xml(solar_system_xml)?;
runtime.solar_system = resource_name;
let bindings = runtime.bind_solar_system(&system)?;
// 这里只得到星体引用和轨道恢复初值，尚未推进到 runtime.time。
let selection = SaveSelection { ship_id: current_ship_id, pod_id: current_pod_id };
let snapshot_xml = runtime.to_xml(&system, selection)?;
~~~

挑战入口为 from_element_with_kind/from_xml_with_kind(..., ChallengeKind)；普通入口明确是 ChallengeKind::None。保存前由调用方更新 ChallengeState，并使用 ShipSaveFlags 等显式上下文筛选待保存节点。

RuntimeNode 的公开表示为 Planet(PlanetNode) / Ship(Box<ShipNode>)：大载荷使用 Box，避免每个 Planet 节点都承担完整 ShipNode 的体积。ship_nodes() 和 WorldBindings 中的船体引用仍为 &ShipNode，装箱不改变读取、绑定或保存语义。

## 本轮验证与未验证边界

已执行：

1. 只读 PC PE 节映射、SHA-256、关键 VA 指令及浮点常量核对；未启动游戏。
2. Python 标准库只读解析真实资源与关卡根属性，核实 16/15/8/5 统计、必需 Terrain、选择属性可缺失。
3. 仅对本人两份 Rust 文件运行 rustfmt --edition 2024 --config skip_children=true。
4. git diff --check 对本人两份 Rust 文件通过。

已新增 [Runtime 离线测试](../../crates/dr-core/src/sr1/runtime.rs) 16 项与 [SolarSystem 离线测试](../../crates/dr-core/src/sr1/solar.rs) 8 项：默认/坏值、f64 精度、不可恢复半径、失败异常角不覆盖、默认星体与树序、可选 pod、未知名称错误、挑战类不猜测、大小写燃料、五项 LowFlyer、资源缩小 fixture、地形 f32 边界、严格水阈值、颜色截断、迭代差异、保存真值表与缺少上下文。**本队友未执行 Cargo**，由 Lead 统一避免构建锁竞争。Lead 首轮反馈本模块除公共 write_bool 返回 true/false 与 PC 0/1 合同不一致所致断言外，其余已通过；标量队友已修正公共合同，装箱表示调整与最终结果待 Lead 统一复跑：

~~~powershell
cargo test -p dr-core sr1::runtime::tests
cargo test -p dr-core sr1::solar::tests
~~~

明确未实现/未声称验证：

- 轨道随时间推进、位置速度重算、SOI 切换、重力/大气/碰撞/地形噪声生成。
- 当前 Ship/Pod 的完整游戏 UI 选择与生命周期；载入请求和保存实际选择分开暴露。
- 原生 Save 三处运行 flag 的业务含义与生命周期。已实现给定三个 flag 后的精确 eligibility，不会从 XML 臆造标志。
- 挑战胜负/计分、耗油与 maxSpeed 遥测对象构造；实现字段恢复与显式快照输出，不编造算法。
- 通用混合内容 XML、排版/注释的词法往返；现有严格编辑合同保持原样。

## 本轮统一验收

Lead 已完成的构建、跨模块回归、真实资源和独立 Rust/CRT 差分结果，见[本体数据语义层验证记录](../sr1-data-restoration.md#验证与后续)。本文的队友交接验证与该统一结果分列。
