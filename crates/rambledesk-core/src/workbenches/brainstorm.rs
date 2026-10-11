use super::*;
use std::collections::{HashMap, HashSet};

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum BrainstormPhase {
    Diverge,
    Organize,
    Focus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum BrainstormIntent {
    Explore,
    Expand,
    Combine,
    Proceed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum BrainstormOrigin {
    Agent,
    Human,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct BrainstormGroup {
    #[schemars(regex(pattern = "^[a-z0-9][a-z0-9_-]{0,79}$"))]
    pub id: String,
    #[schemars(length(max = 200))]
    pub title: String,
}

/// New Agent proposals only. An existing card ID can never be replaced by a proposal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct BrainstormProposal {
    #[schemars(regex(pattern = "^[a-z0-9][a-z0-9_-]{0,79}$"))]
    pub id: String,
    #[schemars(length(min = 1, max = 200))]
    pub title: String,
    #[schemars(length(max = 4000))]
    pub body: String,
    pub group_id: Option<String>,
    #[serde(default)]
    #[schemars(length(max = 20))]
    pub source_card_ids: Vec<String>,
}

/// Agent-authored request input. Server-derived board identity and snapshots are not accepted here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct BrainstormRequestData {
    #[schemars(length(min = 1, max = 200))]
    pub title: String,
    #[serde(default)]
    pub inherit_from_request_id: Option<String>,
    #[serde(default)]
    #[schemars(length(max = 30))]
    pub groups: Vec<BrainstormGroup>,
    #[serde(default)]
    #[schemars(length(max = 200))]
    pub cards: Vec<BrainstormProposal>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct BrainstormCard {
    pub id: String,
    pub title: String,
    pub body: String,
    pub group_id: Option<String>,
    pub source_card_ids: Vec<String>,
    pub origin: BrainstormOrigin,
    pub origin_request_id: String,
    pub parked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct BrainstormBoard {
    pub cards: Vec<BrainstormCard>,
    pub groups: Vec<BrainstormGroup>,
    pub selected_card_ids: Vec<String>,
    pub phase: BrainstormPhase,
}

/// Immutable application-resolved input delivered to the workbench, never accepted from an Agent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct BrainstormData {
    pub title: String,
    pub source_version: String,
    pub board_id: String,
    pub round: u32,
    pub inherit_from_request_id: Option<String>,
    pub initial_board: BrainstormBoard,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(untagged)]
pub enum BrainstormWireData {
    Request(BrainstormRequestData),
    Resolved(BrainstormData),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct BrainstormState {
    pub cards: Vec<BrainstormCard>,
    pub groups: Vec<BrainstormGroup>,
    pub selected_card_ids: Vec<String>,
    pub phase: BrainstormPhase,
    pub intent: Option<BrainstormIntent>,
    pub instruction: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct BrainstormResult {
    pub source_version: String,
    pub board_id: String,
    pub round: u32,
    pub inherit_from_request_id: Option<String>,
    pub cards: Vec<BrainstormCard>,
    pub groups: Vec<BrainstormGroup>,
    pub selected_card_ids: Vec<String>,
    pub phase: BrainstormPhase,
    pub intent: BrainstormIntent,
    pub instruction: String,
}

fn valid_id(id: &str) -> bool {
    let bytes = id.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= 80
        && (bytes[0].is_ascii_lowercase() || bytes[0].is_ascii_digit())
        && bytes
            .iter()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'_' | b'-'))
}
fn valid_text(value: &str, max: usize, required: bool) -> bool {
    !value.contains('\0') && value.chars().count() <= max && (!required || !value.trim().is_empty())
}
fn uuid(value: &str) -> bool {
    uuid::Uuid::parse_str(value).is_ok_and(|id| id.hyphenated().to_string() == value)
}
fn invalid() -> ApplicationError {
    ApplicationError::invalid_argument(
        "Invalid brainstorm board, lineage, selection or limits. Call describe_workbench for the contract.",
    )
}
fn groups_valid(groups: &[BrainstormGroup], complete: bool) -> bool {
    let mut ids = HashSet::new();
    groups.len() <= 30
        && groups
            .iter()
            .all(|g| valid_id(&g.id) && ids.insert(&g.id) && valid_text(&g.title, 200, complete))
}
fn board_valid(
    cards: &[BrainstormCard],
    groups: &[BrainstormGroup],
    selected: &[String],
    complete: bool,
) -> bool {
    if cards.len() > 200 || !groups_valid(groups, complete) || selected.len() > 200 {
        return false;
    }
    let group_ids: HashSet<_> = groups.iter().map(|g| g.id.as_str()).collect();
    let mut card_ids = HashSet::new();
    if !cards.iter().all(|c| {
        let mut sources = HashSet::new();
        valid_id(&c.id)
            && card_ids.insert(c.id.as_str())
            && valid_text(&c.title, 200, complete)
            && valid_text(&c.body, 4000, false)
            && uuid(&c.origin_request_id)
            && c.group_id
                .as_deref()
                .is_none_or(|id| group_ids.contains(id))
            && c.source_card_ids.len() <= 20
            && c.source_card_ids
                .iter()
                .all(|id| valid_id(id) && id != &c.id && sources.insert(id))
    }) {
        return false;
    }
    if !cards.iter().all(|c| {
        c.source_card_ids
            .iter()
            .all(|id| card_ids.contains(id.as_str()))
    }) {
        return false;
    }
    let available: HashSet<_> = cards
        .iter()
        .filter(|c| !c.parked)
        .map(|c| c.id.as_str())
        .collect();
    let mut selected_once = HashSet::new();
    selected
        .iter()
        .all(|id| available.contains(id.as_str()) && selected_once.insert(id))
}

pub fn validate_brainstorm_request(data: &BrainstormRequestData) -> Result<(), ApplicationError> {
    if !valid_text(&data.title, 200, true)
        || data
            .inherit_from_request_id
            .as_deref()
            .is_some_and(|id| !uuid(id))
        || !groups_valid(&data.groups, true)
        || data.cards.len() > 200
    {
        return Err(invalid());
    }
    let mut ids = HashSet::new();
    let groups: HashSet<_> = data.groups.iter().map(|g| g.id.as_str()).collect();
    for card in &data.cards {
        let mut sources = HashSet::new();
        if !valid_id(&card.id)
            || !ids.insert(&card.id)
            || !valid_text(&card.title, 200, true)
            || !valid_text(&card.body, 4000, false)
            || card.source_card_ids.len() > 20
            || !card
                .source_card_ids
                .iter()
                .all(|id| valid_id(id) && id != &card.id && sources.insert(id))
            || card.group_id.as_deref().is_some_and(|id| !valid_id(id))
            || (data.inherit_from_request_id.is_none()
                && (!card.source_card_ids.is_empty()
                    || card
                        .group_id
                        .as_deref()
                        .is_some_and(|id| !groups.contains(id))))
        {
            return Err(invalid());
        }
    }
    Ok(())
}

/// The storage transaction establishes ownership and publication before supplying the parent result.
pub fn resolve_brainstorm_request(
    input: &BrainstormRequestData,
    request_id: &str,
    parent: Option<&BrainstormResult>,
) -> Result<BrainstormData, ApplicationError> {
    validate_brainstorm_request(input)?;
    if !uuid(request_id)
        || input.inherit_from_request_id.as_deref() != parent.map(|p| p.source_version.as_str())
    {
        return Err(invalid());
    }
    let mut board = parent
        .map(|p| BrainstormBoard {
            cards: p.cards.clone(),
            groups: p.groups.clone(),
            selected_card_ids: p.selected_card_ids.clone(),
            phase: p.phase,
        })
        .unwrap_or(BrainstormBoard {
            cards: vec![],
            groups: vec![],
            selected_card_ids: vec![],
            phase: BrainstormPhase::Diverge,
        });
    let mut group_ids: HashSet<_> = board.groups.iter().map(|g| g.id.clone()).collect();
    for group in &input.groups {
        if !group_ids.insert(group.id.clone()) {
            return Err(invalid());
        }
        board.groups.push(group.clone());
    }
    let mut card_ids: HashSet<_> = board.cards.iter().map(|c| c.id.clone()).collect();
    let selected: HashSet<_> = board.selected_card_ids.iter().map(String::as_str).collect();
    for card in &input.cards {
        if !card_ids.insert(card.id.clone())
            || !card
                .source_card_ids
                .iter()
                .all(|id| selected.contains(id.as_str()))
        {
            return Err(invalid());
        }
        board.cards.push(BrainstormCard {
            id: card.id.clone(),
            title: card.title.clone(),
            body: card.body.clone(),
            group_id: card.group_id.clone(),
            source_card_ids: card.source_card_ids.clone(),
            origin: BrainstormOrigin::Agent,
            origin_request_id: request_id.to_owned(),
            parked: false,
        });
    }
    let data = BrainstormData {
        title: input.title.clone(),
        source_version: request_id.to_owned(),
        board_id: parent
            .map(|p| p.board_id.clone())
            .unwrap_or_else(|| request_id.to_owned()),
        round: parent
            .map(|p| p.round.checked_add(1))
            .unwrap_or(Some(1))
            .ok_or_else(invalid)?,
        inherit_from_request_id: input.inherit_from_request_id.clone(),
        initial_board: board,
    };
    validate_resolved(&data)?;
    Ok(data)
}

pub fn initial_brainstorm_state(data: &BrainstormData) -> BrainstormState {
    BrainstormState {
        cards: data.initial_board.cards.clone(),
        groups: data.initial_board.groups.clone(),
        selected_card_ids: data.initial_board.selected_card_ids.clone(),
        phase: data.initial_board.phase,
        intent: None,
        instruction: String::new(),
    }
}

fn validate_resolved(data: &BrainstormData) -> Result<(), ApplicationError> {
    if !valid_text(&data.title, 200, true)
        || !uuid(&data.source_version)
        || !uuid(&data.board_id)
        || data.round == 0
        || data
            .inherit_from_request_id
            .as_deref()
            .is_some_and(|id| !uuid(id) || id == data.source_version)
        || ((data.round == 1) != data.inherit_from_request_id.is_none())
        || (data.round == 1 && data.board_id != data.source_version)
        || !board_valid(
            &data.initial_board.cards,
            &data.initial_board.groups,
            &data.initial_board.selected_card_ids,
            true,
        )
    {
        return Err(invalid());
    }
    Ok(())
}
pub(super) fn validate(data: &BrainstormWireData) -> Result<(), ApplicationError> {
    match data {
        BrainstormWireData::Request(input) => validate_brainstorm_request(input),
        BrainstormWireData::Resolved(data) => validate_resolved(data),
    }
}
fn state_valid(data: &BrainstormData, state: &BrainstormState, complete: bool) -> bool {
    if !board_valid(
        &state.cards,
        &state.groups,
        &state.selected_card_ids,
        complete,
    ) || !valid_text(&state.instruction, 4000, false)
    {
        return false;
    }
    let current: HashMap<_, _> = state.cards.iter().map(|c| (c.id.as_str(), c)).collect();
    let initial: HashMap<_, _> = data
        .initial_board
        .cards
        .iter()
        .map(|c| (c.id.as_str(), c))
        .collect();
    if !initial.iter().all(|(id, old)| {
        current.get(id).is_some_and(|now| {
            now.origin == old.origin
                && now.origin_request_id == old.origin_request_id
                && now.source_card_ids == old.source_card_ids
        })
    }) {
        return false;
    }
    if !state
        .cards
        .iter()
        .filter(|c| !initial.contains_key(c.id.as_str()))
        .all(|c| {
            c.origin == BrainstormOrigin::Human
                && c.origin_request_id == data.source_version
                && c.source_card_ids.is_empty()
        })
    {
        return false;
    }
    !complete
        || match state.intent {
            Some(BrainstormIntent::Explore) => true,
            Some(BrainstormIntent::Expand | BrainstormIntent::Proceed) => {
                !state.selected_card_ids.is_empty()
            }
            Some(BrainstormIntent::Combine) => state.selected_card_ids.len() >= 2,
            None => false,
        }
}
pub(super) fn draft_valid(data: &BrainstormWireData, state: Option<&WorkbenchState>) -> bool {
    match (data, state) {
        (BrainstormWireData::Resolved(data), Some(WorkbenchState::Brainstorm(state))) => {
            state_valid(data, state, false)
        }
        (BrainstormWireData::Resolved(_), None) => true,
        _ => false,
    }
}
pub(super) fn has_input(_: &BrainstormWireData, state: Option<&WorkbenchState>) -> bool {
    matches!(state, Some(WorkbenchState::Brainstorm(_)))
}
pub(super) fn result(
    data: &BrainstormWireData,
    state: Option<&WorkbenchState>,
) -> Option<WorkbenchResult> {
    let (BrainstormWireData::Resolved(data), Some(WorkbenchState::Brainstorm(state))) =
        (data, state)
    else {
        return None;
    };
    if !state_valid(data, state, true) {
        return None;
    }
    Some(WorkbenchResult::Brainstorm(BrainstormResult {
        source_version: data.source_version.clone(),
        board_id: data.board_id.clone(),
        round: data.round,
        inherit_from_request_id: data.inherit_from_request_id.clone(),
        cards: state.cards.clone(),
        groups: state.groups.clone(),
        selected_card_ids: state.selected_card_ids.clone(),
        phase: state.phase,
        intent: state.intent?,
        instruction: state.instruction.clone(),
    }))
}
pub(super) fn complete(data: &BrainstormWireData, result: Option<&WorkbenchResult>) -> bool {
    let (BrainstormWireData::Resolved(data), Some(WorkbenchResult::Brainstorm(value))) =
        (data, result)
    else {
        return false;
    };
    value.source_version == data.source_version
        && value.board_id == data.board_id
        && value.round == data.round
        && value.inherit_from_request_id == data.inherit_from_request_id
        && state_valid(
            data,
            &BrainstormState {
                cards: value.cards.clone(),
                groups: value.groups.clone(),
                selected_card_ids: value.selected_card_ids.clone(),
                phase: value.phase,
                intent: Some(value.intent),
                instruction: value.instruction.clone(),
            },
            true,
        )
}
pub(super) fn result_has_input(value: Option<&WorkbenchResult>) -> bool {
    matches!(value, Some(WorkbenchResult::Brainstorm(_)))
}
pub(super) fn legacy_actions(_: &BrainstormWireData) -> Vec<ActionInput> {
    vec![]
}
pub(super) fn definition() -> WorkbenchDefinition {
    let mut definition = WorkbenchDefinition::new(
        WorkbenchSummary {
            kind: "brainstorm",
            version: 1,
            name: "Brainstorm / 头脑风暴",
            purpose: "Explore, organize and select ideas together across feedback rounds. 多轮发散、分组与选择想法，保留人的编辑。",
            returns: "Full card board, stable lineage, human selections and explicit next-round intent",
            interaction: "brainstorm",
        },
        describe,
    );
    definition.strict_state = true;
    definition
}
fn describe() -> Result<WorkbenchDescription, ApplicationError> {
    Ok(WorkbenchDescription {
        summary: definition().summary,
        input_schema: schemars::schema_for!(BrainstormRequestData),
        result_schema: schemars::schema_for!(BrainstormResult),
        example: WorkbenchSpec {
            kind: "brainstorm".into(),
            version: 1,
            data: WorkbenchData::Brainstorm(BrainstormWireData::Request(BrainstormRequestData {
                title: "Explore the first-run guide".into(),
                inherit_from_request_id: None,
                groups: vec![],
                cards: vec![BrainstormProposal {
                    id: "one-small-task".into(),
                    title: "Complete one small feedback task".into(),
                    body: "Let the first useful result introduce the workflow.".into(),
                    group_id: None,
                    source_card_ids: vec![],
                }],
            })),
        },
        instructions: "Create each round as a new request. For a follow-up, set inherit_from_request_id to a completed, published brainstorm request in the same session. The application inherits the submitted board, edits, groups, selections and parked cards; never retransmit or replace existing IDs. Supply only new groups/cards. Derived cards reference at most 20 previously selected source_card_ids. Managed sessions must match their trusted binding; external requests must both be external and match host_id/host_session_id (a correlation boundary, not additional authentication). Server-created source_version, board_id, round, origin and initial_board cannot be supplied. Humans may edit and group cards, add cards, select directions or park cards; existing cards and lineage are retained. Phase is presentation, not a workflow gate. Submission requires an explicit explore, expand (at least one selection), combine (at least two) or proceed (at least one) intent. Proceed means continue the original task using the selected directions, not authority for unrelated actions. Up to 200 cards, 30 groups, titles 200 and bodies/instruction 4000 Unicode scalars; IDs match ^[a-z0-9][a-z0-9_-]{0,79}$. Empty titles can be saved as drafts but not submitted. New rounds reset intent and instruction. Card attachment references are copied to independent child attachments during inheritance.",
    })
}
