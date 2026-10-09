//! 非交互的章节、边界样例与输入说明层。
use super::*;

/// 说明区不捕获指针；避免改变被演示控件的焦点与命中。
#[allow(clippy::too_many_arguments)]
pub(crate) fn overlay(
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
    use bevy_egui::egui;
    let Some(showcase) = showcase else {
        return;
    };
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };
    if let Some(hint) = &showcase.key_hints.latest {
        paint_key_hint(ctx, hint, Instant::now(), help.open);
    }
    let held: Vec<_> = [MouseButton::Left, MouseButton::Middle, MouseButton::Right]
        .into_iter()
        .filter(|button| mouse.pressed(*button))
        .filter_map(mouse_label)
        .map(|(_, label)| label)
        .collect();
    if !held.is_empty() {
        let now = Instant::now();
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
        paint_input_hint(ctx, hint, Instant::now(), help.open, true);
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
            let painter = ctx.layer_painter(egui::LayerId::new(
                egui::Order::Tooltip,
                egui::Id::new("demo-pointer"),
            ));
            painter.circle_stroke(
                point,
                9.,
                egui::Stroke::new(2., egui::Color32::from_rgb(222, 185, 255)),
            );
            painter.circle_filled(point, 2., egui::Color32::WHITE);
        }
    }
    // 帮助需要完整阅读：暂停章节横幅，并把按键提示放在窗口下方空隙。
    if help.open {
        return;
    }
    if showcase.presentation {
        // 录制模式额外提供顶部短章名；下方仍保留十章说明框，字幕在视频独立底栏。
        let title = if showcase.completed {
            "演示完成"
        } else {
            showcase.chapter().title
        };
        let painter = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Tooltip,
            egui::Id::new("demo-chapter"),
        ));
        painter.text(
            egui::pos2(
                ctx.content_rect().center().x,
                ctx.content_rect().top() + 14.,
            ),
            egui::Align2::CENTER_TOP,
            title,
            egui::FontId::proportional(20.),
            egui::Color32::from_rgb(221, 203, 255),
        );
    }
    let (title, description) = if showcase.completed {
        (
            format!(
                "{1}演示完成：{0}/{0} 章节通过",
                showcase.chapters.len(),
                showcase.mode.label()
            ),
            "原版样本未保存覆盖。截图和 XML 副本已写入演示目录；可以关闭窗口。",
        )
    } else {
        let chapter = showcase.chapter();
        (
            format!(
                "{}演示 {}/{} · {}",
                showcase.mode.label(),
                showcase.index + 1,
                showcase.chapters.len(),
                chapter.title
            ),
            showcase.description.unwrap_or(chapter.description),
        )
    };
    let details = details.as_ref().and_then(|details| details.caption());
    let left_of = if details.is_some() && inspector.is_open() {
        // 用本帧真实控件位置避让，而不是假定模态高度；错误文字和取消必须可见。
        let Some((_, apply)) = property_hits
            .0
            .iter()
            .find(|(action, _)| *action == properties::Action::Apply)
        else {
            return; // 首次布局尚无控件时不闪现一个挡住模态的说明层。
        };
        Some(apply.left())
    } else {
        None
    };
    caption::paint(
        ctx,
        &title,
        description,
        details.as_deref(),
        showcase.step,
        left_of,
    );
}
