use super::*;
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct SortItem {
    #[schemars(length(min = 1, max = 64))]
    pub id: String,
    #[schemars(length(min = 1, max = 200))]
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct SortData {
    #[schemars(length(min = 1, max = 200))]
    pub title: String,
    #[schemars(length(min = 2, max = 30))]
    pub items: Vec<SortItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct SortState {
    /// Every input item ID, exactly once, in the chosen order.
    #[schemars(length(min = 2, max = 30))]
    pub order: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct SortResult {
    /// Every input item ID, exactly once, in the chosen order.
    #[schemars(length(min = 2, max = 30))]
    pub order: Vec<String>,
}

pub(super) fn definition() -> WorkbenchDefinition {
    let mut definition = WorkbenchDefinition::new(
        WorkbenchSummary {
            kind: "sort",
            version: 1,
            name: "Sort / 拖动排序",
            purpose: "Let the user arrange prepared items into their preferred order.",
            returns: "All input item IDs in the user-selected order",
            interaction: "sort",
        },
        describe,
    );
    definition.strict_state = true;
    definition
}

fn describe() -> Result<WorkbenchDescription, ApplicationError> {
    Ok(WorkbenchDescription {
        summary: definition().summary,
        input_schema: schemars::schema_for!(SortData),
        result_schema: schemars::schema_for!(SortResult),
        example: WorkbenchSpec {
            kind: "sort".into(),
            version: 1,
            data: WorkbenchData::Sort(SortData {
                title: "Arrange the next release priorities".into(),
                items: vec![
                    SortItem { id: "reliability".into(), label: "Improve reliability".into() },
                    SortItem { id: "onboarding".into(), label: "Simplify onboarding".into() },
                    SortItem { id: "shortcuts".into(), label: "Add keyboard shortcuts".into() },
                ],
            }),
        },
        instructions: "Supply 2–30 items with unique stable IDs and labels. The user drags items to choose an order; keeping the original order is valid. The result contains every input ID exactly once. Optional explanations belong in the shared feedback body.",
    })
}

pub(super) fn validate(data: &SortData) -> Result<(), ApplicationError> {
    super::validation::text("sort.title", &data.title, 200)?;
    super::validation::count("sort.items", data.items.len(), 2, 30)?;
    let mut ids = HashSet::new();
    for item in &data.items {
        super::validation::text("sort.items[].id", &item.id, 64)?;
        super::validation::text("sort.items[].label", &item.label, 200)?;
        if !ids.insert(item.id.as_str()) {
            return Err(ApplicationError::invalid_argument(
                "sort.items[].id must be unique",
            ));
        }
    }
    Ok(())
}

fn valid_order(data: &SortData, order: &[String]) -> bool {
    if order.len() != data.items.len() {
        return false;
    }
    let mut remaining: HashSet<&str> = data.items.iter().map(|item| item.id.as_str()).collect();
    remaining.len() == data.items.len()
        && order.iter().all(|id| remaining.remove(id.as_str()))
        && remaining.is_empty()
}

pub(super) fn draft_valid(data: &SortData, state: Option<&WorkbenchState>) -> bool {
    match state {
        None => true,
        Some(WorkbenchState::Sort(state)) => valid_order(data, &state.order),
        _ => false,
    }
}

pub(super) fn result(data: &SortData, state: Option<&WorkbenchState>) -> Option<WorkbenchResult> {
    let Some(WorkbenchState::Sort(state)) = state else {
        return None;
    };
    valid_order(data, &state.order).then(|| {
        WorkbenchResult::Sort(SortResult {
            order: state.order.clone(),
        })
    })
}

pub(super) fn has_input(data: &SortData, state: Option<&WorkbenchState>) -> bool {
    result(data, state).is_some()
}

pub(super) fn result_has_input(result: Option<&WorkbenchResult>) -> bool {
    matches!(result, Some(WorkbenchResult::Sort(_)))
}

pub(super) fn complete(data: &SortData, result: Option<&WorkbenchResult>) -> bool {
    matches!(result, Some(WorkbenchResult::Sort(result)) if valid_order(data, &result.order))
}

pub(super) fn legacy_actions(_: &SortData) -> Vec<ActionInput> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data() -> SortData {
        SortData {
            title: "Choose the priorities".into(),
            items: vec![
                SortItem {
                    id: "a".into(),
                    label: "First task".into(),
                },
                SortItem {
                    id: "b".into(),
                    label: "Second task".into(),
                },
                SortItem {
                    id: "c".into(),
                    label: "Third task".into(),
                },
            ],
        }
    }

    fn state(order: &[&str]) -> WorkbenchState {
        WorkbenchState::Sort(SortState {
            order: order.iter().map(|id| (*id).into()).collect(),
        })
    }

    #[test]
    fn original_and_changed_orders_are_both_complete() {
        let data = data();
        validate(&data).unwrap();
        for order in [["a", "b", "c"], ["c", "b", "a"], ["b", "c", "a"]] {
            let state = state(&order);
            assert!(draft_valid(&data, Some(&state)));
            assert!(has_input(&data, Some(&state)));
            let result = result(&data, Some(&state));
            assert!(complete(&data, result.as_ref()));
            assert_eq!(
                result,
                Some(WorkbenchResult::Sort(SortResult {
                    order: order.map(String::from).into()
                }))
            );
        }
    }

    #[test]
    fn missing_state_can_be_saved_but_has_no_publishable_result() {
        let data = data();
        assert!(draft_valid(&data, None));
        assert!(!has_input(&data, None));
        assert!(result(&data, None).is_none());
        assert!(!complete(&data, None));
        assert!(!draft_valid(
            &data,
            Some(&WorkbenchState::WebReview {
                annotations: vec![]
            })
        ));
    }

    #[test]
    fn invalid_permutations_fail_draft_and_completion_checks() {
        let data = data();
        for order in [
            vec![],
            vec!["a"],
            vec!["a", "b"],
            vec!["a", "a", "c"],
            vec!["a", "b", "foreign"],
            vec!["a", "b", "c", "c"],
        ] {
            let state = state(&order);
            assert!(!draft_valid(&data, Some(&state)), "{order:?}");
            assert!(result(&data, Some(&state)).is_none(), "{order:?}");
            let forged = WorkbenchResult::Sort(SortResult {
                order: order.iter().map(|id| (*id).into()).collect(),
            });
            assert!(!complete(&data, Some(&forged)), "{order:?}");
        }
    }

    #[test]
    fn invalid_inputs_are_rejected_and_unicode_ids_remain_stable() {
        let mut unicode = data();
        unicode.items[0].id = "用户选择".into();
        validate(&unicode).unwrap();
        assert!(draft_valid(&unicode, Some(&state(&["用户选择", "b", "c"]))));

        let mut invalid = Vec::new();
        let mut value = data();
        value.title = " ".into();
        invalid.push(value);
        let mut value = data();
        value.title = "x".repeat(201);
        invalid.push(value);
        let mut value = data();
        value.items.truncate(1);
        invalid.push(value);
        let mut value = data();
        value.items = (0..31)
            .map(|n| SortItem {
                id: n.to_string(),
                label: "Task".into(),
            })
            .collect();
        invalid.push(value);
        for (id, label) in [
            ("".to_owned(), "Task".to_owned()),
            (" ".to_owned(), "Task".to_owned()),
            ("x".repeat(65), "Task".to_owned()),
            ("b".to_owned(), "Task".to_owned()),
            ("a".to_owned(), " ".to_owned()),
            ("a".to_owned(), "x".repeat(201)),
            ("a\0".to_owned(), "Task".to_owned()),
        ] {
            let mut value = data();
            value.items[0] = SortItem { id, label };
            invalid.push(value);
        }
        for value in invalid {
            assert!(validate(&value).is_err(), "{value:?}");
        }
    }

    #[test]
    fn typed_draft_and_package_roundtrips_preserve_chosen_ids() {
        let input = WorkbenchSpec {
            kind: "sort".into(),
            version: 1,
            data: WorkbenchData::Sort(data()),
        };
        let state = state(&["c", "a", "b"]);
        let decoded: WorkbenchState = serde_value::to_value(&state)
            .unwrap()
            .deserialize_into()
            .unwrap();
        assert_eq!(decoded, state);
        let package = WorkbenchPackage {
            result: workbench_result(&input, Some(&state)),
            input,
        };
        let restored: WorkbenchPackage = serde_value::to_value(&package)
            .unwrap()
            .deserialize_into()
            .unwrap();
        assert_eq!(restored, package);
        assert!(
            matches!(restored.result, Some(WorkbenchResult::Sort(SortResult { order })) if order == ["c", "a", "b"])
        );
    }

    #[test]
    fn discovery_schema_and_example_describe_sorting() {
        let description = describe().unwrap();
        assert_eq!(description.summary.kind, "sort");
        assert!(definition().strict_state);
        validate_workbench(&description.example).unwrap();
        let schema = description.input_schema.as_value();
        assert_eq!(schema["properties"]["items"]["minItems"], 2);
        assert_eq!(schema["properties"]["items"]["maxItems"], 30);
    }
}
