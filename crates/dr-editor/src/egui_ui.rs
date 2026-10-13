//! Bevy 与 egui 的最小适配层；文本编辑、剪贴板和 IME 均由 bevy_egui 处理。
//!
//! 集成时保留 properties::setup/actions 工具栏；在 Update 原 properties::input
//! 的位置调用 prepare_input，移除旧 properties::input/render 系统。
//! EguiPrimaryContextPass 由插件在 PostUpdate::EndPass 内执行，不应手动运行。

use super::*;
use bevy_egui::{EguiContexts, EguiPlugin, EguiPrimaryContextPass, egui};

/// 本次 egui pass 的逻辑坐标，供窗口自动化直接生成 egui 指针事件。
/// 若测试改为发送 Bevy 窗口事件，还需换算窗口缩放与 ctx.pixels_per_point()。
#[derive(Resource, Default)]
pub struct UiHits(pub Vec<(properties::Action, egui::Rect)>, pub bool);

pub struct EditorEguiPlugin;

impl Plugin for EditorEguiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(EguiPlugin::default())
            .init_resource::<UiHits>()
            .init_resource::<help::HelpState>()
            .init_resource::<files::PendingFileAction>()
            .init_resource::<topology_ui::ConnectionEditor>()
            .init_resource::<panels::egui_panel::UiState>()
            .add_systems(
                EguiPrimaryContextPass,
                (
                    configure,
                    panels::egui_panel::draw,
                    draw,
                    topology_ui::draw,
                    connection_lines::ui,
                    help::draw,
                )
                    .chain(),
            );
    }
}

/// 边栏按钮焦点不独占画布快捷键；只有属性草稿中的文本/模态独占输入。
/// 直接检查事务状态，避免依赖 PostUpdate 才更新的上一帧焦点。
pub fn canvas_input_available(
    inspector: Option<Res<properties::Inspector>>,
    topology: Option<Res<topology_ui::ConnectionEditor>>,
    pending: Option<Res<files::PendingFileAction>>,
    help: Option<Res<help::HelpState>>,
    lines: Option<Res<connection_lines::Settings>>,
) -> bool {
    !inspector.is_some_and(|inspector| inspector.is_open())
        && !topology.is_some_and(|state| state.open)
        && !pending.is_some_and(|state| state.is_blocked())
        && !help.is_some_and(|state| state.open || state.suppress_frame)
        && !lines.is_some_and(|state| state.open)
}

/// 放在 pointer_over_ui、properties::actions 之后，画布输入系统之前。
/// 这里只管理编辑器模式及 F2，不消费/复制 bevy_egui 的任何文字或输入法事件。
#[allow(clippy::too_many_arguments)]
pub fn prepare_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut inspector: ResMut<properties::Inspector>,
    mut topology: Option<ResMut<topology_ui::ConnectionEditor>>,
    document: Res<EditorDocument>,
    mut pointer: ResMut<panels::UiPointer>,
    mut drag: ResMut<DragState>,
    mut cursor: ResMut<EditorCursor>,
    mut camera_drag: ResMut<CameraDrag>,
    pending: Option<Res<files::PendingFileAction>>,
    help: Option<Res<help::HelpState>>,
    lines: Option<ResMut<connection_lines::Settings>>,
    mut touch: Option<ResMut<mobile_input::GestureInput>>,
) {
    if help.is_some_and(|state| state.open || state.suppress_frame) {
        if let Some(touch) = touch.as_mut() {
            touch.navigation = None;
        }
        return;
    }
    let mut line_open = false;
    if let Some(mut lines) = lines {
        if lines.open && keys.just_pressed(KeyCode::Escape) {
            lines.open = false;
        }
        line_open = lines.open;
    }
    if pending.is_some_and(|state| state.is_blocked()) {
        if let Some(touch) = touch.as_mut() {
            touch.navigation = None;
        }
        cursor.manual_connection = None;
        drag.cancel();
        cursor.cancel_placement();
        cursor.paste = None;
        camera_drag.0 = None;
        pointer.blocked = true;
        return;
    }
    if let Some(state) = topology.as_mut() {
        if !inspector.is_open() && keys.just_pressed(KeyCode::F6) {
            state.open = !state.open;
        }
        if state.open && keys.just_pressed(KeyCode::Escape) {
            state.open = false;
        }
        if inspector.is_open() {
            state.open = false;
        }
    }
    let topology_open = topology.as_ref().is_some_and(|state| state.open);
    if !inspector.is_open() && !topology_open && keys.just_pressed(KeyCode::F2) {
        properties::open(&mut inspector, &document);
    }
    if inspector.is_open() || topology_open || line_open {
        if let Some(touch) = touch.as_mut() {
            touch.navigation = None;
        }
        cursor.manual_connection = None;
        drag.cancel();
        cursor.cancel_placement();
        cursor.paste = None;
        camera_drag.0 = None;
    }
    // 沿用现有 Bevy 工具栏的命中结果，而不是覆盖它。
    pointer.blocked |= inspector.is_open() || topology_open || line_open;
}

fn configure_chinese_font(ctx: &egui::Context, paths: &EditorPaths) {
    let path = std::path::Path::new(&paths.assets)
        .join("fonts/HarmonyOS_Sans/HarmonyOS_Sans_SC/HarmonyOS_Sans_SC_Regular.ttf");
    match std::fs::read(&path) {
        Ok(bytes) => {
            let mut fonts = egui::FontDefinitions::default();
            let name = "HarmonyOS Sans SC".to_owned();
            fonts
                .font_data
                .insert(name.clone(), egui::FontData::from_owned(bytes).into());
            // 保留 egui 默认字形及等宽字体；原生顶栏也使用同一份中文字体。
            fonts
                .families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .insert(0, name.clone());
            fonts
                .families
                .entry(egui::FontFamily::Monospace)
                .or_default()
                .push(name);
            ctx.set_fonts(fonts);
        }
        Err(error) => warn!("无法读取 egui 中文字体 {}：{error}", path.display()),
    }
}

pub(crate) fn ui_color(color: Color) -> egui::Color32 {
    let [r, g, b, a] = color.to_srgba().to_u8_array();
    egui::Color32::from_rgba_unmultiplied(r, g, b, a)
}

fn configure_theme(ctx: &egui::Context) {
    use crate::native_buttons::{
        ACCENT, BORDER, BUTTON, BUTTON_FONT_SIZE, HOVERED, PANEL, PRESSED, RADIUS, SELECTED, TEXT,
    };
    // 原生顶栏始终是深色，egui 不随系统主题变成另一套界面。
    ctx.set_theme(egui::Theme::Dark);
    ctx.style_mut_of(egui::Theme::Dark, |style| {
        for (kind, size) in [
            (egui::TextStyle::Body, 15.0),
            (egui::TextStyle::Button, BUTTON_FONT_SIZE),
            (egui::TextStyle::Heading, 20.0),
            (egui::TextStyle::Small, 12.0),
        ] {
            style
                .text_styles
                .insert(kind, egui::FontId::proportional(size));
        }
        style.spacing.item_spacing = egui::vec2(8.0, 6.0);
        style.spacing.button_padding = egui::vec2(10.0, 5.0);
        style.spacing.interact_size = egui::vec2(40.0, 28.0);
        style.spacing.window_margin = egui::Margin::same(12);
        let visuals = &mut style.visuals;
        visuals.panel_fill = ui_color(PANEL);
        visuals.window_fill = ui_color(PANEL);
        visuals.extreme_bg_color = ui_color(Color::srgb(0.08, 0.08, 0.08));
        visuals.faint_bg_color = ui_color(BUTTON);
        visuals.window_stroke = egui::Stroke::new(1.0, ui_color(BORDER));
        visuals.window_corner_radius = egui::CornerRadius::same(RADIUS);
        visuals.menu_corner_radius = egui::CornerRadius::same(RADIUS);
        visuals.weak_text_color = Some(ui_color(Color::srgb(0.62, 0.62, 0.62)));
        visuals.selection.bg_fill = ui_color(SELECTED);
        visuals.selection.stroke = egui::Stroke::new(1.0, ui_color(ACCENT));
        visuals.hyperlink_color = ui_color(ACCENT);
        for (widget, background) in [
            (&mut visuals.widgets.noninteractive, PANEL),
            (&mut visuals.widgets.inactive, BUTTON),
            (&mut visuals.widgets.hovered, HOVERED),
            (&mut visuals.widgets.active, PRESSED),
            (&mut visuals.widgets.open, SELECTED),
        ] {
            widget.bg_fill = ui_color(background);
            widget.weak_bg_fill = ui_color(background);
            widget.bg_stroke = egui::Stroke::new(
                1.0,
                ui_color(
                    if background == HOVERED || background == PRESSED || background == SELECTED {
                        ACCENT
                    } else {
                        BORDER
                    },
                ),
            );
            widget.fg_stroke = egui::Stroke::new(1.0, ui_color(TEXT));
            widget.corner_radius = egui::CornerRadius::same(RADIUS);
            widget.expansion = 0.0;
        }
    });
}

#[cfg(test)]
pub(crate) fn test_context() -> egui::Context {
    let ctx = egui::Context::default();
    configure_theme(&ctx);
    ctx
}

fn configure(mut contexts: EguiContexts, paths: Res<EditorPaths>, mut configured: Local<bool>) {
    if *configured {
        return;
    }
    let Ok(ctx) = contexts.ctx_mut() else { return };
    configure_theme(ctx);
    configure_chinese_font(ctx, &paths);
    *configured = true;
}

pub fn draw(
    mut contexts: EguiContexts,
    paths: Res<EditorPaths>,
    mut inspector: ResMut<properties::Inspector>,
    mut document: ResMut<EditorDocument>,
    mut hits: ResMut<UiHits>,
    mut pending: ResMut<files::PendingFileAction>,
) {
    hits.0.clear();
    hits.1 = false;
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };
    if pending.is_blocked() {
        files::show_confirmation(ctx, &mut pending, &paths);
        return;
    }
    if !inspector.is_open() {
        return;
    }
    hits.1 = ctx.input(|input| input.is_scrolling());
    if let Some(action) = properties::egui_panel::show(ctx, &mut inspector, &document, &mut hits.0)
    {
        properties::act(&action, &mut inspector, &mut document);
    }
}

/// 窗口自测共享驱动：通过真实 egui 指针/滚轮事件操作可见控件。
#[derive(Default)]
pub struct UiTestInput {
    settle: u8,
}

impl UiTestInput {
    pub fn tick(&mut self, input: &mut bevy_egui::EguiInput) -> bool {
        input.0.focused = true;
        if self.settle > 0 {
            self.settle -= 1;
            return true;
        }
        false
    }

    pub fn click_rect(&mut self, rect: egui::Rect, input: &mut bevy_egui::EguiInput) {
        // 一次短点击的两条边沿成对交付，如同系统在一帧间收到完整短按。
        // 不让观看停顿或慢帧把自测短点击变成长按；长按/拖动另用真实分帧输入。
        let pos = rect.center();
        input.0.events.extend([
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            },
        ]);
        self.settle = 2;
    }

    pub fn click(
        &mut self,
        action: properties::Action,
        hits: &UiHits,
        input: &mut bevy_egui::EguiInput,
    ) -> bool {
        // 滚轮有平滑惯性；等正文稳定后再按下/释放，避免控件移出点击位置。
        if hits.1 {
            return false;
        }
        if let Some((_, rect)) = hits.0.iter().find(|(candidate, _)| *candidate == action) {
            self.click_rect(*rect, input);
            true
        } else {
            // 应用/取消固定在正文外，取其上方作为模态内的滚动位置。
            let Some((_, rect)) = hits
                .0
                .iter()
                .find(|(action, _)| *action == properties::Action::Apply)
            else {
                // Modal 首次测量/字体加载的 pass 还没有可交互控件。
                return false;
            };
            let pos = egui::pos2(rect.center().x, rect.top() - 80.0);
            let up = matches!(
                action,
                properties::Action::Focus(properties::Field::Name) | properties::Action::OpenRepair
            );
            input.0.events.extend([
                egui::Event::PointerMoved(pos),
                egui::Event::MouseWheel {
                    phase: egui::TouchPhase::Move,
                    unit: egui::MouseWheelUnit::Point,
                    delta: egui::vec2(0.0, if up { 350.0 } else { -350.0 }),
                    modifiers: egui::Modifiers::NONE,
                },
            ]);
            self.settle = 2;
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{UiTestInput, configure_theme, egui, test_context, ui_color};
    use crate::native_buttons::{self, *};
    use bevy::prelude::*;
    use bevy_egui::EguiInput;

    /// 显式推进 egui 的秒级时间，不 sleep，也不启动窗口或渲染插件。
    struct ButtonFrames {
        ctx: egui::Context,
        rect: egui::Rect,
    }

    impl ButtonFrames {
        fn new() -> Self {
            let mut frames = Self {
                ctx: crate::egui_ui::test_context(),
                rect: egui::Rect::NOTHING,
            };
            for time in [0.0, 0.016, 0.032] {
                assert!(!frames.frame(time, vec![]));
            }
            frames
        }

        fn frame(&mut self, time: f64, events: Vec<egui::Event>) -> bool {
            self.ctx.begin_pass(egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(640.0, 480.0),
                )),
                time: Some(time),
                focused: true,
                events,
                ..Default::default()
            });
            let mut clicked = false;
            egui::Area::new(egui::Id::new("headless_undo_button"))
                .fixed_pos(egui::pos2(32.0, 32.0))
                .movable(false)
                .show(&self.ctx, |ui| {
                    // 固定绘制顺序、文案和布局，排除控件身份漂移这一独立变量。
                    let response = ui.button("Undo");
                    self.rect = response.rect;
                    clicked = response.clicked();
                });
            self.ctx.end_pass().textures_delta.clear();
            clicked
        }

        fn pointer_button(&self, pressed: bool) -> Vec<egui::Event> {
            let pos = self.rect.center();
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ]
        }

        fn press(&mut self) {
            assert!(!self.frame(0.1, self.pointer_button(true)));
        }

        fn release(&mut self, time: f64) -> bool {
            self.frame(time, self.pointer_button(false))
        }
    }

    #[test]
    fn shared_short_click_packet_survives_slow_frames_and_is_not_repeated() {
        for gap in [0.016, 1.0, 10.0] {
            let mut frames = ButtonFrames::new();
            let mut driver = UiTestInput::default();
            let mut input = EguiInput::default();
            driver.click_rect(frames.rect, &mut input);
            assert!(
                frames.frame(0.1 + gap, std::mem::take(&mut input.0.events)),
                "短点击应以完整事件对交给同一次 pass，不能被 {gap}s 的帧间隔变成长按"
            );
            for time in [0.2 + gap, 1.2 + gap] {
                assert!(driver.tick(&mut input));
                assert!(input.0.events.is_empty(), "等待帧不能重复注入释放");
                assert!(!frames.frame(time, std::mem::take(&mut input.0.events)));
            }
            assert!(!driver.tick(&mut input));
        }
    }

    #[test]
    fn clean_450ms_hold_across_empty_frames_still_clicks() {
        let mut frames = ButtonFrames::new();
        frames.press();
        // 真实界面在 smoke 的两个步骤之间仍持续执行 pass。
        for frame in 1..27 {
            assert!(!frames.frame(0.1 + f64::from(frame) / 60.0, vec![]));
        }
        assert!(
            frames.release(0.55),
            "干净的 450ms 按住低于 egui 默认 800ms 阈值，不应自行丢失点击"
        );
    }

    #[test]
    fn pointer_gone_in_an_intermediate_pass_cancels_click() {
        let mut frames = ButtonFrames::new();
        frames.press();
        assert!(!frames.frame(0.116, vec![egui::Event::PointerGone]));
        assert!(!frames.frame(0.132, vec![]));
        assert!(
            !frames.release(0.55),
            "后续回到原点并释放不能恢复被 PointerGone 清掉的点击归属"
        );
    }

    #[test]
    fn distant_move_then_virtual_move_back_still_cancels_short_click() {
        let mut frames = ButtonFrames::new();
        frames.press();
        let pos = frames.rect.center();
        assert!(!frames.frame(
            0.115,
            vec![
                egui::Event::PointerMoved(pos + egui::vec2(80.0, 0.0)),
                egui::Event::PointerMoved(pos),
            ],
        ));
        assert!(!frames.frame(0.13, vec![]));
        assert!(
            !frames.release(0.145),
            "即使只按住 45ms，末尾补虚拟坐标也不能撤销曾经超过点击距离的记录"
        );
    }

    #[test]
    fn demo_filter_then_virtual_move_and_45ms_release_clicks() {
        // 调用生产 restore_pointer，而不是在测试里复制过滤条件。
        let output = tempfile::tempdir().unwrap();
        let args = vec![
            "--demo-showcase".to_owned(),
            output.path().to_string_lossy().into_owned(),
        ];
        let showcase = crate::demo::Showcase::from_args(&args).unwrap().unwrap();
        let mut app = App::new();
        app.insert_resource(showcase)
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<ButtonInput<KeyCode>>()
            .add_systems(Update, crate::demo::restore_pointer);
        let entity = app.world_mut().spawn(EguiInput::default()).id();

        let mut frames = ButtonFrames::new();
        frames.press();
        let pos = frames.rect.center();
        for time in [0.115, 0.13] {
            app.world_mut()
                .get_mut::<EguiInput>(entity)
                .unwrap()
                .0
                .events = vec![
                egui::Event::PointerMoved(pos + egui::vec2(80.0, 0.0)),
                egui::Event::PointerGone,
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                },
                egui::Event::Text("保留非指针事件".to_owned()),
            ];
            app.update();
            let mut input = app.world_mut().get_mut::<EguiInput>(entity).unwrap();
            assert!(
                input.0.events.is_empty(),
                "原生文本和误点都不能混入内部回放"
            );
            let mut events = std::mem::take(&mut input.0.events);
            // 与 demo 的执行顺序一致：先隔离原生输入，再补当帧虚拟位置。
            events.push(egui::Event::PointerMoved(pos));
            assert!(!frames.frame(time, events));
        }
        assert!(
            frames.release(0.145),
            "隔离原生指针后持续补虚拟 Move，45ms 短点击应成功"
        );
    }
    #[test]
    fn egui_and_native_buttons_share_palette_font_and_corners() {
        let ctx = test_context();
        let style = ctx.style_of(egui::Theme::Dark);
        assert_eq!(style.visuals.widgets.inactive.bg_fill, ui_color(BUTTON));
        assert_eq!(style.visuals.widgets.hovered.bg_fill, ui_color(HOVERED));
        assert_eq!(style.visuals.widgets.active.bg_fill, ui_color(PRESSED));
        assert_eq!(style.visuals.selection.bg_fill, ui_color(SELECTED));
        assert_eq!(style.visuals.text_color(), ui_color(TEXT));
        assert_eq!(
            style.text_styles[&egui::TextStyle::Button].size,
            BUTTON_FONT_SIZE
        );
        assert_eq!(
            style.visuals.widgets.inactive.corner_radius,
            egui::CornerRadius::same(RADIUS)
        );
        let mut world = World::new();
        let entity = world
            .spawn((
                native_buttons::press_triggered(),
                Node {
                    border_radius: native_corners(),
                    ..default()
                },
            ))
            .id();
        assert_eq!(
            world.get::<Node>(entity).unwrap().border_radius,
            BorderRadius::all(px(f32::from(RADIUS)))
        );
        assert_eq!(
            world.get::<BorderColor>(entity),
            Some(&BorderColor::all(BORDER))
        );
    }

    #[test]
    fn sandbox_palette_is_neutral_with_lime_accent_and_small_corners() {
        let ctx = test_context();
        let style = ctx.style_of(egui::Theme::Dark);
        assert_eq!(style.visuals.panel_fill, egui::Color32::from_gray(33));
        assert_eq!(
            style.visuals.widgets.inactive.bg_fill,
            egui::Color32::from_gray(48)
        );
        assert_eq!(
            style.visuals.hyperlink_color,
            egui::Color32::from_rgb(179, 219, 5)
        );
        assert_eq!(
            style.visuals.widgets.hovered.bg_stroke.color,
            style.visuals.hyperlink_color
        );
        assert_eq!(
            style.visuals.widgets.inactive.corner_radius,
            egui::CornerRadius::same(2)
        );
    }

    #[test]
    fn theme_preserves_focus_and_shared_spacing() {
        let ctx = egui::Context::default();
        let focus = egui::Id::new("保留草稿焦点");
        ctx.memory_mut(|memory| memory.request_focus(focus));
        configure_theme(&ctx);
        assert_eq!(ctx.memory(|memory| memory.focused()), Some(focus));
        assert_eq!(
            ctx.style_of(egui::Theme::Dark).spacing.interact_size.y,
            28.0
        );
        assert_eq!(
            ctx.style_of(egui::Theme::Dark).spacing.item_spacing,
            egui::vec2(8.0, 6.0)
        );
    }
}
