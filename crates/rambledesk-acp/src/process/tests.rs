use super::*;
use std::{
    io::{Read, Write},
    sync::Arc,
};
use tokio::io::{AsyncBufReadExt, BufReader};

const FIXTURE: &str = "process::tests::fixture_process";
const MODE: &str = "RAMBLEDESK_ACP_TEST_PROCESS_MODE";

/// The Rust test binary doubles as its own controlled process fixture. No Node,
/// shell, global agent configuration, or existing process is involved.
#[test]
#[ignore = "only started as a supervised subprocess by these tests"]
fn fixture_process() {
    let Ok(mode) = std::env::var(MODE) else {
        return;
    };
    if mode == "leaf" {
        loop {
            std::thread::sleep(Duration::from_secs(1));
        }
    }
    if mode == "noisy" {
        std::io::stderr()
            .write_all(&vec![b'x'; 512 * 1024])
            .unwrap();
    }
    let mut command = std::process::Command::new(std::env::current_exe().unwrap());
    command
        .args(["--exact", FIXTURE, "--ignored", "--nocapture"])
        .env(MODE, "leaf")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let descendant = command.spawn().unwrap();

    println!("descendant={}", descendant.id());
    std::io::stdout().flush().unwrap();
    if mode == "stubborn" {
        loop {
            std::thread::sleep(Duration::from_secs(1));
        }
    }
    let _ = std::io::stdin().read_to_end(&mut Vec::new());
    // Deliberately leave the descendant alive: the ownership guard must collect it.
    std::process::exit(0);
}

struct Running {
    process: OwnedProcess,
    root: Observation,
    descendant: Observation,
    drain: tokio::task::JoinHandle<()>,
    _cwd: tempfile::TempDir,
}

async fn tree(mode: &str) -> Running {
    let cwd = tempfile::tempdir().unwrap();
    let mut process = spawn(
        std::env::current_exe().unwrap().to_str().unwrap(),
        &[
            "--exact".into(),
            FIXTURE.into(),
            "--ignored".into(),
            "--nocapture".into(),
        ],
        &BTreeMap::from([(MODE.into(), mode.into())]),
        cwd.path(),
    )
    .unwrap();
    let root = Observation::new(process.id().unwrap());
    let drain = tokio::spawn(drain_stderr(process.take_stderr().unwrap()));
    let mut lines = BufReader::new(process.take_stdout().unwrap()).lines();
    let descendant = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let line = lines
                .next_line()
                .await
                .unwrap()
                .expect("fixture exited before ready");
            if let Some(pid) = line.strip_prefix("descendant=") {
                break Observation::new(pid.parse().unwrap());
            }
        }
    })
    .await
    .expect("fixture did not become ready");
    Running {
        process,
        root,
        descendant,
        drain,
        _cwd: cwd,
    }
}

#[tokio::test]
async fn normal_eof_reaps_leader_and_lingering_descendant_while_draining_stderr() {
    let mut running = tree("noisy").await;
    drop(running.process.take_stdin());
    running
        .process
        .reap_with_grace(Duration::from_secs(2))
        .await
        .unwrap();
    assert_stopped(&running.root).await;
    assert_stopped(&running.descendant).await;
    tokio::time::timeout(Duration::from_secs(2), running.drain)
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn timeout_kills_owned_tree_without_affecting_another_instance() {
    let mut first = tree("stubborn").await;
    let mut second = tree("tree").await;
    first
        .process
        .reap_with_grace(Duration::from_millis(100))
        .await
        .unwrap();
    assert_stopped(&first.root).await;
    assert_stopped(&first.descendant).await;
    assert!(second.root.running());
    assert!(second.descendant.running());
    // Repeating cleanup must be safe even after the OS has released the leader.
    first.process.kill_and_reap().await.unwrap();
    assert!(second.root.running());
    second.process.kill_and_reap().await.unwrap();
    assert_stopped(&second.descendant).await;
    first.drain.await.unwrap();
    second.drain.await.unwrap();
}

#[tokio::test]
async fn dropping_owner_terminates_the_entire_tree() {
    let running = tree("stubborn").await;
    drop(running.process);
    assert_stopped(&running.root).await;
    assert_stopped(&running.descendant).await;
    running.drain.await.unwrap();
}

#[tokio::test]
async fn failed_spawn_and_invalid_directory_are_reported() {
    let cwd = tempfile::tempdir().unwrap();
    assert!(
        spawn(
            cwd.path().join("missing-agent.exe").to_str().unwrap(),
            &[],
            &BTreeMap::new(),
            cwd.path()
        )
        .is_err()
    );
    assert!(matches!(
        spawn(
            "irrelevant",
            &[],
            &BTreeMap::new(),
            Path::new("relative-directory")
        ),
        Err(AcpError::InvalidLaunch(_))
    ));
}

async fn protocol_tree(
    mode: &str,
) -> (
    tokio::task::JoinHandle<Result<crate::AcpConnection, AcpError>>,
    crate::AcpLaunch,
    Observation,
    Observation,
    tempfile::TempDir,
) {
    let cwd = tempfile::tempdir().unwrap();
    let pid_file = cwd.path().join("owned-processes.txt");
    let fixture = cwd.path().join("fixture.mjs");
    std::fs::write(&fixture, include_str!("fixture.mjs")).unwrap();
    let launch = crate::AcpLaunch {
        command: rambledesk_core::find_executable("node")
            .expect("Node.js is required for ACP protocol fixtures")
            .to_str()
            .unwrap()
            .into(),
        args: vec![fixture.to_str().unwrap().into()],
        env: BTreeMap::from([
            (MODE.into(), mode.into()),
            (
                "RAMBLEDESK_ACP_TEST_PID_FILE".into(),
                pid_file.to_str().unwrap().into(),
            ),
        ]),
        cwd: cwd.path().into(),
        mcp_servers: vec![],
    };
    let copied = launch.clone();
    let connecting =
        tokio::spawn(async move { crate::AcpConnection::connect(&copied, Arc::new(|_| {})).await });
    let ids = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if let Ok(text) = std::fs::read_to_string(&pid_file) {
                let values: Vec<u32> = text
                    .split_whitespace()
                    .filter_map(|x| x.parse().ok())
                    .collect();
                if values.len() == 2 {
                    break values;
                }
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    let root = Observation::new(ids[0]);
    let descendant = Observation::new(ids[1]);
    std::fs::write(format!("{}.ready", pid_file.display()), "ready").unwrap();
    (connecting, launch, root, descendant, cwd)
}

#[tokio::test]
async fn initialize_failure_releases_the_newly_owned_tree() {
    let (connecting, _, root, descendant, _cwd) = protocol_tree("protocol-init-error").await;
    assert!(connecting.await.unwrap().is_err());
    assert_stopped(&root).await;
    assert_stopped(&descendant).await;
}

#[tokio::test]
async fn close_error_and_unresponsive_close_still_release_owned_resources() {
    for mode in ["protocol-close-error", "protocol-close-hang"] {
        let (connecting, launch, root, descendant, _cwd) = protocol_tree(mode).await;
        let connection = connecting.await.unwrap().unwrap();
        connection.open_session(&launch, None).await.unwrap();
        assert!(connection.shutdown().await.is_err());
        assert_stopped(&root).await;
        assert_stopped(&descendant).await;
    }
}

async fn assert_stopped(observation: &Observation) {
    tokio::time::timeout(Duration::from_secs(5), async {
        while observation.running() {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("owned process is still running");
}

#[cfg(windows)]
struct Observation(std::os::windows::io::OwnedHandle);

#[cfg(windows)]
impl Observation {
    fn new(pid: u32) -> Self {
        use std::os::windows::io::FromRawHandle;
        use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_SYNCHRONIZE};
        // The fixture supplied this fresh live PID; keep its handle so later
        // assertions cannot accidentally observe a recycled PID.
        let handle = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, 0, pid) };
        assert!(!handle.is_null());
        Self(unsafe { std::os::windows::io::OwnedHandle::from_raw_handle(handle) })
    }
    fn running(&self) -> bool {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::{
            Foundation::WAIT_TIMEOUT, System::Threading::WaitForSingleObject,
        };
        unsafe { WaitForSingleObject(self.0.as_raw_handle(), 0) == WAIT_TIMEOUT }
    }
}

#[cfg(unix)]
struct Observation(u32);

#[cfg(unix)]
impl Observation {
    fn new(pid: u32) -> Self {
        Self(pid)
    }
    fn running(&self) -> bool {
        #[cfg(target_os = "linux")]
        if let Ok(stat) = std::fs::read_to_string(format!("/proc/{}/stat", self.0)) {
            // A reparented descendant may briefly remain a zombie until init reaps
            // it. It can no longer execute, so it is not a surviving process.
            if stat
                .rsplit_once(") ")
                .is_some_and(|(_, tail)| tail.starts_with('Z'))
            {
                return false;
            }
        }
        unsafe { libc::kill(self.0 as libc::pid_t, 0) == 0 }
    }
}
