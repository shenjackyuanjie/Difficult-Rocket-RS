use crate::demo;
use crate::document::EditorDocument;
use crate::panels::*;
use crate::placement;
use crate::placement::EditorCursor;
use crate::window::SmokeTest;
use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use dr_core::Ship;

#[derive(Default)]
pub struct State {
    phase: u8,
    pointer: Option<Vec2>,
    before: Option<Ship>,
    after: Option<Ship>,
    palette_before_scroll: f32,
    input: crate::egui_ui::UiTestInput,
    collapsed_category: Option<String>,
}

fn placement_point(window: &Window) -> Vec2 {
    // 保留原 1440×900 自测的世界偏移，不能把固定屏幕坐标用于另一尺寸。
    Vec2::new(window.width(), window.height()) * 0.5 + Vec2::new(130., 150.)
}

fn palette_scroll_point(ui: &egui_panel::UiState) -> Option<bevy_egui::egui::Pos2> {
    let area = ui.palette_area?;
    // Part 命中矩形已按 ScrollArea 裁剪，确保滚轮落在列表而不是标题/详情。
    ui.hits.iter().find_map(|(action, rect)| {
        (matches!(action, PanelButton::Part(_))
            && rect.is_positive()
            && area.contains(rect.center()))
        .then(|| rect.center())
    })
}

/// 同一实际 Part 命中同时提供 egui 点和 Window 逻辑点，缩放只换算一次。
fn palette_window_point(
    ui: &egui_panel::UiState,
    window: &Window,
) -> Option<(bevy_egui::egui::Pos2, Vec2)> {
    let point = palette_scroll_point(ui)?;
    let scale = ui.pixels_per_point / window.scale_factor();
    (scale.is_finite() && scale > 0.).then(|| (point, Vec2::new(point.x, point.y) * scale))
}

fn panel_description(phase: u8) -> &'static str {
    match phase {
        0..=1 => "左键点击部件抽屉中的部件进入预览；移到画布后才放置，不改原船体。",
        2..=3 => "R 旋转、X 镜像待放置预览；左键确认前仍可取消。",
        4..=5 => "红色预览表示碰撞：左键点击被拒绝，船体保持不变。",
        6..=7 => "移到空白处，左键放置部件并提交一步历史。",
        8..=9 => "Ctrl+Z / Ctrl+Y 撤销与重做整个放置操作。",
        10..=11 => "部件抽屉阻挡画布输入；滚轮只滚抽屉中的列表，Esc 取消待放置预览。",
        12 => "原版分组：左键点击标题收起一个类别，其他类别继续显示。",
        _ => "类别已收起；点击「全部展开」恢复完整目录，折叠不修改船体。",
    }
}

/// 在真实窗口和 ECS 调度中验证目录→画布→撤销重做及 UI 输入隔离。
#[allow(clippy::too_many_arguments)]
pub fn run(
    mode: Res<SmokeTest>,
    mut state: Local<State>,
    document: Res<EditorDocument>,
    cursor: Res<EditorCursor>,
    ui: Res<egui_panel::UiState>,
    mut inputs: Query<&mut bevy_egui::EguiInput, With<bevy_egui::PrimaryEguiContext>>,
    mut windows: Query<(Entity, &mut Window), With<bevy::window::PrimaryWindow>>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut wheels: MessageWriter<MouseWheel>,

    cameras: Query<&Projection, With<Camera2d>>,
    previews: Query<(&Sprite, &Transform), With<placement::PlacementVisual>>,
    mut commands: Commands,
    palette: Res<Palette>,
    mut showcase: Option<ResMut<demo::Showcase>>,
) {
    if !mode.panels || mode.started.elapsed().as_secs() < 3 {
        return;
    }
    assert!(
        demo::within_timeout(showcase.as_deref(), mode.started, 60),
        "面板交互自测超时"
    );
    let Ok((window_id, mut window)) = windows.single_mut() else {
        return;
    };
    // 注入式回归持续保存焦点/光标，避免桌面 CursorMoved 覆盖上一阶段坐标。
    // 操作系统前台是否成立仍由外部驱动独立校验，不据此伪造真实焦点验收。
    window.focused = true;
    if let Some(point) = state.pointer {
        window.set_cursor_position(Some(point));
    }
    let Ok(mut input) = inputs.single_mut() else {
        return;
    };
    if state.input.tick(&mut input) {
        return;
    }
    demo::describe(&mut showcase, panel_description(state.phase));
    match state.phase {
        0 => {
            state.before = Some(document.ship.clone());
            if !state
                .input
                .click_panel(PanelButton::Part(0), &ui, &mut input)
            {
                return;
            }
        }
        1 => {
            assert!(cursor.placing, "目录选择未进入放置预览");
            assert_eq!(state.before.as_ref(), Some(&document.ship));
            window.focused = true;
            let point = placement_point(&window);
            window.set_cursor_position(Some(point));
        }
        2 => {
            assert_eq!(state.before.as_ref(), Some(&document.ship));
            assert!(previews.single().is_ok(), "画布中没有创建放置预览");
            keys.press(KeyCode::KeyR);
            keys.press(KeyCode::KeyX);
        }
        3 => {
            assert_eq!(
                state.before.as_ref(),
                Some(&document.ship),
                "预览变换改写了文档"
            );
            assert_eq!(cursor.rotation, 1);
            assert!(cursor.flip_x);
            let (sprite, transform) = previews.single().unwrap();
            assert!(sprite.flip_x);
            assert!(
                transform
                    .rotation
                    .abs_diff_eq(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2), 0.0001)
            );
            use bevy::render::view::screenshot::{Screenshot, save_to_disk};
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk("target/editor-placement-preview.png"));
        }
        4 => {
            keys.reset_all();
            let pod = document
                .ship
                .all_parts()
                .find(|part| part.pod.is_some())
                .expect("自测样本需要驾驶舱");
            window.focused = true;
            let position = Vec2::new(
                window.width() / 2.0 + pod.x as f32 * 60.0,
                window.height() / 2.0 - pod.y as f32 * 60.0,
            );
            window.set_cursor_position(Some(position));
            mouse.press(MouseButton::Left);
        }
        5 => {
            assert_eq!(
                state.before.as_ref(),
                Some(&document.ship),
                "重叠位置被错误放置"
            );
            assert!(!document.history.can_undo(), "被拒绝的放置产生了撤销记录");
            assert_eq!(
                previews.single().unwrap().0.color,
                Color::srgba(1.0, 0.2, 0.2, 0.65)
            );
            use bevy::render::view::screenshot::{Screenshot, save_to_disk};
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk("target/editor-collision-preview.png"));
        }
        6 => {
            mouse.release(MouseButton::Left);
            window.focused = true;
            let point = placement_point(&window);
            window.set_cursor_position(Some(point));
        }
        7 => {
            keys.reset_all();
            window.focused = true;
            let point = placement_point(&window);
            window.set_cursor_position(Some(point));
            mouse.press(MouseButton::Left);
        }
        8 => {
            assert_eq!(
                document.ship.all_parts().count(),
                state.before.as_ref().unwrap().all_parts().count() + 1,
                "合法放置失败：{}；光标 {:?}，有效 {}，预览 {}，焦点 {}",
                document.status,
                cursor.world,
                cursor.valid,
                cursor.placing,
                window.focused,
            );
            assert!(document.dirty);
            state.after = Some(document.ship.clone());
            mouse.release(MouseButton::Left);
            keys.press(KeyCode::ControlLeft);
            keys.press(KeyCode::KeyZ);
        }
        9 => {
            assert_eq!(
                state.before.as_ref(),
                Some(&document.ship),
                "一次撤销未还原整个放置操作"
            );
            keys.reset_all();
            keys.press(KeyCode::ControlLeft);
            keys.press(KeyCode::KeyY);
        }
        10 => {
            assert_eq!(state.after.as_ref(), Some(&document.ship), "重做未恢复部件");
            keys.reset_all();
            window.focused = true;
            let Some((_, window_point)) = palette_window_point(&ui, &window) else {
                return;
            };
            window.set_cursor_position(Some(window_point));
            mouse.press(MouseButton::Left);
        }
        11 => {
            assert_eq!(
                state.after.as_ref(),
                Some(&document.ship),
                "部件抽屉点击穿透到了画布"
            );
            let Some((point, window_point)) = palette_window_point(&ui, &window) else {
                return;
            };
            state.palette_before_scroll = ui.palette_offset;
            mouse.release(MouseButton::Left);
            keys.press(KeyCode::Escape);
            window.focused = true;
            window.set_cursor_position(Some(window_point));
            input.0.events.extend([
                bevy_egui::egui::Event::PointerMoved(point),
                bevy_egui::egui::Event::MouseWheel {
                    phase: bevy_egui::egui::TouchPhase::Move,
                    unit: bevy_egui::egui::MouseWheelUnit::Point,
                    delta: bevy_egui::egui::vec2(0.0, -120.0),
                    modifiers: bevy_egui::egui::Modifiers::NONE,
                },
            ]);
            wheels.write(MouseWheel {
                phase: bevy::input::touch::TouchPhase::Moved,
                unit: MouseScrollUnit::Line,
                x: 0.0,
                y: -3.0,
                window: window_id,
            });
        }
        12 => {
            assert!(!cursor.placing, "Esc 未取消预览");
            assert!(
                ui.palette_offset > state.palette_before_scroll,
                "目录滚轮没有向下滚动列表：{} → {}",
                state.palette_before_scroll,
                ui.palette_offset
            );
            let Projection::Orthographic(projection) = cameras.single().unwrap() else {
                panic!("相机投影错误")
            };
            assert_eq!(projection.scale, 1.0, "目录滚轮意外缩放了画布");
            assert_eq!(state.after.as_ref(), Some(&document.ship));
            // 分组标题也在滚动区里，不能只等当前可见命中。
            // 复用 click_panel 的现有滚动寻址，让末组标题进入视口后再真实点击。
            let category = palette
                .groups(&document.catalog)
                .last()
                .expect("自测目录需要可见部件分组")
                .0
                .to_owned();
            if !state.input.click_panel(
                PanelButton::ToggleCategory(category.clone()),
                &ui,
                &mut input,
            ) {
                return;
            }
            state.collapsed_category = Some(category);
        }
        13 => {
            let category = state.collapsed_category.as_deref().unwrap();
            assert!(
                palette.collapsed.contains(category),
                "真实分组标题应收起当前类别"
            );
            let indices = palette.indices(&document.catalog);
            assert!(!indices.is_empty());
            let kinds: Vec<_> = document.catalog.visible().collect();
            assert!(
                indices
                    .iter()
                    .all(|index| category_name(kinds[*index]) != category)
            );
            assert!(indices.len() < kinds.len(), "被收起的组应确实隐藏部件");
            assert_eq!(
                state.after.as_ref(),
                Some(&document.ship),
                "目录折叠不应改文档"
            );
            if !state
                .input
                .click_panel(PanelButton::ExpandCategories, &ui, &mut input)
            {
                return;
            }
        }
        14 => {
            assert!(palette.collapsed.is_empty(), "全部展开应恢复所有类别");
            assert_eq!(
                palette.indices(&document.catalog).len(),
                document.catalog.visible().count()
            );
            assert_eq!(state.after.as_ref(), Some(&document.ship));
            use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk};
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk("target/editor-panels-smoke.png"))
                .observe(
                    |_: On<ScreenshotCaptured>, mut exit: MessageWriter<AppExit>| {
                        info!("面板交互自测通过：选择、碰撞拒绝及红色预览、放置、撤销重做、取消及部件抽屉输入隔离");
                        exit.write(AppExit::Success);
                    },
                );
        }
        _ => return,
    }
    state.pointer = window.cursor_position();
    state.phase += 1;
}

#[cfg(test)]
mod tests;
