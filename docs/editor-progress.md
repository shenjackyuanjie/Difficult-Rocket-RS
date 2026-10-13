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
| 界面 | 文件/编辑入口、双侧目录、四组独立折叠、属性/分级、连接树图、帮助与线设置；灰黑/青柠共享样式 |
| 小屏 | 最小 320×320，单侧抽屉、44px 触控入口，按实际画布适配；物理触屏与平台发布另行验证 |
| 演示 / 工具 | brief 10 章；detailed 19 章/45 例/95 证据，独立进程身份及源哈希保护；既有视频/便携包脚本 |

原生按钮使用 Bevy 0.20 的 ui_widgets / picking / Activate，无旧 Interaction 兼容层。依赖版本以 Cargo.lock 为准。DRRS 的具体 API 合同与差异见[核心覆盖核对](drrs-core-coverage.md)。

## 2026-10-13 结构与文档清理

本次清理不新增功能或平台工程、不重写历史、不推送；代码独立提交 `41caff4`。README 从 339 行精简到 51 行，仅保留定位、启动、功能和导航；详细操作/验证各有单一入口，历史状态冻结，不再在首页重复累加测试计数。清除旧 Bevy 0.19、“最小 960×640”、旧独占分类和旧窄窗失败冒充当前状态等矛盾描述。

### 代码与测试整理

- main.rs 从 1,274 行缩为 231 行，只保留资源/插件/系统接线；不改调度顺序。
- 文档/保存点归 document，窗口配置与截图归 window，画布输入归 canvas_input；拖拽、游标、相机与 HUD 归现有 selection、placement、view、render，不增加另一套管理层。
- 共享文档/目录/鼠标夹具归仅测试编译的 test_support；目录直接用内存 catalog_from_xml，不反复创建临时目录文件；删除旧 movement_for 测试包装。
- 根测试归各自模块；目录 UI 大文件按行、布局、手势、分组分开。视频字幕同步四组折叠，不再描述旧独占筛选。
- 删除 7 个测试入口：4 个 egui 依赖自身/旧分帧短点击诊断、1 个缺同步指针的纯 Touch 依赖行为、1 个静态帮助文案检查、1 个重复插值夹具。插值首/末帧、延迟按下及完成状态并回真实调度测试，默认帮助关闭并回实际打开/隔离测试；不减少现行输入、XML 与原子事务的独立断言。

### 本次实际验收

| 检查 | 本次实际结果 / 证据 |
| --- | --- |
| Rust | `cargo test --workspace --release --locked`：372 项（core 104 / editor 268）通过；[日志](../target/cleanup-20261013/rust-final.log) |
| Python | `uv run --no-project --python 3.12 python -B -X utf8 -m unittest discover -s scripts -p 'test_*.py' -v`：148 项通过；[日志](../target/cleanup-20261013/python-passed.log) |
| 格式 / Release | fmt check、全目标 Release Clippy `-D warnings`、workspace Release build 均通过；[fmt](../target/cleanup-20261013/fmt.log)、[Clippy](../target/cleanup-20261013/clippy.log)、[build](../target/cleanup-20261013/build.log) |
| 详细窗口演示 | 同一 Release 程序 DX12 40ms，19 章/45 例/95 个唯一非空证据通过，退出 0；[报告](../target/demo-cleanup-fast-20261013/demo-report.json)、[驱动日志](../target/cleanup-20261013/demo.log) |
| 串行窄窗 | 390×844、640×360、320×640 三重尺寸校验、退出 0；原始 PNG 均已查看；[报告](../target/narrow-cleanup-dx12-20261013/report.json) |
| XML | 27 类/793 字段目录往返退出 0；185 船体/125,856 部件往返、5 个既有异常明确拒绝，verify_ships 如实退出 1；[目录](../target/cleanup-20261013/catalog.log)、[船体](../target/cleanup-20261013/ships.log) |
| 不变性 / 整理审计 | 191 份源 XML 哈希未变，4 份窗口副本与源程序 SHA 一致；[汇总](../target/cleanup-20261013/summary.json)、[迁移正文/调度核对](../target/cleanup-20261013/migration-check.json)、[测试删除核对](../target/cleanup-20261013/test-audit.json) |

本次程序 SHA-256：`b169b9e94806b86ab70aaa83a46cb8ef833cce0e3f8aadc223621e7e384805d8`。本次未运行 450ms 观看档、Vulkan、物理触屏/移动真机、系统键鼠/文字/输入法/原生路径专项，也未录制视频、重新打包。此前 379/373 项和观看结果均属于其他构建，不代替本次。

### 失败与验证边界

模块迁移首次 fmt 因自动提取误加可见性到 anyhow 宏报错；首次 Rust 编译暴露原入口隐式 imports（58 项缺失），第二次编译暴露 HUD 系统签名的私有类型。均以显式所有者导入及适当模块内可见性修正，未压制警告、未放宽断言。[首次编译](../target/cleanup-20261013/refactor-compile-first.log)、[第二次编译](../target/cleanup-20261013/refactor-compile-second.log)、[迁移后回归](../target/cleanup-20261013/rust-first.log)、[测试归位后回归](../target/cleanup-20261013/rust-final.log)。

Python 两轮首次回归均为 147 通过/1 失败：视频字幕已同步组折叠，测试仍要求旧“分类筛选/恢复全部”文字。同步两条断言至现行“独立收起/全部展开”，其余真实三键与显示说明不变，全套 148 项复跑通过。[首轮](../target/cleanup-20261013/python.log)、[第二轮](../target/cleanup-20261013/python-final.log)。文档活动链接及章节已核对；历史 30 个缺失 target 链接明确保留，不伪造日志。记录勘误：`7de95b8` 提交正文将链接数误写为 229，实际报告为 215；这里只更正记录，不重写历史提交。

## 未完成与兼容边界

1. 复杂连接点自动镜像的原版可执行行为尚无定论；flipX/flipY 元数据保留，但不能凭字段名猜测自动选择。非中心接触和混合交互继续按证据收敛。
2. 320×640 / 640×360 初始相机可能将船体置于顶栏后方，可用既有“适配视野”；尺寸验收不等于初始自动整船取景。
3. 真实大型船体目录耗时、不同驱动/设备和长时间混合交互仍需抽验。内部同帧 Transform 回归不是物理显示延迟测量。
4. Android / iOS / Web 宿主、资源/存储及系统文件选择未交付；物理触屏、软键盘、安全区域和第三方输入法需目标设备专项。按当前约定跳过输入法/系统文字专项。
5. Python 扩展兼容、旧 Console/任务栏等应用平台能力仍独立列项；不迁移原版未启用实验，也不照搬有数据丢失风险的 SaveStatus。
6. 新 XML 扩展必须先确认无损合同并加入对照回归；5 个已知异常输入明确拒绝，不为追求“全库退出 0”削弱检查。

原版行为对照以 Difficult-Rocket 的 SR1 Rust/Python 源码、PartList/船体库，以及已实际打开的 SimpleRockets-PC-1.6.11 sandbox/Builder 为依据。没有证据的差异保持未完成，不以 UI 截图或旧通过记录推断兼容。
