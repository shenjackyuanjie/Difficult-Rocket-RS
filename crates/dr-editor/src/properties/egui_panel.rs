//! egui 属性面板：只编辑草稿，结构操作仍通过原有 Action/act 事务提交。
//!
//! TextEdit 独占文本光标、选区、剪贴板与输入法状态；不要同时运行旧的
//! properties::keyboard/native_ime 输入路径，否则一次输入会被处理两次。

use super::*;
use bevy_egui::egui;

/// 命中矩形使用 egui 的逻辑坐标；调用方换算窗口光标时需考虑 pixels_per_point。
/// 本函数追加而不清空 hits，便于调用方同时记录打开属性的工具栏按钮。
pub fn show(
    ctx: &egui::Context,
    inspector: &mut Inspector,
    document: &EditorDocument,
    hits: &mut Vec<(Action, egui::Rect)>,
) -> Option<Action> {
    let Some(draft) = &inspector.draft else {
        return None;
    };
    let title = format!(
        "{} · {}",
        draft.key,
        document
            .catalog
            .get(&draft.original.part_type)
            .map(|kind| kind.name.as_str())
            .unwrap_or(&draft.original.part_type)
    );
    let available = ctx.content_rect();
    let width = (available.width() - 40.0).clamp(160.0, 700.0);
    let body_height = (available.height() - 220.0).max(60.0);
    let mut action = None;
    let mut edited = false;
    let mut focused = None;
    let modal = egui::Modal::new(egui::Id::new("part_properties_modal")).show(ctx, |ui| {
        ui.set_width(width);
        ui.heading(title);
        ui.small(if inspector.repair.is_some() {
            "引用归属 · 草稿"
        } else {
            "属性 / 分级 · 草稿"
        })
        .on_hover_text(
            "修改仅暂存于草稿；应用后可一次撤销，取消不改动文档。引用修复需逐条指定归属。",
        );
        ui.separator();
        egui::ScrollArea::vertical()
            .id_salt(if inspector.repair.is_some() {
                "part_reference_repair_scroll"
            } else {
                "part_properties_scroll"
            })
            .max_height(body_height)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                if let Some(repair) = &inspector.repair {
                    repair_body(ui, repair, hits, &mut action);
                } else if let Some(draft) = &mut inspector.draft {
                    properties_body(
                        ui,
                        draft,
                        document,
                        hits,
                        &mut action,
                        &mut edited,
                        &mut focused,
                    );
                }
            });
        ui.separator();
        // 编辑字段只清除旧错误；本帧 Action 产生的新错误在下帧显示。
        if edited {
            inspector.error.clear();
        }
        if !inspector.error.is_empty() {
            ui.colored_label(egui::Color32::LIGHT_RED, &inspector.error);
        }
        ui.horizontal(|ui| {
            action_button(ui, "应用", Action::Apply, hits, &mut action);
            action_button(ui, "取消", Action::Cancel, hits, &mut action);
        });
    });
    inspector.focus = focused;
    // 点击遮罩不丢弃草稿。Escape 交给 egui 的模态/弹出层规则，输入法
    // 提交或取消候选的同一帧不能再作为关闭整个属性草稿的快捷键。
    let composition_frame = ctx.input(|input| {
        input
            .events
            .iter()
            .any(|event| matches!(event, egui::Event::Ime(_)))
    });
    if !composition_frame && !modal.backdrop_response.clicked() && modal.should_close() {
        action = Some(Action::Cancel);
    }
    action
}

fn record_hit(
    ui: &egui::Ui,
    response: &egui::Response,
    action: Action,
    hits: &mut Vec<(Action, egui::Rect)>,
) {
    let rect = response.rect.intersect(ui.clip_rect());
    if response.enabled() && rect.is_positive() {
        hits.push((action, rect));
    }
}

fn action_button(
    ui: &mut egui::Ui,
    text: &str,
    action: Action,
    hits: &mut Vec<(Action, egui::Rect)>,
    pending: &mut Option<Action>,
) {
    // 身份来自语义动作，不随短标签或引用归属文本变化。
    let response = ui
        .push_id(("property_action", format!("{action:?}")), |ui| {
            ui.button(text)
        })
        .inner;
    let hint = action_hint(&action);
    let response = response.on_hover_text(hint).on_disabled_hover_text(hint);
    record_hit(ui, &response, action.clone(), hits);
    if response.clicked() {
        *pending = Some(action);
    }
}

fn action_hint(action: &Action) -> &'static str {
    match action {
        Action::Apply => "应用草稿到文档；整个修改可一次撤销。",
        Action::Cancel => "取消并丢弃草稿，不修改文档（Esc）。",
        Action::CycleTarget(false) => "选择本组上一个具有唯一编号的目标部件。",
        Action::CycleTarget(true) => "选择本组下一个具有唯一编号的目标部件。",
        Action::AddStep => "在末尾新增分级步骤，仅修改草稿。",
        Action::MoveStep(_, false) => "将此级上移一位；首级不可上移。",
        Action::MoveStep(_, true) => "将此级下移一位；末级不可下移。",
        Action::RemoveStep(_) => "删除此级及其中的激活动作，仅修改草稿。",
        Action::AddActivation(_) => "将上方选定的目标加入此级；目标须为本组唯一编号。",
        Action::RemoveActivation(_, _) => "从此级移除这条激活动作，不删除部件。",
        Action::OpenRepair => "本组存在重复编号：逐条指定连接和分级引用的归属。",
        Action::RepairTarget(_) => "依次切换此条引用所属实例；应用前必须指定所有引用。",
        Action::BackToProperties => "返回属性草稿，丢弃本次引用归属分配；不修改文档。",
        _ => "编辑属性草稿；应用后才写入文档。",
    }
}

fn text_field(
    ui: &mut egui::Ui,
    draft: &mut Draft,
    field: Field,
    title: &str,
    hits: &mut Vec<(Action, egui::Rect)>,
    edited: &mut bool,
    focused: &mut Option<Field>,
) {
    ui.label(title).on_hover_text(match field {
        Field::Target => {
            "新增激活动作的目标部件 ID；仅接受本组唯一编号，也可用下方列表或 ↑↓ 选择。"
        }
        Field::CurrentStage => "当前执行级序号，从 0 起。",
        Field::Fuel => "燃料数量不能小于零或超过部件容量。",
        Field::Throttle => "驾驶舱油门，范围 0～1。",
        Field::Name => "驾驶舱记录的船体名称。",
    });
    let response = ui.add(
        egui::TextEdit::singleline(draft.field(field))
            .id(egui::Id::new(format!("part_properties_{field:?}")))
            .desired_width(f32::INFINITY),
    );
    record_hit(ui, &response, Action::Focus(field), hits);
    *edited |= response.changed();
    if response.has_focus() {
        *focused = Some(field);
    }
}

#[allow(clippy::too_many_arguments)]
fn properties_body(
    ui: &mut egui::Ui,
    draft: &mut Draft,
    document: &EditorDocument,
    hits: &mut Vec<(Action, egui::Rect)>,
    pending: &mut Option<Action>,
    edited: &mut bool,
    focused: &mut Option<Field>,
) {
    let group_parts = document
        .ship
        .group(draft.key.group)
        .map(|(parts, _)| parts)
        .unwrap_or_default();
    if group_parts
        .iter()
        .filter(|part| part.id == draft.key.id)
        .count()
        > 1
    {
        action_button(
            ui,
            "本组编号重复 · 修复引用归属",
            Action::OpenRepair,
            hits,
            pending,
        );
    }
    let mut active = draft.active;
    let response = ui.checkbox(&mut active, "已激活");
    record_hit(ui, &response, Action::Active, hits);
    if response.changed() {
        *pending = Some(Action::Active);
    }
    if draft.original.fuel_kind.is_some() {
        text_field(ui, draft, Field::Fuel, "燃料", hits, edited, focused);
    }
    if draft.original.pod.is_none() {
        return;
    }
    text_field(ui, draft, Field::Name, "船体名称", hits, edited, focused);
    text_field(
        ui,
        draft,
        Field::Throttle,
        "油门（0～1）",
        hits,
        edited,
        focused,
    );
    if draft.staging.is_some() {
        text_field(
            ui,
            draft,
            Field::CurrentStage,
            "当前级（从 0 起）",
            hits,
            edited,
            focused,
        );
    }
    text_field(ui, draft, Field::Target, "目标 ID", hits, edited, focused);
    let mut counts = std::collections::HashMap::<i64, usize>::new();
    for part in group_parts {
        *counts.entry(part.id).or_default() += 1;
    }
    let target = draft.target.trim().parse::<i64>().ok();
    let target_name = target
        .and_then(|id| document.ship.group_part(draft.key.group, id))
        .map(|part| part_description(document, part))
        .unwrap_or_else(|| "未选目标（本组唯一编号）".into());
    ui.horizontal_wrapped(|ui| {
        action_button(ui, "↑ 目标", Action::CycleTarget(false), hits, pending);
        action_button(ui, "↓ 目标", Action::CycleTarget(true), hits, pending);
        egui::ComboBox::from_id_salt("part_activation_target")
            .selected_text(target_name)
            .height(240.0)
            .show_ui(ui, |ui| {
                for part in group_parts.iter().filter(|part| counts[&part.id] == 1) {
                    if ui
                        .selectable_label(target == Some(part.id), part_description(document, part))
                        .clicked()
                    {
                        draft.target = part.id.to_string();
                        *edited = true;
                    }
                }
            });
    });
    ui.separator();
    action_button(ui, "+ 分级", Action::AddStep, hits, pending);
    let Some(staging) = &draft.staging else {
        ui.label("暂无分级 · 点击 + 分级");
        return;
    };
    for (stage, step) in staging.steps.iter().enumerate() {
        ui.push_id(("staging_step", stage), |ui| {
            ui.separator();
            ui.horizontal_wrapped(|ui| {
                ui.strong(format!("第 {stage} 级"));
                ui.add_enabled_ui(stage > 0, |ui| {
                    action_button(ui, "↑ 上移", Action::MoveStep(stage, false), hits, pending);
                });
                ui.add_enabled_ui(stage + 1 < staging.steps.len(), |ui| {
                    action_button(ui, "↓ 下移", Action::MoveStep(stage, true), hits, pending);
                });
                action_button(ui, "× 删级", Action::RemoveStep(stage), hits, pending);
                action_button(ui, "+ 目标", Action::AddActivation(stage), hits, pending);
            });
            for (index, activation) in step.activations.iter().enumerate() {
                ui.push_id(index, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        let name = document
                            .ship
                            .group_part(draft.key.group, activation.id)
                            .map(|part| part_description(document, part))
                            .unwrap_or_else(|| format!("#{} · 未知或不唯一的部件", activation.id));
                        ui.label(format!("激活 {name}"));
                        let mut moved = activation.moved;
                        let response = ui.checkbox(&mut moved, "移动标记");
                        record_hit(ui, &response, Action::Moved(stage, index), hits);
                        if response.changed() {
                            *pending = Some(Action::Moved(stage, index));
                        }
                        action_button(
                            ui,
                            "− 移除",
                            Action::RemoveActivation(stage, index),
                            hits,
                            pending,
                        );
                    });
                });
            }
        });
    }
}

fn part_description(document: &EditorDocument, part: &Part) -> String {
    let name = document
        .catalog
        .get(&part.part_type)
        .map(|kind| kind.name.as_str())
        .unwrap_or(&part.part_type);
    format!("#{} · {name} ({:.2}, {:.2})", part.id, part.x, part.y)
}

fn repair_body(
    ui: &mut egui::Ui,
    repair: &DuplicateRepair,
    hits: &mut Vec<(Action, egui::Rect)>,
    pending: &mut Option<Action>,
) {
    ui.label(format!(
        "待分配引用：{} / {}（点击归属按钮切换实例）",
        repair.unassigned(),
        repair.references().len()
    ));
    for (occurrence, part) in repair
        .original()
        .parts
        .iter()
        .filter(|part| part.id == repair.old_id())
        .enumerate()
    {
        ui.label(format!(
            "实例 {}：{} · ({:.3}, {:.3}) · #{} → #{}",
            occurrence + 1,
            part.part_type,
            part.x,
            part.y,
            repair.old_id(),
            repair.new_ids()[occurrence]
        ));
    }
    ui.separator();
    for (reference, (site, target)) in repair.references().iter().enumerate() {
        ui.push_id(("repair_reference", reference), |ui| {
            match *site {
                ReferenceSite::Connection { index, role } => {
                    let (parent, child) = match repair.original().connections[index] {
                        Connection::Normal { parent, child, .. }
                        | Connection::Dock { parent, child, .. } => (parent, child),
                    };
                    let role_name = match role {
                        ConnectionRole::Parent => "母端",
                        ConnectionRole::Child => "子端",
                        ConnectionRole::Dock => "对接插头",
                    };
                    ui.label(format!(
                        "连接 {}：#{parent} → #{child} · {role_name}",
                        index + 1
                    ));
                    let peer_id = if role == ConnectionRole::Child {
                        parent
                    } else {
                        child
                    };
                    for peer in repair
                        .original()
                        .parts
                        .iter()
                        .filter(|part| part.id == peer_id)
                    {
                        ui.small(format!(
                            "相邻 #{} · {} ({:.2}, {:.2})",
                            peer.id, peer.part_type, peer.x, peer.y
                        ));
                    }
                }
                ReferenceSite::Activation {
                    owner,
                    stage,
                    index,
                } => {
                    ui.label(format!(
                        "驾驶舱 #{} 实例 {} · 第 {stage} 级 · 动作 {}",
                        owner.id,
                        owner.occurrence + 1,
                        index + 1
                    ));
                }
            }
            let text = target
                .map(|target| format!("归属：实例 {}（#{}）", target + 1, repair.new_ids()[target]))
                .unwrap_or_else(|| "归属：未指定".into());
            action_button(ui, &text, Action::RepairTarget(reference), hits, pending);
            ui.separator();
        });
    }
    if repair.references().is_empty() {
        ui.label("没有连接或分级引用；应用后各实例使用独立编号。");
    }
    action_button(ui, "返回属性", Action::BackToProperties, hits, pending);
}

#[cfg(test)]
mod tests;
