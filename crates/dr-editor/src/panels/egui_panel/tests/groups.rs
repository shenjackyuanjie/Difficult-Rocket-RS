use super::*;

#[test]
fn category_header_truncates_long_custom_names_and_keeps_touch_height() {
    let ctx = crate::egui_ui::test_context();
    for width in [240.0, 296.0] {
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(width, 480.0),
                )),
                ..default()
            },
            |ui| {
                ui.spacing_mut().interact_size.y = 44.0;
                let left = ui.cursor().left();
                let group = category_group(ui, &"很长的自定义部件类别".repeat(20), false, |_| {});
                assert!(group.header_response.rect.height() >= 44.0);
                assert!(
                    group.header_response.rect.right() <= left + width + 0.01,
                    "标题不得超出目录宽度：{:?} / {left} + {width}",
                    group.header_response.rect
                );
            },
        );
        output.textures_delta.clear();
    }
}

#[test]
fn actual_group_clicks_share_palette_state_with_keyboard_and_expand_all() {
    let mut document = crate::test_support::document();
    document.catalog = crate::test_support::catalog();
    let catalog = document.catalog.clone();
    let before = document.ship.clone();
    let mut app = App::new();
    app.insert_resource(document)
        .init_resource::<Palette>()
        .init_resource::<ShipBrowser>()
        .init_resource::<EditorCursor>()
        .init_resource::<DragState>()
        .add_message::<files::FileAction>()
        .add_message::<PanelButton>()
        .add_systems(Update, panel_actions);
    let ctx = crate::egui_ui::test_context();
    let mut hits = Vec::<(PanelButton, egui::Rect)>::new();
    let mut driver = crate::egui_ui::UiTestInput::default();
    for frame in 0..9 {
        let mut input = bevy_egui::EguiInput::default();
        if frame == 3 || frame == 6 {
            let action = if frame == 3 {
                PanelButton::ToggleCategory("Gizmos".into())
            } else {
                PanelButton::ExpandCategories
            };
            let rect = hits
                .iter()
                .find(|(candidate, _)| *candidate == action)
                .unwrap()
                .1;
            driver.click_rect(rect, &mut input);
        }
        let mut actions = Vec::new();
        let mut enabled_parts = Vec::new();
        hits.clear();
        let palette = app.world().resource::<Palette>();
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(400.0, 480.0),
                )),
                time: Some(f64::from(frame) * 0.2),
                events: input.0.events,
                ..default()
            },
            |ui| {
                let action = PanelButton::ExpandCategories;
                let response =
                    directory_button(ui, &action, palette.collapsed.is_empty(), "全部展开");
                hits.push((action.clone(), response.rect));
                if response.clicked() {
                    actions.push(action);
                }
                for (name, indices) in palette.groups(&catalog) {
                    let group = category_group(ui, name, !palette.collapsed.contains(name), |ui| {
                        for index in indices {
                            let response = part_row(
                                ui,
                                egui::TextureId::User(321),
                                Vec2::splat(30.0),
                                format!("部件 {index}"),
                                false,
                                ui.available_width(),
                            );
                            if response.enabled() {
                                enabled_parts.push(index);
                            }
                        }
                    });
                    hits.push((
                        PanelButton::ToggleCategory(name.into()),
                        group.header_response.rect,
                    ));
                    if group.header_response.clicked() {
                        actions.push(PanelButton::ToggleCategory(name.into()));
                    }
                }
            },
        );
        output.textures_delta.clear();
        if frame == 5 {
            assert_eq!(enabled_parts, vec![1, 2]);
        }
        if frame == 8 {
            assert_eq!(enabled_parts, vec![0, 1, 2]);
        }
        for action in actions {
            app.world_mut().write_message(action);
        }
        app.update();
        if frame == 3 {
            assert!(
                app.world()
                    .resource::<Palette>()
                    .collapsed
                    .contains("Gizmos")
            );
            assert_eq!(
                app.world().resource::<Palette>().indices(&catalog),
                vec![1, 2]
            );
        }
    }
    assert!(app.world().resource::<Palette>().collapsed.is_empty());
    let document = app.world().resource::<EditorDocument>();
    assert_eq!(document.ship, before);
    assert!(!document.dirty && !document.history.can_undo());
}
