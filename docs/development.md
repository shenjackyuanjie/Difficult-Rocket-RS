# 开发职责与契约导航

[返回 README](../README.md) · [操作指南](editor-usage.md) · [验证命令](validation.md) · [当前验收](editor-progress.md)

本页用于确定“改哪一层、保留什么合同、选哪些测试”；操作步骤和完整验收命令沿用已有指南。

## 构建先决条件

- 安装 Rust/Cargo，并提供 rustfmt、Clippy 组件；依赖以 [Cargo.lock](../Cargo.lock) 为准，构建使用 `--locked`。[工作区清单](../Cargo.toml) 使用 edition 2024，当前锁定的 Bevy 0.20.0 自身声明 Rust 1.97.1；项目尚未声明或单独验收完整依赖图的 MSRV，不能仅按 edition 选择工具链。本轮实际验证使用 rustc/Cargo 1.99.0。
- Windows 默认 MSVC 工具链需要 Visual Studio Build Tools 的 C++ 构建工具与 Windows SDK；桌面窗口还需要 Vulkan 或 DirectX 12 驱动。无窗口核心测试不需要 Bevy、GPU 或运行中的游戏。
- 编辑器默认资源位于相邻的 `../Difficult-Rocket/assets`，至少准备 `builtin/PartList.xml`、`textures/parts/*.png` 和 `fonts/HarmonyOS_Sans/HarmonyOS_Sans_SC/HarmonyOS_Sans_SC_Regular.ttf`；自定义路径使用 `--assets` / `--catalog`。仅构建 Rust 不等于已准备好运行资源。
- Python 验证/打包脚本使用 uv 与 Python 3.12；录制另需 Windows、FFmpeg/ffprobe 与微软雅黑。具体运行、打包和视频前提见[操作指南](editor-usage.md#windows-便携包)与[验证指南](validation.md#录制无配音-1080p-视频)。

## 两套数据层：按用途选择，不合并默认值

| 用途 | 入口与职责 | 必须保留的边界 |
| --- | --- | --- |
| 安全编辑已有文档 | [model](../crates/dr-core/src/model.rs)、[io](../crates/dr-core/src/io.rs)、[edit](../crates/dr-core/src/edit.rs)；公开重导出见 [lib](../crates/dr-core/src/lib.rs) | 严格 XML、未知内容拒绝、按 PartKey 定位组内实例、原子事务与撤销重做；不猜歧义引用、不静默丢数据 |
| 还原 SR1 PC 本体数据语义 | [sr1 模块](../crates/dr-core/src/sr1/mod.rs)，使用接口见[SR1 说明](sr1-data-restoration.md#使用接口) | 独立的目录、部件、船体、分级、Runtime 与星系类型；按证据保留原生数值/布尔、默认值及规范保存行为，不承担完整物理或 XML 词法保真 |

编辑器的 [load_document](../crates/dr-editor/src/document.rs) 仍调用严格的 load_catalog/load_ship；文件操作也沿用[严格保存入口](../crates/dr-editor/src/files.rs)。新增原生导入功能时应显式设计转换与失败处理，不能用 sr1 的宽松数值读取或不同默认值替换严格入口。原生连接点的静态镜像配对与编辑器的自动镜像选择也不是同一合同，见[核心覆盖边界](drrs-core-coverage.md#明确差异与未完成边界)。

## 按改动选择代码入口

| 改动 | 首先查看 | 检查重点 |
| --- | --- | --- |
| XML、目录字段、实例引用 | [io](../crates/dr-core/src/io.rs)、[model](../crates/dr-core/src/model.rs) | 未知字段/坏输入明确失败；分组、重号及往返不丢数据 |
| 几何、连接、拓扑、自动分级 | [geometry](../crates/dr-core/src/geometry.rs)、[connections](../crates/dr-core/src/connections.rs)、[topology](../crates/dr-core/src/topology.rs)、[staging](../crates/dr-core/src/staging.rs) | 网格/连接阈值、碰撞边界、树与图差异、分级生成前置条件保持独立 |
| 编辑命令、历史与保存点 | [edit](../crates/dr-core/src/edit.rs)、[document](../crates/dr-editor/src/document.rs)、[files](../crates/dr-editor/src/files.rs) | 一次提交对应一次历史；失败/no-op 不清重做，取消预览不改文档；保存成功后才更新保存点 |
| 鼠标/触摸、选择与放置 | [canvas_input](../crates/dr-editor/src/canvas_input.rs)、[mobile_input](../crates/dr-editor/src/mobile_input.rs)、[selection](../crates/dr-editor/src/selection.rs)、[placement](../crates/dr-editor/src/placement.rs) | 保持按下/移动/释放边沿、UI 归属、失焦取消及单次提交语义 |
| 属性草稿与界面 | [properties](../crates/dr-editor/src/properties.rs)、[panels](../crates/dr-editor/src/panels.rs)、[egui_ui](../crates/dr-editor/src/egui_ui.rs)、[native_buttons](../crates/dr-editor/src/native_buttons.rs) | 草稿与文档历史分离；模态/停靠、目录共存与快捷键仲裁不能互相替代 |
| 系统接线、相机与绘制 | [main](../crates/dr-editor/src/main.rs)、[view](../crates/dr-editor/src/view.rs)、[render](../crates/dr-editor/src/render.rs) | 系统顺序和同帧同步；内存/主 World 断言不等于 GPU 原图正确 |
| 原生数据语义 | [sr1 模块](../crates/dr-core/src/sr1/mod.rs)、[证据导航](sr1-data-restoration.md#功能分块) | 依据对应 PC 证据与离线回归；不凭字段名统一不同精度、类分派或加载/保存路径 |

### 输入到显示的顺序也是合同

[main 的系统接线](../crates/dr-editor/src/main.rs) 将 Update 链显式排序：演示/专项注入与指针恢复 → 工具栏布局、触摸/UI 归属和操作仲裁 → 文件动作与预览取消 → 视图控制、鼠标/键盘编辑 → render::sync、预览与连接绘制。PostUpdate 的交互校验位于 Transform 传播之后；演示指针隔离位于 egui pass 结束之后。

重排或抽取系统时保留这些依赖，并核对同帧输入、松手末段位移、UI 隔离与最终显示；不能只确认函数各自仍能编译。测试夹具由[仅测试使用的 test_support](../crates/dr-editor/src/test_support.rs)复用，不放进正常运行路径。

## 选择最小验证，再做统一验收

| 改动范围 | 优先验证 |
| --- | --- |
| 严格数据/事务/几何 | dr-core 对应模块旁回归；输入拒绝、失败/no-op、撤销重做与 XML 往返都要保留 |
| sr1 数据处理 | sr1 对应模块回归及[跨模块 sr1_pipeline](../crates/dr-core/tests/sr1_pipeline.rs)；合成夹具可离线运行，真实资源探针另列 |
| 编辑器输入/草稿/系统接线 | dr-editor 相邻测试及对应 demo / smoke；涉及布局或渲染时还需串行窗口验证和查看原图 |
| Python 编排/打包/视频 | 对应脚本的 unittest，检查失败路径、超时、产物不覆盖和进程所有权；模拟 subprocess 不等于真实媒体运行 |
| 公开 API 文档或链接 | 严格 Rustdoc，加上所改 Markdown 的本地路径与锚点检查 |

完整 Rust/Python、格式、Clippy、Release 与严格 Rustdoc 命令统一见[无窗口与 Release 检查](validation.md#无窗口与-release-检查)。窗口、系统输入和媒体检查按改动选择，不把历史通过或新增但未运行的测试写成本次验证。

## 脚本与证据边界

- [demo_editor](../scripts/demo_editor.py) 驱动现有内部输入回放；[rocket_workflow_report](../scripts/rocket_workflow_report.py) 校验火箭报告。章节数、用例、字段和产物有对应测试合同，不能仅改文案或放宽断言。
- [editor_session](../scripts/editor_session.py) 管理独立 EXE 副本及 SHA-256 身份 JSON；PID/HWND 采样由[窄窗验证](../scripts/validate_narrow_windows.py)、[系统输入检查](../scripts/check_native_input.py)和[视频工具](../scripts/render_demo_video.py)各自负责，[demo_editor](../scripts/demo_editor.py) 提供 stop_owned_process 清理助手。GUI 自测串行运行，只结束本工具拥有的进程，不枚举关闭其他编辑器。
- [视频工具](../scripts/render_demo_video.py) 的录制等待和单条媒体命令预算相互独立，见[视频说明](validation.md#录制无配音-1080p-视频)；[打包工具](../scripts/package_editor.py) 的文件预检不代替解压启动验收。
- 每次使用新输出目录，保留旧报告身份。`target/` 是未入库的本地证据，不补造历史缺失产物；PC 证据、离线 CRT 对照、内部回放、系统输入和物理设备验收分别记录。最新无窗口结果见[当前进度](editor-progress.md)，冻结构建结果见[历史验收](validation-history.md)。
