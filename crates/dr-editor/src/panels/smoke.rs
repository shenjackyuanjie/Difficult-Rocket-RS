use super::*;

#[derive(Default)]
pub(crate) struct State {
    phase: u8,
    pointer: Option<Vec2>,
    before: Option<Ship>,
    after: Option<Ship>,
    palette_before_scroll: f32,
    input: crate::egui_ui::UiTestInput,
    filtered_category: Option<String>,
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

fn panel_description(phase: u8) -> &'static str {
    match phase {
        0..=1 => "左键点击右侧部件目录进入预览；移到画布后才放置，不改原船体。",
        2..=3 => "R 旋转、X 镜像待放置预览；左键确认前仍可取消。",
        4..=5 => "红色预览表示碰撞：左键点击被拒绝，船体保持不变。",
        6..=7 => "移到空白处，左键放置部件并提交一步历史。",
        8..=9 => "Ctrl+Z / Ctrl+Y 撤销与重做整个放置操作。",
        10..=11 => "侧栏阻挡画布输入；滚轮只滚部件目录，Esc 取消待放置预览。",
        12 => "已有分类筛选：左键点击一个类别，只显示该类别部件。",
        _ => "分类筛选已生效；点击「全部」恢复完整目录，筛选不修改船体。",
    }
}

/// 在真实窗口和 ECS 调度中验证目录→画布→撤销重做及 UI 输入隔离。
#[allow(clippy::too_many_arguments)]
pub(crate) fn run(
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
            window.set_cursor_position(Some(Vec2::new(120.0, 400.0)));
            mouse.press(MouseButton::Left);
        }
        11 => {
            assert_eq!(
                state.after.as_ref(),
                Some(&document.ship),
                "侧栏点击穿透到了画布"
            );
            let Some(point) = palette_scroll_point(&ui) else {
                return;
            };
            state.palette_before_scroll = ui.palette_offset;
            mouse.release(MouseButton::Left);
            keys.press(KeyCode::Escape);
            window.focused = true;
            let window_point =
                Vec2::new(point.x, point.y) * ui.pixels_per_point / window.scale_factor();
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
            let Some(category) = ui.hits.iter().find_map(|(action, _)| match action {
                PanelButton::Category(Some(category)) => Some(category.clone()),
                _ => None,
            }) else {
                return;
            };
            if !state.input.click_panel(
                PanelButton::Category(Some(category.clone())),
                &ui,
                &mut input,
            ) {
                return;
            }
            if showcase.is_some() {
                let rect = ui
                    .hits
                    .iter()
                    .find(|(action, _)| *action == PanelButton::Category(Some(category.clone())))
                    .map(|(_, rect)| rect);
                info!(phase = state.phase, ?category, ?rect, window_pointer = ?window.cursor_position(),
                    palette_offset = ui.palette_offset, scrolling = ui.scrolling,
                    "演示分类控件注入按下");
            }
            state.filtered_category = Some(category);
        }
        13 => {
            if showcase.is_some() {
                info!(phase = state.phase, category = ?palette.category,
                    expected = ?state.filtered_category, "演示分类动作消费后验证");
            }
            assert_eq!(
                palette.category, state.filtered_category,
                "真实分类按钮应改变目录筛选"
            );
            let indices = palette.indices(&document.catalog);
            assert!(!indices.is_empty());
            let kinds: Vec<_> = document.catalog.visible().collect();
            assert!(
                indices
                    .iter()
                    .all(|index| Some(category_name(kinds[*index])) == palette.category.as_deref())
            );
            assert!(indices.len() < kinds.len(), "选定分类应确实隐藏其他类别");
            assert_eq!(
                state.after.as_ref(),
                Some(&document.ship),
                "目录筛选不应改文档"
            );
            if !state
                .input
                .click_panel(PanelButton::Category(None), &ui, &mut input)
            {
                return;
            }
        }
        14 => {
            assert!(palette.category.is_none(), "全部按钮应清除分类筛选");
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
                        info!("面板交互自测通过：选择、碰撞拒绝及红色预览、放置、撤销重做、取消及侧栏输入隔离");
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
mod tests {
    use super::*;
    use bevy_egui::egui;

    #[test]
    fn smoke_placement_keeps_world_offset_at_both_window_sizes() {
        for (width, height, expected) in [
            (1440, 900, Vec2::new(850., 600.)),
            (1920, 1080, Vec2::new(1090., 690.)),
        ] {
            let window = Window {
                resolution: bevy::window::WindowResolution::new(width, height)
                    .with_scale_factor_override(1.),
                ..default()
            };
            assert_eq!(placement_point(&window), expected);
            assert!(
                crate::view::canvas(Vec2::new(window.width(), window.height())).contains(expected)
            );
            assert_eq!(
                expected - Vec2::new(window.width(), window.height()) / 2.,
                Vec2::new(130., 150.)
            );
        }
    }

    #[test]
    fn smoke_scroll_targets_visible_palette_row_instead_of_fixed_x() {
        for width in [1440., 1920.] {
            let mut ui = egui_panel::UiState::default();
            let left = width - 296.;
            ui.palette_area = Some(egui::Rect::from_min_max(
                egui::pos2(left, 52.),
                egui::pos2(width, 900.),
            ));
            let row = egui::Rect::from_min_max(
                egui::pos2(left + 8., 300.),
                egui::pos2(width - 16., 352.),
            );
            ui.hits = vec![
                (
                    PanelButton::Category(None),
                    egui::Rect::from_min_size(egui::pos2(left + 8., 100.), egui::vec2(60., 20.)),
                ),
                (
                    PanelButton::Part(0),
                    egui::Rect::from_min_size(egui::pos2(100., 100.), egui::vec2(40., 40.)),
                ),
                (PanelButton::Part(1), row),
            ];
            assert_eq!(palette_scroll_point(&ui), Some(row.center()));
            assert!(ui.palette_area.unwrap().contains(row.center()));
            if width == 1920. {
                assert!(row.center().x > 1600.);
            }
        }
    }

    #[test]
    fn smoke_scroll_waits_for_positive_clipped_row_hits() {
        let mut ui = egui_panel::UiState::default();
        assert!(palette_scroll_point(&ui).is_none());
        ui.palette_area = Some(egui::Rect::from_min_size(
            egui::pos2(1600., 52.),
            egui::vec2(296., 900.),
        ));
        ui.hits.push((PanelButton::Part(0), egui::Rect::NOTHING));
        assert!(palette_scroll_point(&ui).is_none());
    }
}
