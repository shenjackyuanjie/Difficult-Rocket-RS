//! egui 属性面板：只编辑草稿，结构操作仍通过原有 Action/act 事务提交。
//!
//! TextEdit 独占文本光标、选区、剪贴板与输入法状态；不要同时运行旧的
//! properties::keyboard/native_ime 输入路径，否则一次输入会被处理两次。

use super::*;
use bevy_egui::egui;

/// 停靠面板与剩余船体区由同一次 egui Panel 布局产生，不能用遮罩替代。
#[derive(Clone, Copy, Debug)]
pub struct Dock {
    pub panel: egui::Rect,
    pub canvas: egui::Rect,
}

pub struct Shown {
    pub action: Option<Action>,
    pub dock: Option<Dock>,
    pub body: egui::Rect,
}

/// hits 使用 egui 逻辑坐标，追加而不清空；available 是既有工具栏之后的区域。
pub fn show(
    ctx: &egui::Context,
    inspector: &mut Inspector,
    document: &EditorDocument,
    hits: &mut Vec<(Action, egui::Rect)>,
    available: egui::Rect,
    images: &mut impl FnMut(&dr_core::PartType) -> (egui::TextureId, Vec2),
) -> Option<Shown> {
    inspector.hovered = None;
    let draft = inspector.draft.as_ref()?;
    let title = format!(
        "{} · {}",
        draft.key,
        document
            .catalog
            .get(&draft.original.part_type)
            .map(|kind| kind.name.as_str())
            .unwrap_or(&draft.original.part_type)
    );
    let docked = inspector.docked();
    let popup_open = egui::Popup::is_any_open(ctx);
    let composition_frame = ctx.input(|input| {
        input
            .events
            .iter()
            .any(|event| matches!(event, egui::Event::Ime(_)))
    });
    let mut close = false;
    let ((mut action, body), dock) = if docked {
        let mut root = egui::Ui::new(
            ctx.clone(),
            egui::Id::new("staging_dock_root"),
            egui::UiBuilder::new().max_rect(available),
        );
        // 短横屏仍用侧栏；窄竖屏放底部，两种布局都保留船体区。
        let panel = if available.width() >= available.height() {
            egui::Panel::left("staging_sidebar")
                .exact_size((available.width() * 0.5).clamp(280.0, 480.0))
        } else {
            egui::Panel::bottom("staging_bottom").exact_size(
                (available.height() * 0.55)
                    .clamp(150.0, 420.0)
                    .min(available.height() - 96.0)
                    .max(60.0),
            )
        };
        let response = panel.resizable(false).show(&mut root, |ui| {
            contents(ui, inspector, document, hits, (&title, true), images)
        });
        (
            response.inner,
            Some(Dock {
                panel: response.response.rect,
                canvas: root.available_rect_before_wrap(),
            }),
        )
    } else {
        let response = egui::Modal::new(egui::Id::new("part_properties_modal")).show(ctx, |ui| {
            ui.set_width((available.width() - 40.0).clamp(160.0, 700.0));
            contents(ui, inspector, document, hits, (&title, false), images)
        });
        close = !response.backdrop_response.clicked() && response.should_close();
        (response.inner, None)
    };
    if docked && !popup_open && !composition_frame {
        close = ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
    }
    if close && !composition_frame {
        action = Some(Action::Cancel);
    }
    Some(Shown { action, dock, body })
}

/// 停靠分级和普通属性/引用修复复用同一份草稿控件、错误与操作栏。
fn contents(
    ui: &mut egui::Ui,
    inspector: &mut Inspector,
    document: &EditorDocument,
    hits: &mut Vec<(Action, egui::Rect)>,
    (title, docked): (&str, bool),
    images: &mut impl FnMut(&dr_core::PartType) -> (egui::TextureId, Vec2),
) -> (Option<Action>, egui::Rect) {
    let short = docked && ui.available_height() < 240.0;
    if short {
        ui.spacing_mut().item_spacing.y = 4.0;
    }
    ui.heading(if docked {
        "分级 / 船体属性"
    } else {
        title
    })
    .on_hover_text(title);
    if docked && !short {
        ui.add(egui::Label::new(title).truncate());
    }
    if !docked || ui.available_height() >= 240.0 {
        ui.small(if inspector.repair.is_some() {
            "引用归属 · 草稿"
        } else if docked {
            "悬停或点选动作，查看船上同色部件"
        } else {
            "属性 · 草稿"
        })
        .on_hover_text("仅修改草稿；应用后一次撤销，取消不改文档。");
    }
    ui.separator();
    let mut action = None;
    let mut edited = false;
    let mut focused = None;
    let body_height = if docked {
        (ui.available_height() - ui.spacing().interact_size.y - 18.0).max(0.0)
    } else {
        (ui.ctx().content_rect().height() - 220.0).max(60.0)
    };
    let scroll = egui::ScrollArea::vertical()
        .id_salt(if inspector.repair.is_some() {
            "part_reference_repair_scroll"
        } else {
            "part_properties_scroll"
        })
        .max_height(body_height)
        .min_scrolled_height(0.0)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            if docked && !inspector.error.is_empty() {
                ui.colored_label(egui::Color32::LIGHT_RED, &inspector.error);
            }
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
                    &mut inspector.preview,
                    &mut inspector.hovered,
                    images,
                );
            }
        });
    inspector.focus = focused;
    if edited {
        inspector.error.clear();
    }
    ui.separator();
    if !docked && !inspector.error.is_empty() {
        ui.colored_label(egui::Color32::LIGHT_RED, &inspector.error);
    }
    ui.horizontal(|ui| {
        action_button(ui, "应用", Action::Apply, hits, &mut action);
        action_button(ui, "取消", Action::Cancel, hits, &mut action);
    });
    (action, scroll.inner_rect)
}

/// 非交互画布标注：复用贴图角点与实际相机/缩放，只标明可唯一解析的目标。
/// 返回实际绘制的矩形供窗口验收核对；不修改船体、选区或历史。
pub fn paint_targets(
    ctx: &egui::Context,
    inspector: &Inspector,
    document: &EditorDocument,
    canvas: egui::Rect,
    (window, camera, projection): (&Window, &Transform, &Projection),
    mut texture_size: impl FnMut(&str) -> Option<Vec2>,
) -> Vec<(PartKey, egui::Rect)> {
    let Projection::Orthographic(projection) = projection else {
        return vec![];
    };
    let factor = window.scale_factor() / ctx.pixels_per_point();
    let screen = |world: Vec2| {
        egui::pos2(
            (window.width() * 0.5 + (world.x - camera.translation.x) / projection.scale) * factor,
            (window.height() * 0.5 - (world.y - camera.translation.y) / projection.scale) * factor,
        )
    };
    let color = crate::egui_ui::ui_color(native_buttons::ACCENT);
    let painter = ctx
        .layer_painter(egui::LayerId::new(
            egui::Order::Foreground,
            egui::Id::new("staging_target_marks"),
        ))
        .with_clip_rect(canvas);
    let mut drawn = vec![];
    for key in inspector.preview_keys(document) {
        let Some(part) = document.ship.part_at(key) else {
            continue;
        };
        let kind = document.catalog.get(&part.part_type);
        let size = kind
            .and_then(|kind| texture_size(&kind.sprite))
            .unwrap_or_else(|| render::fallback_size(kind));
        let points: Vec<_> = render::image_corners(part, size)
            .into_iter()
            .map(screen)
            .collect();
        let rect = points.iter().fold(egui::Rect::NOTHING, |mut rect, point| {
            rect.extend_with(*point);
            rect
        });
        if !rect.intersects(canvas) {
            continue;
        }
        painter.add(egui::Shape::convex_polygon(
            points,
            color.gamma_multiply(0.22),
            egui::Stroke::new(2.0, color),
        ));
        let label = painter.layout_no_wrap(
            format!("#{}", key.id),
            egui::FontId::proportional(14.0),
            color,
        );
        let size = label.size();
        let min = canvas.min + egui::vec2(2.0, 2.0);
        let max = (canvas.max - size - egui::vec2(2.0, 2.0)).max(min);
        let pos = (rect.left_top() - egui::vec2(0.0, size.y + 3.0)).clamp(min, max);
        painter.rect_filled(
            egui::Rect::from_min_size(pos, size).expand(2.0),
            native_buttons::RADIUS,
            crate::egui_ui::ui_color(native_buttons::PANEL),
        );
        painter.galley(pos, label, color);
        drawn.push((key, rect));
    }
    drawn
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
        Action::SelectStage(_) => "在船体上标明这一级的全部可唯一解析目标。",
        Action::SelectActivation(_, _) => "在船体上标明此动作对应的部件。",
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
    preview: &mut Preview,
    hovered: &mut Option<Preview>,
    images: &mut impl FnMut(&dr_core::PartType) -> (egui::TextureId, Vec2),
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
            "执行索引（XML 从 0 起）",
            hits,
            edited,
            focused,
        );
    }
    text_field(ui, draft, Field::Target, "目标 ID", hits, edited, focused);
    if *focused == Some(Field::Target) {
        *preview = Preview::Target;
    }
    let target = draft.target.trim().parse::<i64>().ok();
    let target_name = target
        .and_then(|id| document.ship.group_part(draft.key.group, id))
        .map(|part| part_description(document, part))
        .unwrap_or_else(|| "未选目标（本组唯一编号）".into());
    ui.horizontal_wrapped(|ui| {
        action_button(ui, "↑ 目标", Action::CycleTarget(false), hits, pending);
        action_button(ui, "↓ 目标", Action::CycleTarget(true), hits, pending);
    });
    egui::ComboBox::from_id_salt("part_activation_target")
        .selected_text(target_name)
        .width(ui.available_width().min(200.0))
        .wrap_mode(egui::TextWrapMode::Truncate)
        .height(240.0)
        .show_ui(ui, |ui| {
            for part in unique_target_parts(group_parts) {
                if ui
                    .selectable_label(target == Some(part.id), part_description(document, part))
                    .clicked()
                {
                    draft.target = part.id.to_string();
                    *preview = Preview::Target;
                    *edited = true;
                }
            }
        });
    ui.separator();
    action_button(ui, "+ 分级", Action::AddStep, hits, pending);
    let Some(staging) = &draft.staging else {
        ui.label("暂无分级 · 点击 + 分级");
        return;
    };
    for (stage, step) in staging.steps.iter().enumerate() {
        ui.push_id(("staging_step", stage), |ui| {
            let selected = matches!(*preview, Preview::Stage(value) | Preview::Activation(value, _) if value == stage);
            let title = egui::RichText::new(format!(
                "第 {} 级 · {} 个动作", stage + 1, step.activations.len(),
            ));
            let title = if selected {
                title.color(crate::egui_ui::ui_color(native_buttons::ACCENT))
            } else {
                title
            };
            let response = egui::CollapsingHeader::new(title)
                .id_salt("stage_card")
                .default_open(true)
                .show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
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
                            let part = document.ship.group_part(draft.key.group, activation.id);
                            let kind = part.and_then(|part| document.catalog.get(&part.part_type));
                            let name = kind.map(|kind| kind.name.as_str())
                                .or_else(|| part.map(|part| part.part_type.as_str()))
                                .unwrap_or("未知或不唯一的部件");
                            activation_row(
                                ui, (stage, index, activation),
                                (name, part.map(|part| part_description(document, part)), kind.map(&mut *images)),
                                *preview, hits, pending,
                                hovered,
                            );
                        });
                    }
                });
            record_hit(ui, &response.header_response, Action::SelectStage(stage), hits);
            if response.header_response.hovered() {
                *hovered = Some(Preview::Stage(stage));
            }
            if response.header_response.clicked() {
                *pending = Some(Action::SelectStage(stage));
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

/// 固定图片/名称、移动、移除三列；长名称截断，控制列不会被挤成竖排。
fn activation_row(
    ui: &mut egui::Ui,
    (stage, index, activation): (usize, usize, &Activation),
    (name, description, image): (&str, Option<String>, Option<(egui::TextureId, Vec2)>),
    preview: Preview,
    hits: &mut Vec<(Action, egui::Rect)>,
    pending: &mut Option<Action>,
    hovered: &mut Option<Preview>,
) {
    let height = ui.spacing().interact_size.y.max(44.0);
    let width = ui.available_width();
    let name_width = width - 64.0 - 56.0 - 2.0 * ui.spacing().item_spacing.x;
    ui.horizontal(|ui| {
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
        let selected = preview == Preview::Activation(stage, index);
        let title = format!("#{} · {name}", activation.id);
        let response = ui.add_sized(
            [name_width, height],
            egui::Button::new("").selected(selected),
        );
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), &title)
        });
        let rect = response.rect;
        if response.hovered() {
            *hovered = Some(Preview::Activation(stage, index));
        }
        let image_bounds = egui::Rect::from_center_size(
            egui::pos2(rect.left() + 22.0, rect.center().y),
            egui::Vec2::splat(32.0),
        );
        if let Some(image) = image {
            crate::egui_ui::paint_part_image(ui, image, image_bounds);
        } else {
            ui.painter().text(
                image_bounds.center(),
                egui::Align2::CENTER_CENTER,
                "?",
                egui::FontId::proportional(18.0),
                ui.visuals().weak_text_color(),
            );
        }
        let color = ui
            .style()
            .interact_selectable(&response, selected || response.hovered())
            .text_color();
        let text: egui::WidgetText = title.into();
        let galley = text.into_galley(
            ui,
            Some(egui::TextWrapMode::Truncate),
            (name_width - 50.0).max(0.0),
            egui::TextStyle::Body,
        );
        ui.painter().galley(
            egui::pos2(rect.left() + 44.0, rect.center().y - galley.size().y * 0.5),
            galley,
            color,
        );
        let response = response.on_hover_text(format!(
            "{}\n{}",
            description.unwrap_or_else(|| format!("#{} · {name}", activation.id)),
            action_hint(&Action::SelectActivation(stage, index)),
        ));
        record_hit(ui, &response, Action::SelectActivation(stage, index), hits);
        if response.clicked() {
            *pending = Some(Action::SelectActivation(stage, index));
        }
        let mut moved = activation.moved;
        let response = ui
            .add_sized([64.0, height], egui::Checkbox::new(&mut moved, "移动"))
            .on_hover_text("XML 的 moved 移动标记；仅修改草稿，不移动船上部件。");
        record_hit(ui, &response, Action::Moved(stage, index), hits);
        if response.changed() {
            *pending = Some(Action::Moved(stage, index));
        }
        let action = Action::RemoveActivation(stage, index);
        let response = ui
            .add_sized([56.0, height], egui::Button::new("移除").truncate())
            .on_hover_text(action_hint(&action));
        record_hit(ui, &response, action.clone(), hits);
        if response.clicked() {
            *pending = Some(action);
        }
    });
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
