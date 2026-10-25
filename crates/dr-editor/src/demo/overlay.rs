//! 非交互的章节条、章节入场说明、指针与输入提示层。
use crate::demo::*;
use crate::demo_cases;
use crate::egui_ui;
use crate::files;
use crate::help;
use crate::properties;
use crate::topology_ui;
use bevy_egui::egui;

const ACCENT: egui::Color32 = egui::Color32::from_rgb(179, 219, 5);
const LILAC: egui::Color32 = egui::Color32::from_rgb(221, 203, 255);
const MUTED: egui::Color32 = egui::Color32::from_rgb(154, 144, 174);
const CARD_BG: egui::Color32 = egui::Color32::from_rgb(22, 20, 29);
const CARD_BORDER: egui::Color32 = egui::Color32::from_rgb(64, 54, 92);
const TRACK: egui::Color32 = egui::Color32::from_rgb(48, 44, 60);

/// 章节标题条在章首展开描述，随后收为一行；始终不拦截指针。
const ENTRY: Duration = Duration::from_millis(1800);
const BAR_WIDTH: f32 = 560.;
/// 窄窗下说明卡会让出画布：只保留章节条、指针与输入提示。
const CAPTION_MIN_WIDTH: f32 = 900.;

/// 说明区不捕获指针；避免改变被演示控件的焦点与命中。
#[allow(clippy::too_many_arguments)]
pub fn overlay(
    mut contexts: bevy_egui::EguiContexts,
    showcase: Option<Res<Showcase>>,
    inspector: Res<properties::Inspector>,
    topology: Res<topology_ui::ConnectionEditor>,
    pending: Res<files::PendingFileAction>,
    help: Res<help::HelpState>,
    mouse: Res<ButtonInput<MouseButton>>,
    details: Option<Res<demo_cases::Run>>,
    property_hits: Res<egui_ui::UiHits>,
) {
    let Some(showcase) = showcase else {
        return;
    };
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };
    let now = Instant::now();
    if let Some(hint) = &showcase.key_hints.latest {
        paint_key_hint(ctx, hint, now, help.open);
    }
    let held: Vec<_> = [MouseButton::Left, MouseButton::Middle, MouseButton::Right]
        .into_iter()
        .filter(|button| mouse.pressed(*button))
        .filter_map(mouse_label)
        .map(|(_, label)| label)
        .collect();
    if !held.is_empty() {
        paint_input_hint(
            ctx,
            &KeyHint {
                text: format!("{} · 按住", held.join(" · ")),
                applied: now,
            },
            now,
            help.open,
            true,
        );
    } else if let Some(hint) = &showcase.mouse_hints.latest {
        paint_input_hint(ctx, hint, now, help.open, true);
    }
    if !showcase.completed {
        let pointer = if inspector.is_open() || topology.open || pending.is_blocked() {
            showcase.ui_pointer
        } else {
            showcase
                .pointer
                .map(|point| egui::pos2(point.x, point.y))
                .or(showcase.ui_pointer)
        };
        if let Some(point) = pointer {
            paint_pointer(ctx, point, showcase.mouse_hints.latest.as_ref(), now);
        }
        // 帮助与停靠分级都已占用画布：只留输入提示，不叠加章节条。
        if !help.open && !inspector.docked() {
            chapter_bar(ctx, &showcase, now);
        }
    }
    if showcase.completed {
        completed_card(ctx, &showcase, now);
        return;
    }
    if help.open || inspector.docked() {
        // 分级自身已有标题；让出画布，避免底部说明把真实目标轮廓遮住。
        return;
    }
    if showcase.presentation || ctx.content_rect().width() < CAPTION_MIN_WIDTH {
        // 录制模式由成片字幕承担解说；窄窗说明会让出画布，只保留章节条与提示。
        return;
    }
    let details = details.as_ref().and_then(|details| details.caption());
    let left_of = if details.is_some() && inspector.is_open() {
        // 用本帧真实控件位置避让，而不是假定模态高度；错误文字和取消必须可见。
        let Some((_, apply)) = property_hits
            .actions
            .iter()
            .find(|(action, _)| *action == properties::Action::Apply)
        else {
            return; // 首次布局尚无控件时不闪现一个挡住模态的说明层。
        };
        Some(apply.left())
    } else {
        None
    };
    let (title, description) = (
        format!(
            "{}演示 {}/{} · {}",
            showcase.mode.label(),
            showcase.index + 1,
            showcase.chapters.len(),
            showcase.chapter().title
        ),
        showcase
            .description
            .unwrap_or(showcase.chapter().description),
    );
    caption::paint(ctx, &title, description, details.as_deref(), left_of);
}

/// 顶部章节条：编号、章名、进度；章首短暂展开说明，不遮挡画布目标。
fn chapter_bar(ctx: &egui::Context, showcase: &Showcase, now: Instant) {
    let chapter = showcase.chapter();
    let total = showcase.chapters.len();
    let index = showcase.index + 1;
    let entry = now
        .checked_duration_since(showcase.chapter_started)
        .map(|since| {
            let elapsed = since.as_secs_f32();
            let span = ENTRY.as_secs_f32();
            (span - elapsed).max(0.) / (span * 0.45).max(0.001)
        })
        .unwrap_or(0.)
        .clamp(0., 1.);
    egui::Area::new(egui::Id::new("demo-chapter-bar"))
        .anchor(egui::Align2::CENTER_TOP, egui::vec2(0., 14.))
        .order(egui::Order::Tooltip)
        .interactable(false)
        .show(ctx, |ui| {
            let width = BAR_WIDTH.min((ctx.content_rect().width() - 40.).max(220.));
            egui::Frame::new()
                .fill(CARD_BG)
                .stroke(egui::Stroke::new(1., CARD_BORDER))
                .corner_radius(9.)
                .inner_margin(egui::Margin::symmetric(16, 10))
                .show(ui, |ui| {
                    ui.set_width(width);
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(format!("{index:02}"))
                                .size(21.)
                                .strong()
                                .color(ACCENT),
                        );
                        ui.label(
                            egui::RichText::new(format!("/ {total:02}"))
                                .size(13.)
                                .color(MUTED),
                        );
                        ui.add_space(12.);
                        ui.label(egui::RichText::new(chapter.title).size(17.).color(LILAC));
                    });
                    if entry > 0. {
                        ui.add_space(4.);
                        ui.label(
                            egui::RichText::new(
                                showcase.description.unwrap_or(chapter.description),
                            )
                            .size(14.)
                            .color(MUTED.gamma_multiply(entry)),
                        );
                    }
                    ui.add_space(8.);
                    let (rect, _) =
                        ui.allocate_exact_size(egui::vec2(width, 3.), egui::Sense::hover());
                    let painter = ui.painter();
                    painter.rect_filled(rect, 1.5, TRACK);
                    let done = if showcase.completed {
                        1.
                    } else {
                        (showcase.index + 1) as f32 / total as f32
                    };
                    let mut filled = rect;
                    filled.set_width(rect.width() * done);
                    painter.rect_filled(filled, 1.5, ACCENT);
                });
        });
}

/// 指针：柔光 + 圆环 + 实心点，点击后追加一圈扩散涟漪；视频缩放下仍清晰可辨。
fn paint_pointer(
    ctx: &egui::Context,
    point: egui::Pos2,
    last_click: Option<&KeyHint>,
    now: Instant,
) {
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Tooltip,
        egui::Id::new("demo-pointer"),
    ));
    painter.circle_filled(
        point,
        13.,
        egui::Color32::from_rgba_unmultiplied(196, 150, 255, 46),
    );
    painter.circle_stroke(
        point,
        9.5,
        egui::Stroke::new(2., egui::Color32::from_rgb(222, 185, 255)),
    );
    painter.circle_filled(point, 3.2, egui::Color32::WHITE);
    let Some(hint) = last_click else {
        return;
    };
    let elapsed = now.saturating_duration_since(hint.applied).as_secs_f32();
    let span = 0.36;
    if elapsed >= span {
        return;
    }
    let t = elapsed / span;
    let radius = 11. + 30. * t;
    let alpha = (1. - t) * 210.;
    painter.circle_stroke(
        point,
        radius,
        egui::Stroke::new(
            2.5 * (1. - t) + 0.5,
            egui::Color32::from_rgba_unmultiplied(179, 219, 5, alpha as u8),
        ),
    );
}

/// 完成页：集中展示模式、章节与证据数量，不再依赖底部说明框。
fn completed_card(ctx: &egui::Context, showcase: &Showcase, now: Instant) {
    let _ = now;
    egui::Area::new(egui::Id::new("demo-completed"))
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0., 0.))
        .order(egui::Order::Tooltip)
        .interactable(false)
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(CARD_BG)
                .stroke(egui::Stroke::new(1., CARD_BORDER))
                .corner_radius(12.)
                .inner_margin(egui::Margin::symmetric(28, 22))
                .show(ui, |ui| {
                    ui.set_width(520.);
                    ui.vertical_centered(|ui| {
                        ui.label(
                            egui::RichText::new("演示完成")
                                .size(30.)
                                .strong()
                                .color(LILAC),
                        );
                        ui.add_space(6.);
                        ui.label(
                            egui::RichText::new(format!(
                                "{}模式 · {} 章全部通过",
                                showcase.mode.label(),
                                showcase.chapters.len(),
                            ))
                            .size(15.)
                            .color(MUTED),
                        );
                        ui.label(
                            egui::RichText::new(format!(
                                "产物目录：{}",
                                showcase.output.file_name().map_or_else(
                                    || "target".to_owned(),
                                    |name| { name.to_string_lossy().into_owned() }
                                )
                            ))
                            .size(13.)
                            .color(MUTED),
                        );
                        ui.add_space(10.);
                        ui.label(
                            egui::RichText::new(
                                "内部输入回放；原版样本只读，截图与 XML 副本已写入演示目录",
                            )
                            .size(13.)
                            .color(MUTED),
                        );
                    });
                });
        });
}
