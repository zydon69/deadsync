//! Process-tree ownership for the short-lived CLI version probe.
use std::{
    io,
    process::{Child, Command},
};

pub(super) struct ProcessTree {
    #[cfg(unix)]
    group: Option<libc::pid_t>,
    #[cfg(windows)]
    job: Option<std::os::windows::io::OwnedHandle>,
}
impl ProcessTree {
    pub(super) fn spawn(command: &mut Command) -> io::Result<(Child, Self)> {
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.process_group(0);
            let child = command.spawn()?;
            // Unix process IDs are positive pid_t values.
            let group = child.id() as libc::pid_t;
            Ok((child, Self { group: Some(group) }))
        }
        #[cfg(windows)]
        {
            windows::spawn(command)
        }
        #[cfg(not(any(unix, windows)))]
        {
            command.spawn().map(|child| (child, Self {}))
        }
    }
    pub(super) fn terminate(&mut self) {
        #[cfg(unix)]
        if let Some(group) = self.group.take() {
            // SAFETY: this positive group ID belongs to our isolated probe.
            unsafe { libc::kill(-group, libc::SIGKILL) };
        }
        #[cfg(windows)]
        // KILL_ON_JOB_CLOSE terminates every member, even if the launcher exited.
        drop(self.job.take());
    }
}
impl Drop for ProcessTree {
    fn drop(&mut self) {
        self.terminate();
    }
}

#[cfg(windows)]
mod windows {
    use super::*;
    use std::{
        mem::size_of,
        os::windows::{
            io::{AsRawHandle, FromRawHandle, OwnedHandle},
            process::CommandExt,
        },
        ptr,
    };
    use windows_sys::Win32::{
        Foundation::INVALID_HANDLE_VALUE,
        System::{
            Diagnostics::ToolHelp::{
                CreateToolhelp32Snapshot, TH32CS_SNAPTHREAD, THREADENTRY32, Thread32First,
                Thread32Next,
            },
            JobObjects::{
                AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
                JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
                SetInformationJobObject,
            },
            Threading::{CREATE_SUSPENDED, OpenThread, ResumeThread, THREAD_SUSPEND_RESUME},
        },
    };

    pub(super) fn spawn(command: &mut Command) -> io::Result<(Child, ProcessTree)> {
        // SAFETY: unnamed, non-inheritable job with default security attributes.
        let raw = unsafe { CreateJobObjectW(ptr::null(), ptr::null()) };
        if raw.is_null() {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: the successful creation transfers this unique handle to us.
        let job = unsafe { OwnedHandle::from_raw_handle(raw) };
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        // SAFETY: the buffer and its size describe a valid extended limits value.
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
        // Do not let the launcher run before its descendants are contained.
        command.creation_flags(CREATE_SUSPENDED);
        let mut child = command.spawn()?;
        // SAFETY: both handles are live and the child has not started executing.
        let assigned =
            unsafe { AssignProcessToJobObject(job.as_raw_handle(), child.as_raw_handle()) };
        let result = if assigned == 0 {
            Err(io::Error::last_os_error())
        } else {
            resume(child.id())
        };
        if let Err(error) = result {
            // Never leave a suspended process behind when setup fails.
            let _ = child.kill();
            let _ = child.wait();
            return Err(error);
        }
        Ok((child, ProcessTree { job: Some(job) }))
    }

    fn resume(pid: u32) -> io::Result<()> {
        // Stable Rust exposes the process handle, but not the primary thread
        // handle. A CREATE_SUSPENDED process has not run its primary thread;
        // find that thread using the documented Tool Help API.
        // SAFETY: snapshots require no pointers or inherited handles.
        let raw = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) };
        if raw == INVALID_HANDLE_VALUE {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: a successful snapshot is an owned, closeable handle.
        let snapshot = unsafe { OwnedHandle::from_raw_handle(raw) };
        let mut entry = THREADENTRY32 {
            dwSize: size_of::<THREADENTRY32>() as u32,
            ..Default::default()
        };
        // SAFETY: initialized buffer has the size required by Thread32First.
        let mut found = unsafe { Thread32First(snapshot.as_raw_handle(), &mut entry) } != 0;
        while found {
            if entry.th32OwnerProcessID == pid {
                // SAFETY: request only resume rights; the handle is not inherited.
                let raw = unsafe { OpenThread(THREAD_SUSPEND_RESUME, 0, entry.th32ThreadID) };
                if raw.is_null() {
                    return Err(io::Error::last_os_error());
                }
                // SAFETY: OpenThread returned a unique, owned thread handle.
                let thread = unsafe { OwnedHandle::from_raw_handle(raw) };
                // SAFETY: this is the suspended probe's thread, not a host thread.
                let previous = unsafe { ResumeThread(thread.as_raw_handle()) };
                if previous == u32::MAX {
                    return Err(io::Error::last_os_error());
                }
                if previous != 1 {
                    return Err(io::Error::other(
                        "Unexpected version-probe thread suspension count",
                    ));
                }
                return Ok(());
            }
            entry.dwSize = size_of::<THREADENTRY32>() as u32;
            // SAFETY: snapshot and initialized entry remain live through the loop.
            found = unsafe { Thread32Next(snapshot.as_raw_handle(), &mut entry) } != 0;
        }
        Err(io::Error::new(
            io::ErrorKind::NotFound,
            "Cannot find the suspended version probe's thread",
        ))
    }
}
