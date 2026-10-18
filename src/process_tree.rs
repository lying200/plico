//! Owned operation process tree. Drop terminates descendants before reaping the leader.
//! Reference: Rebased 4aec35256d765c217aa0d662b82ac4e860c396fd,
//! platform/platform-util-io/src/com/intellij/execution/process/OSProcessHandler.java
//! (recursive destruction). Job ownership/process groups are our platform adaptation;
//! this primitive does not implement Git's cancellation or authentication policy.
use std::{io, process::{Child, Command}};

pub struct ProcessTree {
    pub child: Child,
    #[cfg(windows)]
    job: std::os::windows::io::OwnedHandle,
    #[cfg(unix)]
    group: i32,
    terminated: bool,
}
impl ProcessTree {
    pub fn spawn(command: &mut Command) -> io::Result<Self> {
        #[cfg(windows)]
        { windows::spawn(command) }
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.process_group(0);
            let child = command.spawn()?;
            Ok(Self { group: child.id() as i32, child, terminated: false })
        }
    }
    pub fn terminate(&mut self) -> io::Result<()> {
        if self.terminated { return Ok(()); }
        #[cfg(windows)]
        {
            use std::os::windows::io::AsRawHandle;
            // The job is unnamed, non-inheritable and owned only by this operation.
            if unsafe { windows_sys::Win32::System::JobObjects::TerminateJobObject(self.job.as_raw_handle(), 1) } == 0 {
                return Err(io::Error::last_os_error());
            }
        }
        #[cfg(unix)]
        {
            unsafe extern "C" { fn kill(pid: i32, signal: i32) -> i32; }
            if unsafe { kill(-self.group, 9) } != 0 {
                let error = io::Error::last_os_error();
                if error.raw_os_error() != Some(3) { return Err(error); } // ESRCH: already empty
            }
        }
        self.terminated = true;
        Ok(())
    }
}
impl Drop for ProcessTree {
    fn drop(&mut self) {
        let _ = self.terminate();
        // Also reap on failed setup/termination; dropping Child alone does not wait.
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[cfg(windows)]
mod windows {
    use super::*;
    use std::os::windows::{io::{AsRawHandle, FromRawHandle, OwnedHandle}, process::CommandExt};
    use windows_sys::Win32::{Foundation::INVALID_HANDLE_VALUE, System::{
        Diagnostics::ToolHelp::{CreateToolhelp32Snapshot, TH32CS_SNAPTHREAD, THREADENTRY32, Thread32First, Thread32Next},
        JobObjects::{CreateJobObjectW, SetInformationJobObject, AssignProcessToJobObject,
            JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE},
        Threading::{CREATE_NO_WINDOW, CREATE_SUSPENDED, OpenThread, THREAD_SUSPEND_RESUME, ResumeThread},
    }};
    fn owned(handle: *mut core::ffi::c_void) -> io::Result<OwnedHandle> {
        if handle.is_null() || handle == INVALID_HANDLE_VALUE { return Err(io::Error::last_os_error()); }
        Ok(unsafe { OwnedHandle::from_raw_handle(handle) })
    }
    pub(super) fn spawn(command: &mut Command) -> io::Result<ProcessTree> {
        let job = owned(unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) })?;
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        if unsafe { SetInformationJobObject(job.as_raw_handle(), JobObjectExtendedLimitInformation,
            (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(), std::mem::size_of_val(&limits) as u32) } == 0 {
            return Err(io::Error::last_os_error());
        }
        // Suspend before assignment: a fast Git helper cannot escape the job during startup.
        command.creation_flags(CREATE_NO_WINDOW | CREATE_SUSPENDED);
        let child = command.spawn()?;
        let tree = ProcessTree { child, job, terminated: false };
        if unsafe { AssignProcessToJobObject(tree.job.as_raw_handle(), tree.child.as_raw_handle()) } == 0 {
            return Err(io::Error::last_os_error()); // tree Drop also kills the unassigned suspended child
        }
        resume_primary_thread(tree.child.id())?;
        Ok(tree)
    }
    fn resume_primary_thread(pid: u32) -> io::Result<()> {
        // std::process::Child exposes its process handle, not the primary thread handle.
        // A newly created, still-suspended process has not started user code or helpers.
        let snapshot = owned(unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) })?;
        let mut entry = THREADENTRY32 { dwSize: std::mem::size_of::<THREADENTRY32>() as u32, ..Default::default() };
        let mut found = None;
        let mut more = unsafe { Thread32First(snapshot.as_raw_handle(), &mut entry) } != 0;
        while more {
            if entry.th32OwnerProcessID == pid {
                if found.is_some() { return Err(io::Error::other("Suspended process has multiple threads")); }
                found = Some(entry.th32ThreadID);
            }
            entry.dwSize = std::mem::size_of::<THREADENTRY32>() as u32;
            more = unsafe { Thread32Next(snapshot.as_raw_handle(), &mut entry) } != 0;
        }
        let tid = found.ok_or_else(|| io::Error::other("Suspended process thread not found"))?;
        let thread = owned(unsafe { OpenThread(THREAD_SUSPEND_RESUME, 0, tid) })?;
        match unsafe { ResumeThread(thread.as_raw_handle()) } {
            1 => Ok(()),
            u32::MAX => Err(io::Error::last_os_error()),
            _ => Err(io::Error::other("Unexpected initial thread suspend count")),
        }
    }
}
