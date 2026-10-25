//! 跨模块共享的内存夹具；仅在 cfg(test) 编译。

use crate::document::EditorDocument;

use crate::canvas_input::mouse_editor;
use crate::panels;
use crate::placement::EditorCursor;
use crate::selection;
use crate::selection::DragState;
use crate::view;
use crate::view::CameraDrag;
use bevy::prelude::*;
use dr_core::EditorCommand;
use dr_core::EditorHistory;
use dr_core::PartCatalog;
use dr_core::PartKey;
use dr_core::SelectionPose;
use dr_core::Ship;

pub fn document() -> EditorDocument {
    let ship =
        dr_core::ship_from_xml(r#"<Ship><Parts><Part id="1" partType="pod"/></Parts></Ship>"#)
            .unwrap();
    EditorDocument {
        free_mode: false,
        snap_enabled: true,
        revision: 0,
        saved_ship: ship.clone(),
        ship,
        catalog: PartCatalog::default(),
        selected: None,
        selection: default(),
        dirty: false,
        history: EditorHistory::default(),
        status: String::new(),
        sfx: Vec::new(),
    }
}

/// 使用实际 mouse_editor 系统，测试相机固定为 1 世界像素/逻辑像素。
pub fn mouse_app() -> App {
    use bevy::camera::{ComputedCameraValues, RenderTargetInfo};
    let mut document = document();
    document.catalog = crate::test_support::catalog();
    let mut app = App::new();
    app.insert_resource(document)
        .init_resource::<DragState>()
        .init_resource::<CameraDrag>()
        .init_resource::<EditorCursor>()
        .init_resource::<view::ViewOptions>()
        .init_resource::<panels::UiPointer>()
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<ButtonInput<MouseButton>>()
        .add_systems(Update, mouse_editor);
    app.world_mut().spawn((
        Window {
            resolution: bevy::window::WindowResolution::new(1440, 900),
            focused: true,
            ..default()
        },
        bevy::window::PrimaryWindow,
    ));
    app.world_mut().spawn((
        Camera {
            computed: ComputedCameraValues {
                clip_from_view: Mat4::from_scale(Vec3::new(1.0 / 720.0, 1.0 / 450.0, 1.0)),
                target_info: Some(RenderTargetInfo {
                    physical_size: UVec2::new(1440, 900),
                    scale_factor: 1.0,
                }),
                ..default()
            },
            ..default()
        },
        Camera2d,
        Transform::IDENTITY,
        Projection::Orthographic(OrthographicProjection::default_2d()),
        GlobalTransform::IDENTITY,
    ));
    app
}

/// 屏幕像素位移经网格量化后的世界像素位移；y 轴由屏幕向下翻到世界向上。
pub fn grid_pixels(delta: Vec2) -> Vec2 {
    let (x, y) =
        dr_core::geometry::quantize_point((f64::from(delta.x) / 60.0, f64::from(-delta.y) / 60.0));
    Vec2::new((x * 60.0) as f32, (y * 60.0) as f32)
}

/// 将预览落点、断开旧连接和新吸附作为一个可撤销操作。
pub fn move_with_snap(
    ship: &Ship,
    catalog: &PartCatalog,
    id: PartKey,
    position: (f64, f64),
) -> Option<EditorCommand> {
    let source = ship.part_at(id)?;
    let origin = (source.x, source.y);
    let delta = (position.0 - origin.0, position.1 - origin.1);
    Some(
        selection::movement_transformed_for(
            ship,
            catalog,
            &[id],
            SelectionPose {
                pivot: origin,
                ..default()
            },
            delta,
        )
        .1,
    )
}

pub fn movement(
    document: &EditorDocument,
    keys: &[PartKey],
    delta: (f64, f64),
) -> ((f64, f64), EditorCommand, bool) {
    movement_pose(document, keys, (0.0, 0.0), 0, delta)
}

pub fn movement_pose(
    document: &EditorDocument,
    keys: &[PartKey],
    pivot: (f64, f64),
    turns: u8,
    delta: (f64, f64),
) -> ((f64, f64), EditorCommand, bool) {
    selection::movement_transformed(
        document,
        keys,
        SelectionPose {
            pivot,
            radians: (turns % 4) as f64 * std::f64::consts::FRAC_PI_2,
            ..default()
        },
        delta,
    )
}

pub fn catalog() -> PartCatalog {
    dr_core::catalog_from_xml(r#"<PartTypes>
        <PartType id="pod" name="驾驶舱" type="pod" width="2" height="2"><AttachPoints><AttachPoint x="-1"/><AttachPoint x="1"/></AttachPoints></PartType>
        <PartType id="hidden" name="隐藏" hidden="true"/>
        <PartType id="engine" name="发动机" type="engine" width="2" height="2" category="推进" maxOccurrences="1"><Engine power="1"/></PartType>
        <PartType id="tank" name="燃料箱" type="tank" width="2" height="2" category="推进" disableEditorRotation="true"><Tank fuel="100"/></PartType>
    </PartTypes>"#).unwrap()
}
