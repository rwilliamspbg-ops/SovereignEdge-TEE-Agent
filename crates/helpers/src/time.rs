//! Time utilities for consistent timestamp handling

use std::time::{Duration, Instant};

/// Get current time in nanoseconds since UNIX epoch
pub fn now_ns() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("Time went backwards")
        .as_nanos() as u64
}

/// Get current time in milliseconds since UNIX epoch
pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("Time went backwards")
        .as_millis() as u64
}

/// Calculate elapsed time in nanoseconds from a starting instant
pub fn elapsed_ns(since: Instant) -> u64 {
    since.elapsed().as_nanos() as u64
}

/// Calculate elapsed time in milliseconds from a starting instant
pub fn elapsed_ms(since: Instant) -> u64 {
    since.elapsed().as_millis() as u64
}

/// Format duration as human-readable string
pub fn format_duration(duration: Duration) -> String {
    if duration.as_secs() > 60 {
        format!("{:.2}m", duration.as_secs_f64() / 60.0)
    } else if duration.as_secs() > 1 {
        format!("{:.2}s", duration.as_secs_f64())
    } else if duration.as_millis() > 1 {
        format!("{:.2}ms", duration.as_secs_f64() * 1000.0)
    } else {
        format!("{:.2}µs", duration.as_secs_f64() * 1_000_000.0)
    }
}

/// Create an instant from nanoseconds ago
pub fn instant_from_ns_ago(ns: u64) -> Instant {
    Instant::now() - Duration::from_nanos(ns)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_now_ns_increases() {
        let t1 = now_ns();
        std::thread::sleep(Duration::from_millis(10));
        let t2 = now_ns();
        assert!(t2 > t1);
    }

    #[test]
    fn test_elapsed_time() {
        let start = Instant::now();
        std::thread::sleep(Duration::from_millis(50));
        let elapsed = elapsed_ms(start);
        assert!(elapsed >= 50);
        assert!(elapsed < 100); // Should not be too much over
    }

    #[test]
    fn test_format_duration() {
        assert!(format_duration(Duration::from_secs(120)).contains("m"));
        assert!(format_duration(Duration::from_secs(30)).contains("s"));
        assert!(format_duration(Duration::from_millis(500)).contains("ms"));
    }
}
