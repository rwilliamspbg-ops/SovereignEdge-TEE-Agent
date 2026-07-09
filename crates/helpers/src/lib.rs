//! Helper utilities for SovereignEdge-TEE-Agent
//!
//! This crate provides common utilities, test fixtures, and builder patterns
//! to improve developer experience and code quality across all modules.
//!
//! ## Features
//!
//! - **Time utilities**: Proper timestamp handling
//! - **Buffer helpers**: Zero-copy buffer management
//! - **Test fixtures**: Sample data for testing
//! - **Builder patterns**: Fluent construction of complex types
//! - **Metrics helpers**: Prometheus metric registration

pub mod builders;
pub mod fixtures;
pub mod metrics;
pub mod time;

// Re-export commonly used items
pub use builders::*;
pub use fixtures::*;
pub use time::*;

/// Common result type for helper operations
pub type HelperResult<T> = Result<T, HelperError>;

/// Helper error types
#[derive(Debug, thiserror::Error)]
pub enum HelperError {
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    #[error("Buffer operation failed: {0}")]
    BufferError(String),

    #[error("Metrics error: {0}")]
    MetricsError(String),
}

/// Zero-copy buffer wrapper for efficient memory management
pub struct ZeroCopyBuffer {
    data: Vec<u8>,
    capacity: usize,
    read_pos: usize,
    write_pos: usize,
}

impl ZeroCopyBuffer {
    /// Create a new zero-copy buffer with specified capacity
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            data: vec![0u8; capacity],
            capacity,
            read_pos: 0,
            write_pos: 0,
        }
    }

    /// Get writable slice at current write position
    pub fn write_slice(&mut self) -> &mut [u8] {
        &mut self.data[self.write_pos..]
    }

    /// Get readable slice from current read position
    pub fn read_slice(&self) -> &[u8] {
        &self.data[self.read_pos..self.write_pos]
    }

    /// Advance write position by count bytes
    pub fn advance_write(&mut self, count: usize) -> Result<(), HelperError> {
        if self.write_pos + count > self.capacity {
            return Err(HelperError::BufferError(
                "Write would exceed buffer capacity".to_string(),
            ));
        }
        self.write_pos += count;
        Ok(())
    }

    /// Advance read position by count bytes
    pub fn advance_read(&mut self, count: usize) -> Result<(), HelperError> {
        if self.read_pos + count > self.write_pos {
            return Err(HelperError::BufferError(
                "Read would exceed written data".to_string(),
            ));
        }
        self.read_pos += count;
        Ok(())
    }

    /// Reset buffer for reuse
    pub fn reset(&mut self) {
        self.read_pos = 0;
        self.write_pos = 0;
    }

    /// Get available write space
    pub fn write_available(&self) -> usize {
        self.capacity - self.write_pos
    }

    /// Get available read data length
    pub fn read_available(&self) -> usize {
        self.write_pos - self.read_pos
    }
}

impl Default for ZeroCopyBuffer {
    fn default() -> Self {
        Self::with_capacity(65536) // 64KB default
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zero_copy_buffer() {
        let mut buffer = ZeroCopyBuffer::with_capacity(1024);
        assert_eq!(buffer.write_available(), 1024);
        assert_eq!(buffer.read_available(), 0);

        // Simulate write
        buffer.advance_write(100).unwrap();
        assert_eq!(buffer.write_available(), 924);
        assert_eq!(buffer.read_available(), 100);

        // Simulate read
        buffer.advance_read(50).unwrap();
        assert_eq!(buffer.read_available(), 50);
    }

    #[test]
    fn test_buffer_overflow() {
        let mut buffer = ZeroCopyBuffer::with_capacity(100);
        buffer.advance_write(100).unwrap();

        // Should fail
        assert!(buffer.advance_write(1).is_err());
    }
}
