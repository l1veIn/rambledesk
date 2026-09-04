// Only used by a direct wrapper test. Production ownership is provided by the
// outer ACP process group. Never reconstruct this handle from persisted PIDs.
pub struct OwnedGroup(libc::pid_t);
impl OwnedGroup {
    pub fn new(leader: u32) -> Self {
        Self(leader as libc::pid_t)
    }
}
impl Drop for OwnedGroup {
    fn drop(&mut self) {
        // SAFETY: the caller still holds the unreaped Child returned by the
        // process_group(0) spawn, pinning this group until after this guard drops.
        unsafe {
            libc::kill(-self.0, libc::SIGKILL);
        }
    }
}
