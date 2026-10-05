# DRRS 与 dr-core 功能覆盖核对

核对日期：2026-10-05。源码基线：Difficult-Rocket `ccc0973`，dr-rs `5feb86a`。以下原审计按该基线保留；后续实现状态见本节更新。原审计中的源码行号对应旧基线。

## 按用户编号的实现状态（2026-10-05）

| 编号 | 项目 | 当前状态 |
| --- | --- | --- |
| 1 | 部件目录数据 | 已补齐可选物理字段及 Damage/RCS/Solar/Lander 规格；不再静默忽略实际目录数据。 |
| 2 | 目录索引 | 已移除易过期索引，公开列表修改立即反映到查询，重复类型 ID 取当前首项。 |
| 3 | 目录 XML 输出 | 已提供 `catalog_to_xml/save_catalog`，安全替换、保留顺序与显式零坐标；原版 27 类、793 个原始元素/属性往返通过。 |
| 4 | 旧 SaveStatus 策略 | **按用户要求本轮不改**，不复制原版会省略已启用镜像/激活属性的行为。 |
| 5 | 质量统计 | 已新增 `ShipScope::Main/All`：Main 只统计主组，All 包含主组和全部断开组；旧 total_mass 保持 All，HUD 同时显示两者。 |
| 6 | 元数据与范围 | 已提供船体名称/描述及可撤销、可持久化修改；物理和 PNG 图像范围查询下沉核心，编辑器复用像素锚点几何。物理单位与像素单位明确区分，不照搬旧图像范围的原点扩张和特殊角度映射。 |

此外，连接图和生成森林已进入 core，树换父与精确断边共用原子历史。对接的 dock 字段是父/子之一的角色标记，不凭三个字段名制造第三个节点或自环；历史独立第三引用仍保留，不自动改写。

上表不代表 Python 扩展或平台工具已迁移，也不意味着长期完整复刻目标已完成。连接树/图窗口的最新验证以 `editor-progress.md` 的新增记录为准；下面保留最初审计发现，不能再把其中“未实现”当作当前状态。

## 原审计结论与边界

**原审计时 dr-core 没有覆盖 DRRS 的所有功能，也不是原 Python 扩展的可直接替代实现。** 已实现并强化了船体 XML、编辑事务、分组引用和编辑几何，但部件目录数据、目录编辑、部分保存及查询合同仍有明确缺口。

- 下文 `D/` 指相邻仓库 `Difficult-Rocket/mods/dr_game/Difficult_Rocket_rs/src/src/`，`C/` 指本仓库 `crates/dr-core/src/`，`E/` 指 `crates/dr-editor/src/`。
- 先检查 `D/lib.rs:30-46` 实际注册的 7 个函数和 7 个类，再检查对应 Rust 数据实现及 Python 调用方；不把类型提示、注释或未启用模块当成已交付功能。
- 既有公开字段、迭代器和组合 API 能完成的查询，不因缺少同名 Python getter 就判为业务功能缺失；但不能由此宣称 Python API 兼容。
- “船体 XML 字段往返通过”不等于“PartList 的所有静态字段均保留”，更不等于 DRRS 所有功能等价。输入合法性、缺省值和错误返回也不能只凭成功样本推定一致。

## 已具备的主要核心能力

| 功能 | dr-core 中的实现与边界 |
| --- | --- |
| 船体读取、保存与往返 | `C/io.rs` 的 `load_ship`、`ship_from_xml`、`ship_to_xml`、`save_ship`；支持主组、断开组及普通/对接连接，保存使用原子文件替换。格式和错误处理不与旧入口逐字等价。 |
| 部件实例和运行状态 | `C/model.rs` 的 `Part`、`PodState`、分级、燃料来源、降落伞和着陆架状态；注意着陆架实例状态不等于着陆架目录规格。 |
| 部件目录基础读取与查找 | 名称、描述、贴图、质量、尺寸、分类、编辑限制、Tank/Engine 规格、连接点及 Shape 已建模；不是完整目录模型。 |
| 分组、实例及连接查询 | `Ship::groups/all_parts/keyed_parts`、`PartKey`、`Connection` 和公开集合支持相应组合查询；重复实例的编辑比原来的全局编号方式更明确。 |
| 编辑几何 | `C/geometry.rs`、`C/connections.rs` 提供真实轮廓命中、框选、碰撞、连接面、占用和吸附；不能据此推断原来的包围盒单位与便捷接口也已对齐。 |
| 编辑事务 | `C/edit.rs` 及子模块已提供撤销重做、原子批量操作、属性/分级、多选、复制粘贴和重复编号引用修复。 |

## 明确缺口与行为差异

| 项目 | 核对结果 | 主要源码依据 |
| --- | --- | --- |
| 部件静态物理与限制字段 | **未完整保留**：`friction`、`canExplode`、`coverHeight`、`sandboxOnly`、`drag`、`buoyancy`、`Damage`。这些字段在原版实际目录中存在；当前目录反序列化会忽略它们。 | `D/sr1_parse/model.rs:46-88`、`D/sr1_parse/raw/part_list.rs:227-283`；对照 `C/model.rs:121-140`、`C/io.rs:37-72`。 |
| RCS / Solar / Lander 静态规格 | **未建模**：RCS 推力/消耗/尺寸，太阳能充电率，以及着陆架角度、长度、速度、宽度等参数。当前只有 TankSpec 和 EngineSpec。 | `D/sr1_parse/model.rs:14-44`；对照 `C/model.rs:188-205`。 |
| PartList 反向输出 | 原版有 `to_raw_part_list`、`to_raw_part_type` 及可序列化的目录模型；core 没有对应的目录 XML 输出入口，`RawPartList` 仅派生 Deserialize。**船体保存不能替代目录保存。** | `D/sr1_parse/convert.rs:4-15`、`D/sr1_parse/raw/part_list.rs`；`C/io.rs:30-37`、`C/lib.rs:14`。 |
| 目录增删/重排后的索引 | **已复现错误**：原版 `insert_part` 后线性查询仍正确；core 的公开 `types` 修改后，私有 `index` 不会更新。向首部插入类型后，新 ID 查不到，旧 ID 可能查到另一类型。目前只能重新构造 PartCatalog 来重建索引。 | `D/sr1_parse/model.rs:109-117`；`C/model.rs:554-580`。 |
| 重复的目录类型 ID | **查询语义不同**：原版返回首项；core 构造索引时保留末项。这里说的是 PartList 类型 ID，不是已经专门处理过的船体部件实例 ID。 | 同上；对照探针以两个同 ID、质量 10/20 的目录项复现。 |
| 保存策略 | 原版暴露 `SaveStatus_rs(save_default)`；core 只有固定策略的 `ship_to_xml/save_ship`。探针确认该选项会改变输出，不能说已等价覆盖。原版 `save_default=true` 会省略已启用的镜像/激活属性，语义本身可疑，**不应为复刻而直接复制潜在数据丢失行为**。 | `D/python/data/save_status.rs:12-24`、`D/sr1_parse/ship_io.rs:26-46`；`C/io.rs:899-935`。 |
| 船体质量统计 | **口径不同**：DRRS 仅统计主组，core 还统计断开组。主组 10、断开组 20 的同一船体分别返回 10 和 30；需要明确主船体质量与全文件质量的合同，不能默认两者相同。 | `D/sr1_parse/model.rs:407-411`；`C/model.rs:545-550`。 |
| 船体级名称和描述 | 原版有运行时 `SR1Ship.name/description/renamed`，并向 Python 暴露；core 的 Ship 未承载这层元数据。它们不是旧 XML 中已持久化的船体字段，不能误报为本次 XML 往返丢失；Pod 名称也不是船体元数据。 | `D/sr1_parse/model.rs:376-395`、`D/sr1_parse/raw/ship.rs:310-314`；`C/model.rs:360-367`。 |
| 图像范围、部件框与角度便捷查询 | 原版有 `img_pos/get_part_box/get_part_box_by_type/angle_r`。core 提供几何基础，但没有直接对应的完整船体范围 API；真实贴图边界和视图适配主要在 editor。原版还含特殊角度映射，不能仅以同名数值或视图截图断言兼容。 | `D/python/data/ship.rs:58-59,106-108`、`D/python/data/part_data.rs:40-56`、`D/sr1_parse/util.rs:7-15`；`C/geometry.rs:229`、`E/view.rs:27-81`。 |
| 快速格式判定及读取宽容度 | 原 `assert_ship` 只检查 Ship 开始标签和三个属性，不是完整解析；core 的完整解析不能视为完全相同的接口合同。实测样本中原版还接受一个根元素外有文本的文件，core 拒绝，这属于严格性差异，不应直接削弱校验以迎合原版。 | `D/python/api.rs:33-79`、`D/sr1_parse/raw/ship.rs:323-337`；`C/io.rs:565` 及根节点校验。 |

### 实际目录中的缺失字段不是假设

原版目录共 27 类部件。本次统计到：

| 字段或子节点 | 显式出现的部件类型数 |
| --- | ---: |
| friction | 2 |
| canExplode | 3 |
| coverHeight | 1 |
| sandboxOnly | 4 |
| drag | 1 |
| buoyancy | 12 |
| Damage | 9 |
| Rcs / Solar / Lander | 各 1 |

修改这些字段后，原版数据模型能反映摩擦、Damage、RCS、Solar 和 Lander 的差异；当前 core 读取出的 **27 个 PartType 全部与修改前相等**。具体示例：RCS power 从 1 改为 2、Solar chargeRate 从 2 改为 3、Lander maxAngle 从 140 改为 141，原版保留，core 无法表示。

## Python 与平台层，以及不应误算的实验代码

| 原版能力 | 当前判断 |
| --- | --- |
| `SR1Ship_rs`、`SR1PartList_rs` 等 Python 类与扩展版本接口 | dr-core 是 Rust 库，未提供 PyO3 兼容层；基础读写和数据查询可在 Rust 中完成，但不能直接替换原 Python import。 |
| `part_list_read_test/read_ship_test/load_and_save_test` | 属于诊断入口；core 有加载/序列化接口及 verify_ships 等示例，不要求为相同诊断行为再增加一层无用转发。 |
| `Console_rs` 的 stdin 线程、命令队列、提示与停止 | `D/python/console.rs:17-72` 是实际实现，且被 `mods/dr_game/console.py` 使用；core 无此模块。若要兼容应属于应用或适配层。 |
| Windows 任务栏进度条 | `D/platform/win/mod.rs:53-82` 与 `D/renders.rs:23-33` 有实际实现，菜单中有调用；core 未实现。平台 UI 不宜强行放进独立数据核心。 |
| OpenGL 接口 | 初始化和尺寸更新存在，但 `D/renders/opengl.rs:42-44` 的 on_draw 为空；不能把它当成原版已有完整渲染器。当前 Bevy 渲染属于 dr-editor，不是兼容该接口。 |
| Wgpu `render_hack` | `D/lib.rs:36` 和 `D/renders.rs:1-2,11-21` 中未启用；类型提示或保留的源文件不等于实际可用导出。 |
| `EditorArea` | 有命中检测原型，但 Python 类注册被注释；core 已提供编辑几何，不应为名称一致而恢复原型。 |
| Rapier 刚体和广泛碰撞体枚举 | `D/dr_physics/rigid.rs` 主要是数据结构，未发现对应仿真步进实现；不能宣称原版已有完整飞行物理而据此判定 core 漏迁整个模拟器。 |
| `map_ptype_textures` | 只见于 `Difficult_Rocket_rs/__init__.py` 的 TYPE_CHECKING 声明，未注册为当前扩展函数；不计为已交付功能。 |

## 本次实际验证

1. `cargo test -p dr-core --locked`：**66 项通过**，无失败；不是全仓库测试，也未运行窗口或输入法专项。
2. `cargo run --locked -p dr-core --example verify_ships -- ../Difficult-Rocket/assets/ships`：**185 个船体、125,856 个部件**的模型及原始 XML 元素/属性往返通过；另有 **5 个输入被拒绝**，命令如实返回退出码 1，不能写成全部样本命令成功。
3. 使用临时对照探针，直接通过路径引用原版 `sr1_parse` 和基础数学 Rust 源码，同时链接当前 dr-core；没有复制替代实现来冒充原版。验证目录字段丢失、质量口径、SaveStatus 行为、目录插入索引和重复目录 ID 差异。
4. 对 190 个 XML 做读取合同对照：双方接受 185 个，双方拒绝 4 个，仅原版接受 1 个（`X Type 2.xml`，core 因根元素外文本拒绝），仅 core 接受 0 个。
5. 探针及日志保存在 `target/drrs-core-audit/`：`src/main.rs`、`catalog-changes.json`、`probe.log`、`ship-roundtrip.log`。它们是本机审计产物，不是已修复差异的回归测试，也不是需要发布的产品代码。
6. 本次未加载 Python 扩展或启动 OpenGL、控制台线程、任务栏和编辑器窗口；这些条目是源码核对结论，不冒充平台实测。

## 建议的收敛顺序

1. 先补齐目录数据保留及对应反例回归，避免“能加载”掩盖静默忽略字段。
2. 修正目录修改与索引一致性，明确重复类型 ID 的处理方式。
3. 定义目录 XML 输出、保存策略、船体元数据和可复用范围查询的合同；不要盲目复制原版已有问题。
4. 明确主组/全文件质量及读取严格性等兼容决策，再补行为对照测试。
5. Python、控制台和 Windows 平台功能按适配层边界处理；未启用实验不作为必须照搬的功能。

这份清单完成的是核对，不是上述缺口的修复。完整复刻目标仍未完成。
