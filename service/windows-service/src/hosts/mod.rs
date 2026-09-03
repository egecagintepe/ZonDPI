//! Execution hosts for Windows Service (SCM) and Foreground (CLI/Debug).

pub mod foreground;
pub mod service;

pub use foreground::run_foreground;
pub use service::run_service;
