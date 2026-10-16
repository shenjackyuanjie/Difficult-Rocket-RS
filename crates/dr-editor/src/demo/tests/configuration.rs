use super::*;

#[test]
fn detailed_staging_uses_the_full_64_stage_1024_action_fixture_while_brief_stays_small() {
    for (mode, stages, activations, parts) in
        [(Mode::Brief, 8, 48, 7), (Mode::Detailed, 64, 1024, 17)]
    {
        let mut document = crate::test_support::document();
        document.catalog = dr_core::catalog_from_xml(
            r#"<PartTypes>
          <PartType id="pod-1" type="pod" width="4" height="3"/>
          <PartType id="detacher-1" width="4" height="1"/>
        </PartTypes>"#,
        )
        .unwrap();
        document.ship = new_ship(&document.catalog);
        staging_fixture(&mut document, mode);
        let staging = document.ship.parts[0]
            .pod
            .as_ref()
            .unwrap()
            .staging
            .as_ref()
            .unwrap();
        assert_eq!(staging.steps.len(), stages);
        assert_eq!(
            staging
                .steps
                .iter()
                .map(|stage| stage.activations.len())
                .sum::<usize>(),
            activations
        );
        assert_eq!(document.ship.parts.len(), parts);
        assert_eq!(document.ship, document.saved_ship);
        assert!(
            staging
                .steps
                .iter()
                .flat_map(|stage| &stage.activations)
                .all(|activation| document.ship.part(activation.id).is_some())
        );
        let chapter = mode
            .chapters()
            .into_iter()
            .find(|chapter| chapter.id == "staging")
            .unwrap();
        assert_eq!(
            chapter
                .artifacts
                .contains(&"editor-staging-performance.json"),
            mode == Mode::Detailed
        );
    }
}

#[test]
fn modes_are_explicit_and_only_detailed_removes_fixed_smoke_deadlines() {
    let old = Instant::now() - Duration::from_secs(3600);
    assert!(!within_timeout(None, old, 60));
    for (args_mode, expected) in [
        (None, Mode::Brief),
        (Some("brief"), Mode::Brief),
        (Some("detailed"), Mode::Detailed),
    ] {
        let folder = tempfile::tempdir().unwrap();
        let mut args = vec![
            "editor".into(),
            "--demo-showcase".into(),
            folder.path().to_string_lossy().into_owned(),
        ];
        if let Some(mode) = args_mode {
            args.extend(["--demo-mode".into(), mode.into()]);
        }
        let showcase = Showcase::from_args(&args).unwrap().unwrap();
        assert_eq!(showcase.mode, expected);
        assert_eq!(showcase.step, Duration::from_millis(450));
        assert_eq!(
            showcase.case_result_pause(),
            if expected == Mode::Detailed {
                Duration::from_millis(1800)
            } else {
                Duration::ZERO
            }
        );
        assert_eq!(
            within_timeout(Some(&showcase), old, 60),
            expected == Mode::Detailed
        );
        assert_eq!(showcase.chapter().id, "panels");
    }
    for tail in [vec!["--demo-mode"], vec!["--demo-mode", "unknown"]] {
        let folder = tempfile::tempdir().unwrap();
        let mut args = vec![
            "editor".into(),
            "--demo-showcase".into(),
            folder.path().to_string_lossy().into_owned(),
        ];
        args.extend(tail.into_iter().map(String::from));
        assert!(Showcase::from_args(&args).is_err());
    }
}

#[test]
fn chapters_are_unique_and_evidence_is_relative() {
    let ids: BTreeSet<_> = CHAPTERS.iter().map(|chapter| chapter.id).collect();
    assert_eq!(ids.len(), 10);
    for chapter in CHAPTERS {
        assert!(!chapter.artifacts.is_empty());
        for name in chapter.artifacts {
            assert_eq!(std::path::Path::new(name).components().count(), 1);
        }
    }
    let all = Mode::Detailed.chapters();
    // 新章是增量：旧 19 章、45 用例、95 项证据继续单独锁定。
    let detailed: Vec<_> = all
        .iter()
        .copied()
        .filter(|chapter| chapter.id != "rocket-workflow")
        .collect();
    assert_eq!(detailed.len(), 19);
    assert_eq!(
        detailed
            .iter()
            .map(|chapter| demo_cases::cases(chapter.id).len())
            .sum::<usize>(),
        45
    );
    assert_eq!(
        detailed
            .iter()
            .map(|chapter| chapter.id)
            .collect::<BTreeSet<_>>()
            .len(),
        19
    );
    assert_eq!(
        detailed
            .iter()
            .filter(|chapter| demo_cases::cases(chapter.id).is_empty())
            .map(|chapter| chapter.id)
            .collect::<Vec<_>>(),
        CHAPTERS
            .iter()
            .map(|chapter| chapter.id)
            .collect::<Vec<_>>()
    );
    let evidence: Vec<_> = detailed
        .iter()
        .flat_map(|chapter| chapter.evidence())
        .collect();
    assert_eq!(evidence.len(), 95);
    assert_eq!(evidence.iter().collect::<BTreeSet<_>>().len(), 95);
    assert_eq!(all.len(), 20);
    let all_evidence: Vec<_> = all.iter().flat_map(|chapter| chapter.evidence()).collect();
    assert_eq!(all_evidence.len(), 104);
    assert_eq!(all_evidence.iter().collect::<BTreeSet<_>>().len(), 104);
}

#[test]
fn rocket_workflow_is_one_continuous_chapter_in_both_modes() {
    let expected = [
        "editor-rocket-command-pod.png",
        "editor-rocket-placement-preview.png",
        "editor-rocket-assembled.png",
        "editor-rocket-generated.png",
        "editor-rocket-manual.png",
        "rocket-assembled.xml",
        "rocket-basic-staging.xml",
        "rocket-manual-staging.xml",
        "rocket-workflow.json",
    ];
    for (mode, count) in [(Mode::Brief, 11), (Mode::Detailed, 20)] {
        let chapters = mode.chapters();
        assert_eq!(chapters.len(), count);
        let position = chapters
            .iter()
            .position(|chapter| chapter.id == "rocket-workflow")
            .unwrap();
        assert_eq!(chapters[position - 1].id, "connections");
        assert_eq!(
            chapters
                .iter()
                .filter(|chapter| chapter.id == "rocket-workflow")
                .count(),
            1
        );
        assert_eq!(chapters[position].artifacts, expected);
        assert_eq!(
            chapters[position].evidence(),
            expected
                .iter()
                .map(|name| (*name).to_string())
                .collect::<Vec<_>>()
        );
        for name in chapters[position].artifacts {
            assert_eq!(std::path::Path::new(name).components().count(), 1);
        }
    }
}

#[test]
fn cli_requires_positive_step_and_fresh_report() {
    let folder = tempfile::tempdir().unwrap();
    let args = vec![
        "editor".into(),
        "--demo-showcase".into(),
        folder.path().to_string_lossy().into_owned(),
        "--demo-step-ms".into(),
        "0".into(),
    ];
    assert!(Showcase::from_args(&args).is_err());
    let args = args[..3].to_vec();
    let showcase = Showcase::from_args(&args).unwrap().unwrap();
    assert_eq!(showcase.step, Duration::from_millis(450));
    std::fs::write(folder.path().join("demo-report.json"), "{}").unwrap();
    assert!(Showcase::from_args(&args).is_err());
}

#[test]
fn presentation_argument_is_independent_of_fast_pacing() {
    let folder = tempfile::tempdir().unwrap();
    let args = vec![
        "editor".into(),
        "--demo-showcase".into(),
        folder.path().to_string_lossy().into_owned(),
        "--demo-presentation".into(),
        "--demo-step-ms".into(),
        "40".into(),
    ];
    let showcase = Showcase::from_args(&args).unwrap().unwrap();
    assert!(showcase.presentation);
    assert_eq!(showcase.step, Duration::from_millis(40));
}
