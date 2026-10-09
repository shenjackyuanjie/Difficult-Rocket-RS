//! 章节清单、模式及分级夹具。
use super::*;

pub(super) struct Chapter {
    pub(super) id: &'static str,
    pub(super) title: &'static str,
    pub(super) description: &'static str,
    pub(super) artifacts: &'static [&'static str],
}

impl Chapter {
    pub(super) fn evidence(&self) -> Vec<String> {
        self.artifacts
            .iter()
            .map(|name| (*name).to_owned())
            .chain(demo_cases::artifacts(self.id))
            .collect()
    }
}
pub(super) const CHAPTERS: &[Chapter] = &[
    Chapter {
        id: "panels",
        title: "部件目录与放置",
        description: "目录选择 → 旋转/镜像预览 → 碰撞拒绝 → 放置及撤销 → 分类筛选",
        artifacts: &[
            "editor-placement-preview.png",
            "editor-collision-preview.png",
            "editor-panels-smoke.png",
        ],
    },
    Chapter {
        id: "connections",
        title: "连接点与吸附",
        description: "紫色候选连接点 → 长梁沿边吸附 → 连接/撤销 → XML 往返",
        artifacts: &[
            "editor-connection-hints.png",
            "editor-connections-smoke.png",
            "connections-smoke.xml",
        ],
    },
    Chapter {
        id: "transforms",
        title: "旋转镜像、自由连接与连线设置",
        description: "非直角吸附 → 组合镜像 → 自由双点连/断 → 连线样式 → 树/连通/框选/后代整组变换",
        artifacts: &[
            "editor-angled-connection.png",
            "editor-combined-transform.png",
            "editor-free-connections.png",
            "editor-free-placement.png",
            "editor-transforms-smoke.png",
            "editor-lines-hidden.png",
            "editor-lines-flow.png",
            "editor-lines-pulse.png",
            "editor-tree-transforms.png",
            "editor-component-transforms.png",
            "editor-box-transforms.png",
            "editor-descendant-transforms.png",
            "transforms-smoke.xml",
            "transforms-smoke.json",
        ],
    },
    Chapter {
        id: "selection",
        title: "选择与编辑",
        description: "左键选取/拖动，中键框选，右键取消 → 切换框选按键 → 后代跟随 → 删除及撤销",
        artifacts: &[
            "editor-selection-preview.png",
            "editor-selection-smoke.png",
            "editor-drag-rotation.png",
            "selection-smoke.xml",
        ],
    },
    Chapter {
        id: "view",
        title: "视角与显示",
        description: "F 整船 / Shift+F 选区适配 → F3 调试文字 → F4 船体显隐 → F1 帮助",
        artifacts: &["editor-view-smoke.png", "editor-help.png"],
    },
    Chapter {
        id: "staging",
        title: "属性与分级",
        description: "F2 属性草稿 → 分级长列表 → 修改并应用 → 撤销重做",
        artifacts: &[
            "editor-staging-draft.png",
            "editor-staging-smoke.png",
            "staging-smoke.xml",
        ],
    },
    Chapter {
        id: "topology",
        title: "连接树与连接图",
        description: "树/图切换 → 换父节点 → 环路拒绝 → 断开连接及撤销",
        artifacts: &[
            "editor-topology-tree.png",
            "editor-topology-graph.png",
            "editor-topology-smoke.png",
            "topology-smoke.xml",
            "topology-smoke.json",
        ],
    },
    Chapter {
        id: "repair",
        title: "原版重号船体修复",
        description: "只读加载 Heronb → 分配真实歧义引用 → 应用修复 → 撤销重做",
        artifacts: &[
            "editor-repair-draft.png",
            "editor-repair-smoke.png",
            "repair-smoke.xml",
        ],
    },
    Chapter {
        id: "browser",
        title: "船体目录与虚拟滚动",
        description: "扫描临时 1000 船体 → 跳过坏 XML → 滚动末尾 → 点击打开",
        artifacts: &["editor-browser-smoke.png", "editor-browser-smoke.json"],
    },
    Chapter {
        id: "unsaved",
        title: "未保存确认",
        description: "窗口内暗色模态 → 取消保留文档 → 放弃修改；不覆盖原版样本",
        artifacts: &["editor-unsaved-modal.png"],
    },
];

const DETAILED_CHAPTERS: &[(&str, Chapter)] = &[
    (
        "panels",
        Chapter {
            id: "placement-cases",
            title: "放置边界：取消、上限与禁用旋转",
            description: "每个夹具独立初始化；预览/拒绝操作不应改写文档。",
            artifacts: &["placement-cases.json"],
        },
    ),
    (
        "connections",
        Chapter {
            id: "geometry-cases",
            title: "精确重叠边界：4.9% / 5% / 5.1%",
            description: "精确事务参数避免鼠标坐标量化；红/绿轮廓是被拒绝/允许的候选。",
            artifacts: &["geometry-cases.json"],
        },
    ),
    (
        "transforms",
        Chapter {
            id: "free-cases",
            title: "自由连接边界：取消、自连接与类型",
            description: "真实端点点击与快捷键；自由模式依然校验引用和端点类型。",
            artifacts: &["free-cases.json"],
        },
    ),
    (
        "transforms",
        Chapter {
            id: "group-cases",
            title: "整组变换边界：空选区、顺序与后代",
            description: "不相连的多选仍是刚体；旋转/镜像顺序不能交换。",
            artifacts: &["group-cases.json"],
        },
    ),
    (
        "transforms",
        Chapter {
            id: "line-cases",
            title: "连接线边界：上下限、重置与退化路径",
            description: "会话级显示设置不生成文档历史；隐藏线不隐藏手动端点。",
            artifacts: &["line-cases.json"],
        },
    ),
    (
        "staging",
        Chapter {
            id: "history-cases",
            title: "历史与属性边界：回滚、分支与非法草稿",
            description: "失败操作保持完整快照；草稿参数直接设置，不进行系统文字输入专项。",
            artifacts: &["history-cases.json"],
        },
    ),
    (
        "topology",
        Chapter {
            id: "topology-cases",
            title: "拓扑边界：自连接、环路与跨组重号",
            description: "树与图的环路规则不同；跨组合并保持实例与引用归属。",
            artifacts: &["topology-cases.json"],
        },
    ),
    (
        "unsaved",
        Chapter {
            id: "file-cases",
            title: "文件边界：坏 XML、保存失败与模态隔离",
            description: "仅使用 target 临时输入，不覆盖或恢复写入原版样本。",
            artifacts: &["file-cases.json"],
        },
    ),
];

const DETAILED_STAGING: Chapter = Chapter {
    id: "staging",
    title: "属性与长分级：64 级 / 1024 动作",
    description: "滚动至最后一级最后一个动作 → 修改草稿 → 应用 → 撤销重做 → XML 往返",
    artifacts: &[
        "editor-staging-draft.png",
        "editor-staging-smoke.png",
        "staging-smoke.xml",
        "editor-staging-performance.json",
    ],
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Mode {
    #[default]
    Brief,
    Detailed,
}

impl Mode {
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Brief => "brief",
            Self::Detailed => "detailed",
        }
    }

    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Brief => "简略",
            Self::Detailed => "详细",
        }
    }

    pub(super) fn chapters(self) -> Vec<&'static Chapter> {
        let mut chapters = Vec::new();
        for chapter in CHAPTERS {
            chapters.push(if self == Self::Detailed && chapter.id == "staging" {
                &DETAILED_STAGING
            } else {
                chapter
            });
            if self == Self::Detailed {
                chapters.extend(
                    DETAILED_CHAPTERS
                        .iter()
                        .filter(|(after, _)| *after == chapter.id)
                        .map(|(_, extra)| extra),
                );
            }
        }
        chapters
    }
}

pub(super) fn staging_fixture(document: &mut EditorDocument, mode: Mode) {
    let (stages, last_id) = match mode {
        Mode::Brief => (8, 7),
        Mode::Detailed => (64, 17),
    };
    let kind = document.catalog.get("detacher-1").unwrap();
    document.ship.parts.extend((2..=last_id).map(|id| {
        kind.instantiate(
            id,
            (
                ((id - 2) % 3) as f64 * 3. - 3.,
                -((id - 2) / 3) as f64 * 3. - 2.,
            ),
        )
    }));
    document.ship.parts[0].pod.as_mut().unwrap().staging = Some(dr_core::StagingState {
        current_stage: stages / 2,
        steps: (0..stages)
            .map(|_| dr_core::StageStep {
                activations: (2..=last_id)
                    .map(|id| dr_core::Activation { id, moved: false })
                    .collect(),
            })
            .collect(),
    });
    document.saved_ship = document.ship.clone();
}
