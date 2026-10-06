use super::*;
use dr_core::geometry::{WorldShape, world_shapes};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BoxSelectButton {
    Left,
    Middle,
}

impl BoxSelectButton {
    pub fn select(self) -> MouseButton {
        match self {
            Self::Left => MouseButton::Left,
            Self::Middle => MouseButton::Middle,
        }
    }

    pub fn pan(self) -> MouseButton {
        match self {
            Self::Left => MouseButton::Middle,
            Self::Middle => MouseButton::Left,
        }
    }
}

#[derive(Resource)]
pub(crate) struct ViewOptions {
    pub debug: bool,
    pub ship_visible: bool,
    pub box_select_button: BoxSelectButton,
}

impl Default for ViewOptions {
    fn default() -> Self {
        Self {
            debug: false,
            ship_visible: true,
            box_select_button: BoxSelectButton::Middle,
        }
    }
}

/// 输入在 Update 中执行，相机的 GlobalTransform/ComputedCameraValues 要到
/// PostUpdate 才刷新。使用本帧的根相机 Transform、Projection 和窗口尺寸，
/// 避免平移/缩放/适配后仍读取上一帧视角；不改动渲染使用的相机缓存。
pub(crate) fn cursor_world(
    window: &Window,
    camera: &Camera,
    transform: &Transform,
    projection: &Projection,
) -> Option<Vec2> {
    use bevy::camera::RenderTargetInfo;
    let position = window.cursor_position()?;
    let mut camera = camera.clone();
    camera.computed.target_info = Some(RenderTargetInfo {
        physical_size: window.physical_size(),
        scale_factor: window.scale_factor(),
    });
    let size = camera.logical_viewport_size()?;
    if size.min_element() <= 0.0 {
        return None;
    }
    let mut projection = projection.clone();
    projection.update(size.x, size.y);
    camera.computed.clip_from_view = projection.get_clip_from_view();
    camera
        .viewport_to_world_2d(&GlobalTransform::from(*transform), position)
        .ok()
}

/// 左右目录和顶部操作提示之外的画布，预留 16 像素边距。
pub(crate) fn canvas(size: Vec2) -> Rect {
    Rect::from_corners(
        Vec2::new(264.0, 220.0),
        Vec2::new((size.x - 312.0).max(265.0), (size.y - 16.0).max(221.0)),
    )
}

pub(crate) fn fitted(
    document: &EditorDocument,
    size: Vec2,
    selected: bool,
    mut texture_size: impl FnMut(&str) -> Option<Vec2>,
) -> Option<(Vec2, f32)> {
    let mut low = Vec2::splat(f32::INFINITY);
    let mut high = Vec2::splat(f32::NEG_INFINITY);
    for (key, part) in document.ship.keyed_parts() {
        if selected && !document.is_selected(key) {
            continue;
        }
        let kind = document.catalog.get(&part.part_type);
        let image_size = kind
            .and_then(|kind| texture_size(&kind.sprite))
            .unwrap_or_else(|| render::fallback_size(kind));
        for corner in render::image_corners(part, image_size) {
            low = low.min(corner);
            high = high.max(corner);
        }
        if let Some(kind) = kind {
            // 贴图和实体 Shape 都必须进入视野。
            for shape in world_shapes(part, kind) {
                match shape {
                    WorldShape::Polygon(vertices) => {
                        for point in vertices {
                            let p = Vec2::new(point.x as f32, point.y as f32) * 60.0;
                            low = low.min(p);
                            high = high.max(p);
                        }
                    }
                    WorldShape::Circle(center, r) => {
                        let p = Vec2::new(center.x as f32, center.y as f32) * 60.0;
                        low = low.min(p - Vec2::splat(r as f32 * 60.0));
                        high = high.max(p + Vec2::splat(r as f32 * 60.0));
                    }
                }
            }
        }
    }
    if !low.is_finite() || !high.is_finite() {
        return None;
    }
    let area = canvas(size);
    let available = (area.size() - Vec2::splat(32.0)).max(Vec2::splat(1.0));
    let extent = high - low;
    let scale = (extent.x / available.x)
        .max(extent.y / available.y)
        .max(0.1);
    let center = (low + high) * 0.5;
    let screen_offset = area.center() - size * 0.5;
    Some((
        center - Vec2::new(screen_offset.x, -screen_offset.y) * scale,
        scale,
    ))
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn controls(
    keys: Res<ButtonInput<KeyCode>>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    document: Res<EditorDocument>,
    drag: Res<DragState>,
    assets: Res<AssetServer>,
    images: Res<Assets<Image>>,
    mut cameras: Query<(&mut Transform, &mut Projection), With<Camera2d>>,
    mut options: ResMut<ViewOptions>,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    if !window.focused || keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight)
    {
        return;
    }
    if keys.just_pressed(KeyCode::F3) {
        options.debug = !options.debug;
    }
    if keys.just_pressed(KeyCode::F4) {
        options.ship_visible = !options.ship_visible;
    }
    if !keys.just_pressed(KeyCode::KeyF) || drag.id.is_some() || drag.rectangle.is_some() {
        return;
    }
    let selected = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let Some((center, scale)) = fitted(
        &document,
        Vec2::new(window.width(), window.height()),
        selected,
        |texture| render::image_size(texture, &assets, &images),
    ) else {
        return;
    };
    let Ok((mut transform, mut projection)) = cameras.single_mut() else {
        return;
    };
    if let Projection::Orthographic(projection) = &mut *projection {
        transform.translation.x = center.x;
        transform.translation.y = center.y;
        projection.scale = scale;
    }
}

#[derive(Component)]
pub(crate) struct DebugLabel;

#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_debug(
    mut commands: Commands,
    options: Res<ViewOptions>,
    document: Res<EditorDocument>,
    cursor: Res<EditorCursor>,
    drag: Res<DragState>,
    camera: Query<(&Transform, &Projection), With<Camera2d>>,
    mut visuals: Query<&mut Visibility, With<render::PartVisual>>,
    mut labels: Query<(Entity, &mut Text), With<DebugLabel>>,
    assets: Res<AssetServer>,
    mut gizmos: Gizmos,
) {
    for mut visibility in &mut visuals {
        let value = if options.ship_visible {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *visibility != value {
            *visibility = value;
        }
    }
    if !options.debug {
        for (entity, _) in &mut labels {
            commands.entity(entity).despawn();
        }
        return;
    }
    let Ok((camera, Projection::Orthographic(projection))) = camera.single() else {
        return;
    };
    let mouse = if cursor.valid {
        format!("鼠标 ({:.2}, {:.2})", cursor.world.0, cursor.world.1)
    } else {
        "鼠标在画布外".into()
    };
    let text = format!(
        "{mouse}\n视图 ({:.2}, {:.2}) · 缩放 {:.3}",
        camera.translation.x / 60.0,
        camera.translation.y / 60.0,
        projection.scale.recip()
    );
    if let Ok((_, mut label)) = labels.single_mut() {
        if **label != text {
            **label = text;
        }
    } else {
        commands.spawn((
            DebugLabel,
            Text::new(text),
            TextFont {
                font: bevy::text::FontSource::Handle(
                    assets.load(
                        "fonts/HarmonyOS_Sans/HarmonyOS_Sans_SC/HarmonyOS_Sans_SC_Regular.ttf",
                    ),
                ),
                font_size: FontSize::Px(14.0),
                ..default()
            },
            TextColor(Color::srgb(0.4, 0.9, 1.0)),
            Node {
                position_type: PositionType::Absolute,
                left: px(264),
                bottom: px(16),
                ..default()
            },
            BackgroundColor(Color::srgba(0.03, 0.04, 0.06, 0.9)),
        ));
    }
    let color = Color::srgb(0.3, 0.85, 1.0);
    let axis = 15.0 * projection.scale;
    gizmos.line_2d(Vec2::new(-axis, 0.0), Vec2::new(axis, 0.0), color);
    gizmos.line_2d(Vec2::new(0.0, -axis), Vec2::new(0.0, axis), color);
    gizmos.line_2d(Vec2::ZERO, camera.translation.truncate(), color);
    for (key, part) in document
        .ship
        .keyed_parts()
        .filter(|(key, _)| document.is_selected(*key))
    {
        let Some(kind) = document.catalog.get(&part.part_type) else {
            continue;
        };
        let mut part = part.clone();
        if drag.contains(key) {
            let delta = drag.delta();
            part.x += delta.0;
            part.y += delta.1;
        }
        let point = |p: Vec2d| Vec2::new(p.x as f32 * 60.0, p.y as f32 * 60.0);
        for shape in world_shapes(&part, kind) {
            match shape {
                WorldShape::Polygon(vertices) => {
                    for (a, b) in vertices.iter().zip(vertices.iter().cycle().skip(1)) {
                        gizmos.line_2d(point(*a), point(*b), color);
                    }
                }
                WorldShape::Circle(center, radius) => {
                    gizmos.circle_2d(point(center), radius as f32 * 60.0, color);
                }
            }
        }
        for attach in &kind.attach_points {
            let (a, b) = dr_core::connections::segment(&part, kind, attach);
            gizmos.line_2d(point(a), point(b), Color::srgb(1.0, 0.4, 0.6));
            gizmos.circle_2d(
                point(dr_core::part_world_attach(&part, attach)),
                2.0 * projection.scale,
                Color::srgb(1.0, 0.4, 0.6),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_includes_real_texture_outside_shape_for_rotation_and_mirrors() {
        let mut document = crate::tests::document();
        document.catalog = panels::tests::catalog();
        document.ship.parts[0].angle = 0.37;
        document.ship.parts[0].flip_x = true;
        document.ship.parts[0].flip_y = true;
        let size = Vec2::new(960.0, 640.0);
        let texture = Vec2::new(84.0, 207.0);
        let (_, physical_scale) = fitted(&document, size, false, |_| None).unwrap();
        let (center, scale) = fitted(&document, size, false, |_| Some(texture)).unwrap();
        assert!(scale > physical_scale);
        for corner in render::image_corners(&document.ship.parts[0], texture) {
            let offset = (corner - center) / scale;
            assert!(canvas(size).contains(size * 0.5 + Vec2::new(offset.x, -offset.y)));
        }
    }

    #[test]
    fn middle_box_selection_is_default_and_leaves_left_drag_for_panning() {
        let options = ViewOptions::default();
        assert_eq!(options.box_select_button, BoxSelectButton::Middle);
        assert_eq!(options.box_select_button.select(), MouseButton::Middle);
        assert_eq!(options.box_select_button.pan(), MouseButton::Left);
        assert_eq!(BoxSelectButton::Left.select(), MouseButton::Left);
        assert_eq!(BoxSelectButton::Left.pan(), MouseButton::Middle);
    }

    #[test]
    fn fit_places_rotated_ship_inside_canvas_and_selection_excludes_other_parts() {
        let mut document = crate::tests::document();
        document.catalog = panels::tests::catalog();
        let kind = document.catalog.get("pod").unwrap().clone();
        document.ship.parts = vec![
            kind.instantiate(1, (-10.0, -5.0)),
            kind.instantiate(2, (120.0, 50.0)),
        ];
        document.ship.parts[0].angle = 0.71;
        document.select_only(Some(PartKey::new(0, 1, 0)));
        for size in [Vec2::new(960.0, 640.0), Vec2::new(1440.0, 900.0)] {
            let (center, scale) = fitted(&document, size, false, |_| None).unwrap();
            for part in &document.ship.parts {
                for shape in world_shapes(part, &kind) {
                    if let WorldShape::Polygon(vertices) = shape {
                        for p in vertices {
                            let offset =
                                (Vec2::new(p.x as f32, p.y as f32) * 60.0 - center) / scale;
                            let screen = size * 0.5 + Vec2::new(offset.x, -offset.y);
                            assert!(canvas(size).contains(screen));
                        }
                    }
                }
            }
            assert!(fitted(&document, size, true, |_| None).unwrap().1 < scale);
        }
        document.ship.parts.clear();
        assert!(fitted(&document, Vec2::new(960.0, 640.0), false, |_| None).is_none());
    }
}
