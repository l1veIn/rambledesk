use super::*;
fn data() -> TableReviewData {
    TableReviewData {
        title: "Budget".into(),
        source_version: "v1".into(),
        columns: vec![TableReviewColumn {
            id: "amount".into(),
            label: "Amount".into(),
        }],
        rows: vec![TableReviewRow {
            id: "hosting".into(),
            cells: vec!["120".into()],
        }],
    }
}
fn state(body: &str, value: &str) -> WorkbenchState {
    WorkbenchState::TableReview(TableReviewState {
        changes: vec![TableReviewChange {
            row_id: "hosting".into(),
            column_id: "amount".into(),
            value: value.into(),
        }],
        comments: vec![TableReviewComment {
            id: "c-1".into(),
            row_id: "hosting".into(),
            column_id: "amount".into(),
            body: body.into(),
        }],
    })
}
#[test]
fn validates_rectangular_immutable_values_stable_ids_and_budgets() {
    let mut value = data();
    assert!(validate(&value).is_ok());
    value.rows[0].cells.clear();
    assert!(validate(&value).is_err());
    value = data();
    value.columns.push(value.columns[0].clone());
    assert!(validate(&value).is_err());
    value = data();
    value.rows.push(value.rows[0].clone());
    assert!(validate(&value).is_err());
    value = data();
    value.rows[0].cells[0] = "=SUM(A1:A3)".into();
    assert!(validate(&value).is_ok());
    value.rows[0].cells[0] = "".into();
    assert!(validate(&value).is_ok());
    value.rows[0].cells[0] = "\0".into();
    assert!(validate(&value).is_err());
    value.rows[0].cells[0] = "x".repeat(4001);
    assert!(validate(&value).is_err());
    value = data();
    value.rows = (0..126)
        .map(|index| TableReviewRow {
            id: format!("row-{index}"),
            cells: vec!["x".repeat(4000)],
        })
        .collect();
    assert!(validate(&value).is_err());
}
#[test]
fn blank_or_unchanged_drafts_save_but_unknown_duplicate_or_empty_comments_do_not_publish() {
    let data = data();
    let editing = state("", "120");
    assert!(draft_valid(&data, Some(&editing)));
    assert!(result(&data, Some(&editing)).is_none());
    let cleared = state("Clear the estimate", "");
    assert!(result(&data, Some(&cleared)).is_some());
    for body in ["", "\u{85}", "\0"] {
        assert!(result(&data, Some(&state(body, "100"))).is_none());
    }
    assert!(result(&data, Some(&state("\u{feff}", "100"))).is_some());
    let WorkbenchState::TableReview(mut invalid) = state("Changed", "100") else {
        unreachable!()
    };
    invalid.changes.push(invalid.changes[0].clone());
    assert!(!draft_valid(
        &data,
        Some(&WorkbenchState::TableReview(invalid))
    ));
    let WorkbenchState::TableReview(mut invalid) = state("Changed", "100") else {
        unreachable!()
    };
    invalid.comments[0].column_id = "missing".into();
    assert!(!draft_valid(
        &data,
        Some(&WorkbenchState::TableReview(invalid))
    ));
    assert!(complete(&data, result(&data, None).as_ref()));
    assert!(!has_input(&data, None));
}
