# SR1 PC 标量读取与写出证据

[项目首页](../../README.md) · [操作指南](../guides/editor.md) · [验证指南](../validation/README.md)

## 目标与证据等级

本记录对应 [scalars.rs](../../crates/dr-core/src/sr1/scalars.rs)，实现独立的 `dr_core::sr1::scalars`。它不修改现有严格编辑模型和导入入口的默认合同，也不依赖其他本体语义模块。

- **A：PC 原生静态证据**。只读核对 32 位 [SimpleRockets.exe](../../../SimpleRockets-PC-1.6.11/SimpleRockets.exe) 和随游戏附带的 32 位 [msvcr120.dll](../../../SimpleRockets-PC-1.6.11/msvcr120.dll)。下表为实际 VA/文件偏移，不是还原源码行号。
- **B：同版本微软 CRT 离线实测**。使用具有有效 Microsoft Corporation 签名的本机 64 位 [msvcr120.dll](file:///C:/Windows/System32/msvcr120.dll)，通过 Python `ctypes` 调用 `sscanf_s`、`_snprintf_s`；不加载游戏可执行文件、不运行游戏。
- **C：可移植实现及冻结金样**。Rust 用前缀扫描、独立宽度解析和十进制写出模型承接已核实语义；金样在模块测试内，不要求验收机器安装游戏或 CRT。
- **边界**：B 不是游戏进程内执行，也不是把 64 位 DLL 当成 32 位 DLL；没有宣称所有极端输入、全部浮点位模式和所有区域设置逐值等价。

分析命令使用本机现有 `pefile`、`capstone`，入口为 `uv run --no-project --no-cache --python 3.12 python -B`，没有安装新包或访问上游。

## 文件身份

游戏 EXE 的 `ImageBase = 0x00400000`，附带 32 位 CRT 的首选 `ImageBase = 0x10000000`。两个 CRT 文件版本均为 `12.00.40664.0 built by: VSULDR`；Python 宿主指针宽度为 8 字节。

| 对象 | SHA-256 |
| --- | --- |
| [原生 PC EXE](../../../SimpleRockets-PC-1.6.11/SimpleRockets.exe) | `7adf2d73d6f95f79333154a98cca21af734917981e0ccf94171e3846547dd0e9` |
| [游戏附带 x86 CRT](../../../SimpleRockets-PC-1.6.11/msvcr120.dll) | `9a6ac6cc2c960a330ce7e7a8aba5f63d834f34a3508381303308642c356e4f5c` |
| [本机签名有效的 x64 CRT](file:///C:/Windows/System32/msvcr120.dll) | `753ac30c30aae62415cc225e3d057b8b6254afe280696e0a43f1a7c3132632a6` |

## PC 原生地址与文件偏移

以下未特别标记的地址均属于 PC EXE。

| 行为 | PC VA | 文件偏移 | 实际指令或资源依据 |
| --- | --- | --- | --- |
| 数值扫描导入 | `0x0050A190` | `0x00108F90` | IAT 名称为 `MSVCR120.dll!sscanf_s`，不是普通 `sscanf` |
| 整数格式 | `0x005281CC` | `0x00126FCC` | C 字符串 `%d` |
| 直接 float32 格式 | `0x005285F4` | `0x001273F4` | C 字符串 `%f`；同一字符串也用于浮点写出，必须结合调用方判断 |
| float64 格式 | `0x005285F8` | `0x001273F8` | C 字符串 `%lf` |
| 小写布尔常量 | `0x005285D0` / `0x005285C8` | `0x001273D0` / `0x001273C8` | 分别为 `true`、`false` |
| 布尔转换 | `0x00426F20` | `0x00026320` | 先 `%d` 到局部 i32，返回 1 时按非零写 bool；失败后才比较小写常量；全部失败时不写目标 |
| 字符串精确比较 | `0x00426760` | `0x00025B60` | 逐字节比较；布尔调用传 `0x7FFFFFFF`，最终要求两端都结束，未跳过前后空白 |
| 整数查询的缺失分支 | `0x0041CB10` | `0x0001BF10` | 查不到属性立即返回；存在时将调用者目标地址传入 `%d` 扫描 |
| float64 零默认辅助入口 | `0x0043EEF0` | `0x0003E2F0` | 局部 qword 先置零；缺失跳过扫描；存在用 `%lf`，最后 `movsd` 返回 |
| Parachute 直接 float32 扫描 | `0x004ABC95` → `0x004ABC9B` → `0x004ABCA0` | `0x000AB095` → `0x000AB09B` → `0x000AB0A0` | `push %f`、寄存器间接调用 CRT，随后 `movss`；目标 dword 先置零 |
| 另一类字段先 double 后窄化 | `0x0044004E` → `0x0044006E` | `0x0003F44E` → `0x0003F46E` | `%lf` 后 `mulsd`，再 `cvtpd2ps`；这种字段须由上层先 double 运算后窄化 |
| 变参格式写出包装 | `0x00426710` | `0x00025B10` | `_vsnprintf_s`，`count = -1`；调用方传 `%f`，精度未显式指定，使用默认六位 |
| Part 写出调用例 | `0x0046F8ED` → `0x0046F8F8` | `0x0006ECED` → `0x0006ECF8` | `%f`、缓冲区大小 `0xC8`，调用上述包装 |
| float32 写出前提升 | `0x004ABEAB` → `0x004ABEB5` → `0x004ABEDC` | `0x000AB2AB` → `0x000AB2B5` → `0x000AB2DC` | `movss`、`cvtps2pd`，再将实际提升值传给 `%f` |
| 本体存档数值布尔（Runtime） | `0x00450E27` → `0x00450E40` | `0x00050227` → `0x00050240` | `xor ecx`、`test bl`、`setne cl`，随后 `push %d` 并调用格式写出包装，输出 `0`/`1` |
| 本体存档数值布尔（MoonTouchDown） | `0x00457FB5` → `0x00457FD0` | `0x000573B5` → `0x000573D0` | 同样以 `setne` 规范化后按 `%d` 写出 |
| XML 库级单词布尔重载（独立路径） | `0x0042BF10` | `0x0002B310` | 按 bool 在 `true`/`false` 常量间选择；不能据此推广本体存档的输出合同 |
| 编辑器角度映射 | `0x0046F560` | `0x0006E960` | 0 写零、1 取 π/2、2 取 π，其余全部写 π/2 + π，没有 `% 4` |
| π/2 与 π 常量 | `0x005258E0` / `0x00535440` | `0x001246E0` / `0x00134240` | double 值分别为 `1.5707963267948966`、`3.141592653589793` |

32 位附带 CRT 的整数路径另核实如下，不把 C 标准的未定义/实现相关边界当成跨 CRT 保证：

| x86 CRT VA | 文件偏移 | 证据 |
| --- | --- | --- |
| `0x1002F374` | `0x0002E774` | `sscanf_s` 导出入口，进入内部扫描器 `0x1002F392` |
| `0x1002F6A7` | `0x0002EAA7` | `imul eax, [累计值], 0xA`，接着加数字；未以溢出标志提前终止 |
| `0x10059484` | `0x00058884` | 负号路径 `neg eax`，结果仍为 32 位 |
| `0x1002F777` | `0x0002EB77` | 普通 `%d` 目标最终 `mov dword ptr [ecx], eax` |

### 资源交叉核对

[DefaultShip.xml:1–7](../../../SimpleRockets-PC-1.6.11/DefaultShip.xml#L1-L7) 同时含整数 `0`/`1`、六位小数及 fuel；[SmolarSystem.xml:5–13](../../../SimpleRockets-PC-1.6.11/SmolarSystem.xml#L5-L13) 同时含小写 `true`、数值 `1` 和不同小数位数。资源说明两种布尔词法及非固定精度输入确实存在；前缀、失败与极值行为以 PC 指令和 CRT 实测为依据，不从正常资源推断。

### 布尔输出路径区分与集成纠正

本体存档的 `write_bool` 合同是 **`true → "1"`、`false → "0"`**。它对应上表 Runtime/MoonTouchDown 的 `setne + %d` 直接调用证据。`0x0042BF10` 则是 XML 库中另一条输出小写单词的重载，两者必须区分；当前模块不额外公开未被本体存档需求使用的单词写出 API。

此前版本误把库级单词重载推广成本体统一存档策略，导致 Lead 的 93 项集成测试中 7 项布尔输出断言失败。本轮按直接本体证据修正 `write_bool`，保留其他模块期待 `0`/`1` 的测试，并在标量测试内直接断言真写 `1`、假写 `0`。布尔读取仍同时接受数值前缀和精确小写单词，读取词法不等于写出策略。

## 已实现的公共 API

[解析及默认读取](../../crates/dr-core/src/sr1/scalars.rs#L8-L72)：

```rust
pub fn parse_i32(text: &str) -> Option<i32>;
pub fn parse_f32(text: &str) -> Option<f32>;
pub fn parse_f64(text: &str) -> Option<f64>;
pub fn parse_bool(text: &str) -> Option<bool>;
pub fn read_i32(text: Option<&str>, default: i32) -> i32;
pub fn read_f32(text: Option<&str>, default: f32) -> f32;
pub fn read_f64(text: Option<&str>, default: f64) -> f64;
pub fn read_bool(text: Option<&str>, default: bool) -> bool;
```

[写出与角度](../../crates/dr-core/src/sr1/scalars.rs#L73-L161)：

```rust
pub fn write_f32(value: f32) -> String;
pub fn write_f64(value: f64) -> String;
pub fn write_bool(value: bool) -> &'static str;
pub fn editor_angle_radians(angle: i32) -> f64;
```

1. 读取缺失/失败均保留传入初值，包括负零和 NaN payload。
2. 数值允许 C locale ASCII 前导空白、可选正负号和十进制前缀，尾随文本不要求消费完。显式处理垂直制表符 `0x0B`，不把 Unicode NBSP 当作 C 空白。NUL 后按 C 字符串终止；这不意味着 XML 入口接受非法 NUL。
3. i32 按 PC 的 32 位回绕累积；布尔也采用回绕后的 i32 判断零/非零。字符串布尔不修剪、不忽略大小写。
4. f32 与 f64 各自直接解析到自身宽度；上层遇到 `%lf → double 运算 → f32` 的具体字段，应选择 f64 API 后按证据窄化，不提前套用 f32。
5. 十进制溢出可能得到 Inf，不能把“扫描成功”与“结果有限”混为一谈。数据层是否裁剪/拒绝，由具体字段的 PC 行为决定。
6. `write_bool` 仅输出数值 `1`/`0`，对应本体存档路径；不会将库级单词重载强加给上层。

## CRT 离线实测与冻结金样

### 读取结果

以下失败样本的目标初值统一设为 123，CRT 未改变目标；模块以 `None` 表示失败，`read_*` 负责保初值。

| 输入 | 格式 | 实测结果 |
| --- | --- | --- |
| `  -12tail`、`+7.5`、`0x10`、`010` | `%d` | -12、7、0、10 |
| `2147483648`、`-2147483649`、`4294967296` | `%d` | i32::MIN、i32::MAX、0，均返回转换数 1 |
| 36 个 `9` | `%d` | -1 |
| 空串、单独 `+`、NBSP 后跟 `1` | `%d` | 不写入目标 |
| `1.25foo`、`.5x`、`1.x`、`1e2tail` | `%f` / `%lf` | 1.25、0.5、1、100 |
| `1e`、`1e+`、`1e-foo` | `%f` / `%lf` | 1，仍返回转换数 1 |
| `nan`、`NaN`、`inf`、`infinity` | `%f` / `%lf` | 转换数 0，不写目标 |
| `1.#INF`、`-1.#IND`、`0x1p2`、`1,25` | `%f` / `%lf` | 1、-1、0、1，仅消费十进制前缀 |
| `1e309`、`-1e9999` | `%f` / `%lf` | +Inf、-Inf，仍返回转换数 1 |
| `1e-9999` | `%f` / `%lf` | +0 |
| `-0` | `%f` / `%lf` | 位模式 `0x80000000` / `0x8000000000000000` |

特别冻结宽度边界 `3.4028235677973366e38`：

- 直接 `%f` 得到 `0x7F7FFFFF`，即最大有限 f32；
- `%lf` 得到 double 位模式 `0x47EFFFFFF0000000`，再转 f32 得到 `0x7F800000`，即 +Inf。

这证明“先统一读 double 再转 float”不能代替直接 `%f` 的所有输入。模块直接 f32 解析保留前者；并未把这一差异藏在“通常等价”中。

### 六位输出模型

普通 Rust 固定六位格式并不总等于此版 CRT。实测至少确认两层现象：

| 输入 double | CRT `%f` 输出 | 原因/对照 |
| --- | --- | --- |
| 负零 | `-0.000000` | 符号必须保留 |
| `1.0078125` | `1.007813` | 六位小数精确中点远离零，不是 ties-even |
| `-1.0078125` | `-1.007813` | 负中点同样远离零 |
| `0.0000005` 的实际 double | `0.000000` | 实际二进制值略小于十进制中点，不能先乘 1e6 再凭近似 `.5` 判断 |
| `249740264909.23462` 的实际 double | `249740264909.234620` | 直接精确 fixed6 会得到尾数 `234619`；CRT 的中间有效位数有影响 |
| 精确 `416858964277.796875` | `416858964277.796870` | 第 17 位十进制有效数的精确中点取较小绝对值；随后补足六位 |

实现先完整展开有限 IEEE double，再按 **17 位有效数 ROUND_HALF_DOWN → 六位小数 ROUND_HALF_UP** 处理绝对值并恢复符号。这是差分验证的可移植兼容模型，不冒充已恢复的完整 CRT 源码算法。

本轮实际执行的模型差分：

- 随机种子 `12040664`：10,000 个约 1e-12 至 1e12 数量级的随机值，加 `n / 128`（n=-2048..2048）4,097 个值，共 **14,097 个有限数，零差异**。
- 8 组正负中点各取前一/后一相邻 double，共 24 个位模式，另加 2 个 f32 值提升后的 double，共 **26 个值，零差异**。测试中冻结 24 个中点/相邻 ULP 位模式和 f32 提升代表值。
- 随机种子 `314159`：5,000 个任意 64 位模式，剔除 1 个非有限值，得到 **4,999 个覆盖全指数范围的有限数，零差异**。
- 先前验证“普通 fixed6 只修精确中点”的候选模型，在第一组有 **357 个差异**；改为 17 位 ties-even 的候选仍有 **1 个差异**。最终模型才消除这些样本差异，未把失败候选当成最终实现。

非有限输出另外冻结 8 种实际 double 位模式：

| 位模式 | CRT `%f` 输出 |
| --- | --- |
| `0x7FF0000000000000` | `1.#INF00` |
| `0xFFF0000000000000` | `-1.#INF00` |
| `0x7FF8000000000000` | `1.#QNAN0` |
| `0xFFF8000000000000` | `-1.#IND00` |
| `0x7FF8000000000001` / `0xFFF8000000000001` | `1.#QNAN0` / `-1.#QNAN0` |
| `0x7FF0000000000001` / `0xFFF0000000000001` | `1.#SNAN0` / `-1.#SNAN0` |

CRT 自身写出的 `1.#INF00` 用 `%lf` 读回也只是数值前缀 1；这里没有强加一个本体不存在的非有限 round-trip 保证。

### 最小实测复现方式

先用 `Get-AuthenticodeSignature` 验证本机微软 DLL 签名，再运行下列离线脚本。它只调用系统 CRT，不需要游戏运行。

```python
import ctypes
crt = ctypes.CDLL(r"C:\Windows\System32\msvcr120.dll")
scan = crt.sscanf_s
scan.argtypes = [ctypes.c_char_p, ctypes.c_char_p]
scan.restype = ctypes.c_int
for text, fmt, ty in [
    (b"2147483648", b"%d", ctypes.c_int32),
    (b"1e+", b"%lf", ctypes.c_double),
    (b"3.4028235677973366e38", b"%f", ctypes.c_float),
]:
    value = ty(123)
    count = scan(text, fmt, ctypes.byref(value))
    print(text, count, value.value, bytes(value).hex())
write = crt._snprintf_s
write.argtypes = [ctypes.c_void_p, ctypes.c_size_t, ctypes.c_size_t, ctypes.c_char_p]
write.restype = ctypes.c_int
out = ctypes.create_string_buffer(2048)
write(out, len(out), ctypes.c_size_t(-1).value, b"%f", ctypes.c_double(1.0078125))
print(out.value)
```

本地研究附档位于 ignored 的 [crt_probe.py](../../target/sr1-research/sr1-scalars/crt_probe.py)、[crt_probe.json](../../target/sr1-research/sr1-scalars/crt_probe.json)、[crt_difference.py](../../target/sr1-research/sr1-scalars/crt_difference.py)、[crt_difference.json](../../target/sr1-research/sr1-scalars/crt_difference.json)、[midpoint_neighbors.json](../../target/sr1-research/sr1-scalars/midpoint_neighbors.json) 和 [expanded_range.json](../../target/sr1-research/sr1-scalars/expanded_range.json)。这些只用于本机研究，不是纯 Rust 测试的运行依赖。

## Rust 离线测试与验证状态

9 个测试全部位于 [scalars.rs 的测试模块](../../crates/dr-core/src/sr1/scalars.rs#L218-L430)：

- `missing_or_invalid_values_preserve_initial_bits`：初值、NaN payload 和负零保留。
- `crt_integer_prefix_and_wrapping_golden`：CRT 前缀及回绕金样。
- `bool_scans_integer_before_exact_lowercase_words`：数字优先、完整小写、空白边界，以及 `write_bool(true) == "1"`、`write_bool(false) == "0"` 直接断言。
- `crt_float_prefix_and_invalid_exponent_golden`：浮点前缀、不完整指数和特殊字面量拒绝。
- `float_widths_overflow_underflow_and_negative_zero_golden`：直接 f32/f64 宽度、溢出、下溢及零符号。
- `crt_fixed_six_and_float32_promotion_golden`：六位、提升及十七位中间舍入。
- `crt_midpoints_and_adjacent_ulps_golden`：24 个确定输入位模式。
- `crt_nonfinite_output_bit_patterns_golden`：8 种非有限位模式及非 round-trip 行为。
- `editor_angles_use_fallback_instead_of_modulo`：0/1/2/其他，包括负数及整数端点。

本队友已实际执行：PC/x86 CRT 只读反汇编、系统 DLL 签名/版本核验、上述 Python CRT 及格式模型离线差分、仅分配 Rust 文件的 `rustfmt --config skip_children=true`、分配文件的 `git diff --check`，以及逐行检查两份实际文件的尾随空白（均为 0）和 Markdown 链接（断链为 0）。

**Cargo 测试由 Lead 统一运行，本队友没有运行构建或 Cargo。** Lead 已回报首轮 9 项 scalar 测试全部通过；随后按其严格 Clippy 报告修正惰性 `or_else` 和两项有意精确浮点金样的字面量写法，改用 `from_bits`。其后 Lead 的 93 项 SR1 集成测试暴露 7 项由布尔单词写出引起的失败；本轮已将 `write_bool` 和直接断言修为数值 `1`/`0`，修正后的统一复验由 Lead 执行，不把首轮通过当成最终结果。验收命令：

```powershell
cargo test -p dr-core sr1::scalars::tests
```

## 尚未验证与刻意保留的边界

1. 没有启动游戏、注入进程或运行游戏随附 DLL；同版本 x64 CRT 实测不能冒充游戏进程内 x86 的逐值动态验证。整数回绕另有 x86 指令直接佐证；浮点大范围结果仍保留架构边界。
2. f32/f64 十进制转二进制使用各自 Rust 解析器；已验证/冻结的输入一致，不声称 CRT 与 Rust 对所有超长尾数、指数长度、次正规边界或中点十进制输入完全等价。
3. 固定 C locale ASCII 词法与小数点，不模拟运行时更换 CRT locale。
4. 写出模型支持完整 Rust `String`，不模拟游戏 `0xC8` 临时缓冲区的截断和错误处理；扩展范围差分使用 2048 字节缓冲区，不能拿它证明游戏大数缓冲区支路。
5. 非有限测试覆盖列出的 8 种位模式；任意 NaN payload、f32 signaling NaN 提升时的跨硬件 quieting 行为未全域验证。
6. 本模块只负责标量。fuel、throttle、几何等字段的裁剪、计算顺序和窄化位置由对应本体数据模块据 PC 证据处理。

## 本轮统一验收

Lead 已完成的构建、跨模块回归、真实资源和独立 Rust/CRT 差分结果，见[本体数据语义层验证记录](../development/sr1-data.md#验证与后续)。本文的队友交接验证与该统一结果分列。
