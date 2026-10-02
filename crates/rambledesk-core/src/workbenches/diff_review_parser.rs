#[derive(Debug, Clone, Copy)]
pub(super) struct Hunk {
    pub old: (u32, u32),
    pub new: (u32, u32),
}
fn range(value: &str) -> Option<(u32, u32)> {
    let (start, count) = value.split_once(',').unwrap_or((value, "1"));
    if start.is_empty()
        || count.is_empty()
        || !start
            .bytes()
            .chain(count.bytes())
            .all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    let start = start.parse::<u32>().ok()?;
    let count = count.parse::<u32>().ok()?;
    if (count > 0 && start == 0) || start.checked_add(count).is_none() {
        return None;
    }
    Some((start, count))
}
fn header(line: &str) -> Option<Hunk> {
    let content = line.strip_prefix("@@ -")?;
    let (old, content) = content.split_once(" +")?;
    let (new, _) = content.split_once(" @@")?;
    let hunk = Hunk {
        old: range(old)?,
        new: range(new)?,
    };
    (hunk.old.1 > 0 || hunk.new.1 > 0).then_some(hunk)
}
fn metadata(line: &str) -> bool {
    [
        "diff --git ",
        "index ",
        "--- ",
        "+++ ",
        "old mode ",
        "new mode ",
        "deleted file mode ",
        "new file mode ",
        "similarity index ",
        "dissimilarity index ",
        "rename from ",
        "rename to ",
        "copy from ",
        "copy to ",
    ]
    .iter()
    .any(|prefix| line.starts_with(prefix))
}
/// Parse only textual unified diffs; ranges are authoritative only after exact body counts.
pub(super) fn parse(diff: &str) -> Option<Vec<Hunk>> {
    let mut hunks: Vec<Hunk> = vec![];
    let mut consumed = (0u32, 0u32);
    let mut previous_body = false;
    let mut file_headers = (0usize, 0usize, 0usize);
    for raw in diff.split_terminator('\n') {
        let line = raw.strip_suffix('\r').unwrap_or(raw);
        if line.starts_with("@@") {
            if let Some(previous) = hunks.last()
                && consumed != (previous.old.1, previous.new.1)
            {
                return None;
            }
            let next = header(line)?;
            if let Some(previous) = hunks.last()
                && (next.old.0 < previous.old.0 + previous.old.1
                    || next.new.0 < previous.new.0 + previous.new.1)
            {
                return None;
            }
            hunks.push(next);
            consumed = (0, 0);
            previous_body = false;
            continue;
        }
        if line == "\\ No newline at end of file" {
            if !previous_body {
                return None;
            }
            previous_body = false;
            continue;
        }
        if let Some(hunk) = hunks.last() {
            if consumed != (hunk.old.1, hunk.new.1) {
                match line.as_bytes().first() {
                    Some(b' ') => {
                        consumed.0 = consumed.0.checked_add(1)?;
                        consumed.1 = consumed.1.checked_add(1)?;
                    }
                    Some(b'-') => consumed.0 = consumed.0.checked_add(1)?,
                    Some(b'+') => consumed.1 = consumed.1.checked_add(1)?,
                    _ => return None,
                }
                if consumed.0 > hunk.old.1 || consumed.1 > hunk.new.1 {
                    return None;
                }
                previous_body = true;
                continue;
            }
            // File metadata after a hunk denotes another file, not this immutable file input.
            if !line.is_empty() {
                return None;
            }
            previous_body = false;
        } else {
            if line.starts_with("diff --git ") {
                file_headers.0 += 1
            }
            if line.starts_with("--- ") {
                file_headers.1 += 1
            }
            if line.starts_with("+++ ") {
                file_headers.2 += 1
            }
            if file_headers.0 > 1
                || file_headers.1 > 1
                || file_headers.2 > 1
                || (!line.is_empty() && !metadata(line))
            {
                return None;
            }
        }
    }
    let last = hunks.last()?;
    (consumed == (last.old.1, last.new.1)).then_some(hunks)
}
