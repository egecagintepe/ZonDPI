//! Connection tracking and TCP hop distance calculator.

pub struct FlowTracker;

impl FlowTracker {
    pub fn calculate_auto_ttl(incoming_ttl: u8) -> u8 {
        // Measure hop distance based on standard 64 / 128 / 255 initial TTLs
        let initial_ttl: u8 = if incoming_ttl <= 64 {
            64
        } else if incoming_ttl <= 128 {
            128
        } else {
            255
        };
        let hops = initial_ttl.saturating_sub(incoming_ttl);
        (hops + 3).min(10)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_auto_ttl() {
        assert_eq!(FlowTracker::calculate_auto_ttl(60), 7);
        assert_eq!(FlowTracker::calculate_auto_ttl(64), 3);
        assert_eq!(FlowTracker::calculate_auto_ttl(120), 10);
    }
}
