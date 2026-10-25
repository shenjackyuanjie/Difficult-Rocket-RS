# SR1 PC 部件目录与静态数据证据

[项目首页](../../README.md) · [操作指南](../guides/editor.md) · [验证指南](../validation/README.md)

## 范围与证据等级

本次新增独立的 [sr1/catalog.rs](../../crates/dr-core/src/sr1/catalog.rs)，不修改现有 [严格目录读写](../../crates/dr-core/src/io/catalog.rs) 与 [编辑器模型](../../crates/dr-core/src/model.rs) 的默认合同。

- **甲级：PC 指令实证。** 读取本机 [原生 PC 程序](D:/githubs/dr/SimpleRockets-PC-1.6.11/SimpleRockets.exe)，ImageBase 为 `0x400000`。下文地址均为 PC VA，文件偏移由 PE 节映射计算，不是源码行号。
- **乙级：资源实证。** 读取本机 [PartList.xml](D:/githubs/dr/SimpleRockets-PC-1.6.11/PartList.xml)。它包含 27 个部件：Tank 7、Engine 6、Detacher 2、Wheel 2，其余 10 类各 1；所有部件均带 AttachPoints。资源值可证明该输入，缺省行为另由甲级证据确定。
- **丙级：辅助符号定位。** 只读取 [APK x86 原生库](D:/githubs/dr/SimpleRockets-PC-1.6.11/unpack/APK/x86/libNativeModule.so) 的动态符号，如 `PartType::PartType`、`PartsList::GetPartType`、`ObjectCreator::CreatePartObject`；关键行为均回到 PC 核查。没有把 unpack 的 C++/CLI wrapper 当作游戏逻辑。

本轮未运行、注入或修改游戏；分析脚本和中间反汇编仅放在工作区的 `target/sr1-research/sr1-catalog`。没有访问配置、数据库或上游。

| 只读对象 | 字节数 | SHA-256 |
| --- | ---: | --- |
| [原生 PC 程序](D:/githubs/dr/SimpleRockets-PC-1.6.11/SimpleRockets.exe) | 1,744,896 | `7adf2d73d6f95f79333154a98cca21af734917981e0ccf94171e3846547dd0e9` |
| [本机部件资源](D:/githubs/dr/SimpleRockets-PC-1.6.11/PartList.xml) | 14,368 | `1fcd135c222057bcd71c94805d0e56d8bfc3fbd3da3963884bb82282811bbb77` |

这里核对的是上述指纹对应的本机 PC 版本；不据资源文件的修改日期推断它必然等于所有发行包。

## 公共接口与加载上下文

接口已与 parts、ship/staging 和 runtime 队友协调：

```rust
Catalog::from_xml(&str) -> sr1::Result<Catalog>
Catalog::from_element(&Element) -> sr1::Result<Catalog>
Catalog::get(&self, id: &str) -> Option<&PartType>
PartType::from_element(&Element) -> sr1::Result<PartType>
PartType::fuel_capacity(&self) -> Option<f64>
PartType::has_attach_point(&self, id: i32) -> bool
```

- `Catalog.parts: Vec<PartType>` 保留读取顺序，解析时拒绝重复 ID 或显示名称，查询按 ID 精确匹配。直接修改公开向量后，由调用方维持唯一性。
- `PartType.class: PartClass` 是实际工厂类别；`kind: String` 保留资源 type 原文。
- `tank/engine/rcs/solar/lander` 是独立静态规格，不复用旧编辑器 schema。容量为 `f64`，与运行状态部件的 PC `%lf` 读取对齐。
- `attach_points: Vec<AttachPoint>` 的 ID 从 1 自动分配，不读取资源 `@id`。
- `Catalog.raw`、`PartType.raw` 和连接点 `raw` 保留未消费的元素与属性。公共 [XML 适配](../../crates/dr-core/src/sr1/xml.rs) 保序但不保排版、注释或一般混合文本的词法形式。
- 本模块不提供把修改后的有效字段反写成目录资源的规范化 writer。对 `raw` 调用 XML writer 只输出该 raw 树，不会自动同步其他公开字段。

可用空目录为 `<PartTypes/>`；最小 Pod 为：

```xml
<PartTypes>
  <PartType id="pod-1" name="Pod" type="pod"><AttachPoints/></PartType>
</PartTypes>
```

## 目录顺序、重复项与查询

| PC VA | 文件偏移 | 甲级结论与实现 |
| --- | --- | --- |
| `0x43F040`、`0x43F209` | `0x3E440`、`0x3E609` | 载入部件资源，查找 PartTypes 和首个 PartType。 |
| `0x440D73–0x440D93` | `0x40173–0x40193` | 后续项使用无名称参数的 NextSiblingElement，顺序追加；本层从首个 PartType 开始处理后续所有兄弟元素。 |
| `0x440C93–0x440D5F` | `0x40093–0x4015F` | 插入前逐项比较已有项的 ID **或 name**。任一相同即调用 Duplicate part in XML 异常，不是最后一次覆盖。 |
| `0x41DC10`、`0x41DBFC` | `0x1D010`、`0x1CFFC` | 错误格式化最终调用 C++ 异常抛出；本层对应 InvalidData。PC 外层捕获可形成部分目录，本层 Result 不返回部分成功结果。 |
| `0x440EC0–0x441080` | `0x402C0–0x40480` | 从目录首项开始按 ID 精确查找，命中返回对应项。 |
| `0x441020`、`0x441098` | `0x40420`、`0x40498` | ID 不存在则抛出 Unable to find part，不会构造默认 PartType。get 返回 None，由 parts 的 Result 入口报告 InvalidData。 |

首个目标子元素之前的其他元素被跳过。AttachPoint 与 Vertex 的后续兄弟遍历同样不按元素名过滤；Shape 后继使用名称过滤。这些规则有独立测试，不用常规 XML 集合过滤替代原生遍历。

## PartType 默认与尺寸归一

| PC VA | 文件偏移 | 甲级结论与实现 |
| --- | --- | --- |
| `0x455400–0x455539` | `0x54800–0x54939` | 构造字符串为空；drag=1、friction=0.75、buoyancy=1、canExplode=true，激活相关标记初始为 false。常量 1 位于 VA `0x5353D0` / 偏移 `0x1341D0`，0.75 位于 `0x535398` / `0x134198`。 |
| `0x43F6A8–0x43F8B6` | `0x3EAA8–0x3ECB6` | mass 默认 0；hidden、disableEditorRotation、ignoreEditorIntersections、sandboxOnly 默认 false；maxOccurrences 以有符号 i32 读取，默认 0；无额外非负裁剪。物理覆盖属性失败时保构造值。 |
| `0x43FEAA–0x43FF29` | `0x3F2AA–0x3F329` | width、height 以 f64 默认 0 读取，再乘 0.5。本模块公开 width、height 是该归一后的完整尺寸，不是旧模型的原始网格 u32。 |
| `0x43FF3B–0x43FF88` | `0x3F33B–0x3F388` | coverHeight 先取归一后的 height；仅成功解析覆盖属性时将输入乘 0.5。缺失、空串、非法值保 height。 |
| `0x43F8D8–0x43F906`、`0x43FCC7–0x43FCEB` | `0x3ECD8–0x3ED06`、`0x3F0C7–0x3F0EB` | 空 category 的 Engine 归到 Engines，Tank 归到 Fuel Tanks；其他类别保空，不造展示分类。 |
| `0x43FD3E–0x43FEA1` | `0x3F13E–0x3F2A1` | 原生 PartType+0x138 激活相关标记在 Engine、Parachute、Lander、Detacher、Solar 为 true；Wheel 除 wheel-1 外为 true。本层命名 stageable，只还原字段，不执行分级。 |

数值前缀、失败保初值与布尔读取委托公共 scalars。极端 CRT 输入的可验证范围随该模块证据，不在此重新定义解析器。

## 实际工厂类别

PC 工厂入口 VA `0x499EB0`，文件偏移 `0x992B0`；完整分支核对区间至 `0x49A38E`，偏移至 `0x9978E`。type 比较大小写敏感。

| type | PartClass | PC 构造调用 VA | 文件偏移 |
| --- | --- | --- | --- |
| engine | Engine | `0x499F57 → 0x489A80` | `0x99357 → 0x88E80` |
| detacher | Detacher | `0x499FE2 → 0x4B0A70` | `0x993E2 → 0xAFE70` |
| dockconnector | DockConnector | `0x49A06D → 0x4AD500` | `0x9946D → 0xAC900` |
| dockport | DockPort | `0x49A0B8 → 0x4AD040` | `0x994B8 → 0xAC440` |
| lander | Lander | `0x49A105 → 0x47A680`，随后专用 vtable | `0x99505 → 0x79A80` |
| parachute | Parachute | `0x49A199 → 0x4AB840` | `0x99599 → 0xAAC40` |
| pod | Pod | `0x49A1E4 → 0x47C080` | `0x995E4 → 0x7B480` |
| nosecone | Nosecone | `0x49A22F → 0x4B06A0` | `0x9962F → 0xAFAA0` |
| rcs | Rcs | `0x49A27A → 0x4AE5C0` | `0x9967A → 0xAD9C0` |
| solar | Solar | `0x49A2C5 → 0x4AF260` | `0x996C5 → 0xAE660` |
| tank | Tank | `0x49A310 → 0x488210` | `0x99710 → 0x87610` |
| wheel | Wheel | `0x49A357 → 0x4AFF90` | `0x99757 → 0xAF390` |
| 其余，包括 fuselage、strut、空串、未知 type | Generic | `0x49A38E → 0x47A680` | `0x9978E → 0x79A80` |

已存在 PartType 的未知 type 进入 Generic，与船体引用一个不存在的 partType ID 是两种行为。

## Tank、Engine、RCS、Solar 与 Lander 静态字段

| PC VA | 文件偏移 | 甲级结论与实现 |
| --- | --- | --- |
| `0x43F906–0x43FAA3` | `0x3ED06–0x3EEA3` | 仅 Engine 类消费 Engine：power、consumption、size、turn 为 f64 默认 0；fuelType 为 i32 默认 0；throttleExponential 默认 false。turn 乘 `0.017453292519943295` 转弧度。 |
| `0x43FAAF–0x43FB84` | `0x3EEAF–0x3EF84` | Engine 可有 Tank；fuel、dryMass 为 f64，缺 Tank 时两者均为 0。Tank 静态 fuelType 明确继承 Engine@fuelType，不读取 Tank@fuelType。 |
| `0x43FCEB–0x43FD33` | `0x3F0EB–0x3F133` | Tank 类读取自身 Tank@fuel/dryMass/fuelType；默认 0。实际资源 batteries 与 monopropellant 只靠 fuelType 区别，不另造工厂类。 |
| `0x43FBAB–0x43FC86` | `0x3EFAB–0x3F086` | Rcs 类只消费 Rcs@power/consumption/size，均为 f64 默认 0。 |
| `0x43FE3F–0x43FE62` | `0x3F23F–0x3F262` | Solar 存在才读取 chargeRate；存在但缺属性为 0。缺 Solar 的 chargeRate 没有已证实初始化值，公开 solar=None，不填零猜测。 |
| `0x43FD76–0x43FE01` | `0x3F176–0x3F201` | Lander 的 angleSpeed、lengthSpeed、maxAngle、maxLength、minLength、width 直接读 f64，默认 0；**此读取阶段不把 maxAngle、angleSpeed 转弧度**，不从动画或物理用途反推单位。 |
| `0x43EEF0`、`0x41CAD0` | `0x3E2F0`、`0x1BED0` | 共用 f64/i32 属性读取助手均先置 0，再以 %lf/%d 读取，失败保 0。 |

[本体资源](D:/githubs/dr/SimpleRockets-PC-1.6.11/PartList.xml#L164-L175) 中 SRB 的 Engine@fuelType=3 和 Tank@fuel=9000 是容量上下文的真实样例；[Lander 资源](D:/githubs/dr/SimpleRockets-PC-1.6.11/PartList.xml#L271-L277) 的 minLength=2.26、maxAngle=140 不等于部件运行态 length/legAngle 的加载默认。动态状态由 parts 模块处理。

非匹配类别中的 Tank、Engine 等子元素保留在 raw，但不被静态有效字段消费。

## AttachPoint 读取与镜像配对

| PC VA | 文件偏移 | 甲级结论与实现 |
| --- | --- | --- |
| `0x44042B–0x440478` | `0x3F82B–0x3F878` | AttachPoints 下首个 AttachPoint 开始，ID 从 1 自动递增。 |
| `0x4404BA–0x44054A` | `0x3F8BA–0x3F94A` | breakAngle 的度值先转弧度；零（包括解析失败的 0）改为 15° 弧度。breakForce 为零时改为 125；不把显式零理解为无限强或禁止断裂。 |
| `0x44058F–0x440817` | `0x3F98F–0x3FC17` | 八种命名位置按下表派生，忽略其显式 x/y、width/height、maxParts。 |
| `0x440819–0x440885` | `0x3FC19–0x3FC85` | 其他 location（含空串）读取显式 x/y 后乘 0.5；width/height **不乘 0.5**；maxParts 默认 0。未知 location 不套用严格编辑器白名单。 |
| `0x44088B–0x4409AF` | `0x3FC8B–0x3FDAF` | order、group 为 i32 默认 0；fuelLine、flipX、flipY 为 false。输入次序不按 order 重排。 |
| `0x4409B7–0x440A1E` | `0x3FDB7–0x3FE1E` | 左右命名位置默认 swapOnMirror=true，其他默认 false；属性可覆盖。swapId 默认 0；正 swapId 强制启用 swapOnMirror。 |
| `0x440A81–0x440C88` | `0x3FE81–0x40088` | 已启用且 swapId=0 时，寻找首个对应左右位置并写双方 swapId；随后 swapId 大于点总数则清零并禁用。没有对应点时可以保留 true/0，不能自行补假配对。 |

设 `W/H` 为已经乘 0.5 的部件 width/height：

| location | x | y | 连接范围 width | 连接范围 height | maxParts |
| --- | ---: | ---: | ---: | ---: | ---: |
| TopCenter | 0 | H/2 | 0 | 0 | 1 |
| BottomCenter | 0 | -H/2 | 0 | 0 | 1 |
| Top | 0 | H/2 | W-1 | 0 | 0 |
| Bottom | 0 | -H/2 | W-1 | 0 | 0 |
| LeftCenter | -W/2 | 0 | 0 | 0 | 1 |
| RightCenter | W/2 | 0 | 0 | 0 | 1 |
| LeftSide | -W/2 | 0 | 0 | H-1 | 0 |
| RightSide | W/2 | 0 | 0 | H-1 | 0 |

Bottom 和 BottomCenter 另设置原生方向标记，本层公开为 bottom。未读取的 dock 等属性仅保留 raw，不借用旧 DTO 宣称本体会消费它。

## Shape 与 Damage

| PC VA | 文件偏移 | 甲级结论与实现 |
| --- | --- | --- |
| `0x43FF90–0x440154` | `0x3F390–0x3F554` | 显式 Shape 保序；sensor 默认 false。Vertex 的 x/y 先读 f64，乘 0.5，再转 f32。空 Shape 保持空，不替它生成矩形。 |
| `0x440159–0x44028D` | `0x3F559–0x3F68D` | 完全没有 Shape 才生成四点矩形。先将归一后尺寸转 f32：x=width*0.5-0.025，y=height*0.5；依次 (-x,-y)、(x,-y)、(x,y)、(-x,y)。仅 x 收缩；按单精度运算次序实现。 |
| `0x4402CE–0x4403FE` | `0x3F6CE–0x3F7FE` | 存在 Damage 时 disconnect/explode/explosionPower/explosionSize 均默认 0。 |
| `0x440408–0x44041E` | `0x3F808–0x3F81E` | 没有 Damage 时 disconnect=3000、explode=3000、explosionPower=1、explosionSize=5。常量位于 VA `0x535930` / 偏移 `0x134730` 与 VA `0x5358C0` / 偏移 `0x1346C0`。 |

这里还原的是静态读取，不重复 [严格目录入口](../../crates/dr-core/src/io/catalog.rs#L287-L307) 的凸多边形校验、闭合点移除与编辑器修复，也不声称所有退化 Shape 都能被后续物理引擎使用。

## 已实现、验证与未验证边界

**已实现：** 实际工厂分类、资源顺序与重复项错误、ID 查询、公共构造上下文、PC 默认与 f64 精度、尺寸缩放、Shape f32 舍入及缺省矩形、Damage 两级默认、完整连接点范围与镜像配对、按类别消费静态规格、未消费字段保留。

**本队友已执行：** 只读 PC 字符串交叉引用、指令及常量解码；真实资源计数与 SHA-256；9 项 fixture 与原资源部件段逐字比对；文档所有文件链接存在性检查；仅本文件的 `rustfmt --edition 2024 --config skip_children=true` 语法/格式及 `--check` 检查。以上检查均通过。没有自行运行 Cargo，编译及单元测试统一由 Lead 执行。下列 15 个测试已加入，不把新增测试误记为本队友已运行通过：

1. real_resource_fragment_loads_special_classes_and_capacities：逐项摘录的 9 个真实部件，覆盖 Pod、Tank、Engine、SRB、RCS、Solar、Battery、DockPort 多 Shape/sensor 和 Lander。
2. native_general_defaults_and_f32_rectangle_are_not_editor_defaults。
3. present_empty_damage_is_zero_not_missing_damage_defaults。
4. scalar_prefixes_negative_integers_and_failed_defaults_are_preserved。
5. cover_height_only_scales_successfully_parsed_overrides。
6. named_attach_locations_use_native_scaled_dimensions。
7. explicit_attach_points_scale_coordinates_but_not_ranges。
8. zero_and_invalid_break_limits_use_native_nonzero_defaults。
9. mirror_pairing_preserves_native_direction_and_invalid_id_behavior。
10. explicit_shape_coordinates_round_after_f64_scaling_and_empty_shape_stays_empty。
11. engine_tank_inherits_engine_fuel_type_and_nonengine_static_children_are_ignored。
12. factory_is_exact_and_wheel_activation_has_legacy_id_exception。
13. missing_required_native_children_return_explicit_errors_but_solar_stays_optional。
14. duplicate_ids_or_display_names_are_rejected_without_last_wins。
15. sibling_walks_and_unconsumed_fields_are_preserved。

Lead 的模块验证命令：

```powershell
cargo test -p dr-core sr1::catalog
```

**明确边界：**

- 原生缺少 Engine 类的 Engine、Tank 类的 Tank、Rcs 类的 Rcs、Lander 类的 Lander，以及任意项的 AttachPoints，会把空指针交给属性/子元素读取；PC `0x427E66`（偏移 `0x27266`）与 `0x427407`（偏移 `0x26807`）直接解引用 this。本层返回 InvalidData，不仿真访问违例。空 AttachPoints 是合法输入。
- 根元素错误在 Rust 入口返回 WrongRoot；不复现 PC 缺 PartTypes 时的空指针异常。
- 缺 Solar 时保持 None。没有确认的 chargeRate 构造值不填零；存在空 Solar 才读取为 0。
- 本层接受已证实读取可形成的负值、零范围、空 Shape 等，不保证后续 Box2D、渲染、纹理加载或飞行模拟能处理它们。没有实现物理算法、碰撞构建、燃料流网络或运行态推进。
- stageable 和 bottom 是本层对已核原生字段的命名，不伪称拥有对应 C++ 源码成员名。
- 未验证其他 PC 版本、APK 与 PC 的所有差异、恶意超大资源和 CRT 非有限数/溢出的全部平台边界；相应解析能力依赖公共 scalars 的证据范围。

## 本轮统一验收

Lead 已完成的构建、跨模块回归、真实资源和独立 Rust/CRT 差分结果，见[本体数据语义层验证记录](../development/sr1-data.md#验证与后续)。本文的队友交接验证与该统一结果分列。
