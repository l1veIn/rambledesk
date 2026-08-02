use std::{
    ffi::{OsStr, OsString},
    path::PathBuf,
};

/// Locate a command using the current process PATH and platform executable rules.
pub(crate) fn find_executable(name: &str) -> Option<PathBuf> {
    std::env::var_os("PATH").and_then(|paths| find_executable_on_path(name, &paths))
}

pub(crate) fn find_executable_on_path(name: &str, paths: &OsStr) -> Option<PathBuf> {
    let names = executable_names(name);
    std::env::split_paths(paths).find_map(|directory| {
        names
            .iter()
            .map(|candidate| directory.join(candidate))
            .find(|candidate| candidate.is_file())
    })
}

#[cfg(windows)]
fn executable_names(name: &str) -> Vec<OsString> {
    if std::path::Path::new(name).extension().is_some() {
        return vec![OsString::from(name)];
    }

    // npm installs an extensionless POSIX shim next to its Windows shims.
    // CreateProcess cannot execute that file, so only consider native Windows
    // executable and command-script extensions here.
    ["exe", "com", "cmd", "bat"]
        .into_iter()
        .map(|extension| OsString::from(format!("{name}.{extension}")))
        .collect()
}

#[cfg(not(windows))]
fn executable_names(name: &str) -> Vec<OsString> {
    vec![OsString::from(name)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn windows_lookup_ignores_extensionless_npm_shims() {
        let root = tempfile::tempdir().expect("temp dir");
        std::fs::write(root.path().join("pi"), "#!/bin/sh\n").expect("POSIX shim");
        std::fs::write(root.path().join("pi.cmd"), "@echo off\r\n").expect("Windows shim");
        let paths = std::env::join_paths([root.path()]).expect("PATH");

        assert_eq!(
            find_executable_on_path("pi", &paths),
            Some(root.path().join("pi.cmd"))
        );
    }

    #[cfg(not(windows))]
    #[test]
    fn unix_lookup_uses_the_unextended_command_name() {
        let root = tempfile::tempdir().expect("temp dir");
        std::fs::write(root.path().join("pi"), "#!/bin/sh\n").expect("command");
        let paths = std::env::join_paths([root.path()]).expect("PATH");

        assert_eq!(
            find_executable_on_path("pi", &paths),
            Some(root.path().join("pi"))
        );
    }
}
