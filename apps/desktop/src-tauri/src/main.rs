#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if rambledesk_feedback_client::process_requested() {
        std::process::exit(rambledesk_feedback_client::run_process());
    }
    if rambledesk_acp::pi_wrapper::process_requested() {
        std::process::exit(rambledesk_acp::pi_wrapper::run_process());
    }
    if std::env::args_os()
        .nth(1)
        .is_some_and(|arg| arg == "managed-mcp-stdio")
    {
        std::process::exit(rambledesk_mcp::managed_stdio::run_process());
    }
    rambledesk_desktop_lib::run();
}
