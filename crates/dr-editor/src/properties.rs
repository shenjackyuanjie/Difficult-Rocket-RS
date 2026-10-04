use super::*;
use dr_core::{
    Activation, ConnectionRole, DuplicateRepair, ReferenceSite, StageStep, StagingState,
};

pub mod egui_panel;
pub mod egui_smoke;
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

    pub fn command(&self, document: &EditorDocument) -> Result<EditorCommand, String> {
        if document.ship.part_at(self.key) != Some(&self.original) {
            return Err("部件已变更，请取消后重新打开属性".into());
        }
        let id = self.original.id;
        let mut commands = vec![];
        if self.active != self.original.active {
            commands.push(EditorCommand::SetActive(id, self.active));
        }
        if self.original.fuel_kind.is_some() {
            let fuel = number(&self.fuel, "燃料")?;
            if Some(fuel) != self.original.fuel {
                if fuel < 0.0 {
                    return Err("燃料不能小于零".into());
                }
                if let Some(max) = document
                    .catalog
                    .get(&self.original.part_type)
                    .and_then(|kind| kind.tank.as_ref())
                    .map(|tank| tank.fuel)
                    && fuel > max
                {
                    return Err(format!("燃料不能超过容量 {max}"));
                }
                commands.push(EditorCommand::SetFuel(id, fuel));
            }
        }
        if let Some(pod) = &self.original.pod {
            if self.name != pod.name {
                commands.push(EditorCommand::RenamePod(id, self.name.clone()));
            }
            let throttle = number(&self.throttle, "油门")?;
            if throttle != pod.throttle {
                if !(0.0..=1.0).contains(&throttle) {
                    return Err("油门必须在 0 到 1 之间".into());
                }
                commands.push(EditorCommand::SetThrottle(id, throttle));
            }
            let mut staging = self.staging.clone();
            if let Some(staging) = &mut staging {
                staging.current_stage = self
                    .current_stage
                    .parse::<i32>()
                    .ok()
                    .filter(|value| *value >= 0)
                    .ok_or("当前级必须是非负整数")?;
            }
            if staging != pod.staging {
                commands.push(EditorCommand::SetStaging(id, staging));
            }
        }
        Ok(EditorCommand::Batch(commands).at(self.key))
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
}

impl Inspector {
    pub fn is_open(&self) -> bool {
        self.draft.is_some()
    }

    pub fn guard_file_input(&mut self) {
        self.error = "请先应用或取消属性草稿，再打开文件或关闭窗口".into();
    }
}

pub fn closed(inspector: Option<Res<Inspector>>) -> bool {
    inspector.is_none_or(|inspector| inspector.draft.is_none())
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
    AddStep,
    RemoveStep(usize),
    MoveStep(usize, bool),
    AddActivation(usize),
    RemoveActivation(usize, usize),
    Moved(usize, usize),
}

fn label(value: impl Into<String>, font: &Handle<Font>, size: f32) -> impl Bundle {
    (
        Text::new(value),
        TextFont {
            font: bevy::text::FontSource::Handle(font.clone()),
            font_size: FontSize::Px(size),
            ..default()
        },
        TextColor(Color::srgb(0.92, 0.95, 1.0)),
        TextLayout::default().with_no_wrap(),
    )
}

fn button(action: Action, selected: bool) -> impl Bundle {
    (
        Button,
        action,
        Node {
            padding: UiRect::axes(px(10), px(6)),
            flex_shrink: 0.0,
            min_height: px(30),
            height: px(34),
            overflow: Overflow::clip(),
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(if selected {
            Color::srgb(0.2, 0.42, 0.53)
        } else {
            Color::srgb(0.13, 0.18, 0.24)
        }),
    )
}

pub fn setup(mut commands: Commands, assets: Res<AssetServer>) {
    let font = assets.load("fonts/HarmonyOS_Sans/HarmonyOS_Sans_SC/HarmonyOS_Sans_SC_Regular.ttf");
    commands
        .spawn(button(Action::Open, false))
        .insert((
            Node {
                position_type: PositionType::Absolute,
                top: px(12),
                left: px(390),
                padding: UiRect::axes(px(12), px(7)),
                ..default()
            },
            panels::EditorPanel,
        ))
        .with_children(|root| {
            root.spawn(label("属性 / 分级  F2", &font, 16.0));
        });
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
    }
}

pub fn act(action: &Action, inspector: &mut Inspector, document: &mut EditorDocument) {
    match action {
        Action::Open => {
            open(inspector, document);
            return;
        }
        Action::Cancel => {
            *inspector = Inspector::default();
            return;
        }
        Action::Apply => {
            if let Some(draft) = &inspector.draft {
                let command = if let Some(repair) = &inspector.repair {
                    Ok(EditorCommand::RepairDuplicates(Box::new(repair.clone())))
                } else {
                    draft.command(document)
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
        }
        Action::Active => draft.active = !draft.active,
        Action::CycleTarget(forward) => {
            let group = document
                .ship
                .group(draft.key.group)
                .map(|(parts, _)| parts)
                .unwrap_or_default();
            let mut counts = std::collections::HashMap::<i64, usize>::new();
            for part in group {
                *counts.entry(part.id).or_default() += 1;
            }
            let parts: Vec<_> = group
                .iter()
                .filter(|part| counts[&part.id] == 1)
                .map(|part| part.id)
                .collect();
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
        Action::AddStep => {
            draft
                .staging
                .get_or_insert_with(StagingState::default)
                .steps
                .push(StageStep::default());
        }
        Action::RemoveStep(index) => {
            if let Some(staging) = &mut draft.staging
                && index < staging.steps.len()
            {
                staging.steps.remove(index);
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
            }
        }
        _ => {}
    }
}

pub fn actions(
    buttons: Query<(&Interaction, &Action), Changed<Interaction>>,
    mut inspector: ResMut<Inspector>,
    mut document: ResMut<EditorDocument>,
    mut drag: ResMut<DragState>,
    mut cursor: ResMut<EditorCursor>,
) {
    for (interaction, action) in &buttons {
        if *interaction == Interaction::Pressed {
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
