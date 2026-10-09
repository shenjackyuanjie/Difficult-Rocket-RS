//! 详细模式的可审查样例清单：每例都必须有实际断言和独立截图。
#[derive(Clone, Copy, Debug)]
pub(super) enum Cancel {
    Same,
    Escape,
    Blank,
    Revision,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum Scenario {
    PaletteCancel,
    QuantityLimit,
    DisabledRotation,
    Overlap {
        ratio: f64,
        degrees: f64,
        relative: bool,
        mirrors: bool,
    },
    Contained,
    ManualCancel(Cancel),
    ManualSelf,
    ManualMismatch,
    ManualReverse,
    EmptySelection,
    SingleFine,
    Disconnected,
    MirrorTwice,
    TransformOrder,
    Descendants,
    LineWidth(bool),
    LineReset,
    ZeroLength,
    HiddenEndpoint,
    SelfLink,
    Cycle(bool),
    CrossGroup,
    EmptyHistory,
    BatchRollback,
    UndoBranch,
    InvalidFuel(&'static str),
    StaleDraft,
    InvalidOpen,
    SaveFailure,
    UnsavedCancel,
}

pub(crate) struct Case {
    pub id: &'static str,
    pub title: &'static str,
    pub expected: &'static str,
    pub input_path: &'static str,
    pub(super) scenario: Scenario,
}

macro_rules! case {
    ($id:literal, $title:literal, $expected:literal, $path:literal, $scenario:expr) => {
        Case {
            id: $id,
            title: $title,
            expected: $expected,
            input_path: $path,
            scenario: $scenario,
        }
    };
}

pub(crate) const PLACEMENT: &[Case] = &[
    case!(
        "palette-cancel",
        "目录预览取消",
        "预览中旋转/镜像后按 Esc：文档、脏标记及历史都不改变。",
        "内部画布快捷键",
        Scenario::PaletteCancel
    ),
    case!(
        "quantity-limit",
        "自由模式也保留数量上限",
        "已有唯一对接插头，再放一个仍被拒绝；自由模式不是关闭目录约束。",
        "放置事务精确调用",
        Scenario::QuantityLimit
    ),
    case!(
        "disabled-rotation",
        "不可旋转部件",
        "轮子已禁用旋转：R 和 Shift+E 都不能改角度，也不产生历史。",
        "内部画布快捷键",
        Scenario::DisabledRotation
    ),
];
pub(crate) const GEOMETRY: &[Case] = &[
    case!(
        "overlap-below",
        "重叠 4.9%",
        "精确参数移动到 4.9%：允许提交，绿色轮廓是候选实体。",
        "编辑命令精确参数（非鼠标量化）",
        Scenario::Overlap {
            ratio: 0.049,
            degrees: 0.0,
            relative: false,
            mirrors: false
        }
    ),
    case!(
        "overlap-exact",
        "重叠恰好 5%",
        "精确参数移动到 5%：严格拒绝；红色候选轮廓保留，原船体和历史不变。",
        "编辑命令精确参数（非鼠标量化）",
        Scenario::Overlap {
            ratio: 0.05,
            degrees: 0.0,
            relative: false,
            mirrors: false
        }
    ),
    case!(
        "overlap-above",
        "重叠 5.1%",
        "精确参数移动到 5.1%：拒绝；失败操作不能污染撤销栈。",
        "编辑命令精确参数（非鼠标量化）",
        Scenario::Overlap {
            ratio: 0.051,
            degrees: 0.0,
            relative: false,
            mirrors: false
        }
    ),
    case!(
        "overlap-rotated-mirrors",
        "共转 37°、双轴镜像的 5%",
        "非直角共转并双镜像后，数学上恰好 5% 仍严格拒绝。",
        "编辑命令精确参数（非鼠标量化）",
        Scenario::Overlap {
            ratio: 0.05,
            degrees: 37.0,
            relative: false,
            mirrors: true
        }
    ),
    case!(
        "overlap-relative-45",
        "相对旋转 45° 的 5%",
        "尖角形成三角交集：即使两个部件角度不同，恰好 5% 仍拒绝。",
        "编辑命令精确参数（非鼠标量化）",
        Scenario::Overlap {
            ratio: 0.05,
            degrees: 0.0,
            relative: true,
            mirrors: false
        }
    ),
    case!(
        "overlap-contained",
        "较小实体完全被包含",
        "鼻锥落入机身：按较小实体计为 100%，不能用大实体稀释重叠比例。",
        "编辑命令精确参数（非鼠标量化）",
        Scenario::Contained
    ),
];
pub(crate) const FREE: &[Case] = &[
    case!(
        "manual-same",
        "重复点击同一端点",
        "同一端点点击两次只取消待连接状态，不产生自连接或历史。",
        "内部画布鼠标",
        Scenario::ManualCancel(Cancel::Same)
    ),
    case!(
        "manual-self",
        "同一零件的不同端点",
        "同零件的两个不同端点不能自连接；显示错误且不修改文档。",
        "内部画布鼠标",
        Scenario::ManualSelf
    ),
    case!(
        "manual-escape",
        "手动连接中按 Esc",
        "先点端点再按 Esc：清理待连接状态，原有连接不受影响。",
        "内部画布鼠标与快捷键",
        Scenario::ManualCancel(Cancel::Escape)
    ),
    case!(
        "manual-blank",
        "手动连接中点击空白",
        "先点端点再点空白：不会留下幽灵起点或意外连接。",
        "内部画布鼠标",
        Scenario::ManualCancel(Cancel::Blank)
    ),
    case!(
        "manual-stale",
        "连接起点在文档更新后失效",
        "选择起点后改变文档版本：旧起点必须清理，不能引用旧姿态。",
        "内部鼠标 + 文档事务",
        Scenario::ManualCancel(Cancel::Revision)
    ),
    case!(
        "manual-type-mismatch",
        "对接点与普通点不能混接",
        "临时显式 dock=true 对接点不能接普通机身；原版插头的普通安装点语义不变。",
        "内部画布鼠标",
        Scenario::ManualMismatch
    ),
    case!(
        "manual-reverse-unlink",
        "反向点击已有连接",
        "先子后父点击同一已有连接：精确断开而不是新增反向重复边；可一次撤销。",
        "内部画布鼠标与快捷键",
        Scenario::ManualReverse
    ),
];
pub(crate) const GROUPS: &[Case] = &[
    case!(
        "selection-empty",
        "空选区快捷键",
        "没有选中零件时旋转/镜像是空操作，不出现无穷中心或历史记录。",
        "内部画布快捷键",
        Scenario::EmptySelection
    ),
    case!(
        "selection-single",
        "单零件精细反向旋转",
        "Shift+Q 的 -1° 规范化为 359°，中心位置不变，可撤销。",
        "内部画布快捷键",
        Scenario::SingleFine
    ),
    case!(
        "selection-disconnected",
        "不相连的多选零件",
        "没有连接边的多选仍绕共同中心刚性变换，彼此距离和未选零件不变。",
        "内部画布快捷键",
        Scenario::Disconnected
    ),
    case!(
        "mirrors-involution",
        "两次 X、两次 Y",
        "带非直角初始姿态的组合，XX/YY 恢复原位置和镜像；撤销恢复完整夹具。",
        "内部画布快捷键",
        Scenario::MirrorTwice
    ),
    case!(
        "transform-order",
        "旋转与镜像不交换",
        "E→X 与 X→E 的结果应不同；各自两次撤销均恢复原船体。",
        "内部画布快捷键",
        Scenario::TransformOrder
    ),
    case!(
        "descendants-no-ancestor",
        "跟随后代但不带上级",
        "从树中间节点开始拖转：仅自己及全部后代预览/提交，外部父节点不动。",
        "内部画布拖拽与快捷键",
        Scenario::Descendants
    ),
];
pub(crate) const LINES: &[Case] = &[
    case!(
        "line-width-min",
        "连接线最小粗细",
        "连续点击减号到下限后仍点一次：保持 1px，船体和撤销历史不变。",
        "实际 egui 设置按钮",
        Scenario::LineWidth(false)
    ),
    case!(
        "line-width-max",
        "连接线最大粗细",
        "连续点击加号到上限后仍点一次：保持 12px，不生成编辑历史。",
        "实际 egui 设置按钮",
        Scenario::LineWidth(true)
    ),
    case!(
        "line-reset",
        "重置组合显示设置",
        "隐藏/紫色/虚线/流动/箭头等组合一键恢复默认；只改变会话配置。",
        "实际 egui 设置按钮",
        Scenario::LineReset
    ),
    case!(
        "line-zero-length",
        "重合零件与零长度路径",
        "自由连接的同位置部件启用流动箭头：渲染无 NaN、无除零、不中断回放。",
        "显示资源设置 + 实际画布渲染",
        Scenario::ZeroLength
    ),
    case!(
        "line-hidden-endpoint",
        "隐藏连接线仍可选端点",
        "关闭连接线不等于关闭紫色手动端点；实际点击仍能产生待连接起点。",
        "内部画布鼠标",
        Scenario::HiddenEndpoint
    ),
];
pub(crate) const TOPOLOGY: &[Case] = &[
    case!(
        "topology-self",
        "树中拒绝自连接",
        "父/子设为同一实例，点击实际连接按钮后拒绝，文档和历史不变。",
        "实际 egui 连接树控件",
        Scenario::SelfLink
    ),
    case!(
        "topology-tree-cycle",
        "树中拒绝后代作为父节点",
        "把孙节点设为根节点的父：树模式拒绝成环且保留所有旧边。",
        "实际 egui 连接树控件",
        Scenario::Cycle(false)
    ),
    case!(
        "topology-graph-cycle",
        "有向连接图允许环",
        "同一组样例切到图模式：允许闭环，仍可一步撤销完整恢复。",
        "实际 egui 连接图控件",
        Scenario::Cycle(true)
    ),
    case!(
        "topology-cross-group",
        "跨组连接时遇到相同编号",
        "组内编号可重复于另一组：连接合并只重编号冲突实例，保持端点归属及 XML 往返。",
        "连接事务精确调用",
        Scenario::CrossGroup
    ),
];
pub(crate) const HISTORY: &[Case] = &[
    case!(
        "history-empty",
        "空历史撤销与重做",
        "Ctrl+Z / Ctrl+Y 不应改文档、脏标记或创建历史。",
        "内部画布快捷键",
        Scenario::EmptyHistory
    ),
    case!(
        "history-batch-rollback",
        "批处理中后一步失败",
        "先变换再执行非法删除：整批回滚，第一步也不能残留。",
        "编辑事务精确调用",
        Scenario::BatchRollback
    ),
    case!(
        "history-new-branch",
        "撤销后产生新分支",
        "旋转→撤销→镜像：旧旋转重做栈清空；再按重做不恢复旧分支。",
        "内部画布快捷键",
        Scenario::UndoBranch
    ),
    case!(
        "fuel-nan",
        "燃料 NaN 草稿",
        "非有限燃料草稿点击实际应用：留在草稿并显示错误，文档不改。",
        "草稿参数设置 + 实际 egui 应用按钮（非系统文字输入）",
        Scenario::InvalidFuel("NaN")
    ),
    case!(
        "fuel-negative",
        "负燃料草稿",
        "燃料 -1 被拒绝，不能污染文档或历史。",
        "草稿参数设置 + 实际 egui 应用按钮（非系统文字输入）",
        Scenario::InvalidFuel("-1")
    ),
    case!(
        "fuel-over-capacity",
        "超容量燃料草稿",
        "燃料超过目录容量被拒绝，草稿可取消且文档不变。",
        "草稿参数设置 + 实际 egui 应用按钮（非系统文字输入）",
        Scenario::InvalidFuel("1000000000")
    ),
    case!(
        "properties-stale",
        "属性草稿快照已过期",
        "开草稿后部件已由另一事务修改：应用旧草稿必须拒绝，不能覆盖新状态。",
        "文档事务 + 实际 egui 应用按钮",
        Scenario::StaleDraft
    ),
];
pub(crate) const FILES: &[Case] = &[
    case!(
        "file-invalid-open",
        "打开损坏 XML",
        "临时坏 XML 不触发丢弃原文档：路径、脏状态和历史全部保留。",
        "实际文件消息事务",
        Scenario::InvalidOpen
    ),
    case!(
        "file-save-failure",
        "保存目的地是目录",
        "保存失败不能谎报已保存、清除脏状态或丢掉撤销历史；只用临时路径。",
        "实际文件消息事务",
        Scenario::SaveFailure
    ),
    case!(
        "file-modal-cancel",
        "未保存模态取消与输入隔离",
        "新建触发模态后，旋转/删除不能穿透；点击取消保留整份未保存文档。",
        "实际文件消息 + egui 模态按钮",
        Scenario::UnsavedCancel
    ),
];

pub(crate) fn cases(chapter: &str) -> &'static [Case] {
    match chapter {
        "placement-cases" => PLACEMENT,
        "geometry-cases" => GEOMETRY,
        "free-cases" => FREE,
        "group-cases" => GROUPS,
        "line-cases" => LINES,
        "topology-cases" => TOPOLOGY,
        "history-cases" => HISTORY,
        "file-cases" => FILES,
        _ => &[],
    }
}
