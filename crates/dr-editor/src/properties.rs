use crate::document::EditorDocument;
use crate::files;
use crate::help;
use crate::mobile_input;
use crate::native_buttons;
use crate::panels;
use crate::placement::EditorCursor;
use crate::selection::DragState;
use bevy::prelude::*;
use dr_core::EditorCommand;
use dr_core::Part;
use dr_core::PartKey;
use dr_core::{
    Activation, ConnectionRole, DuplicateRepair, ReferenceSite, StageStep, StagingState,
};
use std::collections::BTreeSet;

pub mod egui_panel;
pub mod egui_smoke;
mod history;
pub mod native_ime;
pub mod repair_smoke;
pub mod staging_smoke;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    Name,
    Fuel,
    Throttle,
    CurrentStage,
    Target,
}

#[derive(Clone)]
pub struct Draft {
    pub key: PartKey,
    pub original: Part,
    pub name: String,
    pub fuel: String,
    pub throttle: String,
    pub current_stage: String,
    pub target: String,
    pub active: bool,
    pub staging: Option<StagingState>,
}

impl Draft {
    pub fn new(key: PartKey, part: &Part) -> Self {
        let pod = part.pod.as_ref();
        let staging = pod.and_then(|pod| pod.staging.clone());
        Self {
            key,
            original: part.clone(),
            name: pod.map(|pod| pod.name.clone()).unwrap_or_default(),
            fuel: part.fuel.map(|fuel| fuel.to_string()).unwrap_or_default(),
            throttle: pod.map(|pod| pod.throttle.to_string()).unwrap_or_default(),
            current_stage: staging
                .as_ref()
                .map(|s| s.current_stage)
                .unwrap_or(0)
                .to_string(),
            target: String::new(),
            active: part.active,
            staging,
        }
    }

    pub fn field(&mut self, field: Field) -> &mut String {
        match field {
            Field::Name => &mut self.name,
            Field::Fuel => &mut self.fuel,
            Field::Throttle => &mut self.throttle,
            Field::CurrentStage => &mut self.current_stage,
            Field::Target => &mut self.target,
        }
    }

    /// PartKey 与属性类型仍指向原对象即可；位置、旋转及其他字段可以并行变化。
    fn current_part<'a>(&self, document: &'a EditorDocument) -> Result<&'a Part, String> {
        let current = document
            .ship
            .part_at(self.key)
            .ok_or("属性所属部件已删除，请取消草稿")?;
        if current.part_type != self.original.part_type
            || current.pod.is_some() != self.original.pod.is_some()
            || current.fuel_kind != self.original.fuel_kind
        {
            return Err("部件类型已变更，请取消后重新打开属性".into());
        }
        Ok(current)
    }

    pub fn command(&self, document: &EditorDocument) -> Result<EditorCommand, String> {
        let current = self.current_part(document)?;
        let id = self.original.id;
        let mut commands = vec![];
        let active = merge_field(
            "启用状态",
            &self.original.active,
            &self.active,
            &current.active,
        )?;
        if active != current.active {
            commands.push(EditorCommand::SetActive(id, active));
        }
        if self.original.fuel_kind.is_some() {
            let original_text = self
                .original
                .fuel
                .map(|fuel| fuel.to_string())
                .unwrap_or_default();
            let fuel = if self.fuel == original_text {
                self.original.fuel
            } else {
                Some(number(&self.fuel, "燃料")?)
            };
            if fuel != self.original.fuel {
                let value = fuel.unwrap();
                if value < 0.0 {
                    return Err("燃料不能小于零".into());
                }
                if let Some(max) = document
                    .catalog
                    .get(&self.original.part_type)
                    .and_then(|kind| kind.tank.as_ref())
                    .map(|tank| tank.fuel)
                    && value > max
                {
                    return Err(format!("燃料不能超过容量 {max}"));
                }
            }
            let fuel = merge_field("燃料", &self.original.fuel, &fuel, &current.fuel)?;
            if fuel != current.fuel {
                commands.push(EditorCommand::SetFuel(id, fuel.unwrap()));
            }
        }
        if let Some(pod) = &self.original.pod {
            let current_pod = current.pod.as_ref().unwrap();
            let name = merge_field("名称", &pod.name, &self.name, &current_pod.name)?;
            if name != current_pod.name {
                commands.push(EditorCommand::RenamePod(id, name));
            }
            let throttle = if self.throttle == pod.throttle.to_string() {
                pod.throttle
            } else {
                number(&self.throttle, "油门")?
            };
            if throttle != pod.throttle && !(0.0..=1.0).contains(&throttle) {
                return Err("油门必须在 0 到 1 之间".into());
            }
            let throttle = merge_field("油门", &pod.throttle, &throttle, &current_pod.throttle)?;
            if throttle != current_pod.throttle {
                commands.push(EditorCommand::SetThrottle(id, throttle));
            }
            let mut staging = self.staging.clone();
            if let Some(staging) = &mut staging {
                let original_stage = pod.staging.as_ref().map_or(0, |state| state.current_stage);
                staging.current_stage = if self.current_stage == original_stage.to_string() {
                    original_stage
                } else {
                    self.current_stage
                        .parse::<i32>()
                        .ok()
                        .filter(|value| *value >= 0)
                        .ok_or("当前级必须是非负整数")?
                };
            }
            let staging = merge_staging(&pod.staging, &staging, &current_pod.staging)?;
            if staging != current_pod.staging {
                // 当前级和分级步骤合并后仍然只提交一个 SetStaging。
                commands.push(EditorCommand::SetStaging(id, staging));
            }
        }
        Ok(EditorCommand::Batch(commands).at(self.key))
    }
}

/// 未改字段保留当前文档；同字段已有相同结果也无需重复提交。
fn merge_field<T: PartialEq + Clone>(
    label: &str,
    original: &T,
    draft: &T,
    current: &T,
) -> Result<T, String> {
    if draft == original {
        Ok(current.clone())
    } else if current == original || current == draft {
        Ok(draft.clone())
    } else {
        Err(format!(
            "{label}已变更，与属性草稿冲突；请保留草稿并撤销外部修改，或取消后重开"
        ))
    }
}

fn merge_staging(
    original: &Option<StagingState>,
    draft: &Option<StagingState>,
    current: &Option<StagingState>,
) -> Result<Option<StagingState>, String> {
    match (original, draft, current) {
        (Some(original), Some(draft), Some(current)) => Ok(Some(StagingState {
            current_stage: merge_field(
                "当前级",
                &original.current_stage,
                &draft.current_stage,
                &current.current_stage,
            )?,
            steps: merge_field("分级步骤", &original.steps, &draft.steps, &current.steps)?,
        })),
        _ => merge_field("分级计划", original, draft, current),
    }
}

fn number(value: &str, label: &str) -> Result<f64, String> {
    value
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
        .ok_or_else(|| format!("{label}必须是有限数字"))
}

#[derive(Resource, Default)]
pub struct Inspector {
    pub draft: Option<Draft>,
    pub repair: Option<DuplicateRepair>,
    pub focus: Option<Field>,
    pub error: String,
    pub preview: Preview,
    /// 仅控制高级属性与目标抽屉的展开，不写入草稿或文档。
    pub advanced_open: bool,
    pub targets_open: bool,
    /// 本帧悬停优先显示，离开控件后恢复 preview；不创建编辑动作。
    pub hovered: Option<Preview>,
    history: history::DraftHistory,
    observed_targets: BTreeSet<i64>,
    refresh_error: Option<String>,
}

/// 仅界面焦点，不写分级数据、文档选择或历史。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Preview {
    #[default]
    Owner,
    Target,
    Stage(usize),
    Activation(usize, usize),
}

impl Inspector {
    pub fn can_undo(&self) -> bool {
        self.repair.is_none()
            && self
                .draft
                .as_ref()
                .is_some_and(|draft| self.history.can_undo(draft))
    }

    pub fn can_redo(&self) -> bool {
        self.repair.is_none()
            && self
                .draft
                .as_ref()
                .is_some_and(|draft| self.history.can_redo(draft))
    }

    /// 返回是否消费了草稿历史；耗尽时调用方可以继续文档撤销。
    pub fn undo_draft(&mut self) -> bool {
        if self.repair.is_some() {
            return false;
        }
        let changed = self
            .draft
            .as_mut()
            .is_some_and(|draft| self.history.undo(draft));
        if changed {
            self.after_history_change();
        }
        changed
    }

    pub fn redo_draft(&mut self) -> bool {
        if self.repair.is_some() {
            return false;
        }
        let changed = self
            .draft
            .as_mut()
            .is_some_and(|draft| self.history.redo(draft));
        if changed {
            self.after_history_change();
        }
        changed
    }

    fn after_history_change(&mut self) {
        self.preview = Preview::Owner;
        self.hovered = None;
        self.error.clear();
        self.refresh_error = None;
    }

    /// 观察直接文本编辑与文档变化；保留草稿，不写入文档或文档历史。
    pub fn refresh(&mut self, document: &EditorDocument) {
        self.history.observe(self.draft.as_ref());
        if let Some(previous) = self.refresh_error.take()
            && self.error == previous
        {
            self.error.clear();
        }
        if let Some(draft) = &self.draft {
            if let Some((parts, _)) = document.ship.group(draft.key.group) {
                self.observed_targets
                    .extend(unique_target_parts(parts).map(|part| part.id));
            }
            if self.repair.is_none()
                && let Err(error) = self.validate_references(document)
            {
                self.error.clone_from(&error);
                self.refresh_error = Some(error);
            }
        }
    }

    fn has_changes(&self) -> bool {
        self.repair.is_some()
            || self
                .draft
                .as_ref()
                .is_some_and(history::DraftHistory::changed)
    }

    fn validate_references(&self, document: &EditorDocument) -> Result<(), String> {
        let Some(draft) = &self.draft else {
            return Ok(());
        };
        draft.current_part(document)?;
        let original_ids: BTreeSet<_> = draft
            .original
            .pod
            .as_ref()
            .and_then(|pod| pod.staging.as_ref())
            .into_iter()
            .flat_map(|state| &state.steps)
            .flat_map(|step| &step.activations)
            .map(|activation| activation.id)
            .collect();
        for activation in draft
            .staging
            .as_ref()
            .into_iter()
            .flat_map(|state| &state.steps)
            .flat_map(|step| &step.activations)
        {
            if document
                .ship
                .group_part(draft.key.group, activation.id)
                .is_none()
                && (self.observed_targets.contains(&activation.id)
                    || !original_ids.contains(&activation.id))
            {
                return Err(format!(
                    "分级目标 {} 已删除或不再唯一；草稿已保留，请撤销外部修改或移除该动作",
                    activation.id
                ));
            }
        }
        Ok(())
    }

    pub fn is_open(&self) -> bool {
        self.draft.is_some()
    }

    pub fn guard_file_input(&mut self) {
        self.error = "请先应用或取消属性草稿，再打开文件或关闭窗口".into();
    }

    pub fn docked(&self) -> bool {
        self.repair.is_none()
            && self
                .draft
                .as_ref()
                .is_some_and(|draft| draft.original.pod.is_some())
    }

    pub fn preview_keys(&self, document: &EditorDocument) -> Vec<PartKey> {
        let Some(draft) = self.draft.as_ref().filter(|_| self.docked()) else {
            return vec![];
        };
        if draft.current_part(document).is_err() {
            return vec![];
        }
        let ids: Vec<_> = match self.hovered.unwrap_or(self.preview) {
            Preview::Owner => return vec![draft.key],
            Preview::Target => draft.target.trim().parse().ok().into_iter().collect(),
            Preview::Stage(stage) => draft
                .staging
                .as_ref()
                .and_then(|staging| staging.steps.get(stage))
                .map(|step| {
                    step.activations
                        .iter()
                        .map(|activation| activation.id)
                        .collect()
                })
                .unwrap_or_default(),
            Preview::Activation(stage, index) => draft
                .staging
                .as_ref()
                .and_then(|staging| staging.steps.get(stage))
                .and_then(|step| step.activations.get(index))
                .map(|activation| activation.id)
                .into_iter()
                .collect(),
        };
        ids.into_iter()
            .filter(|id| document.ship.group_part(draft.key.group, *id).is_some())
            .map(|id| PartKey::new(draft.key.group, id, 0))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }
}

#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub enum Action {
    Open,
    Cancel,
    Apply,
    OpenRepair,
    BackToProperties,
    RepairTarget(usize),
    Focus(Field),
    Active,
    CycleTarget(bool),
    ToggleAdvanced,
    ToggleTargets,
    GenerateStaging,
    /// 插入重排整卡，to 是最终下标。
    ReorderStep {
        from: usize,
        to: usize,
    },
    /// to_index 是删除源动作后在目标卡内的最终插入下标。
    MoveActivation {
        from_stage: usize,
        index: usize,
        to_stage: usize,
        to_index: usize,
    },
    AddStep,
    RemoveStep(usize),
    MoveStep(usize, bool),
    AddActivation(usize),
    RemoveActivation(usize, usize),
    Moved(usize, usize),
    SelectStage(usize),
    SelectActivation(usize, usize),
}

fn button(action: Action) -> impl Bundle {
    (
        native_buttons::press_triggered(),
        action,
        BackgroundColor(native_buttons::BUTTON),
    )
}

pub fn setup(mut commands: Commands) {
    commands.spawn(button(Action::Open)).insert((
        Node {
            position_type: PositionType::Absolute,
            bottom: px(4),
            left: px(48),
            width: px(48),
            height: px(48),
            border: UiRect::all(px(1)),
            border_radius: native_buttons::native_corners(),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            ..default()
        },
        panels::EditorPanel,
        panels::ToolbarItem::Properties,
    ));
}

pub fn open(inspector: &mut Inspector, document: &EditorDocument) {
    let part = document
        .selected
        .and_then(|key| document.ship.part_at(key).map(|part| (key, part)))
        .or_else(|| {
            document
                .ship
                .keyed_parts()
                .find(|(_, part)| part.pod.is_some())
        });
    *inspector = Inspector::default();
    if let Some((key, part)) = part {
        inspector.draft = Some(Draft::new(key, part));
        inspector.refresh(document);
    }
}

/// 按本组原部件顺序保留唯一编号目标，排除重号的所有实例。
fn unique_target_parts(parts: &[Part]) -> impl Iterator<Item = &Part> {
    let mut counts = std::collections::HashMap::<i64, usize>::new();
    for part in parts {
        *counts.entry(part.id).or_default() += 1;
    }
    parts.iter().filter(move |part| counts[&part.id] == 1)
}

fn reorder_step(staging: &mut StagingState, from: usize, to: usize) -> Result<(), &'static str> {
    if from >= staging.steps.len() || to >= staging.steps.len() {
        return Err("分级位置已失效");
    }
    let step = staging.steps.remove(from);
    staging.steps.insert(to, step);
    Ok(())
}

fn move_activation(
    staging: &mut StagingState,
    from_stage: usize,
    index: usize,
    to_stage: usize,
    to_index: usize,
) -> Result<(), &'static str> {
    let id = staging
        .steps
        .get(from_stage)
        .and_then(|step| step.activations.get(index))
        .ok_or("激活动作位置已失效")?
        .id;
    let destination = staging.steps.get(to_stage).ok_or("分级位置已失效")?;
    let same_stage = from_stage == to_stage;
    let destination_len = destination.activations.len() - usize::from(same_stage);
    if to_index > destination_len {
        return Err("激活动作插入位置已失效");
    }
    if destination
        .activations
        .iter()
        .enumerate()
        .any(|(candidate, activation)| activation.id == id && (!same_stage || candidate != index))
    {
        return Err("该级已有这个部件的激活动作");
    }

    // 所有索引和目标重复检查完成后再删除源，失败时完整保留草稿。
    let mut activation = staging.steps[from_stage].activations.remove(index);
    if !same_stage {
        activation.moved = true;
    }
    staging.steps[to_stage]
        .activations
        .insert(to_index, activation);
    Ok(())
}

pub fn act(action: &Action, inspector: &mut Inspector, document: &mut EditorDocument) {
    inspector.refresh(document);
    act_inner(action, inspector, document);
    inspector.refresh(document);
}

fn act_inner(action: &Action, inspector: &mut Inspector, document: &mut EditorDocument) {
    match action {
        Action::Open => {
            if !inspector.is_open() {
                open(inspector, document);
            } else if inspector.has_changes() {
                inspector.error = "草稿有修改，请先应用或取消；再次打开会保留当前草稿".into();
            } else {
                *inspector = Inspector::default();
            }
            return;
        }
        Action::Cancel => {
            *inspector = Inspector::default();
            return;
        }
        Action::ToggleAdvanced => {
            inspector.advanced_open = !inspector.advanced_open;
            return;
        }
        Action::ToggleTargets => {
            inspector.targets_open = !inspector.targets_open;
            return;
        }
        Action::Apply => {
            if let Some(draft) = &inspector.draft {
                let command = if let Some(repair) = &inspector.repair {
                    Ok(EditorCommand::RepairDuplicates(Box::new(repair.clone())))
                } else {
                    inspector
                        .validate_references(document)
                        .and_then(|()| draft.command(document))
                };
                match command {
                    Ok(command) => {
                        if document.execute(command) {
                            *inspector = Inspector::default();
                        } else {
                            inspector.error = document.status.clone();
                        }
                    }
                    Err(error) => inspector.error = error,
                }
            }
            return;
        }
        Action::OpenRepair => {
            if let Some(draft) = &inspector.draft {
                let result = draft.command(document).and_then(|command| {
                    let mut candidate = document.ship.clone();
                    command
                        .apply(&mut candidate)
                        .map_err(|error| error.to_string())?;
                    if candidate != document.ship {
                        return Err("请先应用或取消属性修改，再修复重复编号".into());
                    }
                    DuplicateRepair::new(&document.ship, draft.key)
                        .map_err(|error| error.to_string())
                });
                match result {
                    Ok(repair) => {
                        inspector.repair = Some(repair);
                        inspector.focus = None;
                        inspector.error.clear();
                    }
                    Err(error) => inspector.error = error,
                }
            }
            return;
        }
        Action::BackToProperties => {
            inspector.repair = None;
            inspector.error.clear();
            return;
        }
        Action::SelectStage(stage) => {
            inspector.preview = Preview::Stage(*stage);
            return;
        }
        Action::SelectActivation(stage, index) => {
            inspector.preview = Preview::Activation(*stage, *index);
            return;
        }
        Action::RepairTarget(index) => {
            if let Some(repair) = &mut inspector.repair
                && let Some((_, target)) = repair.references().get(*index)
            {
                let next = target.map_or(Some(0), |value| {
                    (value + 1 < repair.new_ids().len()).then_some(value + 1)
                });
                let _ = repair.assign(*index, next);
                inspector.error.clear();
            }
            return;
        }
        _ => {}
    }
    if inspector.repair.is_some() {
        return;
    }
    let Some(draft) = &mut inspector.draft else {
        return;
    };
    inspector.error.clear();
    match *action {
        Action::Focus(field) => {
            inspector.focus = Some(field);
            if field == Field::Target {
                inspector.preview = Preview::Target;
            }
        }
        Action::Active => draft.active = !draft.active,
        Action::CycleTarget(forward) => {
            inspector.preview = Preview::Target;
            let group = document
                .ship
                .group(draft.key.group)
                .map(|(parts, _)| parts)
                .unwrap_or_default();
            let parts: Vec<_> = unique_target_parts(group).map(|part| part.id).collect();
            if !parts.is_empty() {
                let current = draft
                    .target
                    .parse::<i64>()
                    .ok()
                    .and_then(|id| parts.iter().position(|candidate| *candidate == id));
                let index = match (current, forward) {
                    (Some(index), true) => (index + 1) % parts.len(),
                    (Some(index), false) => (index + parts.len() - 1) % parts.len(),
                    (None, true) => 0,
                    (None, false) => parts.len() - 1,
                };
                draft.target = parts[index].to_string();
            }
        }
        Action::GenerateStaging => {
            let result = draft.current_part(document).and_then(|_| {
                dr_core::generate_basic_staging(&document.ship, &document.catalog, draft.key)
                    .map_err(|error| error.to_string())
            });
            match result {
                Ok(staging) => {
                    inspector.preview = if staging.steps.is_empty() {
                        Preview::Owner
                    } else {
                        Preview::Stage(0)
                    };
                    inspector.hovered = None;
                    draft.staging = Some(staging);
                    draft.current_stage = "0".into();
                }
                Err(error) => inspector.error = error,
            }
        }
        Action::ReorderStep { from, to } => {
            match draft
                .staging
                .as_mut()
                .ok_or("尚无分级计划")
                .and_then(|staging| reorder_step(staging, from, to))
            {
                Ok(()) => {
                    inspector.preview = Preview::Stage(to);
                    inspector.hovered = None;
                }
                Err(error) => inspector.error = error.into(),
            }
        }
        Action::MoveActivation {
            from_stage,
            index,
            to_stage,
            to_index,
        } => {
            match draft
                .staging
                .as_mut()
                .ok_or("尚无分级计划")
                .and_then(|staging| move_activation(staging, from_stage, index, to_stage, to_index))
            {
                Ok(()) => {
                    inspector.preview = Preview::Activation(to_stage, to_index);
                    inspector.hovered = None;
                }
                Err(error) => inspector.error = error.into(),
            }
        }
        Action::AddStep => {
            draft
                .staging
                .get_or_insert_with(StagingState::default)
                .steps
                .push(StageStep::default());
            inspector.preview = Preview::Stage(draft.staging.as_ref().unwrap().steps.len() - 1);
        }
        Action::RemoveStep(index) => {
            if let Some(staging) = &mut draft.staging
                && index < staging.steps.len()
            {
                staging.steps.remove(index);
                inspector.preview = Preview::Owner;
            }
        }
        Action::MoveStep(index, down) => {
            if let Some(staging) = &mut draft.staging {
                let to = if down {
                    index.checked_add(1)
                } else {
                    index.checked_sub(1)
                };
                if let Some(to) = to
                    && index < staging.steps.len()
                    && to < staging.steps.len()
                {
                    staging.steps.swap(index, to);
                    inspector.preview = Preview::Stage(to);
                }
            }
        }
        Action::AddActivation(index) => {
            let target = draft.target.trim().parse::<i64>().ok();
            if let Some(target) =
                target.filter(|id| document.ship.group_part(draft.key.group, *id).is_some())
            {
                if let Some(step) = draft.staging.as_mut().and_then(|s| s.steps.get_mut(index)) {
                    if step.activations.iter().any(|a| a.id == target) {
                        inspector.error = "该级已有这个部件的激活动作".into();
                    } else {
                        step.activations.push(Activation {
                            id: target,
                            moved: false,
                        });
                        inspector.preview = Preview::Activation(index, step.activations.len() - 1);
                    }
                }
            } else {
                inspector.error = "请输入本组中唯一存在的目标部件 ID".into();
            }
        }
        Action::RemoveActivation(stage, index) => {
            if let Some(step) = draft.staging.as_mut().and_then(|s| s.steps.get_mut(stage))
                && index < step.activations.len()
            {
                step.activations.remove(index);
                inspector.preview = Preview::Owner;
            }
        }
        Action::Moved(stage, index) => {
            if let Some(activation) = draft
                .staging
                .as_mut()
                .and_then(|s| s.steps.get_mut(stage))
                .and_then(|step| step.activations.get_mut(index))
            {
                activation.moved = !activation.moved;
                inspector.preview = Preview::Activation(stage, index);
            }
        }
        _ => {}
    }
}

#[allow(clippy::too_many_arguments)] // Bevy 系统参数分别声明资源访问，保持调度依赖可见。
pub fn actions(
    mut activated: MessageReader<native_buttons::Activated>,
    buttons: Query<&Action>,
    mut inspector: ResMut<Inspector>,
    mut document: ResMut<EditorDocument>,
    mut drag: ResMut<DragState>,
    mut cursor: ResMut<EditorCursor>,
    pending: Option<Res<files::PendingFileAction>>,
    help: Option<Res<help::HelpState>>,
    touch: Option<Res<mobile_input::GestureInput>>,
) {
    if help.is_some_and(|state| state.open || state.suppress_frame)
        || pending.is_some_and(|state| state.is_blocked())
    {
        activated.clear();
        return;
    }
    if let Some(touch) = touch.as_deref().filter(|input| input.override_mouse) {
        activated.clear();
        if matches!(
            touch.native_click.as_ref(),
            Some(mobile_input::NativeClick::Properties)
        ) {
            act(&Action::Open, &mut inspector, &mut document);
            drag.cancel();
            cursor.cancel_placement();
            cursor.paste = None;
        }
        return;
    }
    for event in activated.read() {
        if let Ok(action) = buttons.get(event.entity) {
            act(action, &mut inspector, &mut document);
            drag.cancel();
            cursor.cancel_placement();
            cursor.paste = None;
        }
    }
}

pub fn cancel_for_file_action(
    mut events: MessageReader<files::FileAction>,
    mut inspector: ResMut<Inspector>,
) {
    if events.read().next().is_some() {
        events.clear();
        *inspector = Inspector::default();
    }
}

#[cfg(test)]
mod tests;
