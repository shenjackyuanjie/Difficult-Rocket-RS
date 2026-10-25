
use super::*;

#[test]
fn resetting_line_color_accepts_only_color_space_roundoff_not_real_configuration_errors() {
    let mut settings = connection_lines::Settings {
        open: true,
        ..default()
    };
    settings.rgba[2] = 0.54999995;
    assert!(line_defaults_match(&settings));
    settings.width = 3.0;
    assert!(!line_defaults_match(&settings));
    settings.width = 2.0;
    settings.rgba[2] = 0.54;
    assert!(!line_defaults_match(&settings));
}
