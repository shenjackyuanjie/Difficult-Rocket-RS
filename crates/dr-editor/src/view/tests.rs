use crate::panels;
use crate::render;
use crate::view::*;
use dr_core::PartKey;

#[test]
fn fallback_canvas_has_no_legacy_sidebar_or_top_text_reservation() {
    for size in [
        Vec2::new(320.0, 640.0),
        Vec2::new(640.0, 360.0),
        Vec2::new(1440.0, 900.0),
    ] {
        let area = canvas(size);
        assert_eq!(area.min, Vec2::splat(16.0));
        assert_eq!(area.max.x, size.x - 16.0);
        assert_eq!(area.max.y, size.y - panels::TOOLBAR_HEIGHT - 16.0);
    }
}

#[test]
fn fit_includes_real_texture_outside_shape_for_rotation_and_mirrors() {
    let mut document = crate::test_support::document();
    document.catalog = crate::test_support::catalog();
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
    let mut document = crate::test_support::document();
    document.catalog = crate::test_support::catalog();
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
                        let offset = (Vec2::new(p.x as f32, p.y as f32) * 60.0 - center) / scale;
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

#[test]
fn fitting_uses_the_actual_remaining_canvas_on_portrait_and_landscape_screens() {
    let mut document = crate::test_support::document();
    document.catalog = crate::test_support::catalog();
    document.ship.parts[0].angle = 0.71;
    let texture = Vec2::new(84.0, 207.0);
    for size in [
        Vec2::new(320.0, 640.0),
        Vec2::new(390.0, 844.0),
        Vec2::new(640.0, 360.0),
        Vec2::new(1440.0, 900.0),
    ] {
        assert!(Rect::from_corners(Vec2::ZERO, size).contains(canvas(size).min));
        assert!(Rect::from_corners(Vec2::ZERO, size).contains(canvas(size).max));
        // 打开单侧目录后，真实画布偏置不能继续按固定桌面边栏估计。
        let area = Rect::from_corners(
            Vec2::new(size.x * 0.7, size.y * 0.4),
            size - Vec2::splat(8.0),
        );
        let (center, scale) = fitted_in(&document, size, area, false, |_| Some(texture)).unwrap();
        assert!(scale.is_finite() && center.is_finite());
        for corner in render::image_corners(&document.ship.parts[0], texture) {
            let offset = (corner - center) / scale;
            let screen = size * 0.5 + Vec2::new(offset.x, -offset.y);
            assert!(area.contains(screen), "{size:?} {area:?}: {screen:?}");
        }
    }
}
