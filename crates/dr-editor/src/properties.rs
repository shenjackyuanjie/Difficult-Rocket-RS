use super::*;
use bevy::input::{ButtonState, keyboard::KeyboardInput};
use bevy::window::Ime;
use dr_core::{
    Activation, ConnectionRole, DuplicateRepair, ReferenceSite, StageStep, StagingState,
};

pub(crate) mod repair_smoke;
pub(crate) mod smoke;
pub(crate) mod staging_smoke;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Field {
    Name,
    Fuel,
    Throttle,
    CurrentStage,
    Target,
}

#[derive(Clone)]
struct Draft {
    key: PartKey,
    original: Part,
    name: String,
    fuel: String,
    throttle: String,
    current_stage: String,
    target: String,
    active: bool,
    staging: Option<StagingState>,
}

impl Draft {
    fn new(key: PartKey, part: &Part) -> Self {
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

    fn field(&mut self, field: Field) -> &mut String {
        match field {
            Field::Name => &mut self.name,
            Field::Fuel => &mut self.fuel,
            Field::Throttle => &mut self.throttle,
            Field::CurrentStage => &mut self.current_stage,
            Field::Target => &mut self.target,
        }
    }

    fn command(&self, document: &EditorDocument) -> Result<EditorCommand, String> {
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
pub(crate) struct Inspector {
    draft: Option<Draft>,
    repair: Option<DuplicateRepair>,
    focus: Option<Field>,
    select_all: bool,
    caret: usize,
    preedit: String,
    error: String,
}

impl Inspector {
    pub(crate) fn is_open(&self) -> bool {
        self.draft.is_some()
    }

    pub(crate) fn guard_file_input(&mut self) {
        self.error = "请先应用或取消属性草稿，再打开文件或关闭窗口".into();
    }
}

pub(crate) fn closed(inspector: Option<Res<Inspector>>) -> bool {
    inspector.is_none_or(|inspector| inspector.draft.is_none())
}

#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub(crate) enum Action {
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

#[derive(Component)]
pub(crate) struct InspectorRoot;
#[derive(Component)]
pub(crate) struct InspectorScroll;

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

pub(crate) fn setup(mut commands: Commands, assets: Res<AssetServer>) {
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

fn open(inspector: &mut Inspector, document: &EditorDocument) {
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

fn act(action: &Action, inspector: &mut Inspector, document: &mut EditorDocument) {
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
                        inspector.preedit.clear();
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
            inspector.caret = draft.field(field).len();
            inspector.select_all = true;
            inspector.preedit.clear();
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
                if inspector.focus == Some(Field::Target) {
                    inspector.caret = draft.target.len();
                    inspector.select_all = true;
                }
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

pub(crate) fn actions(
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
            cursor.placing = false;
            cursor.paste = None;
        }
    }
}

fn insert_text(inspector: &mut Inspector, text: &str) {
    let Some(field) = inspector.focus else {
        return;
    };
    let Some(draft) = &mut inspector.draft else {
        return;
    };
    let value = draft.field(field);
    if inspector.select_all {
        value.clear();
        inspector.caret = 0;
    }
    inspector.select_all = false;
    let text: String = text.chars().filter(|c| !c.is_control()).collect();
    value.insert_str(inspector.caret, &text);
    inspector.caret += text.len();
}

fn edit_key(inspector: &mut Inspector, key: KeyCode) {
    let Some(field) = inspector.focus else {
        return;
    };
    let Some(draft) = &mut inspector.draft else {
        return;
    };
    let value = draft.field(field);
    let previous = value[..inspector.caret]
        .char_indices()
        .next_back()
        .map(|(i, _)| i)
        .unwrap_or(0);
    let next = value[inspector.caret..]
        .chars()
        .next()
        .map(|c| inspector.caret + c.len_utf8())
        .unwrap_or(value.len());
    match key {
        KeyCode::Backspace | KeyCode::Delete if inspector.select_all => {
            value.clear();
            inspector.caret = 0;
        }
        KeyCode::Backspace => {
            value.drain(previous..inspector.caret);
            inspector.caret = previous;
        }
        KeyCode::Delete => {
            value.drain(inspector.caret..next);
        }
        KeyCode::ArrowLeft => inspector.caret = previous,
        KeyCode::ArrowRight => inspector.caret = next,
        KeyCode::Home => inspector.caret = 0,
        KeyCode::End => inspector.caret = value.len(),
        _ => return,
    }
    inspector.select_all = false;
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn input(
    mut inspector: ResMut<Inspector>,
    document: Res<EditorDocument>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut events: MessageReader<KeyboardInput>,
    mut ime: MessageReader<Ime>,
    mut windows: Query<&mut Window>,
    mut drag: ResMut<DragState>,
    mut cursor: ResMut<EditorCursor>,
) {
    if inspector.draft.is_none() {
        if keys.just_pressed(KeyCode::F2) {
            open(&mut inspector, &document);
            drag.cancel();
            cursor.placing = false;
            cursor.paste = None;
        }
        events.clear();
        ime.clear();
    } else {
        let control = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
        let mut committed = false;
        for event in ime.read() {
            match event {
                Ime::Preedit { value, .. } => inspector.preedit = value.clone(),
                Ime::Commit { value, .. } => {
                    insert_text(&mut inspector, value);
                    inspector.preedit.clear();
                    committed = true;
                }
                Ime::Disabled { .. } => inspector.preedit.clear(),
                _ => {}
            }
        }
        if keys.just_pressed(KeyCode::Escape) && inspector.preedit.is_empty() && !committed {
            *inspector = Inspector::default();
        } else if control && keys.just_pressed(KeyCode::KeyA) {
            inspector.select_all = true;
        } else if !control && inspector.preedit.is_empty() && !committed {
            for event in events
                .read()
                .filter(|event| event.state == ButtonState::Pressed)
            {
                edit_key(&mut inspector, event.key_code);
                if let Some(text) = &event.text {
                    insert_text(&mut inspector, text);
                }
            }
        }
        events.clear();
        // 草稿输入独占快捷键，避免输入 R/X/Y/Delete 或 Ctrl+S 改写画布。
        keys.clear();
    }
    for mut window in &mut windows {
        window.ime_enabled = inspector.draft.is_some() && inspector.focus == Some(Field::Name);
        if window.ime_enabled {
            window.ime_position = Vec2::new(window.width() * 0.4, window.height() * 0.35);
        }
    }
}

pub(crate) fn cancel_for_file_action(
    mut events: MessageReader<files::FileAction>,
    mut inspector: ResMut<Inspector>,
) {
    if events.read().next().is_some() {
        events.clear();
        *inspector = Inspector::default();
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn render(
    mut commands: Commands,
    inspector: Res<Inspector>,
    document: Res<EditorDocument>,
    assets: Res<AssetServer>,
    roots: Query<Entity, With<InspectorRoot>>,
    scrolling: Query<&ScrollPosition, With<InspectorScroll>>,
) {
    if !inspector.is_changed() {
        return;
    }
    let offset = scrolling
        .single()
        .map(|scroll| scroll.y)
        .unwrap_or_default();
    for entity in &roots {
        commands.entity(entity).despawn();
    }
    let Some(draft) = &inspector.draft else {
        return;
    };
    let font = assets.load("fonts/HarmonyOS_Sans/HarmonyOS_Sans_SC/HarmonyOS_Sans_SC_Regular.ttf");
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            GlobalZIndex(100),
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.7)),
            Interaction::None,
            panels::EditorPanel,
            InspectorRoot,
        ))
        .with_children(|overlay| {
            overlay
                .spawn((
                    Node {
                        width: px(700),
                        max_width: percent(95),
                        height: percent(85),
                        padding: UiRect::all(px(18)),
                        row_gap: px(8),
                        flex_direction: FlexDirection::Column,
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.04, 0.065, 0.09)),
                ))
                .with_children(|root| {
                    let kind = document.catalog.get(&draft.original.part_type);
                    root.spawn(label(
                        format!(
                            "{} · {}",
                            draft.key,
                            kind.map(|kind| kind.name.as_str())
                                .unwrap_or(&draft.original.part_type)
                        ),
                        &font,
                        22.0,
                    ));
                    root.spawn(label(
                        if inspector.repair.is_some() {
                            "修复重复编号：逐条指定引用归属，应用后可一次撤销。"
                        } else {
                            "修改暂存为草稿；应用后可一次撤销。点击字段后输入替换，Esc 取消。"
                        },
                        &font,
                        14.0,
                    ));
                    root.spawn((
                        Node {
                            flex_direction: FlexDirection::Column,
                            row_gap: px(8),
                            flex_grow: 1.0,
                            min_height: px(0),
                            overflow: Overflow::scroll_y(),
                            ..default()
                        },
                        panels::ScrollArea,
                        InspectorScroll,
                        ScrollPosition(Vec2::new(0.0, offset)),
                    ))
                    .with_children(|body| {
                        if let Some(repair) = &inspector.repair {
                            body.spawn(label(
                                format!(
                                    "待分配引用：{} / {}（点击引用按钮切换实例）",
                                    repair.unassigned(),
                                    repair.references().len()
                                ),
                                &font,
                                16.0,
                            ));
                            for (occurrence, part) in repair
                                .original()
                                .parts
                                .iter()
                                .filter(|part| part.id == repair.old_id())
                                .enumerate()
                            {
                                body.spawn(label(
                                    format!(
                                        "实例 {}：{}  坐标 ({:.3}, {:.3})",
                                        occurrence + 1,
                                        part.part_type,
                                        part.x,
                                        part.y
                                    ),
                                    &font,
                                    16.0,
                                ));
                                body.spawn(label(
                                    format!(
                                        "编号 #{} → #{}",
                                        part.id,
                                        repair.new_ids()[occurrence]
                                    ),
                                    &font,
                                    14.0,
                                ));
                            }
                            for (index, (site, target)) in repair.references().iter().enumerate() {
                                let description = match *site {
                                    ReferenceSite::Connection { index, role } => {
                                        let (parent, child) =
                                            match repair.original().connections[index] {
                                                Connection::Normal { parent, child, .. }
                                                | Connection::Dock { parent, child, .. } => {
                                                    (parent, child)
                                                }
                                            };
                                        let role = match role {
                                            ConnectionRole::Parent => "母端",
                                            ConnectionRole::Child => "子端",
                                            ConnectionRole::Dock => "对接插头",
                                        };
                                        format!("连接 {}：#{parent} → #{child} · {role}", index + 1)
                                    }
                                    ReferenceSite::Activation {
                                        owner,
                                        stage,
                                        index,
                                    } => format!(
                                        "驾驶舱 #{} 实例 {} · 第 {stage} 级 · 动作 {}",
                                        owner.id,
                                        owner.occurrence + 1,
                                        index + 1
                                    ),
                                };
                                body.spawn(label(description, &font, 14.0));
                                if let ReferenceSite::Connection { index, role } = *site {
                                    let peer_id = match repair.original().connections[index] {
                                        Connection::Normal { parent, child, .. }
                                        | Connection::Dock { parent, child, .. } => {
                                            if role == ConnectionRole::Child {
                                                parent
                                            } else {
                                                child
                                            }
                                        }
                                    };
                                    for peer in repair
                                        .original()
                                        .parts
                                        .iter()
                                        .filter(|part| part.id == peer_id)
                                    {
                                        body.spawn(label(
                                            format!(
                                                "相邻 #{} · {} ({:.2}, {:.2})",
                                                peer.id, peer.part_type, peer.x, peer.y
                                            ),
                                            &font,
                                            14.0,
                                        ));
                                    }
                                }
                                let text = target
                                    .map(|target| {
                                        format!(
                                            "归属：实例 {}（#{}）",
                                            target + 1,
                                            repair.new_ids()[target]
                                        )
                                    })
                                    .unwrap_or_else(|| "归属：未指定".into());
                                body.spawn(button(Action::RepairTarget(index), target.is_some()))
                                    .with_children(|button| {
                                        button.spawn(label(text, &font, 16.0));
                                    });
                            }
                            if repair.references().is_empty() {
                                body.spawn(label(
                                    "没有连接或分级引用；应用后各实例使用独立编号。",
                                    &font,
                                    14.0,
                                ));
                            }
                            body.spawn(button(Action::BackToProperties, false))
                                .with_children(|button| {
                                    button.spawn(label("返回属性", &font, 16.0));
                                });
                            return;
                        }
                        if document
                            .ship
                            .group(draft.key.group)
                            .is_some_and(|(parts, _)| {
                                parts.iter().filter(|part| part.id == draft.key.id).count() > 1
                            })
                        {
                            body.spawn(button(Action::OpenRepair, false))
                                .with_children(|button| {
                                    button.spawn(label("本组编号重复 · 修复引用归属", &font, 16.0));
                                });
                        }
                        body.spawn(button(Action::Active, draft.active))
                            .with_children(|button| {
                                button.spawn(label(
                                    format!(
                                        "激活状态：{}",
                                        if draft.active {
                                            "已激活"
                                        } else {
                                            "未激活"
                                        }
                                    ),
                                    &font,
                                    16.0,
                                ));
                            });
                        let mut fields = vec![];
                        if draft.original.fuel_kind.is_some() {
                            fields.push((Field::Fuel, "燃料", draft.fuel.as_str()));
                        }
                        if draft.original.pod.is_some() {
                            fields.push((Field::Name, "船体名称", draft.name.as_str()));
                            fields.push((Field::Throttle, "油门（0～1）", draft.throttle.as_str()));
                            if draft.staging.is_some() {
                                fields.push((
                                    Field::CurrentStage,
                                    "当前级（从 0 起）",
                                    draft.current_stage.as_str(),
                                ));
                            }
                            fields.push((
                                Field::Target,
                                "新增激活动作的目标部件 ID",
                                draft.target.as_str(),
                            ));
                        }
                        for (field, title, value) in fields {
                            body.spawn(label(title, &font, 14.0));
                            let focused = inspector.focus == Some(field);
                            let display = if focused {
                                if inspector.select_all {
                                    format!("【{value}】{}", inspector.preedit)
                                } else {
                                    format!(
                                        "{}{}│{}",
                                        &value[..inspector.caret],
                                        inspector.preedit,
                                        &value[inspector.caret..]
                                    )
                                }
                            } else if value.is_empty() {
                                "（空）".into()
                            } else {
                                value.to_owned()
                            };
                            body.spawn(button(Action::Focus(field), focused))
                                .with_children(|button| {
                                    button.spawn((
                                        label(display, &font, 16.0),
                                        Node {
                                            width: px(590),
                                            ..default()
                                        },
                                    ));
                                });
                        }
                        if draft.original.pod.is_some() {
                            body.spawn((Node {
                                column_gap: px(6),
                                flex_shrink: 0.0,
                                align_items: AlignItems::Center,
                                ..default()
                            },))
                                .with_children(|row| {
                                    for (text, action) in [
                                        ("上一部件", Action::CycleTarget(false)),
                                        ("下一部件", Action::CycleTarget(true)),
                                    ] {
                                        row.spawn(button(action, false)).with_children(|button| {
                                            button.spawn(label(text, &font, 14.0));
                                        });
                                    }
                                    let target = draft.target.parse::<i64>().ok().and_then(|id| {
                                        document.ship.group_part(draft.key.group, id)
                                    });
                                    let target_name = target
                                        .and_then(|part| document.catalog.get(&part.part_type))
                                        .map(|kind| kind.name.as_str())
                                        .unwrap_or("未选目标");
                                    row.spawn(label(target_name, &font, 14.0));
                                });
                            body.spawn(button(Action::AddStep, false))
                                .with_children(|button| {
                                    button.spawn(label("添加分级步骤", &font, 16.0));
                                });
                        }
                        if let Some(staging) = &draft.staging {
                            for (index, step) in staging.steps.iter().enumerate() {
                                body.spawn((Node {
                                    column_gap: px(6),
                                    flex_wrap: FlexWrap::Wrap,
                                    flex_shrink: 0.0,
                                    align_items: AlignItems::Center,
                                    ..default()
                                },))
                                    .with_children(|row| {
                                        row.spawn(label(format!("第 {index} 级"), &font, 18.0));
                                        for (text, action) in [
                                            ("上移", Action::MoveStep(index, false)),
                                            ("下移", Action::MoveStep(index, true)),
                                            ("删除级", Action::RemoveStep(index)),
                                            ("添加目标", Action::AddActivation(index)),
                                        ] {
                                            row.spawn(button(action, false)).with_children(
                                                |button| {
                                                    button.spawn(label(text, &font, 14.0));
                                                },
                                            );
                                        }
                                    });
                                for (activation_index, activation) in
                                    step.activations.iter().enumerate()
                                {
                                    body.spawn((Node {
                                        column_gap: px(6),
                                        flex_shrink: 0.0,
                                        height: px(34),
                                        align_items: AlignItems::Center,
                                        ..default()
                                    },))
                                        .with_children(|row| {
                                            let name = document
                                                .ship
                                                .group_part(draft.key.group, activation.id)
                                                .and_then(|part| {
                                                    document.catalog.get(&part.part_type)
                                                })
                                                .map(|kind| kind.name.as_str())
                                                .unwrap_or("未知部件");
                                            row.spawn((
                                                label(
                                                    format!("激活 #{} · {name}", activation.id),
                                                    &font,
                                                    14.0,
                                                ),
                                                Node {
                                                    width: px(320),
                                                    ..default()
                                                },
                                            ));
                                            for (text, action) in [
                                                (
                                                    format!(
                                                        "移动标记：{}",
                                                        if activation.moved {
                                                            "是"
                                                        } else {
                                                            "否"
                                                        }
                                                    ),
                                                    Action::Moved(index, activation_index),
                                                ),
                                                (
                                                    "移除".into(),
                                                    Action::RemoveActivation(
                                                        index,
                                                        activation_index,
                                                    ),
                                                ),
                                            ] {
                                                row.spawn(button(action, false)).with_children(
                                                    |button| {
                                                        button.spawn(label(text, &font, 14.0));
                                                    },
                                                );
                                            }
                                        });
                                }
                            }
                        }
                    });
                    if !inspector.error.is_empty() {
                        root.spawn(label(&inspector.error, &font, 16.0));
                    }
                    root.spawn((Node {
                        column_gap: px(10),
                        ..default()
                    },))
                        .with_children(|row| {
                            for (text, action) in
                                [("应用", Action::Apply), ("取消", Action::Cancel)]
                            {
                                row.spawn(button(action, false)).with_children(|button| {
                                    button.spawn(label(text, &font, 18.0));
                                });
                            }
                        });
                });
        });
}

#[cfg(test)]
mod tests;
