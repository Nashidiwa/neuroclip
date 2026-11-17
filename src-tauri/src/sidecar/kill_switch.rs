//! OS-level subprocess lifecycle safety guards (Kill-on-Drop & Windows Job Object).
//!
//! Guarantees zero orphaned/zombie child processes (`ADR-007`, `TASK-M4-03`).
//! Strict zero-panic invariant: all fallible operations return `Result`.

/// RAII guard wrapping a spawned `tokio::process::Child` to guarantee termination on drop.
pub struct ChildProcessGuard {
    child: Option<tokio::process::Child>,
    pid: Option<u32>,
}

impl ChildProcessGuard {
    /// Wrap a spawned `tokio::process::Child` and capture its OS process ID.
    #[must_use]
    pub fn new(child: tokio::process::Child) -> Self {
        let pid = child.id();
        Self {
            child: Some(child),
            pid,
        }
    }

    /// Return the captured OS process ID if available.
    #[must_use]
    pub const fn pid(&self) -> Option<u32> {
        self.pid
    }

    /// Take ownership of the inner child process, disarming kill-on-drop.
    pub fn take_child(&mut self) -> Option<tokio::process::Child> {
        self.child.take()
    }

    /// Borrow the inner child process mutably.
    pub fn child_mut(&mut self) -> Option<&mut tokio::process::Child> {
        self.child.as_mut()
    }
}

impl Drop for ChildProcessGuard {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.start_kill();
        }
    }
}

/// Windows Job Object RAII guard ensuring 0 zombie processes on Windows (`ADR-007`).
#[cfg(windows)]
pub struct WindowsJobObjectGuard {
    job_handle: windows_sys::Win32::Foundation::HANDLE,
}

#[cfg(windows)]
unsafe impl Send for WindowsJobObjectGuard {}

#[cfg(windows)]
unsafe impl Sync for WindowsJobObjectGuard {}

#[cfg(windows)]
impl WindowsJobObjectGuard {
    /// Create a Windows Job Object configured with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`.
    ///
    /// # Errors
    /// Returns an I/O error if `CreateJobObjectW` or `SetInformationJobObject` fails.
    pub fn new() -> Result<Self, std::io::Error> {
        use std::mem::size_of;
        use windows_sys::Win32::System::JobObjects::{
            CreateJobObjectW, JobObjectExtendedLimitInformation, SetInformationJobObject,
            JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        };

        unsafe {
            let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if job == 0 {
                return Err(std::io::Error::last_os_error());
            }

            let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;

            let info_size =
                u32::try_from(size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>()).unwrap_or(0);
            let res = SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                std::ptr::addr_of!(info).cast(),
                info_size,
            );

            if res == 0 {
                windows_sys::Win32::Foundation::CloseHandle(job);
                return Err(std::io::Error::last_os_error());
            }

            Ok(Self { job_handle: job })
        }
    }

    /// Assign a running process handle to this Job Object.
    ///
    /// # Errors
    /// Returns an I/O error if `AssignProcessToJobObject` fails.
    pub fn assign_process(
        &self,
        process_handle: windows_sys::Win32::Foundation::HANDLE,
    ) -> Result<(), std::io::Error> {
        use windows_sys::Win32::System::JobObjects::AssignProcessToJobObject;
        unsafe {
            let res = AssignProcessToJobObject(self.job_handle, process_handle);
            if res == 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        }
    }

    /// Assign a spawned `tokio::process::Child` to this Job Object by PID using Win32 `OpenProcess`.
    ///
    /// # Errors
    /// Returns an I/O error if the child has exited (no PID) or Win32 handle assignment fails.
    pub fn assign_child_process(
        &self,
        child: &tokio::process::Child,
    ) -> Result<(), std::io::Error> {
        use windows_sys::Win32::Foundation::CloseHandle;
        use windows_sys::Win32::System::Threading::{
            OpenProcess, PROCESS_SET_QUOTA, PROCESS_TERMINATE,
        };

        if self.job_handle == 0 {
            return Ok(());
        }

        if let Some(pid) = child.id() {
            unsafe {
                let handle = OpenProcess(PROCESS_SET_QUOTA | PROCESS_TERMINATE, 0, pid);
                if handle == 0 {
                    return Err(std::io::Error::last_os_error());
                }
                let res = self.assign_process(handle);
                CloseHandle(handle);
                res
            }
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Child process has no PID",
            ))
        }
    }
}

#[cfg(windows)]
impl Drop for WindowsJobObjectGuard {
    fn drop(&mut self) {
        unsafe {
            if self.job_handle != 0 {
                windows_sys::Win32::Foundation::CloseHandle(self.job_handle);
            }
        }
    }
}

/// Fallback no-op guard on non-Windows target platforms.
#[cfg(not(windows))]
pub struct WindowsJobObjectGuard;

#[cfg(not(windows))]
impl WindowsJobObjectGuard {
    /// Construct a no-op guard on non-Windows platforms.
    ///
    /// # Errors
    /// Always returns `Ok`.
    pub fn new() -> Result<Self, std::io::Error> {
        Ok(Self)
    }

    /// No-op process handle assignment on non-Windows platforms.
    ///
    /// # Errors
    /// Always returns `Ok`.
    pub fn assign_process(&self, _handle: usize) -> Result<(), std::io::Error> {
        Ok(())
    }

    /// No-op child process assignment on non-Windows platforms.
    ///
    /// # Errors
    /// Always returns `Ok`.
    pub fn assign_child_process(
        &self,
        _child: &tokio::process::Child,
    ) -> Result<(), std::io::Error> {
        Ok(())
    }
}

#[cfg(windows)]
static GLOBAL_JOB_GUARD: std::sync::OnceLock<Option<WindowsJobObjectGuard>> =
    std::sync::OnceLock::new();

/// Retrieve the process-wide Windows Job Object guard singleton.
///
/// # Errors
/// Returns an I/O error if initializing the Job Object failed.
#[cfg(windows)]
pub fn get_global_job_guard() -> Result<&'static WindowsJobObjectGuard, std::io::Error> {
    let opt = GLOBAL_JOB_GUARD.get_or_init(|| match WindowsJobObjectGuard::new() {
        Ok(guard) => Some(guard),
        Err(e) => {
            tracing::warn!("Failed to initialize WindowsJobObjectGuard: {e}");
            None
        }
    });

    match opt.as_ref() {
        Some(guard) => Ok(guard),
        None => Err(std::io::Error::other("Windows Job Object is unavailable")),
    }
}

/// Assign a spawned subprocess to the OS-level Job Object guard to guarantee zero zombies (`ADR-007`).
pub fn assign_child_to_job_guard(child: &tokio::process::Child) {
    #[cfg(windows)]
    {
        if let Ok(guard) = get_global_job_guard() {
            if let Err(e) = guard.assign_child_process(child) {
                tracing::warn!("Failed to assign child process to Windows Job Object: {e}");
            }
        }
    }
    #[cfg(not(windows))]
    {
        let _ = child;
    }
}
