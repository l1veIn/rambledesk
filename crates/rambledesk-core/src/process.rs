//! Platform process discovery helpers shared by host installers.

use std::{
    ffi::{OsStr, OsString},
    path::PathBuf,
};

/// Locate a command on PATH, then in bounded conventional install directories.
/// Desktop processes do not always inherit the user's interactive shell PATH.
/// Never run shell profiles or search arbitrary directory trees during discovery.
pub fn find_executable(name: &str) -> Option<PathBuf> {
    find_with_fallbacks(
        name,
        std::env::var_os("PATH").as_deref(),
        &standard_directories(),
    )
}

fn find_with_fallbacks(name: &str, paths: Option<&OsStr>, fallback: &[PathBuf]) -> Option<PathBuf> {
    paths
        .and_then(|paths| find_executable_on_path(name, paths))
        .or_else(|| {
            fallback.iter().find_map(|directory| {
                executable_names(name)
                    .iter()
                    .map(|name| directory.join(name))
                    .find(|candidate| candidate.is_file())
            })
        })
}

fn standard_directories() -> Vec<PathBuf> {
    let mut directories = Vec::new();
    let user_home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
        .map(PathBuf::from)
        .filter(|path| path.is_absolute());
    if let Some(user_home) = user_home {
        directories.extend(
            [
                ".local/bin",
                ".npm-global/bin",
                ".volta/bin",
                ".bun/bin",
                ".cargo/bin",
                ".opencode/bin",
                ".cursor/bin",
                ".nvm/current/bin",
                ".fnm/aliases/default/bin",
                "bin",
            ]
            .into_iter()
            .map(|path| user_home.join(path)),
        );
        #[cfg(unix)]
        directories.push(user_home.join(".hermes/venv/bin"));
    }
    #[cfg(windows)]
    for (variable, relative) in [
        ("APPDATA", "npm"),
        ("LOCALAPPDATA", "Programs/nodejs"),
        ("LOCALAPPDATA", "Programs/cursor/resources/app/bin"),
        ("ProgramFiles", "nodejs"),
        ("NVM_SYMLINK", ""),
    ] {
        if let Some(base) = std::env::var_os(variable)
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
        {
            directories.push(base.join(relative));
        }
    }
    #[cfg(unix)]
    directories
        .extend(["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin", "/bin"].map(PathBuf::from));
    directories
}

pub fn find_executable_on_path(name: &str, paths: &OsStr) -> Option<PathBuf> {
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

    #[test]
    fn lookup_keeps_path_precedence_and_uses_explicit_fallback_without_path() {
        let root = tempfile::tempdir().unwrap();
        let first = root.path().join("first");
        let second = root.path().join("second");
        let fallback = root.path().join("fallback");
        for directory in [&first, &second, &fallback] {
            std::fs::create_dir(directory).unwrap();
            std::fs::write(directory.join(&executable_names("test-agent")[0]), "test").unwrap();
        }
        let paths = std::env::join_paths([&first, &second]).unwrap();
        assert_eq!(
            find_with_fallbacks("test-agent", Some(&paths), std::slice::from_ref(&fallback)),
            Some(first.join(&executable_names("test-agent")[0]))
        );
        assert_eq!(
            find_with_fallbacks("test-agent", None, std::slice::from_ref(&fallback)),
            Some(fallback.join(&executable_names("test-agent")[0]))
        );
        assert_eq!(find_with_fallbacks("unknown", None, &[fallback]), None);
    }

    #[cfg(windows)]
    #[test]
    fn windows_lookup_finds_command_scripts_and_native_executables() {
        let root = tempfile::tempdir().expect("temp dir");
        std::fs::write(root.path().join("pi.cmd"), "@echo off\n").expect("command script");
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
