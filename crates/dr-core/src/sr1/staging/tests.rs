
use super::*;

#[test]
fn load_ignores_moved_and_keeps_duplicate_activations_and_empty_steps() {
    let mut staging = Staging::from_xml(
            r#"<Staging currentStage="-2"><Step><Activate Id="7" moved="0"/><Activate Id="404"/><Activate Id="7"/></Step><Step/></Staging>"#,
        ).unwrap();
    staging.retain_part_ids(&[7]);
    assert_eq!(staging.current_stage, -2);
    assert_eq!(staging.steps.len(), 2);
    assert_eq!(
        staging.steps[0].parts,
        vec![
            StagedPart {
                part_id: 7,
                moved: true
            },
            StagedPart {
                part_id: 7,
                moved: true
            },
        ]
    );
    assert!(staging.steps[1].parts.is_empty());
}

#[test]
fn id_case_missing_and_malformed_follow_native_zero_default() {
    let mut staging = Staging::from_xml(
            r#"<Staging><Step><Activate id="8"/><Activate Id="bad"/><Activate Id=" 8tail"/></Step></Staging>"#,
        ).unwrap();
    staging.retain_part_ids(&[0, 8]);
    assert_eq!(
        staging.steps[0]
            .parts
            .iter()
            .map(|p| p.part_id)
            .collect::<Vec<_>>(),
        [0, 0, 8]
    );
}

#[test]
fn native_sibling_walk_is_named_only_at_first_element() {
    let staging = Staging::from_xml(
            r#"<Staging><Ignored/><Step><Ignored/><Activate Id="1"/><Other Id="2"/></Step><Other><Activate Id="3"/></Other></Staging>"#,
        ).unwrap();
    assert_eq!(staging.steps.len(), 2);
    assert_eq!(
        staging.steps[0]
            .parts
            .iter()
            .map(|p| p.part_id)
            .collect::<Vec<_>>(),
        [1, 2]
    );
    assert_eq!(staging.steps[1].parts[0].part_id, 3);
}

#[test]
fn staging_attribute_overrides_legacy_initial_stage_only_when_parse_succeeds() {
    for (text, expected) in [
        ("<Staging/>", 4),
        (r#"<Staging currentStage="bad"/>"#, 4),
        (r#"<Staging currentStage="2x"/>"#, 2),
    ] {
        let staging =
            Staging::from_element_with_current_stage(&xml::parse(text).unwrap(), 4).unwrap();
        assert_eq!(staging.current_stage, expected);
    }
}

#[test]
fn save_writes_internal_moved_but_reload_reinitializes_it() {
    let staging = Staging {
        current_stage: 5,
        steps: vec![
            StageStep {
                parts: vec![StagedPart {
                    part_id: 9,
                    moved: false,
                }],
            },
            StageStep::default(),
        ],
    };
    let output = staging.to_element();
    assert_eq!(output.attr("currentStage"), Some("5"));
    assert_eq!(output.children[0].children[0].attr("moved"), Some("0"));
    assert!(Staging::from_element(&output).unwrap().steps[0].parts[0].moved);
    assert_eq!(output.children.len(), 2);
}

#[test]
fn wrong_root_is_explicit() {
    assert!(matches!(
        Staging::from_xml("<Ship/>"),
        Err(Error::WrongRoot {
            expected: "Staging",
            ..
        })
    ));
}
