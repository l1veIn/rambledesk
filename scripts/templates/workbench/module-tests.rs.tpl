
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn draft_and_publication_have_separate_completion_rules() {
        let data = __Pascal__Data { title: "Review".into(), material: "Prepared material".into() };
        validate(&data).unwrap();
        let incomplete = WorkbenchState::__Pascal__(__Pascal__State { score: None, note: "An opinion".into() });
        assert!(has_input(&data, Some(&incomplete)));
        assert!(result(&data, Some(&incomplete)).is_none());
        let state = WorkbenchState::__Pascal__(__Pascal__State { score: Some(4), note: "Looks useful".into() });
        let result = result(&data, Some(&state));
        assert!(complete(&data, result.as_ref()));
    }
    #[test]
    fn package_roundtrip_preserves_type_even_with_overlapping_result_shapes() {
        let input = describe().unwrap().example;
        let state = WorkbenchState::__Pascal__(__Pascal__State { score: Some(4), note: "Opinion".into() });
        let package = WorkbenchPackage { result: workbench_result(&input, Some(&state)), input };
        let restored: WorkbenchPackage = serde_value::to_value(&package).unwrap().deserialize_into().unwrap();
        assert_eq!(restored, package);
    }
}
