use super::*;
use std::collections::HashMap;
use std::time::Instant;

#[derive(Default)]
pub(crate) struct State {
    phase: usize,
    original: Option<Ship>,
    entities: HashMap<render::VisualKey, Entity>,
    moving: render::VisualKey,
    origin: (f64, f64),
    last_frame: Option<Instant>,
    frame_ms: Vec<f64>,
    moved_frames: usize,
}

/// 真实渲染窗口中持续拖动 90 帧；验证实体复用并记录帧耗时，不保存源文档。
#[allow(clippy::too_many_arguments)]
pub(crate) fn run(
    mode: Res<SmokeTest>,
    mut state: Local<State>,
    mut document: ResMut<EditorDocument>,
    mut windows: Query<&mut Window, With<bevy::window::PrimaryWindow>>,
    mut cameras: Query<(&mut Transform, &mut Projection), With<Camera2d>>,
    visuals: Query<(Entity, &render::PartVisual, &Transform), Without<Camera2d>>,
    mut drag: ResMut<DragState>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut commands: Commands,
) {
    if !mode.performance || mode.started.elapsed().as_secs() < 5 {
        return;
    }
    assert!(mode.started.elapsed().as_secs() < 180, "大船体性能自测超时");
    let Ok(mut window) = windows.single_mut() else {
        return;
    };
    let Ok((mut camera, mut projection)) = cameras.single_mut() else {
        return;
    };
    let Projection::Orthographic(projection) = &mut *projection else {
        return;
    };
    match state.phase {
        0 => {
            assert!(
                !document.ship.parts.is_empty(),
                "性能自测需要包含部件的船体"
            );
            if visuals.iter().len() != document.ship.all_parts().count() {
                return;
            }
            let mut low = Vec2::splat(f32::INFINITY);
            let mut high = Vec2::splat(f32::NEG_INFINITY);
            for part in document.ship.all_parts() {
                let radius = document
                    .catalog
                    .get(&part.part_type)
                    .map(|kind| {
                        let (w, h) = kind.half_extents();
                        w.hypot(h) as f32 * 60.0
                    })
                    .unwrap_or(30.0);
                let position = Vec2::new(part.x as f32 * 60.0, part.y as f32 * 60.0);
                low = low.min(position - radius);
                high = high.max(position + radius);
            }
            let middle = (low + high) / 2.0;
            camera.translation.x = middle.x;
            camera.translation.y = middle.y;
            projection.scale = ((high.x - low.x) / (window.width() - 640.0).max(100.0))
                .max((high.y - low.y) / (window.height() - 300.0).max(100.0))
                .max(0.1);
            let (key, part) = render::parts(&document.ship)
                .min_by(|(_, a), (_, b)| {
                    (a.x * 60.0 - middle.x as f64)
                        .hypot(a.y * 60.0 - middle.y as f64)
                        .total_cmp(
                            &(b.x * 60.0 - middle.x as f64).hypot(b.y * 60.0 - middle.y as f64),
                        )
                })
                .unwrap();
            state.moving = key;
            state.origin = (part.x, part.y);
            state.original = Some(document.ship.clone());
            state.entities = visuals
                .iter()
                .map(|(entity, visual, _)| ((visual.group, visual.id, visual.occurrence), entity))
                .collect();
        }
        1..=2 => {}
        3..=92 => {
            if let Some(last) = state.last_frame {
                state.frame_ms.push(last.elapsed().as_secs_f64() * 1000.0);
            }
            state.last_frame = Some(Instant::now());
            if state.phase == 3 {
                document.selected =
                    Some(PartKey::new(state.moving.0, state.moving.1, state.moving.2));
                drag.id = Some(PartKey::new(state.moving.0, state.moving.1, state.moving.2));
                if mode.performance_selection_count > 1 {
                    let anchor = document.selected.unwrap();
                    let mut nearest: Vec<_> = document.ship.keyed_parts().collect();
                    nearest.sort_by(|(_, a), (_, b)| {
                        (a.x - state.origin.0)
                            .hypot(a.y - state.origin.1)
                            .total_cmp(&(b.x - state.origin.0).hypot(b.y - state.origin.1))
                    });
                    let mut selected: BTreeSet<_> = nearest
                        .into_iter()
                        .map(|(key, _)| key)
                        .filter(|key| *key != anchor)
                        .take(mode.performance_selection_count - 1)
                        .collect();
                    selected.insert(anchor);
                    document.selection = selected.clone();
                    drag.members = selected;
                }
                drag.origin = state.origin;
                drag.preview = state.origin;
                drag.offset = (0.0, 0.0);
            } else {
                let (_, _, transform) = visuals
                    .get(state.entities[&state.moving])
                    .expect("拖动时重建了部件实体");
                if transform.translation.truncate().distance(Vec2::new(
                    state.origin.0 as f32 * 60.0,
                    state.origin.1 as f32 * 60.0,
                )) > 1.0
                {
                    state.moved_frames += 1;
                }
            }
            let x = state.origin.0 + ((state.phase - 3) % 20) as f64 * 0.1;
            let y = state.origin.1 + 2.0;
            window.focused = true;
            let position = Vec2::new(
                window.width() / 2.0 + (x as f32 * 60.0 - camera.translation.x) / projection.scale,
                window.height() / 2.0 - (y as f32 * 60.0 - camera.translation.y) / projection.scale,
            );
            window.set_cursor_position(Some(position));
            mouse.press(MouseButton::Left);
            mouse.clear(); // 已处于拖动状态，不再触发一次新的选择。
        }
        93 => {
            assert!(state.moved_frames > 0, "采样期间没有更新实际拖动预览");
            keys.press(KeyCode::Escape);
            mouse.reset_all();
        }
        94 => {
            assert_eq!(
                state.original.as_ref(),
                Some(&document.ship),
                "性能自测修改了船体"
            );
            let now: HashMap<_, _> = visuals
                .iter()
                .map(|(entity, visual, _)| ((visual.group, visual.id, visual.occurrence), entity))
                .collect();
            assert_eq!(state.entities, now, "选择、拖动或取消导致部件实体重建");
            assert!(drag.id.is_none());
            keys.reset_all();
            state.frame_ms.sort_by(f64::total_cmp);
            let count = state.frame_ms.len();
            let average = state.frame_ms.iter().sum::<f64>() / count as f64;
            let median = state.frame_ms[count / 2];
            let p95 = state.frame_ms[(count * 95 / 100).min(count - 1)];
            let max = state.frame_ms[count - 1];
            let report = format!(
                "{{\"parts\":{},\"selected\":{},\"frames\":{count},\"average_ms\":{average:.3},\"median_ms\":{median:.3},\"p95_ms\":{p95:.3},\"max_ms\":{max:.3},\"recreated_entities\":0}}\n",
                now.len(),
                document.selected_keys().len()
            );
            std::fs::write("target/editor-performance.json", &report).unwrap();
            info!("大船体性能自测通过：{}", report.trim());
            use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk};
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk("target/editor-performance.png"))
                .observe(
                    |_: On<ScreenshotCaptured>, mut exit: MessageWriter<AppExit>| {
                        exit.write(AppExit::Success);
                    },
                );
        }
        _ => return,
    }
    state.phase += 1;
}
