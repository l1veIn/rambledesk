use super::*;

const FIRST: &str = "019a0010-0000-7000-8000-000000000001";
const SECOND: &str = "019a0010-0000-7000-8000-000000000002";

fn input() -> BrainstormRequestData {
    BrainstormRequestData {
        title: "Ideas".into(),
        inherit_from_request_id: None,
        groups: vec![BrainstormGroup {
            id: "entry".into(),
            title: "Entry".into(),
        }],
        cards: vec![BrainstormProposal {
            id: "small-task".into(),
            title: "One small task".into(),
            body: "First useful feedback".into(),
            group_id: Some("entry".into()),
            source_card_ids: vec![],
        }],
    }
}
fn resolved() -> BrainstormData {
    resolve_brainstorm_request(&input(), FIRST, None).unwrap()
}
fn wire(data: &BrainstormData) -> BrainstormWireData {
    BrainstormWireData::Resolved(data.clone())
}
fn published(data: &BrainstormData, state: BrainstormState) -> BrainstormResult {
    match result(&wire(data), Some(&WorkbenchState::Brainstorm(state))).unwrap() {
        WorkbenchResult::Brainstorm(result) => result,
        _ => panic!("brainstorm result"),
    }
}

#[test]
fn submitted_board_inherits_human_edits_selection_and_lineage_and_appends_only_new_proposals() {
    let data = resolved();
    let mut state = initial_brainstorm_state(&data);
    state.cards[0].title = "Human revised title".into();
    state.groups[0].title = "Human group".into();
    state.cards.push(BrainstormCard {
        id: "human-idea".into(),
        title: "Human idea".into(),
        body: "Details".into(),
        group_id: None,
        source_card_ids: vec![],
        origin: BrainstormOrigin::Human,
        origin_request_id: FIRST.into(),
        parked: true,
    });
    state.selected_card_ids.push("small-task".into());
    state.phase = BrainstormPhase::Focus;
    state.intent = Some(BrainstormIntent::Expand);
    state.instruction = "Explore this".into();
    let parent = published(&data, state);
    let mut request = input();
    request.inherit_from_request_id = Some(FIRST.into());
    request.groups.clear();
    request.cards[0].id = "expanded-task".into();
    request.cards[0].source_card_ids = vec!["small-task".into()];
    let next = resolve_brainstorm_request(&request, SECOND, Some(&parent)).unwrap();
    assert_eq!(next.board_id, FIRST);
    assert_eq!(next.source_version, SECOND);
    assert_eq!(next.round, 2);
    assert_eq!(&next.initial_board.cards[..2], parent.cards.as_slice());
    assert_eq!(next.initial_board.groups, parent.groups);
    assert_eq!(
        next.initial_board.selected_card_ids,
        parent.selected_card_ids
    );
    assert_eq!(next.initial_board.phase, BrainstormPhase::Focus);
    assert_eq!(next.initial_board.cards[2].origin_request_id, SECOND);
    assert_eq!(next.initial_board.cards[2].origin, BrainstormOrigin::Agent);
    let next_state = initial_brainstorm_state(&next);
    assert_eq!(next_state.intent, None);
    assert_eq!(next_state.instruction, "");
}

#[test]
fn inheritance_rejects_overwrites_unselected_sources_wrong_parent_and_excess_total_size() {
    let data = resolved();
    let mut state = initial_brainstorm_state(&data);
    state.intent = Some(BrainstormIntent::Explore);
    let parent = published(&data, state);
    let mut request = input();
    request.inherit_from_request_id = Some(FIRST.into());
    assert!(resolve_brainstorm_request(&request, SECOND, Some(&parent)).is_err()); // group collision
    request.groups.clear();
    assert!(resolve_brainstorm_request(&request, SECOND, Some(&parent)).is_err()); // card collision
    request.cards[0].id = "new-card".into();
    request.cards[0].source_card_ids.push("small-task".into());
    assert!(resolve_brainstorm_request(&request, SECOND, Some(&parent)).is_err()); // unselected source
    request.cards[0].source_card_ids.clear();
    assert!(resolve_brainstorm_request(&request, SECOND, None).is_err());
    request.cards = (0..200)
        .map(|i| BrainstormProposal {
            id: format!("new-{i}"),
            ..input().cards[0].clone()
        })
        .collect();
    assert!(resolve_brainstorm_request(&request, SECOND, Some(&parent)).is_err());
}

#[test]
fn drafts_allow_unfinished_titles_but_never_delete_cards_or_forge_lineage() {
    let data = resolved();
    let mut state = initial_brainstorm_state(&data);
    state.cards[0].title.clear();
    state.groups[0].title.clear();
    assert!(state_valid(&data, &state, false));
    state.intent = Some(BrainstormIntent::Explore);
    assert!(!state_valid(&data, &state, true));
    for invalidate in [
        |s: &mut BrainstormState| s.cards.clear(),
        |s: &mut BrainstormState| s.cards[0].origin = BrainstormOrigin::Human,
        |s: &mut BrainstormState| s.cards[0].origin_request_id = SECOND.into(),
        |s: &mut BrainstormState| s.cards[0].source_card_ids = vec!["made-up".into()],
        |s: &mut BrainstormState| s.cards[0].group_id = Some("missing".into()),
        |s: &mut BrainstormState| s.selected_card_ids = vec!["missing".into()],
    ] {
        let mut state = initial_brainstorm_state(&data);
        invalidate(&mut state);
        assert!(!state_valid(&data, &state, false));
    }
}

#[test]
fn empty_groups_can_be_removed_and_only_new_human_cards_admitted() {
    let data = resolved();
    let mut state = initial_brainstorm_state(&data);
    state.cards[0].group_id = None;
    state.groups.clear();
    let mut human = state.cards[0].clone();
    human.id = "human-new".into();
    human.origin = BrainstormOrigin::Human;
    state.cards.push(human);
    assert!(state_valid(&data, &state, false));
    state.cards[1].origin = BrainstormOrigin::Agent;
    assert!(!state_valid(&data, &state, false));
    state.cards[1].origin = BrainstormOrigin::Human;
    state.cards[1].origin_request_id = SECOND.into();
    assert!(!state_valid(&data, &state, false));
}

#[test]
fn submission_requires_explicit_intent_and_matching_selection_without_gating_phase() {
    let data = resolved();
    for phase in [
        BrainstormPhase::Diverge,
        BrainstormPhase::Organize,
        BrainstormPhase::Focus,
    ] {
        let mut state = initial_brainstorm_state(&data);
        state.phase = phase;
        assert!(!state_valid(&data, &state, true));
        state.intent = Some(BrainstormIntent::Explore);
        assert!(state_valid(&data, &state, true));
        for intent in [BrainstormIntent::Expand, BrainstormIntent::Proceed] {
            state.intent = Some(intent);
            assert!(!state_valid(&data, &state, true));
            state.selected_card_ids = vec!["small-task".into()];
            assert!(state_valid(&data, &state, true));
            state.cards[0].parked = true;
            assert!(!state_valid(&data, &state, false));
            state.cards[0].parked = false;
            state.selected_card_ids.clear();
        }
        state.intent = Some(BrainstormIntent::Combine);
        state.selected_card_ids = vec!["small-task".into()];
        assert!(!state_valid(&data, &state, true));
        let mut human = state.cards[0].clone();
        human.id = "human-new".into();
        human.origin = BrainstormOrigin::Human;
        state.cards.push(human);
        state.selected_card_ids.push("human-new".into());
        assert!(state_valid(&data, &state, true));
    }
}

#[test]
fn agent_cannot_supply_server_fields_and_result_identity_cannot_be_forged() {
    let data = resolved();
    let spec = WorkbenchSpec {
        kind: "brainstorm".into(),
        version: 1,
        data: WorkbenchData::Brainstorm(wire(&data)),
    };
    assert!(validate_workbench(&spec).is_ok());
    assert!(validate_workbench_request(&spec).is_err());
    let serde_value::Value::Map(mut raw) = serde_value::to_value(input()).unwrap() else {
        unreachable!()
    };
    raw.insert(
        serde_value::Value::String("board_id".into()),
        serde_value::Value::String(FIRST.into()),
    );
    assert!(BrainstormWireData::deserialize(serde_value::Value::Map(raw)).is_err());
    let mut state = initial_brainstorm_state(&data);
    state.intent = Some(BrainstormIntent::Explore);
    let mut value = published(&data, state);
    assert!(complete(
        &wire(&data),
        Some(&WorkbenchResult::Brainstorm(value.clone()))
    ));
    value.source_version = SECOND.into();
    assert!(!complete(
        &wire(&data),
        Some(&WorkbenchResult::Brainstorm(value))
    ));
}

#[test]
fn limits_use_unicode_scalars_and_reject_bad_ids_nul_and_duplicate_selection() {
    let mut input = input();
    input.cards[0].body = "😀".repeat(4000);
    assert!(validate_brainstorm_request(&input).is_ok());
    input.cards[0].body.push('x');
    assert!(validate_brainstorm_request(&input).is_err());
    for id in ["", "BAD", "a b", &"a".repeat(81)] {
        input.cards[0].body.clear();
        input.cards[0].id = id.into();
        assert!(validate_brainstorm_request(&input).is_err());
    }
    let data = resolved();
    let mut state = initial_brainstorm_state(&data);
    state.instruction = "bad\0".into();
    assert!(!state_valid(&data, &state, false));
    state.instruction.clear();
    state.selected_card_ids = vec!["small-task".into(), "small-task".into()];
    assert!(!state_valid(&data, &state, false));
}
