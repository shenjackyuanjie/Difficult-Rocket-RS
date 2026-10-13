# 编辑器当前进度与验收

完整复刻仍在进行，不把独立任务完成视为全项目完成。历史日期、旧测试计数与失败集中在[历史验收](validation-history.md)，操作见[操作指南](editor-usage.md)，复现命令见[验证指南](validation.md)。

## 当前能力

| 功能块 | 现行合同 |
| --- | --- |
| XML / 核心 | 严格读写船体和 PartList，保留静态/运行/分级字段；未知内容明确拒绝，同目录临时文件原子保存 |
| 事务 / 历史 | 放置/吸附/整组变换/删除/属性原子提交，失败/no-op 保留重做；256 步/64 MiB 快照预算 |
| 分组 / 重号 | PartKey 定位具体实例，跨组合并只重映射相关冲突及引用；歧义已有显式修复，不自动猜测 |
| 几何 / 连接 | 任意角度与镜像、真实 Shape/轮子、严格 5% 重叠和 0.35 吸附；自由模式跳过几何/占用但保留端点合法性及部件对唯一性 |
| 输入 / 显示 | 连续抓取偏移、松手末段位移、同帧视觉同步；鼠标按下触发，触摸抬指一次，UI/模态隔离 |
| 界面 | 四组独立目录、分级停靠/部件图标/悬停船上联动、普通属性与重号修复 Modal；灰黑/青柠共享样式 |
| 小屏 | 最小 320×320，单侧抽屉、44px 触控入口，按实际画布适配；物理触屏与平台发布另行验证 |
| 演示 / 工具 | brief 10 章；detailed 19 章/45 例/95 证据，独立进程身份及源哈希保护；既有视频/便携包脚本 |

原生按钮使用 Bevy 0.20 的 ui_widgets / picking / Activate，无旧 Interaction 兼容层。依赖版本以 Cargo.lock 为准。DRRS 的具体 API 合同与差异见[核心覆盖核对](drrs-core-coverage.md)。

## 2026-10-13 分级停靠、图标与悬停联动

按实际打开的 SR sandbox / Builder 分级侧栏与部件图标组织方式调整；复用现有属性草稿、Action/历史事务、目录贴图注册与等比绘制，不增加素材体系或平台工程。普通属性/重号修复仍用 Modal。README 保持 51 行；上一阶段结构/文档清理已移入[历史记录](validation-history.md#2026-10-13-结构与文档清理)，不混用旧构建验收。

- 宽屏/短横屏左侧停靠，宽屏最多 480px；窄竖屏底部停靠，两侧目录暂时收起，自动显示/适配船体，不用整屏遮罩。
- 固定部件图标/名称、移动、移除三列，长名截断，动作控件不换行、至少 44px 高；短横屏正文可完整容纳一行动作，应用/取消固定在正文外。
- 悬停级标题或动作图标/名称时，本组相应部件同步显示青柠填充、轮廓和 ID，移开恢复已选焦点；不改船体/选区/历史，不猜缺失或歧义引用。点击/修改继续保留目标联动。
- UI 卡片从第 1 级显示，XML 执行索引仍从 0 起。分级编辑期间隐藏会遮挡船底目标的演示横幅；驱动复用真实滚动正文，不猜固定位置。

### 本次最终构建的实际验收

| 检查 | 实际结果 / 本地证据 |
| --- | --- |
| Rust | `cargo test --workspace --release --locked`：383 项（core 104 / editor 279）通过；本次进程 TEMP/TMP 使用工作区目录；[日志](../target/staging-ui-20261013/rust-current.log) |
| Python | 151 项通过；[日志](../target/staging-ui-20261013/python-hover-final.log) |
| 格式 / Release | fmt check、全目标 Release Clippy `-D warnings`、workspace Release build 均通过；[fmt](../target/staging-ui-20261013/fmt-current.log)、[Clippy](../target/staging-ui-20261013/clippy-current.log)、[build](../target/staging-ui-20261013/build-current.log) |
| 完整窗口演示 | DX12 40ms，19 章/45 例/95 个唯一非空证据，退出 0；[报告](../target/demo-staging-hover-final-20261013/demo-report.json)、[日志](../target/staging-ui-20261013/demo-hover-final.log) |
| 三窄窗分级 | 390×844、640×360、320×640 串行，均退出 0；各自真实滚动 64 级/1,024 动作，验证仅悬停目标可见及移开恢复，再修改末项/应用/撤销重做/XML；Win32、Bevy/DPI、原始 PNG 三重核对通过，三张草稿原图均已查看；[报告](../target/narrow-staging-hover-final-20261013/report.json) |
| 三窄窗启动 | 同三种尺寸串行，均退出 0，三重尺寸通过，原始 PNG 均已查看；仍不代表初始相机能完整取景；[报告](../target/narrow-staging-startup-final-20261013/report.json) |
| 布局 / 视觉 | 分级面板与船体矩形不重叠，悬停和修改目标完整处于船体区；桌面草稿原图已查看，图标/控制列单行、船底标记不再被横幅遮住；[几何与悬停报告](../target/demo-staging-hover-final-20261013/editor-staging-performance.json)、[修改状态截图](../target/demo-staging-hover-final-20261013/editor-staging-draft.png) |
| XML / 不变性 | 目录 27 类/793 字段往返退出 0；185 船体/125,856 部件往返，另 5 个既有异常明确拒绝，verify_ships 如实退出 1；191 份源 XML 哈希未变，7 个 GUI 副本与最终 EXE 的 SHA 一致；[目录](../target/staging-ui-20261013/catalog-final.log)、[船体](../target/staging-ui-20261013/ships-final.log)、[汇总](../target/staging-ui-20261013/summary.json) |

最终 EXE SHA-256：`883c080b6213b20a1466b77098ad3089e23b6efeadcaad1a8d7105d691414976`。系统 TEMP 拒绝访问、早期短横屏越界、旧 320×640 分级超时及横幅遮挡都如实留在[中间失败记录](validation-history.md#2026-10-13-分级界面中间验证与失败)；未放宽断言、未把失败称为焦点干扰。

本次未运行 450ms 观看档、Vulkan、物理触屏/移动真机、系统键鼠/文字/输入法/原生路径专项，未录制视频或重新打包，不推送。悬停由真实 egui 逐帧内部移动事件与绘制几何核对，截图为修改状态；不声称测量系统输入或物理显示延迟。

## 未完成与兼容边界

1. 复杂连接点自动镜像的原版可执行行为尚无定论；flipX/flipY 元数据保留，但不能凭字段名猜测自动选择。非中心接触和混合交互继续按证据收敛。
2. 320×640 / 640×360 初始相机可能将船体置于顶栏后方，可用既有“适配视野”；尺寸验收不等于初始自动整船取景。
3. 真实大型船体目录耗时、不同驱动/设备和长时间混合交互仍需抽验。内部同帧 Transform 回归不是物理显示延迟测量。
4. Android / iOS / Web 宿主、资源/存储及系统文件选择未交付；物理触屏、软键盘、安全区域和第三方输入法需目标设备专项。按当前约定跳过输入法/系统文字专项。
5. Python 扩展兼容、旧 Console/任务栏等应用平台能力仍独立列项；不迁移原版未启用实验，也不照搬有数据丢失风险的 SaveStatus。
6. 新 XML 扩展必须先确认无损合同并加入对照回归；5 个已知异常输入明确拒绝，不为追求“全库退出 0”削弱检查。

原版行为对照以 Difficult-Rocket 的 SR1 Rust/Python 源码、PartList/船体库，以及已实际打开的 SimpleRockets-PC-1.6.11 sandbox/Builder 为依据。没有证据的差异保持未完成，不以 UI 截图或旧通过记录推断兼容。
