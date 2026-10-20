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
        Diagnostics::ProcessSnapshotting::*,
        JobObjects::{CreateJobObjectW, SetInformationJobObject, AssignProcessToJobObject,
            JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE},
        Threading::{CREATE_NO_WINDOW, CREATE_SUSPENDED, OpenThread, THREAD_SUSPEND_RESUME, ResumeThread, GetCurrentProcess},
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
        resume_primary_thread(&tree.child)?;
        Ok(tree)
    }
    fn resume_primary_thread(child: &Child) -> io::Result<()> {
        // Inspect only our still-suspended child, not every thread on the machine.
        // No address space, handles or thread contexts are captured.
        struct Snapshot(HPSS);
        impl Drop for Snapshot { fn drop(&mut self) { unsafe { PssFreeSnapshot(GetCurrentProcess(), self.0); } } }
        struct Marker(HPSSWALK);
        impl Drop for Marker { fn drop(&mut self) { unsafe { PssWalkMarkerFree(self.0); } } }
        fn checked(code: u32) -> io::Result<()> {
            if code == 0 { Ok(()) } else { Err(io::Error::from_raw_os_error(code as i32)) }
        }
        let mut handle = std::ptr::null_mut();
        checked(unsafe { PssCaptureSnapshot(child.as_raw_handle(), PSS_CAPTURE_THREADS, 0, &mut handle) })?;
        let snapshot = Snapshot(handle);
        let mut marker = std::ptr::null_mut();
        checked(unsafe { PssWalkMarkerCreate(std::ptr::null(), &mut marker) })?;
        let marker = Marker(marker);
        let mut entry = PSS_THREAD_ENTRY::default();
        checked(unsafe { PssWalkSnapshot(snapshot.0, PSS_WALK_THREADS, marker.0,
            (&mut entry as *mut PSS_THREAD_ENTRY).cast(), std::mem::size_of::<PSS_THREAD_ENTRY>() as u32) })?;
        if entry.ProcessId != child.id() { return Err(io::Error::other("Unexpected captured process")); }
        let tid = entry.ThreadId;
        let next = unsafe { PssWalkSnapshot(snapshot.0, PSS_WALK_THREADS, marker.0,
            (&mut entry as *mut PSS_THREAD_ENTRY).cast(), std::mem::size_of::<PSS_THREAD_ENTRY>() as u32) };
        if next != windows_sys::Win32::Foundation::ERROR_NO_MORE_ITEMS {
            checked(next)?;
            return Err(io::Error::other("Suspended process has multiple threads"));
        }
        let thread = owned(unsafe { OpenThread(THREAD_SUSPEND_RESUME, 0, tid) })?;
        match unsafe { ResumeThread(thread.as_raw_handle()) } {
            1 => Ok(()),
            u32::MAX => Err(io::Error::last_os_error()),
            _ => Err(io::Error::other("Unexpected initial thread suspend count")),
        }
    }
}
