use std::{
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};

use async_trait::async_trait;
use rambledesk_core::WorkspaceInfoProvider;

/// Reads only Git metadata; no Git process, hooks, configuration or working-tree scan.
pub struct LocalWorkspaceInfoProvider;

#[async_trait]
impl WorkspaceInfoProvider for LocalWorkspaceInfoProvider {
    async fn branch(&self, cwd: &str) -> Option<String> {
        let cwd = PathBuf::from(cwd);
        tokio::task::spawn_blocking(move || read_branch(&cwd))
            .await
            .ok()
            .flatten()
    }
}

fn read_branch(cwd: &Path) -> Option<String> {
    // Missing working directories must not borrow an ancestor's repository.
    if !cwd.is_dir() {
        return None;
    }
    for directory in cwd.ancestors() {
        let marker = directory.join(".git");
        let metadata = match std::fs::metadata(&marker) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(_) => return None,
        };
        let git_directory = if metadata.is_dir() {
            marker
        } else if metadata.is_file() {
            let contents = read_small_file(&marker)?;
            let target = contents.trim().strip_prefix("gitdir: ")?.trim();
            if target.is_empty() {
                return None;
            }
            // Git uses paths relative to the worktree containing the .git file.
            directory.join(target)
        } else {
            return None;
        };
        // Stop at the nearest repository even if its HEAD is unavailable.
        return head_label(read_small_file(&git_directory.join("HEAD"))?.trim());
    }
    None
}

fn read_small_file(path: &Path) -> Option<String> {
    const MAX_BYTES: u64 = 8192;
    // Check before opening too: opening a FIFO can block before metadata reads.
    if !std::fs::metadata(path).ok()?.is_file() {
        return None;
    }
    let file = File::open(path).ok()?;
    if !file.metadata().ok()?.is_file() {
        return None;
    }
    let mut contents = String::new();
    file.take(MAX_BYTES + 1)
        .read_to_string(&mut contents)
        .ok()?;
    (contents.len() as u64 <= MAX_BYTES).then_some(contents)
}

fn head_label(head: &str) -> Option<String> {
    if let Some(branch) = head.strip_prefix("ref: refs/heads/") {
        if !branch.is_empty()
            && !branch
                .chars()
                .any(|character| character.is_whitespace() || character.is_control())
        {
            return Some(branch.into());
        }
    } else if matches!(head.len(), 40 | 64) && head.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Some(head[..7].into());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_head(root: &Path, head: &str) {
        std::fs::create_dir_all(root.join(".git")).unwrap();
        std::fs::write(root.join(".git/HEAD"), head).unwrap();
    }

    #[test]
    fn nested_working_directory_reads_branch_even_before_first_commit() {
        let root = tempfile::tempdir().unwrap();
        let nested = root.path().join("src/nested");
        std::fs::create_dir_all(&nested).unwrap();
        write_head(root.path(), "ref: refs/heads/codex/new-agent-ui\n");
        assert_eq!(read_branch(&nested).as_deref(), Some("codex/new-agent-ui"));
        std::fs::write(root.path().join(".git/HEAD"), "ref: refs/heads/main\n").unwrap();
        assert_eq!(read_branch(&nested).as_deref(), Some("main"));
        assert_eq!(read_branch(&nested.join("missing")), None);
    }

    #[test]
    fn linked_worktree_uses_its_own_head_and_supports_relative_gitdir() {
        let root = tempfile::tempdir().unwrap();
        write_head(root.path(), "ref: refs/heads/main\n");
        let worktree = root.path().join("linked tree");
        let git_directory = root.path().join(".git/worktrees/linked");
        std::fs::create_dir_all(&worktree).unwrap();
        std::fs::create_dir_all(&git_directory).unwrap();
        std::fs::write(worktree.join(".git"), "gitdir: ../.git/worktrees/linked\n").unwrap();
        std::fs::write(git_directory.join("HEAD"), "ref: refs/heads/codex/linked\n").unwrap();
        assert_eq!(read_branch(&worktree).as_deref(), Some("codex/linked"));
        std::fs::write(
            worktree.join(".git"),
            format!("gitdir: {}\n", git_directory.display()),
        )
        .unwrap();
        assert_eq!(read_branch(&worktree).as_deref(), Some("codex/linked"));
    }

    #[test]
    fn detached_head_returns_short_commit_and_unknown_metadata_is_absent() {
        let root = tempfile::tempdir().unwrap();
        assert_eq!(read_branch(root.path()), None);
        write_head(root.path(), "0123456789abcdef0123456789abcdef01234567\n");
        assert_eq!(read_branch(root.path()).as_deref(), Some("0123456"));
        assert_eq!(head_label(&"a".repeat(64)).as_deref(), Some("aaaaaaa"));
        for invalid in [
            "",
            "garbage",
            "ref: refs/tags/v1",
            "ref: refs/heads/",
            "ref: refs/heads/main\nother",
        ] {
            std::fs::write(root.path().join(".git/HEAD"), invalid).unwrap();
            assert_eq!(read_branch(root.path()), None);
        }
        std::fs::write(root.path().join(".git/HEAD"), "x".repeat(9000)).unwrap();
        assert_eq!(read_branch(root.path()), None);
    }

    #[test]
    fn broken_inner_repository_does_not_display_parent_branch() {
        let root = tempfile::tempdir().unwrap();
        write_head(root.path(), "ref: refs/heads/main\n");
        let nested = root.path().join("nested");
        std::fs::create_dir(&nested).unwrap();
        std::fs::write(nested.join(".git"), "gitdir: ../missing\n").unwrap();
        assert_eq!(read_branch(&nested), None);
    }
}
