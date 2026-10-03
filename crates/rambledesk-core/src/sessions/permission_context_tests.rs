use super::*;

fn request() -> SessionInteraction {
    SessionInteraction {
        request_id: "permission".into(),
        session_id: "session".into(),
        title: String::new(),
        details: None,
        kind: SessionInteractionKind::Permission {
            tool_call_id: Some("exact-tool".into()),
            options: vec![],
        },
    }
}

fn stream() -> StreamState {
    let mut tool = SessionToolCall::new("exact-tool".into());
    tool.title = "Run feedback request".into();
    tool.raw_input = Some(
        r#"{"command":"feedback request --input probe.json","cwd":"C:/isolated/project-a"}"#.into(),
    );
    tool.raw_output = Some("Earlier output is not permission input".into());
    let row = SessionActivity {
        id: "row".into(),
        session_id: "session".into(),
        sequence: 1,
        turn_id: Some("turn".into()),
        kind: SessionActivityKind::ToolCall,
        text: String::new(),
        content: Some(SessionActivityContent::ToolCall { tool }),
        tool_call_id: Some("exact-tool".into()),
        created_at: "today".into(),
    };
    StreamState {
        turn_id: Some("turn".into()),
        tools: HashMap::from([("exact-tool".into(), row)]),
        ..Default::default()
    }
}

#[test]
fn id_only_permission_uses_exact_current_tool_without_exposing_output() {
    let mut permission = request();
    hydrate(&mut permission, &stream());
    assert_eq!(permission.title, "Run feedback request");
    let details = permission.details.unwrap();
    assert!(details.contains("feedback request --input probe.json"));
    assert!(details.contains("C:/isolated/project-a"));
    assert!(!details.contains("Earlier output"));
}

#[test]
fn unknown_foreign_and_old_turn_tools_never_supply_permission_context() {
    for mismatch in [
        "unknown_id",
        "foreign_session",
        "old_turn",
        "no_turn",
        "different_content_id",
    ] {
        let mut stream = stream();
        let row = stream.tools.get_mut("exact-tool").unwrap();
        match mismatch {
            "unknown_id" => {
                stream.tools.clear();
            }
            "foreign_session" => row.session_id = "other-session".into(),
            "old_turn" => row.turn_id = Some("old-turn".into()),
            "no_turn" => stream.turn_id = None,
            _ => {
                let Some(SessionActivityContent::ToolCall { tool }) = &mut row.content else {
                    panic!()
                };
                tool.id = "other-tool".into();
            }
        }
        let mut permission = request();
        hydrate(&mut permission, &stream);
        assert_eq!(permission.title, "Agent tool operation", "{mismatch}");
        assert_eq!(permission.details, None, "{mismatch}");
    }
}

#[test]
fn explicit_permission_title_and_details_remain_authoritative() {
    let mut permission = request();
    permission.title = "New explicit command".into();
    permission.details = Some("Explicit new input".into());
    let expected = permission.clone();
    hydrate(&mut permission, &stream());
    assert_eq!(permission, expected);
    permission.details = Some(String::new());
    hydrate(&mut permission, &stream());
    assert_eq!(permission.details.as_deref(), Some(""));
}

#[test]
fn fallback_preserves_diffs_and_bounds_multibyte_input_visibly() {
    let mut tool = SessionToolCall::new("tool".into());
    tool.content.push(SessionContentBlock::Diff {
        path: "C:/isolated/file".into(),
        old_text: Some("before".into()),
        new_text: "after".into(),
    });
    let details = describe(&tool).unwrap();
    assert!(details.contains("Before:\nbefore\nAfter:\nafter"));
    tool.raw_input = Some("汉🙂".repeat(10000));
    let details = describe(&tool).unwrap();
    assert!(details.len() <= MAX_BYTES);
    assert!(details.ends_with(TRUNCATED));
    assert!(!details.contains('\u{fffd}'));
}

#[test]
fn old_permission_snapshots_without_a_tool_id_remain_readable() {
    use serde_value::Value;
    let snapshot = Value::Map(
        [
            ("request_id", Value::String("old".into())),
            ("session_id", Value::String("session".into())),
            ("title", Value::String("Old permission".into())),
            ("details", Value::Unit),
            ("kind", Value::String("permission".into())),
            ("options", Value::Seq(vec![])),
        ]
        .into_iter()
        .map(|(key, value)| (Value::String(key.into()), value))
        .collect(),
    );
    let permission: SessionInteraction = snapshot.deserialize_into().unwrap();
    assert!(matches!(
        permission.kind,
        SessionInteractionKind::Permission {
            tool_call_id: None,
            ..
        }
    ));
    let Value::Map(fields) = serde_value::to_value(permission).unwrap() else {
        panic!("permission snapshot must be an object")
    };
    assert!(!fields.contains_key(&Value::String("tool_call_id".into())));
}
