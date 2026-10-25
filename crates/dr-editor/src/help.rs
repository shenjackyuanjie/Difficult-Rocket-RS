//! 按需显示操作说明，不让常驻 HUD 占据画布。
use crate::canvas_input;
use crate::files;
use crate::mobile_input;
use crate::native_buttons;
use crate::panels;
use crate::placement::EditorCursor;
use crate::properties;
use crate::render;
use crate::selection::DragState;
use crate::topology_ui;
use crate::view::CameraDrag;
use bevy::prelude::*;
use bevy_egui::{EguiContexts, egui};

#[derive(Resource, Default)]
pub struct HelpState {
    pub open: bool,
    pub suppress_frame: bool,
}

#[derive(Component)]
pub struct HelpButton;

/// 图标由共享 egui Painter 覆盖，命中与按压仍属于这个正式原生按钮。
pub fn setup(mut commands: Commands) {
    commands.spawn((
        native_buttons::press_triggered(),
        HelpButton,
        Name::new("操作帮助"),
        panels::ToolbarItem::Help,
        panels::EditorPanel,
        Node {
            position_type: PositionType::Absolute,
            left: px(96),
            bottom: px(4),
            width: px(48),
            height: px(48),
            min_width: px(48),
            min_height: px(48),
            border: UiRect::all(px(1)),
            border_radius: native_buttons::native_corners(),
            flex_shrink: 0.0,
            ..default()
        },
        BackgroundColor(native_buttons::BUTTON),
    ));
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn input(
    keys: Res<ButtonInput<KeyCode>>,
    mut activated: MessageReader<native_buttons::Activated>,
    buttons: Query<Entity, With<HelpButton>>,
    mut state: ResMut<HelpState>,
    inspector: Res<properties::Inspector>,
    topology: Res<topology_ui::ConnectionEditor>,
    pending: Res<files::PendingFileAction>,
    touch: Option<Res<mobile_input::GestureInput>>,
    mut drag: ResMut<DragState>,
    mut cursor: ResMut<EditorCursor>,
    mut camera_drag: ResMut<CameraDrag>,
    mut pointer: ResMut<panels::UiPointer>,
    mut ui: Option<ResMut<panels::egui_panel::UiState>>,
    mut menu: Option<ResMut<files::FileMenu>>,
) {
    let touch = touch.as_deref();
    state.suppress_frame = false;
    if inspector.is_open() || topology.open || pending.is_blocked() {
        activated.clear();
        state.open = false;
        if let Some(menu) = menu.as_deref_mut() {
            menu.close();
        }
        return;
    }
    let mut toggle = keys.just_pressed(KeyCode::F1)
        || touch.is_some_and(|input| {
            matches!(
                input.native_click.as_ref(),
                Some(mobile_input::NativeClick::Help)
            )
        });
    if touch.is_some_and(|input| input.override_mouse) {
        // 本帧触摸独占原生按钮，丢弃 picking 已经桥接的消息，避免兼容鼠标重复触发。
        activated.clear();
    } else {
        for event in activated.read() {
            toggle |= buttons.contains(event.entity);
        }
    }
    let was_open = state.open;
    if toggle {
        state.open = !state.open;
    }
    if state.open && keys.just_pressed(KeyCode::Escape) {
        state.open = false;
    }
    // 关闭的这一帧也不把 Esc/按钮点击漏给画布。
    state.suppress_frame = state.open || was_open || toggle;
    if state.suppress_frame {
        if let Some(ui) = ui.as_deref_mut() {
            ui.close_drawer();
        }
        if let Some(menu) = menu.as_deref_mut() {
            menu.close();
        }
        canvas_input::cancel_preview(&mut drag, &mut cursor, &mut camera_drag);
        pointer.blocked = true;
    }
}

const SHORTCUTS: &[(&str, &str)] = &[
    ("左键 / Shift+左键", "选择、拖动 / 增减选择"),
    ("R · Q/E · Shift+Q/E", "旋转 90° · ±15° · ±1°"),
    ("−15° / +15°", "精细旋转选区"),
    ("自由模式（图标开关）", "手动连断；移动不自动断连、不吸附"),
    ("吸附（图标开关）", "关闭后不再自动对齐与连边，已有连接保留"),
    ("Esc / 右键", "取消预览；重叠落下断开外部连接"),
    ("中键拖动 / Ctrl+A", "框选 / 全选；工具中可切换框选键"),
    ("Tab / Shift+Tab · P", "切换目录部件 · 放置"),
    ("Delete · X / Y", "删除 · 镜像；拖动中也可用"),
    ("跟随子节点（图标开关）", "默认开启；拖父节点时带上全部子孙"),
    ("拖到 Parts 抽屉", "删除相连分量；Ctrl+Z 撤销"),
    ("Ctrl+C / X / V", "复制 / 剪切 / 粘贴"),
    ("Ctrl+Z / Y", "撤销 / 重做"),
    ("Ctrl+N / O / S / Shift+S", "新建 / 打开 / 保存 / 另存为"),
    ("滚轮 · 空白拖动", "缩放 · 平移（框选键之外的键）"),
    ("触控 · 单指", "拖部件移动；拖空白平移"),
    ("触控 · 选目录后", "按住画布定位，抬指放置"),
    ("触控 · 双指", "取消预览，平移 / 缩放"),
    ("触控 · 全抬指", "全抬指后重新开始单指操作"),
    ("Home · F / Shift+F", "复位 · 适配整船 / 选区"),
    ("F2 · F6", "属性与分级 · 连接树 / 图"),
    ("F3 · F4 · F12", "轮廓 · 贴图 · 截图"),
    ("F1 / Esc", "开关帮助 / 关闭"),
];

fn legend_colors() -> [egui::Color32; 3] {
    // 与 attachment_hints 的候选、吸附 sRGB 一致；未连接透明度直接复用渲染合同。
    [
        Color::srgb(0.67, 0.56, 0.88),
        Color::srgb(0.85, 0.74, 1.0),
        Color::WHITE.with_alpha(render::UNLINKED_ALPHA),
    ]
    .map(crate::egui_ui::ui_color)
}

fn legend(ui: &mut egui::Ui) {
    let colors = legend_colors();
    ui.with_layout(
        egui::Layout::left_to_right(egui::Align::Center)
            .with_main_align(egui::Align::Center)
            .with_main_wrap(true),
        |ui| {
            for (text, color) in ["○ 候选", "○ 吸附", "◌ 未连接"].into_iter().zip(colors)
            {
                ui.colored_label(color, text);
            }
        },
    );
}

fn shortcuts(ui: &mut egui::Ui) {
    let width = ui.available_width();
    ui.set_width(width);
    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
    ui.vertical_centered(|ui| {
        if width < 420.0 {
            for (key, meaning) in SHORTCUTS {
                ui.add(egui::Label::new(egui::RichText::new(*key).strong()).wrap());
                ui.add(egui::Label::new(*meaning).wrap());
                ui.add_space(6.0);
            }
        } else {
            let column = (width - 20.0) / 2.0;
            egui::Grid::new("editor_shortcuts")
                .striped(true)
                .spacing([20.0, 9.0])
                .num_columns(2)
                .min_col_width(column)
                .max_col_width(column)
                .show(ui, |ui| {
                    for (key, meaning) in SHORTCUTS {
                        ui.add(
                            egui::Label::new(egui::RichText::new(*key).strong())
                                .wrap()
                                .halign(egui::Align::Center),
                        );
                        ui.add(
                            egui::Label::new(*meaning)
                                .wrap()
                                .halign(egui::Align::Center),
                        );
                        ui.end_row();
                    }
                });
        }
        ui.separator();
        legend(ui);
    });
}

fn show(ctx: &egui::Context, open: &mut bool) {
    if !*open {
        return;
    }
    let size = topology_ui::window_size(ctx, egui::vec2(720.0, 560.0));
    egui::Window::new("? 操作帮助")
        .id(egui::Id::new("editor_help"))
        .open(open)
        .collapsible(false)
        .resizable(false)
        // 默认位置只用于首次打开；不以 anchor 锁死正式 Window 的标题栏拖动。
        .pivot(egui::Align2::CENTER_CENTER)
        .default_pos(ctx.content_rect().center())
        .movable(true)
        // 固定的是当前可用区域内的外框尺寸，标题栏与正文共享同一实际宽度。
        .fixed_size(size)
        .vscroll(true)
        .show(ctx, shortcuts);
}

pub fn draw(mut contexts: EguiContexts, mut state: ResMut<HelpState>) {
    if let Ok(ctx) = contexts.ctx_mut() {
        show(ctx, &mut state.open);
    }
}

#[cfg(test)]
mod tests;
