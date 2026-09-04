// Adapted from Codeg 3ebdfed, src-tauri/src/commands/acp.rs (Apache-2.0):
// sanitize_custom_version / extract_version_token. Changed: bounded inputs and
// require numeric release components rather than accepting arbitrary labels.
pub(super) fn sanitize(input: &str) -> Option<String> {
    let value = input.trim();
    let value = value.strip_prefix(['v', 'V']).unwrap_or(value);
    if value.len() > 128
        || !value.chars().next()?.is_ascii_digit()
        || !value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '+'))
    {
        return None;
    }
    let release = value.split(['-', '+']).next()?;
    let numbers: Vec<_> = release.split('.').collect();
    if numbers.len() < 2
        || numbers
            .iter()
            .any(|part| part.is_empty() || !part.bytes().all(|c| c.is_ascii_digit()))
    {
        return None;
    }
    Some(value.into())
}

pub(super) fn extract(text: &str) -> Option<String> {
    text.split_whitespace()
        .filter(|token| !token.contains("://"))
        .flat_map(|token| {
            token
                .trim_matches(['(', ')', ',', ';', ':'])
                .split(['/', '@'])
        })
        .find_map(sanitize)
}

pub(super) fn meets(actual: &str, required: &str) -> bool {
    let parse = |value: &str| -> Option<Vec<u64>> {
        value
            .trim_start_matches('v')
            .split(['-', '+'])
            .next()?
            .split('.')
            .map(|part| part.parse().ok())
            .collect()
    };
    match (parse(actual), parse(required)) {
        (Some(actual), Some(required)) => actual >= required,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn custom_versions_are_concrete_and_cannot_change_the_install_target() {
        for value in [
            "latest",
            "next",
            "^1.0.0",
            "1.0.0 ../other",
            "1.0.0@other",
            "../1.0",
            "1",
            "1..0",
            "1.a",
        ] {
            assert!(sanitize(value).is_none(), "{value}");
        }
        assert_eq!(sanitize(" v0.1.2-rc.1 ").as_deref(), Some("0.1.2-rc.1"));
        assert_eq!(
            extract("fixture-cli/2.3.4\nhttps://host/9.8.7").as_deref(),
            Some("2.3.4")
        );
    }
}
