use std::{
    io,
    mem::{size_of, zeroed},
    os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle},
    ptr::null,
    time::Duration,
};
use tokio::process::{Child, Command};
use windows_sys::Win32::{
    Foundation::{HANDLE, INVALID_HANDLE_VALUE},
    System::{
        Diagnostics::ToolHelp::{
            CreateToolhelp32Snapshot, TH32CS_SNAPTHREAD, THREADENTRY32, Thread32First, Thread32Next,
        },
        JobObjects::{
            AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
            SetInformationJobObject, TerminateJobObject,
        },
        Threading::{
            CREATE_NO_WINDOW, CREATE_SUSPENDED, OpenThread, ResumeThread, THREAD_SUSPEND_RESUME,
        },
    },
};

pub(super) struct Ownership {
    job: OwnedHandle,
}

impl Ownership {
    pub(super) fn spawn(mut command: Command) -> io::Result<(Child, Self)> {
        let ownership = Self::new()?;
        command.creation_flags(CREATE_NO_WINDOW | CREATE_SUSPENDED);
        let mut child = command.spawn()?;
        let result = ownership.attach_and_resume(&child);
        if let Err(error) = result {
            // Both cleanup paths use owned handles, including failed assignment.
            let _ = ownership.terminate_job();
            let _ = child.start_kill();
            return Err(error);
        }
        Ok((child, ownership))
    }

    fn new() -> io::Result<Self> {
        // SAFETY: null security/name create a private, non-inheritable job handle.
        let job = unsafe { owned_handle(CreateJobObjectW(null(), null()))? };
        // SAFETY: zero is a valid empty Win32 limit structure.
        let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { zeroed() };
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        // SAFETY: exact structure size and a live pointer for this call.
        let result = unsafe {
            SetInformationJobObject(
                job.as_raw_handle(),
                JobObjectExtendedLimitInformation,
                (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        };
        if result == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Self { job })
    }

    fn attach_and_resume(&self, child: &Child) -> io::Result<()> {
        let handle = child
            .raw_handle()
            .ok_or_else(|| io::Error::other("ACP child handle is unavailable"))?;
        // SAFETY: both handles remain owned. This suspended process cannot spawn
        // a descendant before entering the job.
        if unsafe { AssignProcessToJobObject(self.job.as_raw_handle(), handle) } == 0 {
            return Err(io::Error::last_os_error());
        }
        resume_primary_thread(
            child
                .id()
                .ok_or_else(|| io::Error::other("ACP child exited before assignment completed"))?,
        )
    }

    fn terminate_job(&self) -> io::Result<()> {
        // SAFETY: the job handle is owned, never reconstructed from an ID.
        if unsafe { TerminateJobObject(self.job.as_raw_handle(), 1) } == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    pub(super) fn terminate(&mut self) -> io::Result<()> {
        self.terminate_job()
    }

    pub(super) async fn wait_before_cleanup(
        &self,
        child: &mut Child,
        grace: Duration,
    ) -> io::Result<bool> {
        match tokio::time::timeout(grace, child.wait()).await {
            Ok(result) => result.map(|_| true),
            Err(_) => Ok(false),
        }
    }
}

/// Rust's process-attribute API is unstable. Suspended spawn and primary-thread
/// resume work on the stable MSRV. Enumeration only resumes this newly-created
/// process; it is never used to discover processes to terminate.
fn resume_primary_thread(process_id: u32) -> io::Result<()> {
    // SAFETY: own the snapshot until enumeration finishes.
    let snapshot = unsafe { owned_handle(CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0))? };
    // SAFETY: initialize the size required by Thread32First/Next.
    let mut entry: THREADENTRY32 = unsafe { zeroed() };
    entry.dwSize = size_of::<THREADENTRY32>() as u32;
    let mut present = unsafe { Thread32First(snapshot.as_raw_handle(), &mut entry) };
    while present != 0 {
        if entry.th32OwnerProcessID == process_id {
            // The suspended, owned process pins the primary-thread identity while
            // we acquire its handle. It cannot create extra threads before resume.
            let thread =
                unsafe { owned_handle(OpenThread(THREAD_SUSPEND_RESUME, 0, entry.th32ThreadID))? };
            if unsafe { ResumeThread(thread.as_raw_handle()) } == u32::MAX {
                return Err(io::Error::last_os_error());
            }
            return Ok(());
        }
        present = unsafe { Thread32Next(snapshot.as_raw_handle(), &mut entry) };
    }
    Err(io::Error::other("ACP primary thread was not found"))
}

/// SAFETY: the caller transfers a newly-created handle, never a borrowed handle.
unsafe fn owned_handle(handle: HANDLE) -> io::Result<OwnedHandle> {
    if handle.is_null() || handle == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: this function's contract transfers sole ownership.
    Ok(unsafe { OwnedHandle::from_raw_handle(handle) })
}
