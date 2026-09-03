//! Process adapters for GoodbyeDPI and ByeDPI networking engines.

pub mod byedpi;
pub mod goodbye;

pub use byedpi::{ByeDpiAdapter, ByeDpiAdapterError, ByeDpiConfig};
pub use goodbye::{GoodbyeAdapterError, GoodbyeConfig, GoodbyeDpiAdapter};
