use super::*;
use std::{path::Path, time::Duration};

fn read(request: &str, session: &str, after: Option<u64>) -> ReadTerminalSessionInput {
    ReadTerminalSessionInput {
        request_id: request.into(),
        session_id: session.into(),
        after_sequence: after,
    }
}

async fn until(
    manager: &TerminalSessionManager,
    request: &str,
    session: &str,
    expected: &str,
) -> TerminalSessionSnapshot {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let snapshot = manager.read(&read(request, session, None)).unwrap();
            if snapshot.output.contains(expected) {
                break snapshot;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .unwrap_or_else(|_| {
        panic!(
            "PTY did not produce {expected:?}: {:?}",
            manager.read(&read(request, session, None))
        )
    })
}

async fn initial_cursor(manager: &TerminalSessionManager, request: &str, session: &str) {
    if cfg!(windows) {
        until(manager, request, session, "\x1b[6n").await;
        manager
            .write(WriteTerminalSessionInput {
                request_id: request.into(),
                session_id: session.into(),
                data: "\x1b[1;1R".into(),
            })
            .await
            .unwrap();
    }
}

fn shell_command(cwd: &Path) -> String {
    let node = crate::find_executable("node")
        .expect("Node is required by the existing subprocess test suite");
    let fixture = cwd.join("terminal-fixture.cjs");
    std::fs::write(&fixture, r#"
process.stdout.write('\x1b[31mREADY\x1b[0m\r\n');
process.stdout.write('终端输出');
process.stdin.setRawMode(true);
process.stdin.resume();
process.stdin.on('data',data=>{
  if(data[0]===3){process.stdout.write('\r\nINTERRUPTED\r\n');process.exit(7)}
  else if(data[0]===66){
    require('node:child_process').spawn(process.execPath,['-e',`const fs=require('node:fs');setInterval(()=>fs.writeFileSync('heartbeat',String(Date.now())),10)`],{stdio:'inherit',windowsHide:true});
    process.stdout.write('\r\nCHILD_STARTED\r\n');
  }
  else process.stdout.write('\r\nKEY:'+data[0]+'\r\n');
});
"#).unwrap();
    #[cfg(windows)]
    {
        format!("\"{}\" \"{}\"\r", node.display(), fixture.display())
    }
    #[cfg(not(windows))]
    {
        format!("'{}' '{}'\r", node.display(), fixture.display())
    }
}

#[test]
fn output_buffer_preserves_split_utf8_and_ansi_then_bounds_replay() {
    let mut output = buffer::OutputBuffer::default();
    let text = "\x1b[31m终端\x1b[0m";
    for byte in text.as_bytes() {
        output.append(&[*byte], false);
    }
    assert_eq!(output.read(None).unwrap(), (text.into(), 0));
    assert!(
        output.read(Some(6)).is_err(),
        "cursor inside Chinese UTF-8 must be rejected"
    );
    assert_eq!(output.read(Some(5)).unwrap().0, "终端\x1b[0m");
    output.append(b"\0\xff", false);
    assert!(!output.text.contains('\0'));
    assert!(output.text.ends_with('\u{fffd}'));
    output.append("中".repeat(MAX_TERMINAL_OUTPUT_BYTES).as_bytes(), false);
    let (tail, first) = output.read(Some(0)).unwrap();
    assert!(output.truncated);
    assert!(tail.len() <= MAX_TERMINAL_OUTPUT_BYTES);
    assert!(first > 0);
    assert_eq!(first + tail.len() as u64, output.next);
    assert!(output.read(Some(output.next + 1)).is_err());
}

#[tokio::test]
async fn real_pty_handles_input_resize_interrupt_and_reconnection_without_respawn() {
    let directory = tempfile::tempdir().unwrap();
    let manager = TerminalSessionManager::default();
    let opened = manager
        .open(
            "request-a".into(),
            directory.path().to_string_lossy().into(),
            None,
            80,
            24,
        )
        .await
        .unwrap();
    let session = opened.session_id.clone();
    initial_cursor(&manager, "request-a", &session).await;
    manager
        .write(WriteTerminalSessionInput {
            request_id: "request-a".into(),
            session_id: session.clone(),
            data: shell_command(directory.path()),
        })
        .await
        .unwrap();
    let started = until(&manager, "request-a", &session, "终端输出").await;
    assert!(started.output.contains("\x1b[31m"));
    let clone = manager.clone();
    let reopened = clone
        .open(
            "request-a".into(),
            directory.path().to_string_lossy().into(),
            None,
            120,
            40,
        )
        .await
        .unwrap();
    assert_eq!(reopened.session_id, session);
    assert!(reopened.output.contains("终端输出"));
    let resized = clone
        .resize(ResizeTerminalSessionInput {
            request_id: "request-a".into(),
            session_id: session.clone(),
            cols: 110,
            rows: 31,
        })
        .await
        .unwrap();
    assert_eq!((resized.cols, resized.rows), (110, 31));
    manager
        .write(WriteTerminalSessionInput {
            request_id: "request-a".into(),
            session_id: session.clone(),
            data: "A".into(),
        })
        .await
        .unwrap();
    let after = until(&manager, "request-a", &session, "KEY:65").await;
    let delta = manager
        .read(&read("request-a", &session, Some(started.next_sequence)))
        .unwrap();
    assert_eq!(delta.first_sequence, started.next_sequence);
    assert_eq!(delta.next_sequence, after.next_sequence);
    assert!(delta.output.contains("KEY:65"));
    manager
        .write(WriteTerminalSessionInput {
            request_id: "request-a".into(),
            session_id: session.clone(),
            data: "\x03".into(),
        })
        .await
        .unwrap();
    until(&manager, "request-a", &session, "INTERRUPTED").await;
    assert!(
        manager
            .read(&read("another-request", &session, None))
            .is_err()
    );
    assert!(
        manager
            .read(&read("request-a", "other-session", None))
            .is_err()
    );
    let stopped = manager
        .stop(TerminalSessionInput {
            request_id: "request-a".into(),
            session_id: session.clone(),
        })
        .await
        .unwrap();
    assert_eq!(stopped.status, TerminalSessionStatus::Stopped);
    assert!(stopped.output.contains("INTERRUPTED"));
    assert!(stopped.exit_code.is_some(), "stop must reap the shell");
    assert!(
        manager
            .write(WriteTerminalSessionInput {
                request_id: "request-a".into(),
                session_id: session,
                data: "secret-do-not-record".into()
            })
            .await
            .is_err()
    );
    assert!(!stopped.output.contains("secret-do-not-record"));
}

#[tokio::test]
async fn terminal_request_completion_stops_live_process_and_prevents_reopening() {
    let directory = tempfile::tempdir().unwrap();
    let manager = TerminalSessionManager::default();
    let opened = manager
        .open(
            "request".into(),
            directory.path().to_string_lossy().into(),
            None,
            80,
            24,
        )
        .await
        .unwrap();
    initial_cursor(&manager, "request", &opened.session_id).await;
    manager.stop_request("request");
    let stopped = manager
        .stop(TerminalSessionInput {
            request_id: "request".into(),
            session_id: opened.session_id.clone(),
        })
        .await
        .unwrap();
    assert_eq!(stopped.status, TerminalSessionStatus::Stopped);
    assert!(stopped.exit_code.is_some());
    assert!(
        manager
            .open(
                "request".into(),
                directory.path().to_string_lossy().into(),
                None,
                80,
                24
            )
            .await
            .is_err()
    );
    assert!(
        manager
            .resize(ResizeTerminalSessionInput {
                request_id: "request".into(),
                session_id: opened.session_id,
                cols: 80,
                rows: 24
            })
            .await
            .is_err()
    );
}

#[tokio::test]
async fn rejects_invalid_sizes_missing_directories_and_large_input() {
    let manager = TerminalSessionManager::default();
    assert!(
        manager
            .open("request".into(), "relative".into(), None, 80, 24)
            .await
            .is_err()
    );
    assert!(
        manager
            .open("request".into(), "relative".into(), None, 0, 0)
            .await
            .is_err()
    );
    let error = manager
        .write(WriteTerminalSessionInput {
            request_id: "request".into(),
            session_id: "session".into(),
            data: "x".repeat(MAX_TERMINAL_INPUT_BYTES + 1),
        })
        .await
        .unwrap_err();
    assert!(error.message().contains("16384"));
}

#[tokio::test]
async fn naturally_exited_shell_publishes_large_final_output_before_exit_status() {
    let directory = tempfile::tempdir().unwrap();
    let manager = TerminalSessionManager::default();
    let node = crate::find_executable("node").unwrap();
    let fixture = directory.path().join("terminal-final-output.cjs");
    std::fs::write(&fixture, r#"
const block = ('log-'+ 'x'.repeat(120) +'\r\n').repeat(4096);
process.stdout.write(block,()=>process.stdout.write('\r\nFINAL_终端_正常退出\r\n',()=>process.exit(0)));
"#).unwrap();
    let opened = manager
        .open(
            "request".into(),
            directory.path().to_string_lossy().into(),
            None,
            80,
            24,
        )
        .await
        .unwrap();
    initial_cursor(&manager, "request", &opened.session_id).await;
    #[cfg(windows)]
    let command = format!(
        "\"{}\" \"{}\" & exit 23\r",
        node.display(),
        fixture.display()
    );
    #[cfg(unix)]
    let command = format!("'{}' '{}'; exit 23\r", node.display(), fixture.display());
    manager
        .write(WriteTerminalSessionInput {
            request_id: "request".into(),
            session_id: opened.session_id.clone(),
            data: command,
        })
        .await
        .unwrap();
    let mut cursor = 0;
    let mut tail = String::new();
    let exited = tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            let next = manager
                .read(&read("request", &opened.session_id, Some(cursor)))
                .unwrap();
            if next.first_sequence > cursor {
                tail.clear();
            }
            tail.push_str(&next.output);
            cursor = next.next_sequence;
            if next.status == TerminalSessionStatus::Exited {
                break next;
            }
            assert_eq!(
                next.exit_code, None,
                "a draining shell must not publish its exit code prematurely"
            );
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    })
    .await
    .expect("normal shell exit with large output");
    assert_eq!(exited.exit_code, Some(23));
    assert!(
        tail.contains("FINAL_终端_正常退出"),
        "the first exited snapshot must contain every final byte"
    );
    let full = manager
        .stop(TerminalSessionInput {
            request_id: "request".into(),
            session_id: opened.session_id.clone(),
        })
        .await
        .unwrap();
    assert_eq!(
        full.status,
        TerminalSessionStatus::Exited,
        "draining a naturally exited shell must retain its exit status"
    );
    assert_eq!(full.exit_code, Some(23));
    assert!(full.output.contains("FINAL_终端_正常退出"));
    assert!(full.output.len() <= MAX_TERMINAL_OUTPUT_BYTES);
    assert!(
        full.truncated,
        "the fixture exceeds the bounded capture limit"
    );
    assert_eq!(full.next_sequence, exited.next_sequence);
    let last = manager
        .read(&read("request", &opened.session_id, Some(cursor)))
        .unwrap();
    assert!(
        last.output.is_empty(),
        "no output can arrive after publishing Exited"
    );
}

#[cfg(windows)]
#[tokio::test]
async fn conpty_stop_and_application_drop_drain_without_any_frontend_cursor_reply() {
    for drop_application in [false, true] {
        let directory = tempfile::tempdir().unwrap();
        let manager = TerminalSessionManager::default();
        let opened = manager
            .open(
                "request".into(),
                directory.path().to_string_lossy().into(),
                None,
                80,
                24,
            )
            .await
            .unwrap();
        // Deliberately do not call initial_cursor() or write any frontend input.
        let runtime = manager.session("request", &opened.session_id).unwrap();
        let finished = tokio::time::timeout(Duration::from_secs(3), async {
            if drop_application {
                drop(manager);
            }
            runtime.stop().await.unwrap()
        })
        .await
        .expect("unanswered ConPTY teardown must complete");
        assert_eq!(finished.status, TerminalSessionStatus::Stopped);
        assert!(
            finished.exit_code.is_some(),
            "the shell must actually be reaped"
        );
        assert!(
            !finished.truncated,
            "the output reader must finish, rather than time out during cursor inheritance"
        );
    }
}

#[tokio::test]
async fn missing_shell_open_fails_promptly_and_preserves_the_request_for_retry() {
    let directory = tempfile::tempdir().unwrap();
    let manager = TerminalSessionManager::default();
    let missing = directory.path().join("missing-shell-executable");
    let result = tokio::time::timeout(
        Duration::from_secs(3),
        manager.open(
            "request".into(),
            directory.path().to_string_lossy().into(),
            Some(missing.to_string_lossy().into()),
            80,
            24,
        ),
    )
    .await
    .expect("invalid shell open must return promptly");
    assert!(result.is_err());
    assert!(
        manager.inner.registry.lock().unwrap().sessions.is_empty(),
        "failed shells must not occupy a request binding"
    );
    let opened = manager
        .open(
            "request".into(),
            directory.path().to_string_lossy().into(),
            None,
            80,
            24,
        )
        .await
        .unwrap();
    let stopped = manager
        .stop(TerminalSessionInput {
            request_id: "request".into(),
            session_id: opened.session_id,
        })
        .await
        .unwrap();
    assert!(!stopped.truncated);
    assert!(stopped.exit_code.is_some());
}

#[cfg(windows)]
#[tokio::test]
async fn failed_conpty_spawn_cleanup_closes_and_drains_its_reader_without_a_frontend() {
    use portable_pty::{CommandBuilder, PtySize, native_pty_system};
    let directory = tempfile::tempdir().unwrap();
    let pair = native_pty_system()
        .openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })
        .unwrap();
    let missing = directory.path().join("missing-shell-executable");
    assert!(
        pair.slave
            .spawn_command(CommandBuilder::new(missing))
            .is_err()
    );
    let cleanup = runtime::cleanup_failed_pair(pair, None, None);
    tokio::time::timeout(
        Duration::from_secs(3),
        tokio::task::spawn_blocking(move || cleanup.join().unwrap()),
    )
    .await
    .expect("failed ConPTY close and output drain must finish")
    .unwrap();
}

#[tokio::test]
async fn stop_and_application_drop_terminate_shell_descendants() {
    for drop_application in [false, true] {
        let directory = tempfile::tempdir().unwrap();
        let manager = TerminalSessionManager::default();
        let opened = manager
            .open(
                "request".into(),
                directory.path().to_string_lossy().into(),
                None,
                80,
                24,
            )
            .await
            .unwrap();
        initial_cursor(&manager, "request", &opened.session_id).await;
        manager
            .write(WriteTerminalSessionInput {
                request_id: "request".into(),
                session_id: opened.session_id.clone(),
                data: shell_command(directory.path()),
            })
            .await
            .unwrap();
        until(&manager, "request", &opened.session_id, "终端输出").await;
        manager
            .write(WriteTerminalSessionInput {
                request_id: "request".into(),
                session_id: opened.session_id.clone(),
                data: "B".into(),
            })
            .await
            .unwrap();
        until(&manager, "request", &opened.session_id, "CHILD_STARTED").await;
        let heartbeat = directory.path().join("heartbeat");
        tokio::time::timeout(Duration::from_secs(5), async {
            while !heartbeat.is_file() {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .expect("descendant heartbeat");
        if drop_application {
            drop(manager);
        } else {
            manager
                .stop(TerminalSessionInput {
                    request_id: "request".into(),
                    session_id: opened.session_id,
                })
                .await
                .unwrap();
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
        let frozen = std::fs::read(&heartbeat).unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert_eq!(
            std::fs::read(&heartbeat).unwrap(),
            frozen,
            "descendants must stop with their terminal owner"
        );
    }
}
