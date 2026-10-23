# SR1 PC 部件构造、加载与保存证据

## 范围与证据等级

实现入口为 [parts.rs](../../crates/dr-core/src/sr1/parts.rs)，独立于现有严格的 [model.rs](../../crates/dr-core/src/model.rs) 与 [io.rs](../../crates/dr-core/src/io.rs)。目录类型与燃料容量复用 [catalog.rs](../../crates/dr-core/src/sr1/catalog.rs)，数值读取及六位输出复用 [scalars.rs](../../crates/dr-core/src/sr1/scalars.rs)，分级结构复用 [staging.rs](../../crates/dr-core/src/sr1/staging.rs)。本模块不接 UI，不执行物理模拟。

- **甲级：PC 原生静态实证**。只读分析 [SimpleRockets.exe](../../../SimpleRockets-PC-1.6.11/SimpleRockets.exe) 的 x86 指令、常量、调用链与虚函数表。没有运行或修改游戏。本文的地址是 PC VA 和 PE 文件偏移，不是伪造的源码行号。
- **乙级：PC 配套资源佐证**。[PartList.xml](../../../SimpleRockets-PC-1.6.11/PartList.xml) 与 [DefaultShip.xml](../../../SimpleRockets-PC-1.6.11/DefaultShip.xml) 用于核实字段与建立缩小测试样本，不以单个资源样本推断构造默认。
- **未验证边界**：运行时物理结果、编辑器镜像操作、异常浮点文本的 CRT 全平台词法一致性，没有通过启动游戏验证，也不由本模块补猜。

原生文件标识：ImageBase `0x400000`，文件长度 `1744896` 字节，SHA-256 `7adf2d73d6f95f79333154a98cca21af734917981e0ccf94171e3846547dd0e9`。该文件不是 unpack 目录中的空 C++/CLI wrapper；本次结论均直接来自该 PC 原生文件，没有以 APK 符号代替 PC 证据。

只读工具为本机已有 `pefile` / `capstone`，通过如下解释器入口读取二进制：

```powershell
uv run --no-project --no-cache --python 3.12 python -B target/sr1-research/sr1-parts/disasm.py 0x46fa80:0x28b 0x47a8c0:0x11f 0x47c2e0:0x11e
```

分析脚本与临时反汇编仅写在本仓库 `target/sr1-research/sr1-parts`，不写游戏目录。函数名是对调用位置和行为的语义标注，不声称持有原生源码。

## 公共接口与两阶段加载

```rust
Part::from_element(&Element, &Catalog) -> sr1::Result<Part>
Part::from_xml(&str, &Catalog) -> sr1::Result<Part>
Part::to_element(&self) -> Element
Part::to_xml(&self) -> sr1::Result<String>
Part::pod(&self) -> Option<&PodState>
Part::pod_mut(&mut self) -> Option<&mut PodState>
Part::source_element(&self) -> &Element
normalize_throttle(f64) -> f64
clamp_loaded_fuel(f64, f64) -> f64
```

`PartState` 包含 `Generic / Pod / Tank / Engine / Parachute / Lander / Solar`。`Part.class` 仍保留目录工厂的完整十三类，不把只有通用 XML 状态的 Wheel、Rcs 等误称成没有专用运行时类。

`PodState.staging: Option<Staging>` 保留“输入确实没有自身 Staging”的信息，让船体加载器在建树之后执行旧根迁移和自身子树引用过滤；不在部件工厂阶段丢弃尚未建立的引用。`current_stage` 只在公共 `Staging` 中定义，没有重复 schema。

原始输入完整保留于 `source_element()`，仅作诊断。`to_element()` 重新建立本体保存支持的属性和子元素；未知属性、未知子元素、其他类别的 Tank/Engine 状态不会被冒充成本体保存结果。

## 单部件外层读取与写出

| 行为 | PC VA | 文件偏移 | 甲级证据与已实现语义 |
| --- | --- | --- | --- |
| 单部件加载入口 | `0x46FA80` | `0x6EE80` | 先取 `partType`、查目录、以 `id` 初值 0 构造部件，再调用具体类的虚 Load。 |
| 目录 ID 未命中 | `0x440EC0`，异常路径 `0x441020–0x441098` | `0x402C0`，`0x40420–0x40498` | 精确按 ID 找目录项，失败抛出含 `Unable to find part: ` 的异常，没有默认类型回退。本层转为 `InvalidData`。 |
| 位置 | `0x46FB7B–0x46FBF9` | `0x6EF7B–0x6EFF9` | `x/y` 的局部初值各为 0，均按 `%lf` 扫描并存 double。 |
| 编辑方向 | `0x46FBFE–0x46FC3C` | `0x6EFFE–0x6F03C` | `editorAngle` 按 `%d`、初值 0，保留原始 i32，不自动取模或改写 `angle`。 |
| 旋转与角速度 | `0x46FC35–0x46FCC7` | `0x6F035–0x6F0C7` | `angle/angleV` 均 `%lf`、初值 0，double 存入物理体。 |
| 外层保存 | `0x46F820–0x46FA70` | `0x6EC20–0x6EE70` | 保存 `partType/id/x/y/angle/angleV/editorAngle`；位置、角度用 `%f` 六位格式，ID/编辑方向用 `%d`。 |

输入 `angle` 与 `editorAngle` 是两个不同字段。加载路径不会因为 `flippedX/Y` 或编辑方向而额外取反 `angle`。编辑器四向弧度映射由公共数值模块提供，不能反过来用于覆盖运行态的 `angle`。

## 实际工厂与通用状态的继承差异

工厂入口 `0x499EB0` / 文件偏移 `0x992B0` 按目录的 `type` 选择原生类：

| 目录 type | 构造入口 PC VA / 文件偏移 | 虚函数表 VA | XML 状态加载 / 保存 VA |
| --- | --- | --- | --- |
| 未命中的 type（已存在的目录项） | `0x47A680 / 0x79A80` | `0x531090` | `0x47A8C0 / 0x47A9F0` |
| engine | `0x489A80 / 0x88E80` | `0x531968` | `0x48A0A0 / 0x48A150` |
| detacher | `0x4B0A70 / 0xAFE70` | `0x533FE8` | 通用加载 / 通用保存 |
| dockconnector | `0x4AD500 / 0xAC900` | `0x533C68` | 通用加载 / 通用保存 |
| dockport | `0x4AD040 / 0xAC440` | `0x533BAC` | 通用加载 / 通用保存 |
| lander | 内联 `0x49A101–0x49A135 / 0x99501–0x99535` | `0x533D40` | `0x4ADA90 / 0x4ADB90` |
| parachute | `0x4AB840 / 0xAAC40` | `0x533AFC` | `0x4ABBE0 / 0x4ABDE0` |
| pod | `0x47C080 / 0x7B480` | `0x531134` | `0x47C2E0 / 0x47C4F0` |
| nosecone | `0x4B06A0 / 0xAFAA0` | `0x533F44` | 通用加载 / 通用保存 |
| rcs | `0x4AE5C0 / 0xAD9C0` | `0x533DCC` | 通用加载 / 通用保存 |
| solar | `0x4AF260 / 0xAE660` | `0x533E40` | `0x4AF630 / 0x4AF690` |
| tank | `0x488210 / 0x87610` | `0x5317EC` | `0x488320 / 0x4883D0` |
| wheel | `0x4AFF90 / 0xAF390` | `0x533ED0` | 通用加载 / 通用保存 |

核对方式为：构造器写入虚表指针；虚表 `+0x34` 为初始化、`+0x38` 为 Load、`+0x40` 为 Save。**Pod 和 Tank 的覆盖函数不调用通用 Load/Save**，不是所有部件统一读写布尔字段。

通用构造 `0x47A705–0x47A74D` / `0x79B05–0x79B4D` 将 activated、exploded 与双向 flipped 对应内存清零。通用 Load `0x47A8C0` / `0x79CC0` 依次以 false 初值读取 `activated/exploded/flippedX/flippedY`；通用 Save `0x47A9F0` / `0x79DF0` 将四项写为 `0/1`。

`0x47A980–0x47A9D6` / `0x79D80–0x79DD6` 从 flipped 选择 `+1.0f32/-1.0f32` 再乘视觉缩放。已实现对应的布尔状态，未伪造图形对象、形状变换或镜像编辑算法。Pod/Tank 的这四项按构造 false 保持，输入中的同名属性只留在诊断原始元素中，不参与本体写出。

## Pod 油门、名称与分级

| 项目 | PC VA / 文件偏移 | 已实现语义 |
| --- | --- | --- |
| 构造 | `0x47C0D9`、`0x47C149–0x47C17C / 0x7B4D9`、`0x7B549–0x7B57C` | throttle=0、name 为空、分级表为空、currentStage=0，挂起 Staging 指针为空。 |
| 子节点查找 | `0x47C2FC–0x47C31F / 0x7B6FC–0x7B71F` | 只看首个 Pod 子元素与其首个 Staging；无 Pod 时保构造状态。 |
| 油门读取 | `0x47C325–0x47C365 / 0x7B725–0x7B765` | `%lf` 读取到 f64，缺失/失败初值 0；不是先舍入到 f32。 |
| 油门吸附 | `0x47C35D–0x47C38A / 0x7B75D–0x7B78A` | 严格大于上阈值才置 1，严格小于下阈值才置 0，等于阈值保值。 |
| 名称读取 | `0x47C392–0x47C3D7 / 0x7B792–0x7B7D7` | Pod 子元素的 name，缺失为空字符串。 |
| 分级二阶段 | `0x47C430–0x47C4BD / 0x7B830–0x7B8BD` | 部件树已建好后读挂起的 Staging，currentStage 也来自这个 Staging 节点。 |
| 保存 | `0x47C4F0–0x47C67B / 0x7B8F0–0x7BA7B` | 写 Pod throttle/name；`0x47C5C8–0x47C60F` 无条件创建并保存 Staging 及其 currentStage，因此输入 None 保存为默认空表。 |

油门常量并非十进制 `0.99_f64/0.01_f64`：

| 常量 VA / 文件偏移 | double 原始位 | 实际值 |
| --- | --- | --- |
| `0x5353C0 / 0x1341C0` | `0x3FEFAE1480000000` | `0.9900000095367432`，即 `0.99_f32 as f64` |
| `0x5352F8 / 0x1340F8` | `0x3F847AE140000000` | `0.009999999776482582`，即 `0.01_f32 as f64` |

例如输入 `0.990000005` 保值，不能误吸附为 1。测试同时覆盖两阈值本身与相邻 f64 值。

## Tank 与 Engine 燃料

- Tank 构造 `0x488251–0x488263` / `0x87651–0x87663`，Engine 构造 `0x489B4F–0x489B54` / `0x88F4F–0x88F54`：从目录的 fuel 容量 double 初始化当前 fuel。没有对应运行态子元素时，保持容量初值，不执行裁剪。
- Tank Load `0x488320–0x4883C3` / `0x87720–0x877C3`，Engine Load `0x48A0A0–0x48A14B` / `0x894A0–0x8954B`：存在 Tank/Engine 子元素时，以局部 double 0 读取 fuel；因此 `<Tank/>` 与完全没有 `<Tank>` 语义不同。
- 两个加载器都用 `%lf` 与 `comisd/movsd`。容量也为 f64，普通有限值按 `[0, capacity]` 裁剪；原始比较顺序通过 `clamp_loaded_fuel` 明确表达，没有用会对负容量 panic 的 Rust `clamp`。
- 同名重复子元素只消费第一项；非对应类别的 Engine/Tank 不会驱动状态。
- Tank Save `0x4883D0` / `0x877D0` 与 Engine Save `0x48A150` / `0x89550` 无条件写对应子元素的 fuel，用 double 六位输出；Engine 另外调用通用保存，Tank 不调用。

容量接口统一使用 `PartType::fuel_capacity() -> Option<f64>`，不重复定义静态 schema。静态 Engine 没有 Tank 时的零容量由目录层还原。直接函数测试包含负容量与 NaN 比较分支，但这不等于声称 PC CRT 接受所有 Rust 的非有限浮点文本。

## Parachute 构造与加载覆盖

构造 `0x4AB884–0x4AB8D9` / `0xAAC84–0xAACD9`：chuteX/Y、chuteAngle/Height 为零，inflation 内存为 `0x3DCCCCCD` 即 `0.1f32`，inflate/deployed/rope 均为 false。

加载入口 `0x4ABBE0` / `0xAAFE0` 先调用通用 Load，然后按字段自己的局部初值读入：

| 字段 | 精度与初值 | 关键 PC VA / 文件偏移 |
| --- | --- | --- |
| chuteX、chuteY | `%lf` → f64，各初值 0 | `0x4ABC1E / 0xAB01E`、`0x4ABC5A / 0xAB05A` |
| chuteAngle | `%f` 直接扫描 f32，初值 0 | `0x4ABC95–0x4ABCAC / 0xAB095–0xAB0AC` |
| chuteHeight | `%f` 直接扫描 f32，初值 0 | `0x4ABCD0–0x4ABCE7 / 0xAB0D0–0xAB0E7` |
| inflation | `%f` 直接扫描 f32，初值 0；覆盖构造的 0.1 | `0x4ABD0B–0x4ABD24 / 0xAB10B–0xAB124` |
| inflate、deployed、rope | bool，各初值 false | `0x4ABD1D–0x4ABDD6 / 0xAB11D–0xAB1D6` |

保存入口 `0x4ABDE0` / `0xAB1E0` 写出通用四项及上述全部八项。没有擅自将 inflation、height、angle 限制到“看起来合理”的范围。

## Lander 与太阳能板

Lander 构造内联在工厂 `0x49A101–0x49A135` / `0x99501–0x99535`：length 从目录 `minLength` double 通过 `cvtpd2ps` 写成 f32，legAngle=0，lower/raise=false。加载 `0x4ADA90–0x4ADB81` / `0xACE90–0xACF81` 则重新用局部零读取 length/legAngle，**缺失时不会保留构造 minLength**。lower 与 raise 独立读取，允许同时为 true。

Lander Save `0x4ADB90–0x4ADCE2` / `0xACF90–0xAD0E2` 明确依次写 `lower/raise/length/legAngle`。后两项从 f32 提升为 double 后按六位输出。本模块全部实现，不沿用旧 DTO 曾经漏存 Lander 属性的行为。加载未根据 flippedX 自动反转 legAngle，保存也直接写当前字段。

`extension` 的实际所有者是 **SolarPanel**：工厂的 solar 分支调用 `0x4AF260` / `0xAE660`，其虚表 `0x533E40` 的 Load 为 `0x4AF630`。构造 `0x4AF368` / `0xAE768` 将 extension 置零；Load `0x4AF647–0x4AF67E` / `0xAEA47–0xAEA7E` 按 `%lf` 读 double 后 `cvtpd2ps` 存 f32，初值 0；Save `0x4AF690` / `0xAEA90` 写出该字段。没有把它错误归到 Lander，也没有额外裁到 `[0,1]`。

## 资源与测试依据

- [PartList.xml:54–75](../../../SimpleRockets-PC-1.6.11/PartList.xml#L54-L75) 提供 fueltank-2 的 `fuel=3000` 与 `dryMass=0.85`。
- [DefaultShip.xml:3–7](../../../SimpleRockets-PC-1.6.11/DefaultShip.xml#L3-L7) 同时包含没有 Pod 子元素的 pod-1、带 Tank 子元素的 fueltank-2、没有 Engine 子元素的 engine-2，佐证“子元素缺失保构造值”路径。
- [parts.rs 的离线测试](../../crates/dr-core/src/sr1/parts.rs#L324) 将目录缩小为纯内联 fixture；极端数值、额外字段与 `12.125` 容量为专门构造的边界案例，不冒充原资源内容。所有测试无需游戏目录。

十三项测试覆盖：通用值前缀与精度；缺失/坏值；fuel 子元素缺失与 fuel 属性缺失差异；f64 裁剪；Pod/Tank 跳过通用字段；油门 f32 提升阈值；分级输入存在性和保存空表；Parachute 混合精度；Lander 四字段保存；Solar 窄化且不裁剪；直接 `%f` 与 `%lf` 后窄化在 f32 溢出中点的不同结果；原始数据诊断与本体输出分离；错误根和未知目录 ID。该中点金样由数值模块对同版 x64 CRT 辅助核实，PC 的字段精度路径则直接由 x86 指令确认，不将辅助平台测试写成游戏动态测试。

## 本轮验证记录

- 已实际完成：PC 可执行文件只读反汇编、全部十三种工厂虚表逐项核对、原始资源读取与 SHA-256 标识校验。
- 已实际完成：仅对本模块执行 `rustfmt --edition 2024 --config skip_children=true crates/dr-core/src/sr1/parts.rs`，随后 `rustfmt --check` 通过；对两个新增文件逐行检查，尾随空白为零；本文十二个本地文件链接全部存在。
- 本队友未运行 Cargo，遵守统一构建和锁竞争约定。新增 Rust 测试的集成运行由 Lead 统一执行，建议命令：

```powershell
cargo test -p dr-core sr1::parts::tests
```

## 已实现与未验证边界

已实现：本体属性级工厂状态、构造与 Load 缺省差异、f32/f64 精度路径、阈值归一、燃料容量裁剪、类继承读写差异、支持字段的规范化保存、诊断输入保留，以及与 Ship/Staging 的两阶段接口。

未实现或未验证：

1. 物理引擎推进、质量/惯量重算、部件视觉缩放对象、降落伞绳约束及 Lander 接地行为。本文不以字段加载成功代表模拟一致。
2. 编辑器镜像、拖动、旋转等操作的全过程。属性级 Load 不额外修改角度；完整编辑算法应在另有 PC 证据后新增独立入口。
3. CRT 对溢出、地区设置与特殊浮点文本的所有词法细节；共同边界见数值模块。已还原比较分支不等于保证所有文本输入会产生该值。
4. 字节级 XML 重排保真，以及原生 C++ 异常对象的 ABI；本层使用公共 XML 元素和 Rust 错误类型表达可审查的语义。

## 本轮统一验收

Lead 已完成的构建、跨模块回归、真实资源和独立 Rust/CRT 差分结果，见[本体数据语义层验证记录](../sr1-data-restoration.md#验证与后续)。本文的队友交接验证与该统一结果分列。
