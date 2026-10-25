# Difficult-Rocket Rust 编辑器

独立的 Bevy 2D 船体编辑器与可复用的 SR1 数据核心。复用原版部件、贴图和 XML；**完整复刻仍未完成，尚无移动端安装包**。

## 快速开始

先准备 Rust/Cargo、桌面构建工具与原版资源，见[构建先决条件](docs/development/README.md#构建先决条件)。在仓库根目录运行，资源默认来自相邻的 ../Difficult-Rocket/assets：

```powershell
# 打开范例；省略 --ship 创建带默认驾驶舱的新船体
cargo run --locked --release -p dr-editor -- --ship ../Difficult-Rocket/assets/ships/Test.xml
```

自定义资源使用 --assets 和 --catalog；加载错误明确退出，不悄悄改成空白文档。桌面运行需要 Vulkan 或 DirectX 12 驱动；资源布局、快捷键与 Windows 便携包见[操作指南](docs/guides/editor.md)。

## 主要能力

- 0.5 网格吸附与连续自由放置，连接吸附优先；旋转/镜像、多选、复制粘贴、撤销重做。
- 连接树/图、可调连接线、按分组实例编辑和歧义重号显式修复。
- 船体/部件目录、属性草稿、分级停靠与真实拖放、未保存确认及原子保存。
- 统一原生/egui 风格、触摸抬指单次提交、窄屏抽屉和合成界面音效；设备边界见[触控说明](docs/guides/editor.md#触摸与小屏)。
- 严格 XML 编辑入口与独立 dr_core::sr1 本体语义层并存，不隐式改变默认值或替换原有编辑路径。
- 简略 11 章、详细 23 章演示；详细含 51 个边界用例及 3 个连续火箭用例，共 54 项、114 项章节产物。[报告合同](docs/development/demo-contract.md)与[实际验收状态](README.md#当前状态)分开记录。

## 当前状态

2026-10-25 本轮实际验证：

| 检查 | 结果 |
| --- | --- |
| Rust 工作区 Release | 663 项通过：核心 223、跨模块 6、编辑器 434；比 649 基线新增 14 项 |
| Python 离线回归 | 215 项通过；比 203 基线新增 12 项 |
| 质量与构建 | 严格 Release Clippy、Release build、严格全工作区 Rustdoc、rustfmt 全部通过 |
| 本体资源只读复验 | 27 种部件、3 部件旧船、12 节点/2 船及 16 星体绑定通过，四份输入 SHA256 不变 |

本轮未重跑 GUI、系统输入、真实媒体或便携包启动；内部回归不等于物理设备验收。物理推进、地形生成、运行对象生命周期和移动端交付仍未实现；严格编辑器未接入自动镜像，独立 SR1 静态配对不等于该交互功能。历史具体范围只查[对应记录](docs/archive/validation-history.md)，命令只查[验证指南](docs/validation/README.md)。

## 文档入口

| 目的 | 唯一入口 |
| --- | --- |
| 运行、编辑、触控与便携包 | [操作指南](docs/guides/editor.md) |
| 构建、模块职责与严格核心兼容 | [开发指南](docs/development/README.md) |
| 使用本体数据接口、查 PC 证据 | [SR1 数据层](docs/development/sr1-data.md) |
| 维护章节、火箭拓扑与报告字段 | [演示合同](docs/development/demo-contract.md) |
| 重跑测试、窗口和媒体检查 | [验证指南](docs/validation/README.md) |
| 核对过去构建的通过、失败与证据 | [单一历史记录](docs/archive/validation-history.md) |

操作、开发、复现命令各维护一份；当前结果留在本页，历史不冒充当前，未排期建议不另建文档。

## 代码与协作

- dr-core 提供严格数据/XML、几何、连接/拓扑、编辑事务，以及独立 SR1 原生语义层，不依赖 Bevy 或 GPU。
- dr-editor 负责 Bevy 应用、输入、文档状态和界面；源码职责与测试选择集中在[开发导航](docs/development/README.md)。

依赖以 [Cargo.lock](Cargo.lock) 为准，项目声明 MPL-2.0；协作与提交约定见 [AGENTS.md](AGENTS.md)。先运行 cargo test --workspace --release --locked，再按改动选择[其余验收](docs/validation/README.md)。target 下的截图、报告和日志不入库，历史产物不代表当前构建。
