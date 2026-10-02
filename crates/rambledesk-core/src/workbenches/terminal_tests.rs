use super::*;

fn spec() -> WorkbenchSpec {
    describe_workbench(&DescribeWorkbenchInput {
        kind: "terminal".into(),
        version: None,
    })
    .unwrap()
    .example
}

fn session() -> TerminalTrialSession {
    TerminalTrialSession {
        id: "trial-1".into(),
        cwd: "/project".into(),
        shell: "sh".into(),
        cols: 80,
        rows: 24,
        status: TerminalTrialStatus::Stopped,
        exit_code: None,
        output: "\u{1b}[32mUsage: my-cli 😀\u{1b}[0m\r\n".into(),
        screen: "Usage: my-cli 😀".into(),
        truncated: false,
    }
}

fn draft(sessions: Vec<TerminalTrialSession>) -> FeedbackDraft {
    FeedbackDraft {
        workbench_state: Some(WorkbenchState::Terminal { sessions }),
    }
}

#[test]
fn terminal_trial_preserves_original_ansi_screen_and_session_outcome() {
    let evidence = session();
    let published = prepare_feedback_submission(
        Some(&spec()),
        Some(&draft(vec![evidence.clone()])),
        "Help is clear",
    )
    .unwrap()
    .unwrap();
    let Some(WorkbenchResult::Terminal(result)) = published.result else {
        panic!("missing terminal result")
    };
    assert_eq!(result.sessions, vec![evidence]);
    assert!(!WorkbenchKind::Terminal.supports_approval());
    assert!(!WorkbenchKind::Terminal.uses_legacy_action_identity());
    assert!(workbench_actions(&spec()).unwrap().is_empty());
    let serialized = serde_value::to_value(result).unwrap();
    let serde_value::Value::Map(fields) = serialized else {
        panic!("result must be an object")
    };
    assert!(!fields.contains_key(&serde_value::Value::String("input".into())));
}

#[test]
fn terminal_notes_only_empty_banner_and_cancellation_are_distinct() {
    let banner = session();
    assert_eq!(
        prepare_feedback_submission(Some(&spec()), Some(&draft(vec![banner.clone()])), "")
            .unwrap_err(),
        WorkbenchSubmissionError::Empty
    );
    assert!(
        prepare_feedback_submission(
            Some(&spec()),
            Some(&draft(vec![banner])),
            "Menu was confusing"
        )
        .is_ok()
    );
    let notes_only = prepare_feedback_submission(Some(&spec()), None, "Could not start the CLI")
        .unwrap()
        .unwrap();
    assert!(
        matches!(notes_only.result, Some(WorkbenchResult::Terminal(TerminalResult { sessions })) if sessions.is_empty())
    );
    assert!(
        workbench_package(&spec(), Some(&draft(vec![session()])), false)
            .result
            .is_none()
    );
}

#[test]
fn terminal_rejects_invalid_dimensions_duplicate_ids_exit_context_and_capture_overflow() {
    let cases: &[fn(&mut TerminalTrialSession)] = &[
        |item| item.cols = 19,
        |item| item.rows = 101,
        |item| item.cwd.clear(),
        |item| {
            item.status = TerminalTrialStatus::Running;
            item.exit_code = Some(0);
        },
        |item| item.output = "x".repeat(262145),
        |item| item.screen = "😀".repeat(65537),
    ];
    for mutate in cases {
        let mut invalid = session();
        mutate(&mut invalid);
        assert_eq!(
            prepare_feedback_submission(Some(&spec()), Some(&draft(vec![invalid])), "Useful notes")
                .unwrap_err(),
            WorkbenchSubmissionError::Incomplete
        );
    }
    for sessions in [vec![session(), session()], vec![session(); 17]] {
        assert_eq!(
            prepare_feedback_submission(Some(&spec()), Some(&draft(sessions)), "Useful notes")
                .unwrap_err(),
            WorkbenchSubmissionError::Incomplete
        );
    }
    let mut bounded = session();
    bounded.output = "x".repeat(262144);
    bounded.screen = "😀".repeat(65536);
    bounded.truncated = true;
    assert!(
        prepare_feedback_submission(
            Some(&spec()),
            Some(&draft(vec![bounded.clone()])),
            "Helpful output"
        )
        .is_ok()
    );
    let mut second = bounded.clone();
    second.id = "trial-2".into();
    assert_eq!(
        prepare_feedback_submission(
            Some(&spec()),
            Some(&draft(vec![bounded, second])),
            "Useful notes"
        )
        .unwrap_err(),
        WorkbenchSubmissionError::Incomplete
    );
}

#[test]
fn terminal_input_omits_legacy_commands_and_still_reads_existing_requests() {
    let example = spec();
    assert!(validate_workbench(&example).is_ok());
    let serde_value::Value::Map(fields) = serde_value::to_value(example.clone()).unwrap() else {
        unreachable!()
    };
    let serde_value::Value::Map(data) = &fields[&serde_value::Value::String("data".into())] else {
        unreachable!()
    };
    assert!(!data.contains_key(&serde_value::Value::String("commands".into())));
    assert_eq!(
        serde_value::Value::Map(fields)
            .deserialize_into::<WorkbenchSpec>()
            .unwrap(),
        example
    );
    let mut legacy = example;
    let WorkbenchData::Terminal(data) = &mut legacy.data else {
        unreachable!()
    };
    data.commands.push(TerminalCommand {
        id: "help".into(),
        title: "Help".into(),
        command: "my-cli --help".into(),
        description: None,
    });
    assert!(validate_workbench(&legacy).is_ok());
    assert_eq!(
        serde_value::to_value(legacy.clone())
            .unwrap()
            .deserialize_into::<WorkbenchSpec>()
            .unwrap(),
        legacy
    );
}

#[test]
fn terminal_discovery_rejects_unsafe_suggestion_controls_and_preserves_future_contracts() {
    for control in ['\n', '\r', '\t', '\u{1b}', '\u{7f}'] {
        let mut invalid = spec();
        let WorkbenchData::Terminal(data) = &mut invalid.data else {
            unreachable!()
        };
        data.commands.push(TerminalCommand {
            id: "help".into(),
            title: "Help".into(),
            command: format!("my-cli --help{control}"),
            description: None,
        });
        assert!(validate_workbench(&invalid).is_err());
    }
    let serde_value::Value::Map(mut fields) = serde_value::to_value(spec()).unwrap() else {
        unreachable!()
    };
    fields.insert(
        serde_value::Value::String("version".into()),
        serde_value::Value::U32(2),
    );
    let wire = serde_value::Value::Map(fields);
    let restored: WorkbenchSpec = wire.clone().deserialize_into().unwrap();
    assert!(matches!(restored.data, WorkbenchData::Unknown(_)));
    assert_eq!(serde_value::to_value(restored).unwrap(), wire);
}
