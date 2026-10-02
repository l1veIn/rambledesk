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
    /// Surviving input item IDs, exactly once, in the chosen order.
    #[schemars(length(max = 30))]
    pub order: Vec<String>,
    /// Explicitly deleted input item IDs, disjoint from order.
    #[serde(default)]
    #[schemars(length(max = 30))]
    pub removed_ids: Vec<String>,
    /// Label edits remain in the draft when an item is deleted and restored.
    #[serde(default)]
    #[schemars(length(max = 30))]
    pub edited_items: Vec<SortLabelEdit>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct SortLabelEdit {
    #[schemars(length(min = 1, max = 64))]
    pub id: String,
    /// May be empty while editing; surviving published labels need visible text.
    #[schemars(length(max = 200))]
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct SortResult {
    /// Surviving input item IDs, exactly once, in the chosen order.
    #[schemars(length(max = 30))]
    pub order: Vec<String>,
    /// Final surviving labels in order. Absent only in legacy sort packages.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    #[schemars(length(max = 30))]
    pub items: Option<Vec<SortItem>>,
    /// Explicitly deleted input item IDs in the original input order.
    #[serde(default)]
    #[schemars(length(max = 30))]
    pub removed_ids: Vec<String>,
}

pub(super) fn definition() -> WorkbenchDefinition {
    let mut definition = WorkbenchDefinition::new(
        WorkbenchSummary {
            kind: "sort",
            version: 1,
            name: "Sort / 拖动排序",
            purpose: "Let the user reorder prepared items, edit labels, and delete items.",
            returns: "Surviving item IDs and final labels in the selected order, plus deleted IDs",
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
        instructions: "Supply 2–30 items with unique stable IDs and labels. The user may reorder items, edit their labels, or explicitly delete them. Keeping the original order and explicitly deleting all items are both valid. The result contains surviving IDs and final labels in order, plus deleted IDs; together they account for every input ID exactly once. Surviving labels need visible text before publication. Optional explanations belong in the shared feedback body.",
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

fn valid_partition(data: &SortData, order: &[String], removed_ids: &[String]) -> bool {
    if order.len() + removed_ids.len() != data.items.len() {
        return false;
    }
    let mut remaining: HashSet<&str> = data.items.iter().map(|item| item.id.as_str()).collect();
    remaining.len() == data.items.len()
        && order
            .iter()
            .chain(removed_ids)
            .all(|id| remaining.remove(id.as_str()))
        && remaining.is_empty()
}

fn valid_edits(data: &SortData, edits: &[SortLabelEdit]) -> bool {
    let ids: HashSet<&str> = data.items.iter().map(|item| item.id.as_str()).collect();
    let mut edited_ids = HashSet::new();
    edits.iter().all(|edit| {
        ids.contains(edit.id.as_str())
            && edited_ids.insert(edit.id.as_str())
            && !edit.label.contains('\0')
            && edit.label.chars().count() <= 200
    })
}

fn valid_state(data: &SortData, state: &SortState) -> bool {
    valid_partition(data, &state.order, &state.removed_ids)
        && valid_edits(data, &state.edited_items)
}

pub(super) fn draft_valid(data: &SortData, state: Option<&WorkbenchState>) -> bool {
    match state {
        None => true,
        Some(WorkbenchState::Sort(state)) => valid_state(data, state),
        _ => false,
    }
}

pub(super) fn result(data: &SortData, state: Option<&WorkbenchState>) -> Option<WorkbenchResult> {
    let Some(WorkbenchState::Sort(state)) = state else {
        return None;
    };
    if !valid_state(data, state) {
        return None;
    }
    let items: Option<Vec<SortItem>> = state
        .order
        .iter()
        .map(|id| {
            let original = data.items.iter().find(|item| &item.id == id)?;
            let label = state
                .edited_items
                .iter()
                .find(|edit| &edit.id == id)
                .map_or(&original.label, |edit| &edit.label);
            super::validation::text("sort.items[].label", label, 200).ok()?;
            Some(SortItem {
                id: id.clone(),
                label: label.clone(),
            })
        })
        .collect();
    Some(WorkbenchResult::Sort(SortResult {
        order: state.order.clone(),
        items: Some(items?),
        removed_ids: data
            .items
            .iter()
            .filter(|item| state.removed_ids.contains(&item.id))
            .map(|item| item.id.clone())
            .collect(),
    }))
}

pub(super) fn has_input(data: &SortData, state: Option<&WorkbenchState>) -> bool {
    state.is_some() && draft_valid(data, state)
}

pub(super) fn result_has_input(result: Option<&WorkbenchResult>) -> bool {
    matches!(result, Some(WorkbenchResult::Sort(_)))
}

pub(super) fn complete(data: &SortData, result: Option<&WorkbenchResult>) -> bool {
    let Some(WorkbenchResult::Sort(result)) = result else {
        return false;
    };
    if !valid_partition(data, &result.order, &result.removed_ids) {
        return false;
    }
    match &result.items {
        Some(items) => {
            items.len() == result.order.len()
                && items.iter().zip(&result.order).all(|(item, id)| {
                    &item.id == id
                        && super::validation::text("sort.items[].label", &item.label, 200).is_ok()
                })
        }
        None => result.removed_ids.is_empty(),
    }
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
            removed_ids: vec![],
            edited_items: vec![],
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
                    order: order.map(String::from).into(),
                    items: Some(
                        order
                            .iter()
                            .map(|id| data
                                .items
                                .iter()
                                .find(|item| item.id == *id)
                                .unwrap()
                                .clone())
                            .collect()
                    ),
                    removed_ids: vec![],
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
                items: None,
                removed_ids: vec![],
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
            matches!(restored.result, Some(WorkbenchResult::Sort(SortResult { order, .. })) if order == ["c", "a", "b"])
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

    fn edited_state(order: &[&str], removed: &[&str], edits: &[(&str, &str)]) -> WorkbenchState {
        WorkbenchState::Sort(SortState {
            order: order.iter().map(|id| (*id).into()).collect(),
            removed_ids: removed.iter().map(|id| (*id).into()).collect(),
            edited_items: edits
                .iter()
                .map(|(id, label)| SortLabelEdit {
                    id: (*id).into(),
                    label: (*label).into(),
                })
                .collect(),
        })
    }

    #[test]
    fn edits_survive_deletion_and_restoration_without_mutating_input() {
        let data = data();
        let removed = edited_state(
            &["c", "a"],
            &["b"],
            &[("a", "Edited first"), ("b", "Edited deleted")],
        );
        assert!(draft_valid(&data, Some(&removed)));
        let removed_result = result(&data, Some(&removed)).unwrap();
        assert!(complete(&data, Some(&removed_result)));
        assert_eq!(
            removed_result,
            WorkbenchResult::Sort(SortResult {
                order: vec!["c".into(), "a".into()],
                items: Some(vec![
                    data.items[2].clone(),
                    SortItem {
                        id: "a".into(),
                        label: "Edited first".into()
                    }
                ]),
                removed_ids: vec!["b".into()],
            })
        );
        let restored = edited_state(
            &["c", "a", "b"],
            &[],
            &[("a", "Edited first"), ("b", "Edited deleted")],
        );
        let Some(WorkbenchResult::Sort(restored_result)) = result(&data, Some(&restored)) else {
            panic!("restored result missing");
        };
        assert_eq!(restored_result.items.unwrap()[2].label, "Edited deleted");
        assert_eq!(data.items[0].label, "First task");
        assert_eq!(data.items[1].label, "Second task");
    }

    #[test]
    fn blank_labels_are_saved_but_surviving_blanks_cannot_publish() {
        let data = data();
        for label in ["", " ", "\n\t"] {
            let draft = edited_state(&["a", "b", "c"], &[], &[("a", label)]);
            assert!(draft_valid(&data, Some(&draft)));
            assert!(has_input(&data, Some(&draft)));
            assert!(result(&data, Some(&draft)).is_none());
            let draft = edited_state(&["b", "c"], &["a"], &[("a", label)]);
            let package = result(&data, Some(&draft));
            assert!(complete(&data, package.as_ref()));
        }
    }

    #[test]
    fn all_items_can_be_explicitly_removed_and_results_have_canonical_removed_order() {
        let data = data();
        let draft = edited_state(&[], &["c", "a", "b"], &[("b", "")]);
        assert!(draft_valid(&data, Some(&draft)));
        assert!(has_input(&data, Some(&draft)));
        let package = result(&data, Some(&draft));
        assert!(complete(&data, package.as_ref()));
        assert_eq!(
            package,
            Some(WorkbenchResult::Sort(SortResult {
                order: vec![],
                items: Some(vec![]),
                removed_ids: vec!["a".into(), "b".into(), "c".into()],
            }))
        );
        assert!(!draft_valid(&data, Some(&state(&[]))));
    }

    #[test]
    fn forged_partitions_and_label_edits_are_rejected() {
        let data = data();
        for (order, removed, edits) in [
            (vec!["a", "c"], vec![], vec![]),
            (vec!["a", "b"], vec!["a"], vec![]),
            (vec!["a", "b"], vec!["foreign"], vec![]),
            (vec!["a"], vec!["b", "b"], vec![]),
            (vec!["a", "b", "c"], vec![], vec![("foreign", "New")]),
            (
                vec!["a", "b", "c"],
                vec![],
                vec![("a", "New"), ("a", "Other")],
            ),
            (vec!["a", "b", "c"], vec![], vec![("b", "Nul\0")]),
        ] {
            let draft = edited_state(&order, &removed, &edits);
            assert!(!draft_valid(&data, Some(&draft)), "{draft:?}");
            assert!(result(&data, Some(&draft)).is_none());
        }
        let too_long = "x".repeat(201);
        assert!(!draft_valid(
            &data,
            Some(&edited_state(&["a", "b", "c"], &[], &[("a", &too_long)]))
        ));
        let boundary = "🙂".repeat(200);
        assert!(draft_valid(
            &data,
            Some(&edited_state(&["a", "b", "c"], &[], &[("a", &boundary)]))
        ));
    }

    #[test]
    fn publication_rechecks_result_labels_and_order_agreement() {
        let data = data();
        let valid = SortResult {
            order: vec!["a".into(), "c".into()],
            items: Some(vec![data.items[0].clone(), data.items[2].clone()]),
            removed_ids: vec!["b".into()],
        };
        assert!(complete(&data, Some(&WorkbenchResult::Sort(valid.clone()))));
        let mut mismatched = valid.clone();
        mismatched.items.as_mut().unwrap().swap(0, 1);
        let mut incomplete = valid.clone();
        incomplete.items.as_mut().unwrap().pop();
        let mut blank = valid.clone();
        blank.items.as_mut().unwrap()[0].label = " ".into();
        let mut no_items = valid.clone();
        no_items.items = None;
        let mut foreign = valid;
        foreign.items.as_mut().unwrap()[0].id = "foreign".into();
        for invalid in [mismatched, incomplete, blank, no_items, foreign] {
            assert!(!complete(&data, Some(&WorkbenchResult::Sort(invalid))));
        }
    }

    #[test]
    fn legacy_order_only_drafts_and_packages_deserialize_and_remain_complete() {
        #[derive(Serialize)]
        struct LegacyState {
            #[serde(rename = "type")]
            kind: &'static str,
            order: Vec<String>,
        }
        #[derive(Serialize)]
        struct LegacyResult {
            order: Vec<String>,
        }
        #[derive(Serialize)]
        struct LegacyPackage {
            #[serde(rename = "type")]
            kind: &'static str,
            version: u32,
            data: SortData,
            result: LegacyResult,
        }
        let value = serde_value::to_value(LegacyState {
            kind: "sort",
            order: vec!["c".into(), "a".into(), "b".into()],
        })
        .unwrap();
        let state: WorkbenchState = value.deserialize_into().unwrap();
        assert!(draft_valid(&data(), Some(&state)));
        let Some(WorkbenchResult::Sort(current_result)) = result(&data(), Some(&state)) else {
            panic!("legacy state result missing");
        };
        assert!(current_result.items.is_some());
        let package: WorkbenchPackage = serde_value::to_value(LegacyPackage {
            kind: "sort",
            version: 1,
            data: data(),
            result: LegacyResult {
                order: vec!["c".into(), "a".into(), "b".into()],
            },
        })
        .unwrap()
        .deserialize_into()
        .unwrap();
        assert!(complete(&data(), package.result.as_ref()));
        assert!(matches!(
            package.result,
            Some(WorkbenchResult::Sort(SortResult { items: None, .. }))
        ));
        let restored: WorkbenchPackage = serde_value::to_value(&package)
            .unwrap()
            .deserialize_into()
            .unwrap();
        assert_eq!(restored, package);
    }
}
