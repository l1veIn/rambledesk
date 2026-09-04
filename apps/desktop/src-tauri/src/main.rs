#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if std::env::args_os()
        .nth(1)
        .is_some_and(|arg| arg == "managed-mcp-stdio")
    {
        std::process::exit(rambledesk_mcp::managed_stdio::run_process());
    }
    rambledesk_desktop_lib::run();
}
