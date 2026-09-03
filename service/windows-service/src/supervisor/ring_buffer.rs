//! Bounded thread-safe ring buffer for capturing worker process stdout/stderr diagnostics.

use std::collections::VecDeque;
use std::sync::Mutex;
use zondpi_ipc_protocol::LogEntryDto;

/// A bounded in-memory ring buffer holding recent log entries.
#[derive(Debug)]
pub struct LogRingBuffer {
    capacity: usize,
    entries: Mutex<VecDeque<LogEntryDto>>,
}

impl LogRingBuffer {
    /// Creates a new ring buffer with the specified maximum line capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            entries: Mutex::new(VecDeque::with_capacity(capacity)),
        }
    }

    /// Pushes a new log line into the ring buffer, discarding the oldest entry if full.
    pub fn push(&self, stream: &str, line: impl Into<String>) {
        let line_str = line.into();
        let trimmed = line_str.trim_end();
        if trimmed.is_empty() {
            return;
        }

        let now = chrono_timestamp();
        let entry = LogEntryDto {
            timestamp: now,
            stream: stream.to_string(),
            line: trimmed.to_string(),
        };

        if let Ok(mut lock) = self.entries.lock() {
            if lock.len() >= self.capacity {
                lock.pop_front();
            }
            lock.push_back(entry);
        }
    }

    /// Returns a copy of the most recent N log entries.
    pub fn recent_entries(&self, count: Option<usize>) -> Vec<LogEntryDto> {
        if let Ok(lock) = self.entries.lock() {
            let limit = count.unwrap_or(self.capacity).min(lock.len());
            let skip = lock.len().saturating_sub(limit);
            lock.iter().skip(skip).cloned().collect()
        } else {
            Vec::new()
        }
    }

    /// Clears all entries from the ring buffer.
    pub fn clear(&self) {
        if let Ok(mut lock) = self.entries.lock() {
            lock.clear();
        }
    }
}

fn chrono_timestamp() -> String {
    let now = std::time::SystemTime::now();
    let duration = now
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = duration.as_secs();
    let millis = duration.subsec_millis();

    // Format as simple HH:MM:SS.mmm
    let s = secs % 86400;
    let hours = s / 3600;
    let mins = (s % 3600) / 60;
    let sec = s % 60;
    format!("{:02}:{:02}:{:02}.{:03}", hours, mins, sec, millis)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ring_buffer_bounded_capacity() {
        let buffer = LogRingBuffer::new(3);
        buffer.push("stdout", "line 1");
        buffer.push("stderr", "line 2");
        buffer.push("stdout", "line 3");
        buffer.push("stdout", "line 4");

        let entries = buffer.recent_entries(None);
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].line, "line 2");
        assert_eq!(entries[1].line, "line 3");
        assert_eq!(entries[2].line, "line 4");

        let last_two = buffer.recent_entries(Some(2));
        assert_eq!(last_two.len(), 2);
        assert_eq!(last_two[0].line, "line 3");
        assert_eq!(last_two[1].line, "line 4");
    }
}
