# DRRS 与 dr-core 功能覆盖核对

此页维护当前 API 与兼容决策，不把 2026-10-05 修复前的审计结论当成当前缺陷。完整复刻仍未完成；运行结果见[编辑器进度](editor-progress.md)，验证入口见[验证指南](validation.md)。

## 已具备的核心能力

| 能力 | 当前合同与源码 |
| --- | --- |
| 船体 / 目录 XML | `catalog_from_xml/load_catalog`、`catalog_to_xml/save_catalog` 及对应 ship API；保留静态规格、运行状态、两类连接与分级。[io](../crates/dr-core/src/io.rs)、[catalog](../crates/dr-core/src/io/catalog.rs) |
| 目录查询 | `PartCatalog.types` 可增删/重排/改名；`get` 查当前列表，同类型 ID 返回首项，无易失效旁路索引。[model](../crates/dr-core/src/model.rs) |
| 静态规格 | friction、canExplode、coverHeight、sandboxOnly、drag、buoyancy、Damage、RCS / Solar / Lander，以及连接点/Shape 元数据已建模；可选值区别缺省与零。[model](../crates/dr-core/src/model.rs)、[目录回归](../crates/dr-core/src/io/catalog/tests.rs) |
| 船体元数据 / 质量 | `Ship.name/description` 不等于 Pod 名称；`SetMetadata` 纳入事务/XML。`mass(...Main)` 只主组，`All` 和 `total_mass` 包含断开组。[model](../crates/dr-core/src/model.rs)、[edit](../crates/dr-core/src/edit.rs) |
| 范围查询 | `part_bounds/ship_bounds` 使用 Ship 单位；`image_corners/image_bounds` 使用调用方原图像素，保留奇数锚点和镜像；空船返回 None。[bounds](../crates/dr-core/src/bounds.rs) |
| 分组实例 / 连接 | `groups/all_parts/keyed_parts`、`PartKey` 定位组/ID/出现次数；相同 ID 不跨组误改。占用、吸附、对接与无向部件对唯一性有事务校验。[connections](../crates/dr-core/src/connections.rs) |
| 几何与拓扑 | 真实轮廓、框选、旋转/镜像、严格 5% 重叠，生成森林不丢额外边；`reachable/subtree` 和原子换父/断边。[geometry](../crates/dr-core/src/geometry.rs)、[topology](../crates/dr-core/src/topology.rs) |
| 编辑历史 | 原子批量、属性/分级、多选/剪贴板、重复编号修复；失败/no-op 不破坏重做，单快照交换与内存预算。[edit](../crates/dr-core/src/edit.rs) |

## 明确差异与未完成边界

- **保存策略**：未照搬旧 `SaveStatus_rs(save_default)`；原策略可能省略已启用镜像/激活属性，不能为了名称兼容复制潜在数据丢失。当前 API 使用固定、安全保存合同。
- **读取严格性**：完整解析和原 `assert_ship` 的快速标签检查不是同一合同。未知字段、异常根或尾随内容明确拒绝；不为了接受历史坏输入削弱检查。
- **重复实例引用**：已有同组重号连接/分级若歧义，保留读取数据但拒绝相关编辑，要求显式修复；不自动猜测归属。
- **连接点镜像元数据**：字段保留，但原版自动选择镜像的可执行行为尚未核实；当前预览由用户显式镜像，不仅凭字段名推断自动行为。
- **Python 接口**：没有 PyO3 的 `SR1Ship_rs` / `SR1PartList_rs` 或扩展版本兼容层。Rust 库能力不等于可直接替换旧 Python import。
- **应用 / 平台**：旧 Console stdin/队列和 Windows 任务栏进度属于实际应用层能力，未放进独立核心。Android / iOS / Web 宿主、资源/存储及系统文件选择另行实现；当前不新增平台工程。

## 不重复迁移的诊断与实验

原 `part_list_read_test/read_ship_test/load_and_save_test` 可由公开 API 和现有 verify 示例复现，不加无用转发。未注册的 EditorArea 原型、未启用的 Wgpu render_hack、只有 TYPE_CHECKING 声明的 map_ptype_textures 不算已交付接口；OpenGL 空 on_draw 与物理结构声明也不证明原版具有完整渲染/仿真。

2026-10-05 原版路径对照为 `Difficult-Rocket/mods/dr_game/Difficult_Rocket_rs/src/src`，临时探针在本机 `target/drrs-core-audit/`；该探针及 66 项 core 的当时通过仅是历史审计，不是产品代码或当前验收。原审计中目录字段/输出、索引、质量口径、元数据与范围接口的缺口已收敛为上面的现行合同；后续只在新增证据支持时修改边界，不复刻有数据丢失风险的旧策略。
