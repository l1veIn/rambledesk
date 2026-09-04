use std::{io, mem::zeroed, time::Duration};
use tokio::process::{Child, Command};

pub(super) struct Ownership {
    group: Option<libc::pid_t>,
}

impl Ownership {
    pub(super) fn spawn(mut command: Command) -> io::Result<(Child, Self)> {
        command.process_group(0);
        let child = command.spawn()?;
        let group = child
            .id()
            .ok_or_else(|| io::Error::other("ACP process ID is unavailable"))?
            as libc::pid_t;
        Ok((child, Self { group: Some(group) }))
    }

    pub(super) fn terminate(&mut self) -> io::Result<()> {
        if let Some(group) = self.group {
            // SAFETY: only this wrapper can reap its leader, which therefore pins
            // this group ID until cleanup completes, even after the leader exits.
            let result = unsafe { libc::kill(-group, libc::SIGKILL) };
            if result != 0 {
                let error = io::Error::last_os_error();
                if error.raw_os_error() != Some(libc::ESRCH) {
                    return Err(error);
                }
            }
            self.group = None;
        }
        Ok(())
    }

    pub(super) async fn wait_before_cleanup(
        &self,
        _child: &mut Child,
        grace: Duration,
    ) -> io::Result<()> {
        let Some(group) = self.group else {
            return Ok(());
        };
        let deadline = tokio::time::Instant::now() + grace;
        loop {
            // WNOWAIT observes exit without reaping. Child::wait/try_wait would
            // release the PID before the final group signal, permitting PID reuse.
            let mut status: libc::siginfo_t = unsafe { zeroed() };
            let result = unsafe {
                libc::waitid(
                    libc::P_PID,
                    group as libc::id_t,
                    &mut status,
                    libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
                )
            };
            if result != 0 {
                let error = io::Error::last_os_error();
                if error.kind() != io::ErrorKind::Interrupted {
                    return Err(error);
                }
            } else if unsafe { status.si_pid() } != 0 {
                return Ok(());
            }
            if tokio::time::Instant::now() >= deadline {
                return Ok(());
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }
}
