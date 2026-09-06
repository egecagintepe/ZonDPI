//! Windows Job Object-backed process supervisor with bounded crash detection, graceful stop, and log capture.

use std::ffi::OsString;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use thiserror::Error;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;
use tracing::{error, info, warn};

use super::job_object::{JobObjectError, JobObjectHandle};
use super::ring_buffer::LogRingBuffer;
use zondpi_ipc_protocol::LogEntryDto;

#[derive(Error, Debug)]
pub enum SupervisorError {
    #[error("Job Object initialization failed: {0}")]
    JobObject(#[from] JobObjectError),
    #[error("Failed to spawn worker process: {0}")]
    SpawnFailed(#[from] std::io::Error),
    #[error("Process is already running (PID: {0})")]
    AlreadyRunning(u32),
    #[error("Process is currently in state {0:?}, cannot start")]
    InvalidState(SupervisorState),
    #[error("Worker executable not found at: {0}")]
    ExecutableNotFound(PathBuf),
    #[error("Restart budget exhausted ({0} crashes within window)")]
    RestartBudgetExhausted(usize),
}

/// Deterministic lifecycle state machine for the process supervisor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SupervisorState {
    Stopped,
    Starting,
    Running,
    Stopping,
    Failed,
}

impl std::fmt::Display for SupervisorState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SupervisorState::Stopped => write!(f, "Stopped"),
            SupervisorState::Starting => write!(f, "Starting"),
            SupervisorState::Running => write!(f, "Running"),
            SupervisorState::Stopping => write!(f, "Stopping"),
            SupervisorState::Failed => write!(f, "Failed"),
        }
    }
}

/// Configuration parameters for a supervised worker process.
#[derive(Debug, Clone)]
pub struct SupervisorConfig {
    pub name: String,
    pub executable: PathBuf,
    pub arguments: Vec<OsString>,
    pub working_directory: Option<PathBuf>,
    pub graceful_stop_timeout: Duration,
    pub max_restart_attempts: usize,
    pub restart_window: Duration,
}

impl SupervisorConfig {
    pub fn new(name: impl Into<String>, executable: PathBuf, arguments: Vec<OsString>) -> Self {
        Self {
            name: name.into(),
            executable,
            arguments,
            working_directory: None,
            graceful_stop_timeout: Duration::from_secs(3),
            max_restart_attempts: 3,
            restart_window: Duration::from_secs(60),
        }
    }
}

/// Snapshot of the live supervisor status.
#[derive(Debug, Clone)]
pub struct SupervisorStatus {
    pub state: SupervisorState,
    pub pid: Option<u32>,
    pub uptime_seconds: Option<u64>,
    pub restart_count: usize,
    pub last_exit_code: Option<i32>,
    pub last_error: Option<String>,
}

struct Inner {
    config: SupervisorConfig,
    state: SupervisorState,
    job_object: Option<JobObjectHandle>,
    started_at: Option<Instant>,
    pid: Option<u32>,
    restart_timestamps: Vec<Instant>,
    last_exit_code: Option<i32>,
    last_error: Option<String>,
}

/// Process supervisor managing worker child process in a dedicated Windows Job Object.
#[derive(Clone)]
pub struct ProcessSupervisor {
    inner: Arc<Mutex<Inner>>,
    ring_buffer: Arc<LogRingBuffer>,
    manual_stop: Arc<AtomicBool>,
}

impl ProcessSupervisor {
    /// Creates a new process supervisor with default 200-line log ring buffer.
    pub fn new(config: SupervisorConfig) -> Self {
        let ring_buffer = Arc::new(LogRingBuffer::new(200));
        let manual_stop = Arc::new(AtomicBool::new(false));
        Self {
            inner: Arc::new(Mutex::new(Inner {
                config,
                state: SupervisorState::Stopped,
                job_object: None,
                started_at: None,
                pid: None,
                restart_timestamps: Vec::new(),
                last_exit_code: None,
                last_error: None,
            })),
            ring_buffer,
            manual_stop,
        }
    }

    /// Returns a copy of the log ring buffer.
    pub fn ring_buffer(&self) -> Arc<LogRingBuffer> {
        self.ring_buffer.clone()
    }

    /// Spawns the worker process inside a Windows Job Object.
    pub async fn spawn(&self) -> Result<(), SupervisorError> {
        let mut inner = self.inner.lock().await;

        if inner.state == SupervisorState::Running {
            if let Some(pid) = inner.pid {
                return Err(SupervisorError::AlreadyRunning(pid));
            }
        }

        self.manual_stop.store(false, Ordering::SeqCst);
        inner.state = SupervisorState::Starting;

        if !inner.config.executable.is_file() {
            let path = inner.config.executable.clone();
            inner.state = SupervisorState::Failed;
            inner.last_error = Some(format!("Executable not found: {}", path.display()));
            return Err(SupervisorError::ExecutableNotFound(path));
        }

        let config = inner.config.clone();
        match Self::spawn_internal(&config, &self.ring_buffer, &mut inner.job_object).await {
            Ok((child, pid)) => {
                inner.pid = Some(pid);
                inner.started_at = Some(Instant::now());
                inner.state = SupervisorState::Running;
                inner.last_error = None;
                info!(name = %inner.config.name, pid = pid, "Worker process successfully spawned in Job Object");

                let self_clone = self.clone();
                tokio::spawn(async move {
                    self_clone.monitor_loop(child, pid).await;
                });

                Ok(())
            }
            Err(e) => {
                inner.state = SupervisorState::Failed;
                inner.last_error = Some(e.to_string());
                error!(name = %inner.config.name, error = %e, "Failed to spawn worker process");
                Err(e)
            }
        }
    }

    /// Internal process spawn, piping, and Job Object assignment.
    async fn spawn_internal(
        config: &SupervisorConfig,
        ring_buffer: &Arc<LogRingBuffer>,
        job_slot: &mut Option<JobObjectHandle>,
    ) -> Result<(Child, u32), SupervisorError> {
        let job = JobObjectHandle::create_kill_on_close(None)?;

        let mut cmd = Command::new(&config.executable);
        cmd.args(&config.arguments);
        cmd.stdin(Stdio::null());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        if let Some(ref cwd) = config.working_directory {
            cmd.current_dir(cwd);
        }

        #[cfg(windows)]
        {
            // CREATE_NO_WINDOW = 0x08000000
            cmd.creation_flags(0x08000000);
        }

        let mut child = cmd.spawn()?;
        let pid = child.id().unwrap_or(0);

        // Assign child process to Job Object
        #[cfg(windows)]
        if let Some(raw_handle) = child.raw_handle() {
            // SAFETY: raw_handle belongs to freshly spawned child process with valid rights
            let assign_res = unsafe { job.assign_process(raw_handle as _) };
            if let Err(e) = assign_res {
                warn!(error = %e, "Failed to assign child process to Job Object");
            }
        }

        *job_slot = Some(job);

        // Pipe stdout asynchronously to ring buffer
        if let Some(stdout) = child.stdout.take() {
            let buffer = ring_buffer.clone();
            tokio::spawn(async move {
                let reader = BufReader::new(stdout);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    buffer.push("stdout", line);
                }
            });
        }

        // Pipe stderr asynchronously to ring buffer
        if let Some(stderr) = child.stderr.take() {
            let buffer = ring_buffer.clone();
            tokio::spawn(async move {
                let reader = BufReader::new(stderr);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    buffer.push("stderr", line);
                }
            });
        }

        Ok((child, pid))
    }

    /// Iterative monitor loop handling process termination and bounded crash recovery.
    async fn monitor_loop(self, mut child: Child, mut pid: u32) {
        loop {
            let exit_status = child.wait().await;
            let exit_code = exit_status.ok().and_then(|s| s.code());
            let is_manual = self.manual_stop.load(Ordering::SeqCst);

            let mut lock = self.inner.lock().await;
            lock.last_exit_code = exit_code;
            info!(name = %lock.config.name, pid = pid, exit_code = ?exit_code, manual = is_manual, "Supervised process exited");

            if is_manual {
                lock.state = SupervisorState::Stopped;
                lock.pid = None;
                lock.started_at = None;
                lock.job_object = None;
                break;
            }

            // Unexpected termination (crash)
            lock.state = SupervisorState::Failed;
            lock.pid = None;
            lock.started_at = None;
            let now = Instant::now();
            let restart_window = lock.config.restart_window;
            lock.restart_timestamps
                .retain(|t| now.duration_since(*t) < restart_window);
            lock.restart_timestamps.push(now);

            let attempts = lock.restart_timestamps.len();
            if attempts > lock.config.max_restart_attempts {
                let err_msg = format!(
                    "Worker crashed repeatedly: {} failures within {}s",
                    attempts,
                    lock.config.restart_window.as_secs()
                );
                error!(name = %lock.config.name, attempts = attempts, "Restart limit exhausted");
                lock.last_error = Some(err_msg);
                break;
            }

            // Calculate bounded exponential backoff
            let backoff_secs = match attempts {
                1 => 1,
                2 => 2,
                _ => 5,
            };
            warn!(name = %lock.config.name, attempt = attempts, backoff_secs = backoff_secs, "Attempting automatic restart after backoff");
            let config = lock.config.clone();
            drop(lock);

            tokio::time::sleep(Duration::from_secs(backoff_secs)).await;

            if self.manual_stop.load(Ordering::SeqCst) {
                let mut lock = self.inner.lock().await;
                lock.state = SupervisorState::Stopped;
                break;
            }

            let mut lock = self.inner.lock().await;
            lock.state = SupervisorState::Starting;
            match Self::spawn_internal(&config, &self.ring_buffer, &mut lock.job_object).await {
                Ok((new_child, new_pid)) => {
                    lock.pid = Some(new_pid);
                    lock.started_at = Some(Instant::now());
                    lock.state = SupervisorState::Running;
                    info!(name = %lock.config.name, pid = new_pid, "Worker process successfully restarted");
                    child = new_child;
                    pid = new_pid;
                }
                Err(e) => {
                    lock.state = SupervisorState::Failed;
                    lock.last_error = Some(format!("Restart attempt failed: {}", e));
                    error!(name = %lock.config.name, error = %e, "Worker restart failed");
                    break;
                }
            }
        }
    }

    /// Gracefully stops the worker process by terminating the Job Object and releasing handles.
    pub async fn graceful_stop(&self) -> Result<(), SupervisorError> {
        self.manual_stop.store(true, Ordering::SeqCst);
        let mut inner = self.inner.lock().await;

        if inner.state == SupervisorState::Stopped {
            return Ok(());
        }

        inner.state = SupervisorState::Stopping;

        if let Some(job) = inner.job_object.take() {
            let timeout = inner.config.graceful_stop_timeout;
            let name = inner.config.name.clone();
            info!(name = %name, timeout_secs = timeout.as_secs(), "Terminating worker process via Job Object");

            // Explicitly terminate all processes in this Job Object first
            let _ = job.terminate(0);
            // Dropping the job object handle when configured with KILL_ON_JOB_CLOSE
            // guarantees all child processes in the job are terminated by Windows kernel.
            drop(job);
        }

        #[cfg(windows)]
        if let Some(pid) = inner.pid {
            // Defense in depth: Terminate ONLY the tracked child PID spawned by ZonDPI
            use windows_sys::Win32::Foundation::CloseHandle;
            use windows_sys::Win32::System::Threading::{
                OpenProcess, TerminateProcess, WaitForSingleObject,
                PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE, PROCESS_TERMINATE,
            };

            unsafe {
                let handle = OpenProcess(
                    PROCESS_TERMINATE | PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
                    0,
                    pid,
                );
                if !handle.is_null()
                    && handle != windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE
                {
                    let _ = TerminateProcess(handle, 0);
                    let _ = WaitForSingleObject(handle, 1000);
                    CloseHandle(handle);
                }
            }
        }

        inner.pid = None;
        inner.started_at = None;
        inner.state = SupervisorState::Stopped;
        Ok(())
    }

    /// Forcibly stops the worker process immediately.
    pub async fn force_stop(&self) -> Result<(), SupervisorError> {
        self.graceful_stop().await
    }

    /// Queries the current supervisor state and operational metrics.
    pub async fn status(&self) -> SupervisorStatus {
        let inner = self.inner.lock().await;
        let uptime = inner.started_at.map(|t| t.elapsed().as_secs());
        SupervisorStatus {
            state: inner.state,
            pid: inner.pid,
            uptime_seconds: uptime,
            restart_count: inner.restart_timestamps.len(),
            last_exit_code: inner.last_exit_code,
            last_error: inner.last_error.clone(),
        }
    }

    /// Returns recent log lines from the in-memory ring buffer.
    pub async fn get_recent_logs(&self, count: Option<usize>) -> Vec<LogEntryDto> {
        self.ring_buffer.recent_entries(count)
    }

    /// Returns true if the worker is actively running.
    pub async fn is_running(&self) -> bool {
        let inner = self.inner.lock().await;
        inner.state == SupervisorState::Running
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_supervisor_initial_state_is_stopped() {
        let config = SupervisorConfig::new("test-worker", PathBuf::from("nonexistent.exe"), vec![]);
        let supervisor = ProcessSupervisor::new(config);
        let status = supervisor.status().await;
        assert_eq!(status.state, SupervisorState::Stopped);
        assert_eq!(status.pid, None);
        assert!(!supervisor.is_running().await);
    }

    #[tokio::test]
    async fn test_supervisor_fails_on_missing_executable() {
        let config = SupervisorConfig::new(
            "missing-worker",
            PathBuf::from("C:\\definitely_missing_file_12345.exe"),
            vec![],
        );
        let supervisor = ProcessSupervisor::new(config);
        let spawn_res = supervisor.spawn().await;
        assert!(spawn_res.is_err());
        let status = supervisor.status().await;
        assert_eq!(status.state, SupervisorState::Failed);
    }

    #[tokio::test]
    async fn test_supervisor_idempotent_stop() {
        let config = SupervisorConfig::new("idle-worker", PathBuf::from("dummy.exe"), vec![]);
        let supervisor = ProcessSupervisor::new(config);
        assert!(supervisor.graceful_stop().await.is_ok());
        assert_eq!(supervisor.status().await.state, SupervisorState::Stopped);
    }
}
