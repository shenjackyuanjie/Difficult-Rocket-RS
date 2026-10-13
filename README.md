# Difficult-Rocket Rust 编辑器

独立的 Bevy 2D 船体编辑器与可复用的 SR1 数据核心。复用原版部件、贴图和 XML；**完整复刻仍未完成，尚无移动端安装包**。

## 快速开始

在本仓库根目录运行，资源默认来自相邻的 `../Difficult-Rocket/assets`：

```powershell
# 打开范例；省略 --ship 创建带默认驾驶舱的新船体
cargo run --locked --release -p dr-editor -- --ship ../Difficult-Rocket/assets/ships/Test.xml
```

自定义资源使用 `--assets <资源目录>`、`--catalog <PartList.xml>`。资源目录需包含原部件贴图与中文字体；加载错误会明确退出，不会悄悄改成空白文档。桌面运行需要支持 Vulkan 或 DirectX 12 的显卡驱动。

Windows 便携 ZIP 的生成与使用见[操作指南](docs/editor-usage.md#windows-便携包)。历史包不代表当前源码已重新打包。

## 主要功能

- 部件放置、连续拖动、旋转/镜像、多选、复制粘贴、撤销重做。
- 辅助吸附与自由连接；连接树/图、连接线显示设置、歧义重复编号的显式修复。
- 船体目录、四组独立折叠的部件列表、属性/燃料/分级草稿与未保存确认。
- 灰黑底与青柠强调的统一原生/egui 风格；鼠标按下触发、触摸抬指单次提交，窄屏自动收起侧栏。
- 严格 XML 读写、原子保存、按分组实例编辑。未知扩展字段和歧义引用明确拒绝，不猜测或静默丢失数据。

鼠标左键选择/拖动，中键默认框选，滚轮缩放；`F` 适配整船，`F1` 打开完整帮助。更多操作见[操作指南](docs/editor-usage.md)，触摸与设备边界见[触控说明](docs/mobile-support.md)。

## 文档导航

| 文档 | 内容 |
| --- | --- |
| [操作指南](docs/editor-usage.md) | 快捷键、目录、连接、文件与 Windows 便携包 |
| [验证指南](docs/validation.md) | Rust/Python/Release、窗口演示、窄窗、系统输入与视频命令 |
| [当前进度](docs/editor-progress.md) | 当前能力、未完成事项与最近实际验收 |
| [历史验收](docs/validation-history.md) | 按日期冻结的通过、失败及本地证据，不代表当前构建 |
| [核心覆盖核对](docs/drrs-core-coverage.md) | dr-core API 与 DRRS 的兼容决策和缺口 |
| [演示评审建议](docs/demo-review-suggestions.md) | 已采纳项与尚未实施的回放候选 |

`target/` 中的截图、报告与日志是未入库的本地验收产物；历史链接可能已失效，不用旧结果代替本次运行。

## 代码结构

- `crates/dr-core`：SR1 模型、XML、几何、连接/拓扑与编辑事务；不依赖 Bevy 或 GPU。
- `crates/dr-editor/src/main.rs`：资源、插件与系统接线；`document` 管理文档/保存点，`window` 管理启动与截图策略。
- 编辑流程：`canvas_input` / `mobile_input` 处理输入，`selection` / `placement` 共用核心事务，`view` / `render` 负责导航与显示。
- 界面：`files`、`panels`、`properties`、`topology_ui`；`native_buttons` / `egui_ui` 复用原生控件与共享样式。
- 验收：各模块旁的测试、仅测试编译的 `test_support`、`demo` / `*_smoke` 及 `scripts`；不把测试夹具放进普通运行路径。

依赖锁定于 `Cargo.lock`：Bevy 0.20.0、bevy_egui 0.43.1、quick-xml 0.42.0、serde_json 1.0.154。原生按钮直接使用 `ui_widgets` / picking / `Activate`，没有老版本兼容层。

开发时先运行 `cargo test --workspace --release --locked`；完整检查与验收边界见[验证指南](docs/validation.md)。协作与提交约定见 [AGENTS.md](AGENTS.md)，许可证为 MPL-2.0。
