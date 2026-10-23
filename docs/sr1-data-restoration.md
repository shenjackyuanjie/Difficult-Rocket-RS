# SR1 本体数据语义层

本轮依据 PC 1.6.11 原生指令、实际资源和同版本 CRT 实测，在 [dr-core::sr1](../crates/dr-core/src/sr1/mod.rs) 实现独立的数据读取、构造、归一和保存接口。无需 Bevy、窗口或物理世界即可调用；暂不接编辑器也可用于导入分析、离线转换与后续运行态实现。

## 与现有编辑器的边界

[原有严格入口](../crates/dr-core/src/io.rs) 继续负责安全编辑，保留已有默认值、未知字段拒绝及编辑事务。新层不隐式替换它，也不把本体的宽松数值读取套到旧模型。

- 本体接口按实际类分派，区分构造、加载、绑定和保存，不只是另一套 Serde DTO。
- 原始元素用于诊断，不把读取时被本体丢弃或覆盖的字段当成完整运行状态。
- 缺少必需节点、非法引用或环时返回明确错误，不复现本体崩溃或无限循环。
- 本层已实现数据读取和已证实的处理；不推进物理、不生成地形、不构造对象生命周期或挑战遥测。

## 功能分块

| 模块 | 已实现内容 | 证据 |
| --- | --- | --- |
| [xml](../crates/dr-core/src/sr1/xml.rs) | 属性与子项保序、实体解码、紧凑输出、五字符转义、深度保护 | 面向属性数据，不提供一般混合内容的词法保真 |
| [scalars](../crates/dr-core/src/sr1/scalars.rs) | 数值前缀、32位回绕、独立f32/f64、失败保初值、整数优先bool、0/1保存、四向角、CRT六位格式 | [标量证据](sr1-evidence/scalars.md) |
| [catalog](../crates/dr-core/src/sr1/catalog.rs) | 13类工厂、目录保序/重复检查、真实默认、尺寸/Shape、Damage、燃料与引擎、附着点和镜像 | [目录证据](sr1-evidence/catalog.md) |
| [parts](../crates/dr-core/src/sr1/parts.rs) | 按虚函数加载；Pod/Tank跳过base；f64燃料/油门；裁剪/阈值；Lander四字段；Parachute/Solar的不同精度路径 | [部件证据](sr1-evidence/parts.md) |
| [ship](../crates/dr-core/src/sr1/ship.rs)、[staging](../crates/dr-core/src/sr1/staging.rs) | 实际父链根、实例索引、普通/对接连接、前序、Pod选择、旧分级迁移、子树过滤、moved、空步骤、Editor分离组 | [船体分级证据](sr1-evidence/ship-staging.md) |
| [runtime](../crates/dr-core/src/sr1/runtime.rs)、[solar](../crates/dr-core/src/sr1/solar.rs) | Runtime默认、选择请求、节点/行星绑定、异常角覆盖、星系/轨道/大气/地形数据、质量/颜色/范围、挑战状态与保存条件 | [运行态星系证据](sr1-evidence/runtime-world.md) |

### 已纠正的常见推断

1. fuel/throttle 是 f64；油门阈值来自 f32 提升。不能把所有状态统一降为 f32。
2. Pod/Tank 的虚函数不调用通用状态 Load/Save，不能统一读写 activated/flipped/exploded。
3. 旧根分级只迁移给实际父链根自身的 Pod；每个 Pod 的动作引用按自己的子树过滤。
4. XML 库级 bool 重载会写单词，但本体存档用 %d 写0/1。首轮集成发现了这项泛化错误，现由跨模块回归固定区别。
5. Runtime 不使用输入 planetRadius 恢复行星，也不把载入选择请求冒充实际当前选择。

## 使用接口

### 原生目录与船体

```rust
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use dr_core::sr1::{catalog::Catalog, ship::Ship};

    let catalog = Catalog::from_xml(&std::fs::read_to_string("PartList.xml")?)?;
    let ship = Ship::from_editor_xml(&std::fs::read_to_string("Ship.xml")?, &catalog)?;
    let saved = ship.to_editor_xml()?;
    Ok(())
}
```

Ship 的 from_element/from_xml/to_element/to_xml 对应 Runtime 内基础船体，不读写 Editor 的 DisconnectedParts；编辑器外层显式使用 from_editor_* / to_editor_*。

PartTree 提供原始实例切片、根、前序遍历及不可达项。连接使用实例索引，重复编号不自动合并。规范保存按本体根前序输出，不自动把诊断保留的孤立项加入运行树。

### Runtime 绑定与快照保存

```rust
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use dr_core::sr1::{
        catalog::Catalog,
        runtime::{Runtime, SaveSelection},
        solar::SolarSystem,
    };

    let catalog = Catalog::from_xml(&std::fs::read_to_string("PartList.xml")?)?;
    let system = SolarSystem::from_xml(&std::fs::read_to_string("SmolarSystem.xml")?)?;
    let runtime = Runtime::from_xml(&std::fs::read_to_string("Save.xml")?, &catalog)?;
    let bindings = runtime.bind_solar_system(&system)?;
    // 由调用方的实际运行上下文提供，不直接照抄载入请求。
    let selection = SaveSelection { ship_id: 1, pod_id: None };
    let saved = runtime.to_xml(&system, selection)?;
    Ok(())
}
```

- 默认星系来源名缺失时，调用方先解析默认资源并补充来源；本层不访问任意磁盘路径或伪造空星系。
- ShipSaveFlags 保留已核实的原始偏移命名，通过三个显式flag及根前序项数判断本体保存条件；不猜业务含义或默认值。
- Runtime保存接口序列化调用方已筛选的快照。对象生命周期和实际选择需由运行上下文提供。
- 挑战类型通过 from_*_with_kind 显式选择，不按某个属性猜派生类。覆盖 MoonTouchDown、大小写独立的 FuelUsed/fuelUsed、maxSpeed 和 LowFlyer 五字段。

### 独立检查工具

[inspect_sr1](../crates/dr-core/examples/inspect_sr1.rs) 读取真实资源，在内存中验证，不修改输入文件、不启动游戏：

```powershell
cargo run -p dr-core --release --locked --example inspect_sr1 -- ../SimpleRockets-PC-1.6.11/PartList.xml ../SimpleRockets-PC-1.6.11/DefaultShip.xml
cargo run -p dr-core --release --locked --example inspect_sr1 -- ../SimpleRockets-PC-1.6.11/PartList.xml ../SimpleRockets-PC-1.6.11/LevelAdvancedDocking.xml ../SimpleRockets-PC-1.6.11/SmolarSystem.xml
```

本轮已实际检查：27种部件完整目录；DefaultShip的3个部件、版本0→1和旧分级保存；AdvancedDocking的12个XML节点、2艘船与16星体上下文绑定。检查工具不代替实际选择或关卡上下文，不擅自保存Runtime。

## 验证与后续

2026-10-24 本轮统一实测：

| 检查 | 结果 |
| --- | --- |
| 全工作区 Release 测试 | 649项通过：核心212、跨模块6、编辑器431；相对119+431基线新增99项 |
| 工作区全部目标 Release Clippy | -D warnings通过 |
| 全项目 rustfmt | 通过 |
| 原版实际资源检查 | 27种部件、3部件旧船、12节点/2船与16星体绑定通过 |
| 实际Rust格式器对同版本x64 CRT | 19,137个独立样本全部一致，包含全指数、中点/相邻ULP、负零及非有限模式；不是只验证Python模型 |
| 证据文件链接 | 本机目标存在性检查通过 |

直接Rust差分使用本轮编译的核心库读取独立CRT金样；它与队友此前19,122项离线模型差分分列，均不等于游戏x86内逐值动态证明。

[跨模块回归](../crates/dr-core/tests/sr1_pipeline.rs) 使用仓库内合成夹具，不依赖本机游戏；模块内另有真实资源缩小fixture与CRT冻结金样。

```powershell
cargo test --workspace --release --locked
cargo clippy --workspace --all-targets --release --locked -- -D warnings
cargo fmt --all -- --check
```

各证据文档给出PC VA、文件偏移、文件身份及资源依据，不伪造游戏C#源码行号。同版本x64 CRT离线模型差分与x86静态证据分列；未枚举的极端浮点、locale、游戏临时缓冲截断等边界见标量证据。

新层保持独立可调用，编辑器默认入口未切换。后续可基于已有证据和测试增加原版导入选项、运行对象适配及物理阶段。
