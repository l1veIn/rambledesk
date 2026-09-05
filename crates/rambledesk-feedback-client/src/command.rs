use std::{
    io::{Read, Write},
    path::PathBuf,
};

use clap::{Parser, Subcommand};
use rambledesk_core::{GetFeedbackInput, ManagedFeedbackRecoverInput, ManagedFeedbackRequestInput};
use serde_json::Value;

use crate::{ClientError, MAX_INPUT_BYTES};

#[derive(Parser)]
#[command(
    name = "rambledesk feedback",
    about = "Communicate with the user in the current RambleDesk Agent session. Request once, end your turn; RambleDesk resumes you after feedback."
)]
struct Arguments {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Create a feedback request from JSON. Preserve its request_id for recovery.
    Request {
        /// JSON file, or - for standard input. Fields: what_happened, actions,
        /// optional request_id (UUID), title, context_refs, attachments, allow_finish, final_summary.
        #[arg(long, default_value = "-")]
        input: PathBuf,
    },
    /// Read this session's request, including its terminal feedback package.
    Get {
        #[arg(long)]
        request_id: String,
    },
    /// Recover the original request after interruption. Never creates a request.
    Recover {
        #[arg(long)]
        request_id: Option<String>,
    },
}

fn read_input(path: &PathBuf) -> Result<ManagedFeedbackRequestInput, ClientError> {
    let reader: Box<dyn Read> = if path.as_os_str() == "-" {
        Box::new(std::io::stdin())
    } else {
        Box::new(std::fs::File::open(path).map_err(|_| ClientError::InputUnavailable)?)
    };
    let mut bytes = Vec::new();
    reader
        .take((MAX_INPUT_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| ClientError::InputUnavailable)?;
    if bytes.len() > MAX_INPUT_BYTES {
        return Err(ClientError::InputUnavailable);
    }
    // Windows PowerShell's UTF-8 file writer may prefix a BOM.
    let bytes = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(&bytes);
    serde_json::from_slice(bytes).map_err(|_| ClientError::InvalidInput)
}

fn payload(command: Command) -> Result<(&'static str, Value, Option<String>), ClientError> {
    let (operation, value) = match command {
        Command::Request { input } => {
            let mut input = read_input(&input)?;
            // Allocate before I/O and return this ID even for uncertain failures.
            input
                .request_id
                .get_or_insert_with(|| uuid::Uuid::now_v7().to_string());
            ("request", serde_json::to_value(input))
        }
        Command::Get { request_id } => {
            ("get", serde_json::to_value(GetFeedbackInput { request_id }))
        }
        Command::Recover { request_id } => (
            "recover",
            serde_json::to_value(ManagedFeedbackRecoverInput { request_id }),
        ),
    };
    let value = value.map_err(|_| ClientError::InvalidInput)?;
    let id = value
        .get("request_id")
        .and_then(Value::as_str)
        .map(str::to_owned);
    Ok((operation, value, id))
}

fn output(success: bool, value: Value) -> i32 {
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    if serde_json::to_writer(&mut out, &value).is_err() || writeln!(out).is_err() {
        return 1;
    }
    if success { 0 } else { 1 }
}

pub(super) fn run() -> i32 {
    let args = Arguments::try_parse_from(
        std::iter::once(std::ffi::OsString::from("rambledesk feedback"))
            .chain(std::env::args_os().skip(2)),
    );
    let args = match args {
        Ok(args) => args,
        Err(error) if error.kind() == clap::error::ErrorKind::DisplayHelp => {
            let _ = error.print();
            return 0;
        }
        Err(_) => return output(false, ClientError::InvalidInput.json(None)),
    };
    // Validate the environment before touching input files. Never read external
    // token files or initialize desktop services when invoked as a command.
    let endpoint = match crate::endpoint_from_env() {
        Ok(endpoint) => endpoint,
        Err(error) => return output(false, error.json(None)),
    };
    let (operation, value, id) = match payload(args.command) {
        Ok(payload) => payload,
        Err(error) => return output(false, error.json(None)),
    };
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build();
    let result = runtime
        .map_err(|_| ClientError::RuntimeUnavailable)
        .and_then(|runtime| runtime.block_on(crate::call(&endpoint, operation, &value)));
    match result {
        Ok((success, mut result)) => {
            if !success && let Some(id) = id {
                result["request_id"] = id.into();
            }
            output(success, result)
        }
        Err(error) => output(false, error.json(id.as_deref())),
    }
}
