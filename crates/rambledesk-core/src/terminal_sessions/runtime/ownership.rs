#[cfg(windows)]
use std::{
    io,
    mem::{size_of, zeroed},
    os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle},
    ptr::null,
};
#[cfg(windows)]
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
    SetInformationJobObject, TerminateJobObject,
};

pub(super) struct Ownership {
    terminated: bool,
    shell_has_exited: bool,
    #[cfg(windows)]
    job: OwnedHandle,
    #[cfg(unix)]
    killer: Box<dyn portable_pty::ChildKiller + Send + Sync>,
    #[cfg(unix)]
    master: Option<std::os::fd::OwnedFd>,
}

impl Ownership {
    pub fn new(
        child: &dyn portable_pty::Child,
        master: &dyn portable_pty::MasterPty,
    ) -> std::io::Result<Self> {
        #[cfg(windows)]
        {
            let _ = master;
            // SAFETY: no name or security descriptor creates a private owned job.
            let raw = unsafe { CreateJobObjectW(null(), null()) };
            if raw.is_null() {
                return Err(io::Error::last_os_error());
            }
            // SAFETY: ownership of this newly-created handle transfers exactly once.
            let job = unsafe { OwnedHandle::from_raw_handle(raw) };
            // SAFETY: zero is a valid empty job-limit structure.
            let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { zeroed() };
            limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            // SAFETY: the owned job and initialized structure remain live for both calls.
            if unsafe {
                SetInformationJobObject(
                    job.as_raw_handle(),
                    JobObjectExtendedLimitInformation,
                    (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                    size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                )
            } == 0
            {
                return Err(io::Error::last_os_error());
            }
            let handle = child
                .as_raw_handle()
                .ok_or_else(|| io::Error::other("PTY child handle is unavailable"))?;
            // Only this freshly-spawned shell is assigned; descendants inherit it.
            if unsafe { AssignProcessToJobObject(job.as_raw_handle(), handle) } == 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(Self {
                job,
                terminated: false,
                shell_has_exited: false,
            })
        }
        #[cfg(unix)]
        {
            use std::os::fd::FromRawFd;
            let fd = master
                .as_raw_fd()
                .ok_or_else(|| std::io::Error::other("PTY master is unavailable"))?;
            // SAFETY: duplicate a borrowed, live descriptor into an owned handle.
            let duplicated = unsafe { libc::dup(fd) };
            if duplicated < 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(Self {
                killer: child.clone_killer(),
                master: Some(unsafe { std::os::fd::OwnedFd::from_raw_fd(duplicated) }),
                terminated: false,
                shell_has_exited: false,
            })
        }
    }

    pub fn terminate(&mut self) -> std::io::Result<()> {
        if self.terminated {
            return Ok(());
        }
        self.terminated = true;
        #[cfg(windows)]
        {
            // SAFETY: this handle refers only to our private owned process job.
            if unsafe { TerminateJobObject(self.job.as_raw_handle(), 1) } == 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        }
        #[cfg(unix)]
        {
            use std::os::fd::AsRawFd;
            // SAFETY: foreground group is read from our owned PTY, never from
            // caller input. Do not signal the application's own process group.
            if let Some(master) = self.master.take() {
                let foreground = unsafe { libc::tcgetpgrp(master.as_raw_fd()) };
                if !self.shell_has_exited
                    && foreground > 0
                    && foreground != unsafe { libc::getpgrp() }
                {
                    unsafe {
                        libc::kill(-foreground, libc::SIGKILL);
                    }
                }
            }
            if !self.shell_has_exited {
                self.killer.kill()
            } else {
                Ok(())
            }
        }
    }

    pub fn shell_exited(&mut self) {
        self.shell_has_exited = true;
    }
}

pub(super) fn wait(
    child: &mut dyn portable_pty::Child,
    ownership: &std::sync::Mutex<Ownership>,
) -> std::io::Result<portable_pty::ExitStatus> {
    #[cfg(windows)]
    {
        let result = child.wait();
        ownership
            .lock()
            .expect("terminal process ownership")
            .shell_exited();
        result
    }
    #[cfg(unix)]
    loop {
        {
            let mut ownership = ownership.lock().expect("terminal process ownership");
            // Reaping and clearing the signal target share the termination lock,
            // so stop cannot send a signal after the shell PID becomes reusable.
            if let Some(status) = child.try_wait()? {
                ownership.shell_exited();
                return Ok(status);
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}
