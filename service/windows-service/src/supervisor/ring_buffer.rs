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

        let sanitized_line = sanitize_log_presentation(trimmed);
        let now = chrono_timestamp();
        let entry = LogEntryDto {
            timestamp: now,
            stream: stream.to_string(),
            line: sanitized_line,
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

/// Sanitizes donation wallet addresses and crypto donation strings from presentation output.
/// Preserves exact upstream source files while preventing wallet/donation strings in UI/CLI logs.
pub fn sanitize_log_presentation(line: &str) -> String {
    let lower = line.to_lowercase();
    if lower.contains("donate")
        || lower.contains("bitcoin")
        || lower.contains("btc")
        || lower.contains("wallet")
        || lower.contains("monero")
    {
        let words: Vec<&str> = line.split_whitespace().collect();
        let mut new_words = Vec::new();
        for w in words {
            let clean_w = w.trim_matches(|c: char| !c.is_alphanumeric());
            let is_btc = (clean_w.starts_with("bc1")
                || clean_w.starts_with('1')
                || clean_w.starts_with('3'))
                && clean_w.len() >= 26
                && clean_w.len() <= 45;
            let is_eth = clean_w.starts_with("0x") && clean_w.len() == 42;
            if is_btc || is_eth {
                new_words.push("[redacted]");
            } else {
                new_words.push(w);
            }
        }
        new_words.join(" ")
    } else {
        line.to_string()
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

    #[test]
    fn test_sanitize_log_presentation() {
        let input = "GoodbyeDPI running. Donate Bitcoin: 1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa or ETH 0x71C7656EC7ab88b098defB751B7401B5f6d8976F thank you!";
        let sanitized = sanitize_log_presentation(input);
        assert!(!sanitized.contains("1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa"));
        assert!(!sanitized.contains("0x71C7656EC7ab88b098defB751B7401B5f6d8976F"));
        assert!(sanitized.contains("[redacted]"));
    }
}
