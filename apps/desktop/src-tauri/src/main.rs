#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if rambledesk_feedback_client::process_requested() {
        std::process::exit(rambledesk_feedback_client::run_process());
    }
    rambledesk_desktop_lib::run();
}
