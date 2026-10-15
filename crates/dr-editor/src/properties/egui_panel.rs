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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DragSource {
    Stage(usize),
    Activation(usize, usize),
}

#[derive(Clone)]
struct StagingDrag {
    owner: PartKey,
    plan: StagingState,
    source: DragSource,
}

#[derive(Clone, Copy)]
enum DropZone {
    Header {
        stage: usize,
        len: usize,
        rect: egui::Rect,
    },
    Tile {
        stage: usize,
        index: usize,
        rect: egui::Rect,
    },
}

fn drag_id() -> egui::Id {
    egui::Id::new("staging_card_drag")
}

fn press_id() -> egui::Id {
    egui::Id::new("staging_card_press")
}

fn record_drag_press(
    response: &egui::Response,
    owner: PartKey,
    plan: &StagingState,
    source: DragSource,
) {
    if response.is_pointer_button_down_on()
        && response
            .ctx
            .input(|input| input.pointer.button_pressed(egui::PointerButton::Primary))
    {
        response.ctx.data_mut(|data| {
            data.insert_temp(
                press_id(),
                StagingDrag {
                    owner,
                    plan: plan.clone(),
                    source,
                },
            );
        });
    }
}

/// hits 使用正式控件裁剪后的逻辑坐标；拖放状态只存在 egui 会话中。
pub fn show(
    ctx: &egui::Context,
    inspector: &mut Inspector,
    document: &EditorDocument,
    hits: &mut Vec<(Action, egui::Rect)>,
    available: egui::Rect,
    images: &mut impl FnMut(&dr_core::PartType) -> (egui::TextureId, Vec2),
) -> Option<Shown> {
    inspector.hovered = None;
    let Some(draft) = inspector.draft.as_ref() else {
        ctx.data_mut(|data| {
            data.remove::<StagingDrag>(drag_id());
            data.remove::<StagingDrag>(press_id());
        });
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
    let docked = inspector.docked();
    let mut drag = ctx.data_mut(|data| data.get_temp::<StagingDrag>(drag_id()));
    let mut press = ctx.data_mut(|data| data.get_temp::<StagingDrag>(press_id()));
    for state in [&mut drag, &mut press] {
        if state.as_ref().is_some_and(|state| {
            !docked
                || state.owner != draft.key
                || Some(&state.plan) != draft.staging.as_ref()
                || document.ship.part_at(draft.key) != Some(&draft.original)
        }) {
            *state = None;
        }
    }
    let was_dragging = drag.is_some();
    let was_pressing = press.is_some();
    let popup_open = egui::Popup::is_any_open(ctx);
    let composition_frame = ctx.input(|input| {
        input
            .events
            .iter()
            .any(|event| matches!(event, egui::Event::Ime(_)))
    });
    let interrupted = ctx.input(|input| {
        !input.focused
            || input.events.iter().any(|event| {
                matches!(
                    event,
                    egui::Event::PointerGone
                        | egui::Event::Touch {
                            phase: egui::TouchPhase::Cancel,
                            ..
                        }
                )
            })
    });
    let escape = if !popup_open && !composition_frame {
        ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
    } else {
        false
    };
    let cancel_drag = interrupted || ((was_dragging || was_pressing) && escape);
    if cancel_drag {
        drag = None;
        press = None;
    }
    if press.is_none() {
        ctx.data_mut(|data| data.remove::<StagingDrag>(press_id()));
    }
    let mut zones = vec![];
    let mut close = false;
    let ((mut action, body), dock) = if docked {
        let mut root = egui::Ui::new(
            ctx.clone(),
            egui::Id::new("staging_dock_root"),
            egui::UiBuilder::new().max_rect(available),
        );
        let panel = if available.width() >= 600.0 {
            egui::Panel::left("staging_sidebar")
                .exact_size(320.0_f32.min(available.width() - 160.0))
        } else {
            egui::Panel::bottom("staging_bottom").exact_size(
                (available.height() * 0.55)
                    .min(420.0)
                    .min((available.height() - 96.0).max(60.0)),
            )
        };
        let response = panel.resizable(false).show(&mut root, |ui| {
            contents(
                ui,
                inspector,
                document,
                hits,
                (&title, true),
                images,
                &mut drag,
                &mut zones,
                !cancel_drag,
            )
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
            contents(
                ui,
                inspector,
                document,
                hits,
                (&title, false),
                images,
                &mut drag,
                &mut zones,
                false,
            )
        });
        close = !response.backdrop_response.clicked() && response.should_close();
        (response.inner, None)
    };
    // egui 可先被 Esc 清除交互；保留真实控件的按下来源，覆盖跨起拖阈值的同帧取消。
    let handling_drag = was_dragging || drag.is_some() || (was_pressing && escape);
    if handling_drag && escape {
        drag = None;
    }
    if handling_drag || cancel_drag {
        // 拖动释放只能产生结构 Action，不能顺带触发 Apply 或取消整个草稿。
        action = None;
    }
    if let Some(active) = &drag {
        let pointer = ctx.input(|input| input.pointer.interact_pos());
        let drop = pointer
            .filter(|pos| body.contains(*pos))
            .and_then(|pos| drop_action(active, &zones, pos));
        if let Some((_, marker)) = &drop {
            let painter = ctx
                .layer_painter(egui::LayerId::new(
                    egui::Order::Foreground,
                    egui::Id::new("staging_drop_marker"),
                ))
                .with_clip_rect(body);
            painter.line_segment(
                *marker,
                egui::Stroke::new(3.0, egui::Color32::from_rgb(190, 225, 70)),
            );
        }
        if let Some(pos) = pointer {
            crate::icons::paint(
                &ctx.layer_painter(egui::LayerId::new(
                    egui::Order::Tooltip,
                    egui::Id::new("staging_drag_ghost"),
                )),
                egui::Rect::from_min_size(pos + egui::vec2(12.0, 12.0), egui::vec2(28.0, 28.0)),
                crate::icons::Icon::Drag,
                egui::Color32::from_rgb(190, 225, 70),
            );
        }
        if ctx.input(|input| input.pointer.button_released(egui::PointerButton::Primary)) {
            action = drop.map(|(action, _)| action);
            drag = None;
        } else if !ctx.input(|input| input.pointer.primary_down()) {
            drag = None;
        }
        ctx.request_repaint();
    }
    if docked && escape && !handling_drag {
        close = true;
    }
    if close && !composition_frame && !handling_drag && !cancel_drag {
        action = Some(Action::Cancel);
    }
    let released = !ctx.input(|input| input.pointer.primary_down());
    ctx.data_mut(|data| {
        if released || cancel_drag {
            data.remove::<StagingDrag>(press_id());
        }
        if let Some(drag) = drag {
            data.insert_temp(drag_id(), drag);
        } else {
            data.remove::<StagingDrag>(drag_id());
        }
    });
    Some(Shown { action, dock, body })
}

fn drop_action(
    drag: &StagingDrag,
    zones: &[DropZone],
    pos: egui::Pos2,
) -> Option<(Action, [egui::Pos2; 2])> {
    for zone in zones {
        match (*zone, drag.source) {
            (DropZone::Header { stage, rect, .. }, DragSource::Stage(from))
                if rect.contains(pos) =>
            {
                let after = pos.y >= rect.center().y;
                let insertion = stage + usize::from(after);
                let to = insertion - usize::from(from < insertion);
                let y = if after { rect.bottom() } else { rect.top() };
                return Some((
                    Action::ReorderStep { from, to },
                    [egui::pos2(rect.left(), y), egui::pos2(rect.right(), y)],
                ));
            }
            (DropZone::Header { stage, len, rect }, DragSource::Activation(from_stage, index))
                if rect.contains(pos) =>
            {
                let to_index = len - usize::from(from_stage == stage);
                return Some((
                    Action::MoveActivation {
                        from_stage,
                        index,
                        to_stage: stage,
                        to_index,
                    },
                    [rect.left_bottom(), rect.right_bottom()],
                ));
            }
            (
                DropZone::Tile {
                    stage,
                    index: target,
                    rect,
                },
                DragSource::Activation(from_stage, index),
            ) if rect.contains(pos) => {
                let after = pos.x >= rect.center().x;
                let insertion = target + usize::from(after);
                let to_index = insertion - usize::from(from_stage == stage && index < insertion);
                let x = if after { rect.right() } else { rect.left() };
                return Some((
                    Action::MoveActivation {
                        from_stage,
                        index,
                        to_stage: stage,
                        to_index,
                    },
                    [egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())],
                ));
            }
            _ => {}
        }
    }
    None
}

fn icon_action(
    ui: &mut egui::Ui,
    icon: crate::icons::Icon,
    action: Action,
    selected: bool,
    hits: &mut Vec<(Action, egui::Rect)>,
    pending: &mut Option<Action>,
) {
    let response = ui
        .push_id(("property_action", format!("{action:?}")), |ui| {
            crate::icons::selected_button(ui, icon, action_hint(&action), selected)
        })
        .inner;
    record_hit(ui, &response, action.clone(), hits);
    if response.clicked() {
        *pending = Some(action);
    }
}

#[allow(clippy::too_many_arguments)]
fn contents(
    ui: &mut egui::Ui,
    inspector: &mut Inspector,
    document: &EditorDocument,
    hits: &mut Vec<(Action, egui::Rect)>,
    (title, docked): (&str, bool),
    images: &mut impl FnMut(&dr_core::PartType) -> (egui::TextureId, Vec2),
    drag: &mut Option<StagingDrag>,
    zones: &mut Vec<DropZone>,
    drag_allowed: bool,
) -> (Option<Action>, egui::Rect) {
    use crate::icons::Icon;
    ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
    ui.spacing_mut().interact_size.y = ui.spacing().interact_size.y.max(44.0);
    let mut action = None;
    if docked {
        ui.horizontal(|ui| {
            icon_action(
                ui,
                Icon::Staging,
                Action::GenerateStaging,
                false,
                hits,
                &mut action,
            );
            icon_action(ui, Icon::Add, Action::AddStep, false, hits, &mut action);
            icon_action(
                ui,
                Icon::Target,
                Action::ToggleTargets,
                inspector.targets_open,
                hits,
                &mut action,
            );
            icon_action(
                ui,
                Icon::Settings,
                Action::ToggleAdvanced,
                inspector.advanced_open,
                hits,
                &mut action,
            );
            icon_action(ui, Icon::Check, Action::Apply, false, hits, &mut action);
            icon_action(ui, Icon::Cancel, Action::Cancel, false, hits, &mut action);
        })
        .response
        .on_hover_text(title);
    } else {
        ui.heading(title);
    }
    if !inspector.error.is_empty() {
        ui.add(
            egui::Label::new(egui::RichText::new(&inspector.error).color(egui::Color32::LIGHT_RED))
                .truncate(),
        )
        .on_hover_text(&inspector.error);
    }
    ui.separator();
    let mut edited = false;
    let mut focused = None;
    let body_height = if docked {
        ui.available_height().max(0.0)
    } else {
        (ui.ctx().content_rect().height() - 220.0).max(60.0)
    };
    let advanced = !docked || inspector.advanced_open;
    let targets = !docked || inspector.targets_open;
    let body_hits_start = hits.len();
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
                    advanced,
                    targets,
                    drag,
                    zones,
                    drag_allowed,
                );
            }
            if drag.is_some() && ui.input(|input| input.pointer.primary_down()) {
                let clip = ui.clip_rect();
                if let Some(pos) = ui
                    .input(|input| input.pointer.interact_pos())
                    .filter(|pos| clip.contains(*pos))
                {
                    let edge = 28.0_f32.min(clip.height() / 3.0);
                    let speed = if pos.y < clip.top() + edge {
                        (clip.top() + edge - pos.y) / edge
                    } else if pos.y > clip.bottom() - edge {
                        -(pos.y - clip.bottom() + edge) / edge
                    } else {
                        0.0
                    };
                    let delta = speed * 480.0 * ui.input(|input| input.stable_dt.min(0.05));
                    ui.scroll_with_delta(egui::vec2(0.0, delta));
                    ui.ctx().request_repaint();
                }
            }
        });
    let body = scroll.inner_rect.intersect(ui.clip_rect());
    for (_, rect) in &mut hits[body_hits_start..] {
        *rect = rect.intersect(body);
    }
    hits.retain(|(_, rect)| rect.is_positive());
    for zone in zones {
        match zone {
            DropZone::Header { rect, .. } | DropZone::Tile { rect, .. } => {
                *rect = rect.intersect(body)
            }
        }
    }
    inspector.focus = focused;
    if edited {
        inspector.error.clear();
    }
    if !docked {
        ui.separator();
        ui.horizontal(|ui| {
            action_button(ui, "应用", Action::Apply, hits, &mut action);
            action_button(ui, "取消", Action::Cancel, hits, &mut action);
        });
    }
    (action, body)
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
        Action::GenerateStaging => "按驾驶舱连接树重置基础分级；仅改草稿，应用后一次撤销。",
        Action::ToggleTargets => "展开或收起目标部件选择。",
        Action::ToggleAdvanced => "展开或收起名称、油门及高级属性。",
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
    advanced: bool,
    targets: bool,
    drag: &mut Option<StagingDrag>,
    zones: &mut Vec<DropZone>,
    drag_allowed: bool,
) {
    use crate::icons::Icon;
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
        action_button(ui, "修复重复编号引用", Action::OpenRepair, hits, pending);
    }
    if advanced {
        let mut active = draft.active;
        let response = ui.checkbox(&mut active, "已激活");
        record_hit(ui, &response, Action::Active, hits);
        if response.changed() {
            *pending = Some(Action::Active);
        }
        if draft.original.fuel_kind.is_some() {
            text_field(ui, draft, Field::Fuel, "燃料", hits, edited, focused);
        }
        if draft.original.pod.is_some() {
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
        }
        ui.separator();
    }
    if draft.original.pod.is_none() {
        return;
    }
    if targets {
        text_field(ui, draft, Field::Target, "目标 ID", hits, edited, focused);
        if *focused == Some(Field::Target) {
            *preview = Preview::Target;
        }
        let target = draft.target.trim().parse::<i64>().ok();
        let target_name = target
            .and_then(|id| document.ship.group_part(draft.key.group, id))
            .map(|part| part_description(document, part))
            .unwrap_or_else(|| "选择本组目标".into());
        ui.horizontal(|ui| {
            icon_action(
                ui,
                Icon::ChevronUp,
                Action::CycleTarget(false),
                false,
                hits,
                pending,
            );
            icon_action(
                ui,
                Icon::ChevronDown,
                Action::CycleTarget(true),
                false,
                hits,
                pending,
            );
            egui::ComboBox::from_id_salt("part_activation_target")
                .selected_text(target_name)
                .width(ui.available_width().min(200.0))
                .wrap_mode(egui::TextWrapMode::Truncate)
                .height(240.0)
                .show_ui(ui, |ui| {
                    for part in unique_target_parts(group_parts) {
                        if ui
                            .selectable_label(
                                target == Some(part.id),
                                part_description(document, part),
                            )
                            .clicked()
                        {
                            draft.target = part.id.to_string();
                            *preview = Preview::Target;
                            *edited = true;
                        }
                    }
                });
        });
        ui.separator();
    }
    let Some(staging) = &draft.staging else {
        return;
    };
    for (stage, step) in staging.steps.iter().enumerate() {
        ui.push_id(("staging_step", stage), |ui| {
            let selected = matches!(*preview, Preview::Stage(value) | Preview::Activation(value, _) if value == stage);
            let accent = crate::egui_ui::ui_color(native_buttons::ACCENT);
            egui::Frame::new().fill(egui::Color32::from_gray(if selected { 42 } else { 31 }))
                .stroke(egui::Stroke::new(1.0, if selected { accent.gamma_multiply(0.65) } else { egui::Color32::from_gray(58) }))
                .corner_radius(4.0).inner_margin(5.0).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let width = (ui.available_width() - 96.0).max(44.0);
                        let response = ui.add_sized([width, 44.0], egui::Button::new(
                            egui::RichText::new(format!("{:02}     · {}", stage + 1, step.activations.len()))
                                .color(if selected { accent } else { ui.visuals().text_color() }))
                            .selected(selected).sense(egui::Sense::click_and_drag()))
                            .on_hover_text("拖动整级；放到级标题上半插前、下半插后。点击在船体上预览。");
                        record_hit(ui, &response, Action::SelectStage(stage), hits);
                        let rect = response.rect.intersect(ui.clip_rect());
                        if rect.is_positive() { zones.push(DropZone::Header { stage, len: step.activations.len(), rect }); }
                        if response.hovered() { *hovered = Some(Preview::Stage(stage)); }
                        if response.clicked() { *pending = Some(Action::SelectStage(stage)); }
                        if drag_allowed {
                            record_drag_press(&response, draft.key, staging, DragSource::Stage(stage));
                            if response.drag_started() {
                                *drag = Some(StagingDrag { owner: draft.key, plan: staging.clone(), source: DragSource::Stage(stage) });
                            }
                        }
                        icon_action(ui, Icon::Add, Action::AddActivation(stage), false, hits, pending);
                        icon_action(ui, Icon::Delete, Action::RemoveStep(stage), false, hits, pending);
                    });
                    if selected && matches!(*preview, Preview::Stage(_)) {
                        ui.horizontal(|ui| {
                            ui.add_enabled_ui(stage > 0, |ui| icon_action(ui, Icon::ChevronUp,
                                Action::MoveStep(stage, false), false, hits, pending));
                            ui.add_enabled_ui(stage + 1 < staging.steps.len(), |ui| icon_action(ui, Icon::ChevronDown,
                                Action::MoveStep(stage, true), false, hits, pending));
                        });
                    }
                    ui.horizontal_wrapped(|ui| {
                        for (index, activation) in step.activations.iter().enumerate() {
                            // 由父级预分配尺寸，让 wrapped 布局在建立稳定 ID 子作用域前换行。
                            ui.allocate_ui(egui::vec2(56.0, 56.0), |ui| {
                                ui.push_id(index, |ui| {
                                    let part = document.ship.group_part(draft.key.group, activation.id);
                                    let kind = part.and_then(|part| document.catalog.get(&part.part_type));
                                    let name = kind.map(|kind| kind.name.as_str()).or_else(|| part.map(|part| part.part_type.as_str()))
                                        .unwrap_or("未知或不唯一的部件");
                                    let response = activation_row(ui, (stage, index, activation),
                                        (name, part.map(|part| part_description(document, part)), kind.map(&mut *images)),
                                        *preview, hits, pending, hovered);
                                    let rect = response.rect.intersect(ui.clip_rect());
                                    if rect.is_positive() { zones.push(DropZone::Tile { stage, index, rect }); }
                                    if drag_allowed {
                                        record_drag_press(&response, draft.key, staging, DragSource::Activation(stage, index));
                                        if response.drag_started() {
                                            *drag = Some(StagingDrag { owner: draft.key, plan: staging.clone(),
                                                source: DragSource::Activation(stage, index) });
                                        }
                                    }
                                });
                            });
                        }
                    });
                    if let Preview::Activation(selected_stage, index) = *preview
                        && selected_stage == stage && let Some(activation) = step.activations.get(index) {
                        ui.horizontal(|ui| {
                            let mut moved = activation.moved;
                            let response = ui.add_sized([ui.available_width() - 48.0, 44.0],
                                egui::Checkbox::new(&mut moved, "moved"))
                                .on_hover_text("XML 移动标记；跨级拖动自动开启，不改变船上位置。");
                            record_hit(ui, &response, Action::Moved(stage, index), hits);
                            if response.changed() { *pending = Some(Action::Moved(stage, index)); }
                            icon_action(ui, Icon::Delete, Action::RemoveActivation(stage, index), false, hits, pending);
                        });
                    }
                });
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

/// 正式 click_and_drag 缩略图；移动标记和删除在选中后的卡片上下文中显示。
fn activation_row(
    ui: &mut egui::Ui,
    (stage, index, activation): (usize, usize, &Activation),
    (name, description, image): (&str, Option<String>, Option<(egui::TextureId, Vec2)>),
    preview: Preview,
    hits: &mut Vec<(Action, egui::Rect)>,
    pending: &mut Option<Action>,
    hovered: &mut Option<Preview>,
) -> egui::Response {
    let selected = preview == Preview::Activation(stage, index);
    let response = ui.add_sized(
        [56.0, 56.0],
        egui::Button::new("")
            .selected(selected)
            .sense(egui::Sense::click_and_drag()),
    );
    let rect = response.rect;
    let image_bounds =
        egui::Rect::from_center_size(rect.center() - egui::vec2(0.0, 7.0), egui::vec2(32.0, 32.0));
    if let Some(image) = image {
        crate::egui_ui::paint_part_image(ui, image, image_bounds);
    } else {
        crate::icons::paint(
            ui.painter(),
            image_bounds,
            crate::icons::Icon::Parts,
            ui.visuals().weak_text_color(),
        );
    }
    ui.painter().text(
        egui::pos2(rect.center().x, rect.bottom() - 4.0),
        egui::Align2::CENTER_BOTTOM,
        format!("#{}", activation.id),
        egui::FontId::proportional(11.0),
        if selected {
            crate::egui_ui::ui_color(native_buttons::ACCENT)
        } else {
            ui.visuals().text_color()
        },
    );
    response.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::WidgetType::Button,
            ui.is_enabled(),
            format!("#{} · {name}", activation.id),
        )
    });
    if response.hovered() {
        *hovered = Some(Preview::Activation(stage, index));
    }
    let response = response.on_hover_text(format!(
        "{}\n拖到级标题追加，拖到缩略图两侧插入。",
        description.unwrap_or_else(|| format!("#{} · {name}", activation.id))
    ));
    record_hit(ui, &response, Action::SelectActivation(stage, index), hits);
    if response.clicked() {
        *pending = Some(Action::SelectActivation(stage, index));
    }
    response
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
