//! Process supervisor, Windows Job Object management, and diagnostic ring-buffers.

pub mod job_object;
pub mod process_supervisor;
pub mod ring_buffer;

pub use job_object::{JobObjectError, JobObjectHandle};
pub use process_supervisor::{
    ProcessSupervisor, SupervisorConfig, SupervisorError, SupervisorState, SupervisorStatus,
};
pub use ring_buffer::LogRingBuffer;
