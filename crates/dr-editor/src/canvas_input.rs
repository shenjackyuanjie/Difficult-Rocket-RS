//! 画布鼠标/键盘入口；触摸桥接共用既有事务。
use crate::{
    CameraDrag, DragState, EditorCursor, EditorDocument, free_mode, mobile_input, panels,
    placement, selection, view,
};
use bevy::prelude::*;
use dr_core::{
    EditorCommand, SelectionTransform,
    geometry::{Vec2d, contains_point},
};

/// 取消未提交的画布交互；保留剪贴板、目录选择和预览变换设置。
/// 输入屏蔽、触摸导航及光标有效性由各入口按自身策略处理。
pub(crate) fn cancel_preview(
    drag: &mut DragState,
    cursor: &mut EditorCursor,
    camera_drag: &mut CameraDrag,
) {
    cursor.manual_connection = None;
    drag.cancel();
    cursor.cancel_placement();
    cursor.paste = None;
    camera_drag.0 = None;
}

/// 处理全局快捷键，并将所有会改变船体的操作记录到历史栈。
pub(crate) fn keyboard_commands(
    keys: Res<ButtonInput<KeyCode>>,
    mut document: ResMut<EditorDocument>,
    mut cursor: ResMut<EditorCursor>,
    drag: Res<DragState>,
    palette: Res<panels::Palette>,
    pointer: Res<panels::UiPointer>,
) {
    if drag.id.is_some()
        || drag.rectangle.is_some()
        || !keys.any_just_pressed([
            KeyCode::KeyA,
            KeyCode::KeyC,
            KeyCode::KeyV,
            KeyCode::KeyZ,
            KeyCode::KeyY,
            KeyCode::KeyX,
            KeyCode::KeyQ,
            KeyCode::KeyE,
            KeyCode::KeyR,
            KeyCode::KeyP,
            KeyCode::Tab,
            KeyCode::Delete,
        ])
    {
        // &mut ResMut 会标记文档变更；无快捷键边沿时保持渲染的增量路径。
        return;
    }
    let control = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    if control && keys.just_pressed(KeyCode::KeyZ) {
        if cursor.palette_drag {
            cursor.cancel_placement();
        }
        cursor.paste = None;
        document.undo();
        return;
    }
    if control && keys.just_pressed(KeyCode::KeyY) {
        if cursor.palette_drag {
            cursor.cancel_placement();
        }
        cursor.paste = None;
        document.redo();
        return;
    }
    if selection::keyboard(&mut document, &mut cursor, &keys, pointer.blocked) {
        return;
    }
    if control {
        return;
    }
    if keys.just_pressed(KeyCode::Tab) {
        let reverse = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
        panels::cycle_part(&document, &palette, &mut cursor, reverse);
        return;
    }
    if cursor.placing {
        if let Some(radians) = selection::rotation_input(&keys) {
            cursor.fine_rotation += radians;
        }
        if keys.just_pressed(KeyCode::KeyR) {
            cursor.rotation = (cursor.rotation + 1).rem_euclid(4);
        }
        if keys.just_pressed(KeyCode::KeyX) {
            cursor.flip_x = !cursor.flip_x;
        }
        if keys.just_pressed(KeyCode::KeyY) {
            cursor.flip_y = !cursor.flip_y;
        }
    }
    if keys.just_pressed(KeyCode::KeyP) && cursor.valid && !pointer.blocked {
        if cursor.palette_drag {
            placement::finish_palette_drag(&mut document, &mut cursor, true);
        } else {
            placement::place(&mut document, &cursor);
        }
        return;
    }
    if cursor.placing {
        return;
    }
    if let Some(radians) = selection::rotation_input(&keys) {
        let selected = document.selected_keys();
        if !selected.is_empty() {
            let center = selection::center(&document, &selected);
            document.execute(EditorCommand::TransformSelection {
                parts: selected,
                transform: SelectionTransform::RotateBy { center, radians },
            });
        }
        return;
    }
    if keys.just_pressed(KeyCode::Delete)
        && let Some(id) = document.selected
    {
        let _ = document.execute(EditorCommand::Delete(id.id).at(id));
    }
    if let Some(id) = document.selected {
        if keys.just_pressed(KeyCode::KeyR) {
            let can_rotate = document
                .ship
                .part_at(id)
                .and_then(|part| document.catalog.get(&part.part_type))
                .map(|part_type| !part_type.disable_editor_rotation)
                .unwrap_or(false);
            if can_rotate {
                placement::transform(&mut document, id, EditorCommand::Rotate(id.id));
            }
        }
        if keys.just_pressed(KeyCode::KeyX) {
            placement::transform(&mut document, id, EditorCommand::FlipX(id.id));
        }
        if keys.just_pressed(KeyCode::KeyY) {
            placement::transform(&mut document, id, EditorCommand::FlipY(id.id));
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn mouse_editor(
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    cameras: Query<(&Camera, &Transform, &Projection), With<Camera2d>>,
    mut drag: ResMut<DragState>,
    mut document: ResMut<EditorDocument>,
    mut cursor: ResMut<EditorCursor>,
    options: Res<view::ViewOptions>,
    pointer: Res<panels::UiPointer>,
    mut camera_drag: ResMut<CameraDrag>,
    touch: Option<Res<mobile_input::GestureInput>>,
) {
    let touch = touch.as_deref().filter(|input| input.override_mouse);
    let mouse = touch.map_or(&*mouse, |input| &input.buttons);
    let position = touch.map_or_else(
        || windows.single().ok().and_then(Window::cursor_position),
        |input| input.position,
    );
    if keys.just_pressed(KeyCode::Escape) || mouse.just_pressed(MouseButton::Right) {
        drag.cancel();
        cursor.cancel_placement();
        cursor.paste = None;
        cursor.manual_connection = None;
        return;
    }
    let Ok(window) = windows.single() else {
        return;
    };
    // 失焦或窗口外释放必须取消预览，不能让部件继续粘在鼠标上。
    if !window.focused || position.is_none() {
        cursor.valid = false;
        cursor.manual_connection = None;
        if !window.focused || !mouse.pressed(MouseButton::Left) {
            drag.cancel();
            if cursor.palette_drag {
                cursor.cancel_placement();
            }
        }
        return;
    }
    let box_button = if touch.is_some() {
        MouseButton::Middle
    } else {
        options.box_select_button.select()
    };
    if pointer.palette_drop && drag.id.is_some() {
        cursor.valid = false;
        if !mouse.pressed(MouseButton::Left) {
            let roots: Vec<_> = drag.keys().into_iter().collect();
            match selection::connected_keys(&document.ship, &roots) {
                Ok(parts) => {
                    let count = parts.len();
                    if document.execute(EditorCommand::DeleteSelection(parts.into_iter().collect()))
                    {
                        document.status = format!("已删除 {count} 个相连部件 · Ctrl+Z 撤销");
                        document.clear_selection();
                    }
                }
                Err(message) => document.status = message,
            }
            drag.cancel();
        }
        return;
    }
    if pointer.blocked {
        cursor.valid = false;
        if (drag.id.is_some() && !mouse.pressed(MouseButton::Left))
            || (drag.rectangle.is_some() && !mouse.pressed(box_button))
        {
            drag.cancel();
        }
        if cursor.palette_drag && !mouse.pressed(MouseButton::Left) {
            cursor.cancel_placement();
        }
        return;
    }
    let Ok((camera, transform, projection)) = cameras.single() else {
        return;
    };
    let Some(world) = view::point_world(window, camera, transform, projection, position.unwrap())
    else {
        return;
    };
    let logical = (world.x as f64 / 60.0, world.y as f64 / 60.0);
    cursor.world = logical;
    cursor.valid = true;
    free_mode::discard_stale(&document, &mut cursor);
    let commit = if touch.is_some() {
        mouse.just_released(MouseButton::Left)
    } else {
        mouse.just_pressed(MouseButton::Left)
    };
    if cursor.paste.is_some() {
        if commit {
            selection::commit_paste(&mut document, &mut cursor);
        }
        return;
    }
    if cursor.placing {
        if cursor.palette_drag {
            // egui 的拖动消息在下一帧消费，松手可能已不在 just_released 帧。
            if !mouse.pressed(MouseButton::Left) {
                placement::finish_palette_drag(&mut document, &mut cursor, true);
            }
        } else if commit {
            placement::place(&mut document, &cursor);
        }
        return;
    }
    if document.free_mode && mouse.just_pressed(MouseButton::Left) && drag.id.is_none() {
        let scale = match projection {
            Projection::Orthographic(p) => p.scale,
            _ => 1.0,
        };
        if free_mode::click(
            &mut document,
            &mut cursor,
            logical,
            if touch.is_some() { 22.0 } else { 8.0 } * scale as f64 / 60.0,
        ) {
            camera_drag.0 = None;
            return;
        }
    }
    let left_pressed = mouse.just_pressed(MouseButton::Left);
    let box_pressed = mouse.just_pressed(box_button);
    if left_pressed || box_pressed {
        let start = touch
            .and_then(|input| input.press_position)
            .and_then(|point| view::point_world(window, camera, transform, projection, point))
            .map_or(logical, |point| {
                (point.x as f64 / 60.0, point.y as f64 / 60.0)
            });
        let selected = document
            .ship
            .keyed_parts()
            .filter(|(_, part)| {
                document.catalog.get(&part.part_type).is_some_and(|ty| {
                    contains_point(
                        part,
                        ty,
                        Vec2d {
                            x: start.0,
                            y: start.1,
                        },
                    )
                })
            })
            .map(|(key, _)| key)
            .last();
        let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
        if left_pressed && selected.is_some() {
            drag.cancel();
            selection::click(&mut document, selected, shift);
            if !shift && let Some(part) = selected.and_then(|key| document.ship.part_at(key)) {
                drag.id = selected;
                drag.members = selection::drag_keys(
                    &document.ship,
                    &document.selected_keys(),
                    options.follow_children,
                );
                drag.origin = (part.x, part.y);
                drag.preview = drag.origin;
                drag.offset = (part.x - start.0, part.y - start.1);
            }
        } else if left_pressed && selected.is_none() {
            drag.cancel();
            selection::click(&mut document, None, shift);
            if box_pressed {
                drag.rectangle = Some(logical);
                drag.rect_end = logical;
                drag.additive = shift;
            }
        } else if box_pressed && selected.is_none() {
            drag.cancel();
            drag.rectangle = Some(logical);
            drag.rect_end = logical;
            drag.additive = shift;
            if !shift {
                document.clear_selection();
            }
        }
    }
    if let Some(start) = drag.rectangle {
        drag.rect_end = logical;
        if mouse.just_released(box_button) {
            selection::rectangle(&mut document, start, logical, drag.additive);
            drag.cancel();
        }
        return;
    }
    if drag.id.is_some()
        && (mouse.pressed(MouseButton::Left) || mouse.just_released(MouseButton::Left))
    {
        // 保留抓取偏移和非网格坐标，小幅移动也要立即响应。
        drag.follow(logical);
    }
    if drag.id.is_some() {
        let selected: Vec<_> = drag.keys().into_iter().collect();
        if keys.just_pressed(KeyCode::KeyR) {
            if selected.iter().any(|key| {
                document
                    .ship
                    .part_at(*key)
                    .and_then(|part| document.catalog.get(&part.part_type))
                    .is_some_and(|kind| kind.disable_editor_rotation)
            }) {
                document.status = "所拖部件中有禁止旋转的部件".into();
            } else {
                drag.turns = (drag.turns + 1) % 4;
            }
        }
        if let Some(radians) = selection::rotation_input(&keys) {
            if selected.iter().any(|key| {
                document
                    .ship
                    .part_at(*key)
                    .and_then(|part| document.catalog.get(&part.part_type))
                    .is_some_and(|kind| kind.disable_editor_rotation)
            }) {
                document.status = "所拖部件中有禁止旋转的部件".into();
            } else {
                drag.fine_rotation += radians;
            }
        }
        let control = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
        if !control && keys.just_pressed(KeyCode::KeyX) {
            drag.mirror(true);
        }
        if !control && keys.just_pressed(KeyCode::KeyY) {
            drag.mirror(false);
        }
        let (delta, command, clear) = selection::movement_transformed(
            &document,
            &selected,
            drag.selection_pose(),
            drag.delta(),
        );
        drag.preview = (drag.origin.0 + delta.0, drag.origin.1 + delta.1);
        drag.command = Some(command);
        drag.blocked = !clear;
    }
    if drag.id.is_some() && !mouse.pressed(MouseButton::Left) {
        drag.finish(&mut document);
    }
}

#[cfg(test)]
mod input_latency_tests;
#[cfg(test)]
mod interaction_tests;
#[cfg(test)]
mod tests;
