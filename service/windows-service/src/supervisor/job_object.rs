//! Windows Job Object RAII wrapper ensuring child worker processes are killed on parent termination.

use std::ptr::null_mut;
use thiserror::Error;
use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, HANDLE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
    SetInformationJobObject, JOBOBJECT_BASIC_LIMIT_INFORMATION,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
};

#[derive(Error, Debug)]
pub enum JobObjectError {
    #[error("Failed to create Windows Job Object: Win32 error code {0}")]
    CreateFailed(u32),
    #[error("Failed to set Job Object extended limit information: Win32 error code {0}")]
    SetInfoFailed(u32),
    #[error("Failed to assign process to Job Object: Win32 error code {0}")]
    AssignProcessFailed(u32),
    #[error("Job Object handle is invalid or closed")]
    InvalidHandle,
}

/// RAII wrapper around a Win32 Job Object handle.
#[derive(Debug)]
pub struct JobObjectHandle(HANDLE);

// SAFETY: Windows Job Object handles can be safely transferred between threads.
unsafe impl Send for JobObjectHandle {}
unsafe impl Sync for JobObjectHandle {}

impl JobObjectHandle {
    /// Creates a new Windows Job Object configured with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`.
    pub fn create_kill_on_close(name: Option<&str>) -> Result<Self, JobObjectError> {
        let name_wide: Option<Vec<u16>> =
            name.map(|s| s.encode_utf16().chain(std::iter::once(0)).collect());
        let name_ptr = name_wide
            .as_ref()
            .map_or(null_mut(), |v| v.as_ptr() as *mut _);

        // SAFETY: Calling CreateJobObjectW with optional name
        let handle = unsafe { CreateJobObjectW(null_mut(), name_ptr) };
        if handle.is_null() || handle == INVALID_HANDLE_VALUE {
            let err = unsafe { GetLastError() };
            return Err(JobObjectError::CreateFailed(err));
        }

        let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { std::mem::zeroed() };
        info.BasicLimitInformation = JOBOBJECT_BASIC_LIMIT_INFORMATION {
            LimitFlags: JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            ..unsafe { std::mem::zeroed() }
        };

        // SAFETY: Setting extended limit information on valid Job Object handle
        let success = unsafe {
            SetInformationJobObject(
                handle,
                JobObjectExtendedLimitInformation,
                &info as *const _ as *const _,
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        };

        if success == 0 {
            let err = unsafe { GetLastError() };
            unsafe { CloseHandle(handle) };
            return Err(JobObjectError::SetInfoFailed(err));
        }

        Ok(Self(handle))
    }

    /// Assigns a running process to this Job Object.
    ///
    /// # Safety
    /// `process_handle` must be a valid open Win32 process HANDLE with `PROCESS_SET_QUOTA` and `PROCESS_TERMINATE` access rights.
    pub unsafe fn assign_process(&self, process_handle: HANDLE) -> Result<(), JobObjectError> {
        if self.0.is_null() || self.0 == INVALID_HANDLE_VALUE {
            return Err(JobObjectError::InvalidHandle);
        }

        let success = AssignProcessToJobObject(self.0, process_handle);
        if success == 0 {
            let err = GetLastError();
            return Err(JobObjectError::AssignProcessFailed(err));
        }

        Ok(())
    }

    /// Returns the raw Win32 HANDLE.
    pub fn as_raw_handle(&self) -> HANDLE {
        self.0
    }
}

impl Drop for JobObjectHandle {
    fn drop(&mut self) {
        if !self.0.is_null() && self.0 != INVALID_HANDLE_VALUE {
            // SAFETY: Closing open Job Object handle on drop
            unsafe {
                CloseHandle(self.0);
            }
            self.0 = null_mut();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_and_drop_job_object() {
        let job = JobObjectHandle::create_kill_on_close(None);
        assert!(job.is_ok());
        let job = job.unwrap();
        assert!(!job.as_raw_handle().is_null());
        assert_ne!(job.as_raw_handle(), INVALID_HANDLE_VALUE);
    }
}
