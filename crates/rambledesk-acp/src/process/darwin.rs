//! Darwin killpg skips zombies and can report EPERM for an already exited group.
//! Only the owning wrapper calls this before reaping its pinned group leader.
use std::{
    collections::HashSet,
    io,
    mem::{size_of, zeroed},
};

// Apple SDK <sys/proc_info.h>; proc_listpids returns a byte count, including zombies.
const PROC_PGRP_ONLY: u32 = 2;

pub(crate) fn no_live_members(group: libc::pid_t) -> io::Result<bool> {
    if !is_zombie(group, group)? {
        return Ok(false);
    }
    let mut members = group_members(group)?;
    for _ in 0..4 {
        let mut exited = HashSet::new();
        for pid in members {
            if !is_zombie(pid, group)? {
                return Ok(false);
            }
            exited.insert(pid);
        }
        // A member could fork between the first enumeration and becoming a
        // zombie. Require a second complete enumeration with no unseen member.
        members = group_members(group)?;
        if members.iter().all(|pid| exited.contains(pid)) {
            return Ok(true);
        }
    }
    Err(io::Error::other(
        "ACP process group membership did not settle",
    ))
}

fn group_members(group: libc::pid_t) -> io::Result<Vec<libc::pid_t>> {
    for capacity in [32, 128, 512, 2048, 8192] {
        let mut pids = vec![0; capacity];
        let bytes = (pids.len() * size_of::<libc::pid_t>()) as libc::c_int;
        // libproc returns zero for both an empty group and an error. Clearing
        // this thread's errno preserves the distinction; any query failure errs.
        let count = unsafe {
            *libc::__error() = 0;
            libc::proc_listpids(
                PROC_PGRP_ONLY,
                group as u32,
                pids.as_mut_ptr().cast(),
                bytes,
            )
        };
        if count < 0 || (count == 0 && io::Error::last_os_error().raw_os_error() != Some(0)) {
            return Err(io::Error::last_os_error());
        }
        if count >= bytes {
            continue;
        }
        if !(count as usize).is_multiple_of(size_of::<libc::pid_t>()) {
            return Err(io::Error::other(
                "ACP process group query returned invalid data",
            ));
        }
        pids.truncate(count as usize / size_of::<libc::pid_t>());
        if pids.iter().any(|pid| *pid <= 0) {
            return Err(io::Error::other(
                "ACP process group query returned an invalid identity",
            ));
        }
        return Ok(pids);
    }
    Err(io::Error::other(
        "ACP process group query exceeded its limit",
    ))
}

fn is_zombie(pid: libc::pid_t, group: libc::pid_t) -> io::Result<bool> {
    let mut info: libc::proc_bsdshortinfo = unsafe { zeroed() };
    let size = size_of::<libc::proc_bsdshortinfo>() as libc::c_int;
    // Apple proc_info.c explicitly enables zombie lookup when arg is nonzero.
    let count = unsafe {
        libc::proc_pidinfo(
            pid,
            libc::PROC_PIDT_SHORTBSDINFO,
            1,
            (&mut info as *mut libc::proc_bsdshortinfo).cast(),
            size,
        )
    };
    if count == 0 {
        return Err(io::Error::last_os_error());
    }
    if count != size {
        return Err(io::Error::other(
            "ACP process status query returned invalid data",
        ));
    }
    Ok(info.pbsi_pid == pid as u32
        && info.pbsi_pgid == group as u32
        && info.pbsi_status == libc::SZOMB)
}
