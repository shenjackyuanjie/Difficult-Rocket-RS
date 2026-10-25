//! 拖动时显示连接面与可提交的邻近连接候选，不改变文档或历史。

use crate::document::EditorDocument;
use crate::free_mode;
use crate::panels;
use crate::placement;
use crate::placement::EditorCursor;
use crate::selection::DragState;
use crate::view;
use bevy::prelude::*;
use dr_core::EditorCommand;
use dr_core::LinkKind;
use dr_core::Part;
use dr_core::PartKey;
use dr_core::geometry::Vec2d;

const SOURCE_COLOR: Color = Color::srgb(0.55, 0.51, 0.66);
const CANDIDATE_COLOR: Color = Color::srgb(0.67, 0.56, 0.88);
const SNAPPED_COLOR: Color = Color::srgb(0.85, 0.74, 1.0);

#[derive(Debug)]
pub struct Hint {
    pub source: (Vec2d, Vec2d),
    pub target: (Vec2d, Vec2d),
    pub contact: (Vec2d, Vec2d),
    pub snapped: bool,
}

pub fn nearby(document: &EditorDocument, source: &Part, key: Option<PartKey>) -> Vec<Hint> {
    if !document.snap_enabled {
        return vec![];
    }
    let Some(st) = document.catalog.get(&source.part_type) else {
        return vec![];
    };
    if key.is_none()
        && st
            .max_occurrences
            .is_some_and(|limit| document.ship.count_type(&st.id) >= limit as usize)
    {
        return vec![];
    }
    let mut hints = vec![];
    for (target_key, target) in document.ship.keyed_parts() {
        if Some(target_key) == key {
            continue;
        }
        let Some(tt) = document.catalog.get(&target.part_type) else {
            continue;
        };
        // 提前显示附近的有效候选，真正吸附仍使用原来的 0.35 阈值。
        for candidate in dr_core::connections::candidates(source, st, target, tt, 0.75) {
            let mut proposed = source.clone();
            proposed.x = candidate.position.x;
            proposed.y = candidate.position.y;
            if !placement::candidate_allowed(
                &document.ship,
                &document.catalog,
                &proposed,
                key,
                target,
                target_key,
                &candidate,
                document.free_mode,
            ) {
                continue;
            }
            let a = dr_core::connections::segment(
                source,
                st,
                &st.attach_points[candidate.source_index],
            );
            let b = dr_core::connections::segment(
                target,
                tt,
                &tt.attach_points[candidate.target_index],
            );
            hints.push(Hint {
                source: a,
                target: b,
                contact: dr_core::connections::closest_points(a, b),
                snapped: candidate.distance < 1e-6,
            });
            // 候选按距离稳定排序，每对部件只展示最近的一对合法连接点。
            break;
        }
    }
    hints
}

fn point(p: Vec2d) -> Vec2 {
    Vec2::new(p.x as f32, p.y as f32) * 60.0
}

// 置于部件和放置虚影（z <= 5）上方，避免连接点被贴图遮住。
fn line(gizmos: &mut Gizmos, a: Vec2d, b: Vec2d, color: Color) {
    gizmos.line(point(a).extend(8.0), point(b).extend(8.0), color);
}

fn circle(gizmos: &mut Gizmos, p: Vec2d, radius: f32, color: Color) {
    gizmos.circle(
        Isometry3d::from_translation(point(p).extend(8.0)),
        radius,
        color,
    );
}

fn draw_hint(gizmos: &mut Gizmos, hint: &Hint, radius: f32) {
    let color = if hint.snapped {
        SNAPPED_COLOR
    } else {
        CANDIDATE_COLOR
    };
    line(gizmos, hint.source.0, hint.source.1, color);
    line(gizmos, hint.target.0, hint.target.1, color);
    line(gizmos, hint.contact.0, hint.contact.1, color);
    circle(gizmos, hint.contact.0, radius, color);
    circle(gizmos, hint.contact.1, radius, color);
}

// 多选已由 selection::movement 原子校验，只展示其实际连接命令，
// 不把单个部件可以连接误当作整体也能连接。
fn group_hints(document: &EditorDocument, drag: &DragState, command: &EditorCommand) -> Vec<Hint> {
    if !document.snap_enabled {
        return vec![];
    }
    match command {
        EditorCommand::Batch(commands) => commands
            .iter()
            .flat_map(|command| group_hints(document, drag, command))
            .collect(),
        EditorCommand::ConnectParts {
            parent,
            child,
            kind,
        } => {
            let (Some(mut a), Some(mut b)) = (
                document.ship.part_at(*parent).cloned(),
                document.ship.part_at(*child).cloned(),
            ) else {
                return vec![];
            };
            a = drag.pose(*parent, &a);
            b = drag.pose(*child, &b);
            let (Some(st), Some(tt)) = (
                document.catalog.get(&b.part_type),
                document.catalog.get(&a.part_type),
            ) else {
                return vec![];
            };
            // 对接也要按真实连接面定位，不能把部件中心误画成吸附点。
            dr_core::connections::candidates(&b, st, &a, tt, 1e-6)
                .into_iter()
                .find(|candidate| match kind {
                    LinkKind::Normal {
                        parent_attach,
                        child_attach,
                    } => {
                        !candidate.dock
                            && candidate.target_index as i32 + 1 == *parent_attach
                            && candidate.source_index as i32 + 1 == *child_attach
                    }
                    LinkKind::Dock { .. } => candidate.dock,
                })
                .map(|candidate| {
                    let source = dr_core::connections::segment(
                        &b,
                        st,
                        &st.attach_points[candidate.source_index],
                    );
                    let target = dr_core::connections::segment(
                        &a,
                        tt,
                        &tt.attach_points[candidate.target_index],
                    );
                    vec![Hint {
                        source,
                        target,
                        contact: dr_core::connections::closest_points(source, target),
                        snapped: true,
                    }]
                })
                .unwrap_or_default()
        }
        _ => vec![],
    }
}

pub fn draw(
    mut gizmos: Gizmos,
    document: Res<EditorDocument>,
    drag: Res<DragState>,
    cursor: Res<EditorCursor>,
    pointer: Res<panels::UiPointer>,
    options: Res<view::ViewOptions>,
    cameras: Query<&Projection, With<Camera2d>>,
) {
    if !options.ship_visible {
        return;
    }
    let radius = cameras
        .single()
        .ok()
        .and_then(|projection| match projection {
            Projection::Orthographic(projection) => Some(4.0 * projection.scale),
            _ => None,
        })
        .unwrap_or(4.0);
    if document.free_mode {
        free_mode::draw(
            &mut gizmos,
            &document,
            &drag,
            &cursor,
            radius,
            !pointer.blocked,
        );
    }
    if !document.snap_enabled || pointer.blocked || !cursor.valid {
        return;
    }
    let mut sources = vec![];
    if drag.id.is_some() {
        for key in drag.keys() {
            if let Some(part) = document.ship.part_at(key) {
                sources.push((Some(key), drag.pose(key, part)));
            }
        }
    } else if cursor.placing
        && let Some((part, _, _)) = placement::preview(&document, &cursor)
    {
        sources.push((None, part));
    }
    for (key, part) in sources {
        if !document.free_mode
            && let Some(kind) = document.catalog.get(&part.part_type)
        {
            for attach in &kind.attach_points {
                let (a, b) = dr_core::connections::segment(&part, kind, attach);
                let color = SOURCE_COLOR;
                line(&mut gizmos, a, b, color);
                circle(
                    &mut gizmos,
                    Vec2d {
                        x: (a.x + b.x) / 2.0,
                        y: (a.y + b.y) / 2.0,
                    },
                    radius,
                    color,
                );
            }
        }
        if drag.members.len() <= 1 {
            for hint in nearby(&document, &part, key) {
                draw_hint(&mut gizmos, &hint, radius);
            }
        }
    }
    if drag.members.len() > 1
        && !drag.blocked
        && let Some(command) = &drag.command
    {
        for hint in group_hints(&document, &drag, command) {
            draw_hint(&mut gizmos, &hint, radius);
        }
    }
}

#[cfg(test)]
mod tests;
