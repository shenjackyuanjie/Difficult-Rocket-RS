use crate::staging_shots::*;
use dr_core::PartKey;
#[test]
fn png_header_requires_signature_ihdr_and_positive_size() {
    let mut bytes = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
    bytes.extend(1440_u32.to_be_bytes());
    bytes.extend(900_u32.to_be_bytes());
    assert_eq!(png_size(&bytes), Some([1440, 900]));
    assert_eq!(png_size(&bytes[..20]), None);
    bytes[16..20].copy_from_slice(&0_u32.to_be_bytes());
    assert_eq!(png_size(&bytes), None);
    bytes[0] = 0;
    assert_eq!(png_size(&bytes), None);
}
#[test]
fn pointer_press_move_release_preserves_real_event_sequence() {
    let mut input = bevy_egui::EguiInput::default();
    let start = egui::pos2(10.0, 20.0);
    let end = egui::pos2(80.0, 70.0);
    pointer(&mut input, start, Some(true));
    pointer(&mut input, end, None);
    pointer(&mut input, end, Some(false));
    assert_eq!(input.0.events.len(), 5);
    assert!(matches!(
        input.0.events[1],
        egui::Event::PointerButton { pressed: true, .. }
    ));
    assert!(matches!(input.0.events[2], egui::Event::PointerMoved(p) if p == end));
    assert!(matches!(
        input.0.events[4],
        egui::Event::PointerButton { pressed: false, .. }
    ));
}
#[test]
fn phase_order_captures_six_states_before_done() {
    let mut phase = Phase::Init;
    let mut shots = vec![];
    for _ in 0..32 {
        if let Some(i) = phase.shot() {
            shots.push(NAMES[i]);
        }
        if phase == Phase::Done {
            break;
        }
        phase = phase.next();
    }
    assert_eq!(shots, NAMES);
    assert_eq!(phase, Phase::Done);
    assert_eq!(phase.next(), Phase::Done);
}
#[test]
fn paths_and_report_preserve_directory_and_schema() {
    let directory = Path::new("target/验收 shots");
    assert_eq!(
        shot_path(directory, 2),
        directory.join("staging-palette-coexist.png")
    );
    let r = report(
        &[json!({"state": NAMES[0], "file": "staging-empty.png"})],
        false,
    );
    assert_eq!(r["schema_version"], 1);
    assert_eq!(r["complete"], false);
    assert_eq!(r["shots"][0]["state"], NAMES[0]);
    assert!(report(&[], true)["complete"].as_bool().unwrap());
}
#[test]
fn coexist_requires_positive_disjoint_panels_and_canvas() {
    let rect = |x, w| egui::Rect::from_min_size(egui::pos2(x, 0.0), egui::vec2(w, 500.0));
    assert!(coexist(
        Some(rect(0.0, 280.0)),
        Some(rect(900.0, 300.0)),
        Some(rect(280.0, 620.0))
    ));
    assert!(!coexist(
        Some(rect(0.0, 1000.0)),
        Some(rect(900.0, 300.0)),
        Some(rect(280.0, 620.0))
    ));
    assert!(!coexist(
        Some(rect(0.0, 280.0)),
        None,
        Some(rect(280.0, 620.0))
    ));
    assert!(!coexist(
        Some(rect(0.0, 280.0)),
        Some(rect(900.0, 300.0)),
        Some(rect(280.0, 0.0))
    ));
    assert_eq!(rect_json(None), Value::Null);
    assert_eq!(rect_json(Some(rect(0.0, 280.0)))["positive"], true);
}
#[test]
fn staging_shots_arguments_require_smoke_and_one_directory() {
    let parse = |args: &[&str]| {
        crate::window::staging_shots_directory(
            &args.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
        )
    };
    assert!(parse(&[]).unwrap().is_none());
    assert_eq!(
        parse(&["--smoke-test", "--staging-shots", "target/截图 shots"]).unwrap(),
        Some(PathBuf::from("target/截图 shots"))
    );
    for args in [
        vec!["--staging-shots", "out"],
        vec!["--smoke-test", "--staging-shots"],
        vec!["--smoke-test", "--staging-shots", "--ship"],
        vec![
            "--smoke-test",
            "--staging-shots",
            "a",
            "--staging-shots",
            "b",
        ],
        vec![
            "--smoke-test",
            "--staging-shots",
            "out",
            "--staging-smoke-test",
        ],
    ] {
        assert!(parse(&args).is_err());
    }
}
#[test]
fn fixture_has_empty_pod_and_generates_multiple_real_stages() {
    let catalog = dr_core::catalog_from_xml(r#"<PartTypes><PartType id="pod-1" type="pod"/><PartType id="fueltank-0" type="tank"><Tank fuel="20"/></PartType><PartType id="engine-0" type="engine"><Engine/></PartType><PartType id="detacher-1" type="detacher"/></PartTypes>"#).unwrap();
    let ship = fixture(&catalog);
    assert_eq!(ship.parts.len(), 6);
    assert!(ship.parts[0].pod.as_ref().unwrap().staging.is_none());
    let p = dr_core::generate_basic_staging(&ship, &catalog, PartKey::new(0, 1, 0)).unwrap();
    assert_eq!(p.steps.len(), 3);
}
