fn main() {
    tauri_build::build();
    // tauri-build links the Windows manifest into app binaries only. Native
    // integration tests also need Common-Controls v6 (TaskDialogIndirect).
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc")
    {
        let resources =
            std::path::PathBuf::from(std::env::var_os("OUT_DIR").expect("build output"))
                .join("resource.lib");
        println!("cargo:rustc-link-arg-tests={}", resources.display());
    }
}
