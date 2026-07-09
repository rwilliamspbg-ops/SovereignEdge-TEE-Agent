//! Common types and utilities for SovereignEdge-TEE-Agent
//!
//! This crate provides shared types used across all modules:
//! - Frame structures for telemetry data
//! - Configuration types
//! - Error types
//! - Telemetry and metrics structures

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Telemetry frame structure for edge-to-cloud communication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryFrame {
    pub frame_id: u64,
    pub source_ip: String,
    pub dest_ip: Option<String>,
    pub timestamp_ns: u64,
    pub payload: Vec<u8>,
    pub metadata: FrameMetadata,
}

/// Metadata attached to telemetry frames
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FrameMetadata {
    pub latency_ms: Option<f64>,
    pub packet_loss_pct: Option<f64>,
    pub jitter_ms: Option<f64>,
    pub agent_mode: Option<String>,
    pub session_id: Option<String>,
}

/// Network quality measurement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkQuality {
    pub latency_ms: f64,
    pub packet_loss_pct: f64,
    pub jitter_ms: f64,
    pub bandwidth_mbps: f64,
    pub measured_at: u64,
}

impl NetworkQuality {
    /// Check if network is in degraded state
    pub fn is_degraded(&self) -> bool {
        self.latency_ms > 200.0
            || self.packet_loss_pct > 5.0
            || self.jitter_ms > 50.0
    }

    /// Check if network is effectively offline
    pub fn is_offline(&self) -> bool {
        self.latency_ms > 5000.0 || self.packet_loss_pct > 50.0
    }
}

/// Agent operating modes
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentMode {
    Online,
    Degraded,
    Offline,
    Transitioning,
}

impl Default for AgentMode {
    fn default() -> Self {
        AgentMode::Online
    }
}

/// PQC hybrid public key
#[derive(Debug, Clone)]
pub struct HybridPublicKey {
    pub x25519_pubkey: [u8; 32],
    pub mlkem_pubkey: [u8; 1088],
}

/// Encrypted frame for transport
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedFrame {
    pub frame_id: u64,
    pub nonce: [u8; 12],
    pub ciphertext: Vec<u8>,
    pub tag: [u8; 16],
}

/// Common error types
#[derive(Error, Debug)]
pub enum CommonError {
    #[error("Invalid frame size: expected {expected}, got {actual}")]
    InvalidFrameSize { expected: usize, actual: usize },

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Buffer overflow: attempted to write {requested} bytes but only {available} available")]
    BufferOverflow { requested: usize, available: usize },

    #[error("Configuration error: {0}")]
    Config(String),
}

pub type Result<T> = std::result::Result<T, CommonError>;

/// Rolling context buffer with LRU eviction
pub struct ContextBuffer {
    frames: Vec<TelemetryFrame>,
    max_frames: usize,
    max_bytes: usize,
    total_bytes: usize,
}

impl ContextBuffer {
    pub fn new(max_frames: usize, max_bytes: usize) -> Self {
        Self {
            frames: Vec::with_capacity(max_frames),
            max_frames,
            max_bytes,
            total_bytes: 0,
        }
    }

    pub fn push(&mut self, frame: TelemetryFrame) {
        let frame_size = frame.payload.len();

        // Evict oldest frames if necessary
        while self.total_bytes + frame_size > self.max_bytes && !self.frames.is_empty() {
            if let Some(oldest) = self.frames.remove(0) {
                self.total_bytes = self.total_bytes.saturating_sub(oldest.payload.len());
            }
        }

        // Enforce max frames limit
        while self.frames.len() >= self.max_frames && !self.frames.is_empty() {
            if let Some(oldest) = self.frames.remove(0) {
                self.total_bytes = self.total_bytes.saturating_sub(oldest.payload.len());
            }
        }

        self.total_bytes += frame_size;
        self.frames.push(frame);
    }

    pub fn get_recent(&self, count: usize) -> Vec<&TelemetryFrame> {
        self.frames.iter().rev().take(count).collect()
    }

    pub fn len(&self) -> usize {
        self.frames.len()
    }

    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }

    pub fn clear(&mut self) {
        self.frames.clear();
        self.total_bytes = 0;
    }

    pub fn total_bytes(&self) -> usize {
        self.total_bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_network_quality_degraded() {
        let quality = NetworkQuality {
            latency_ms: 250.0,
            packet_loss_pct: 1.0,
            jitter_ms: 10.0,
            bandwidth_mbps: 100.0,
            measured_at: 0,
        };
        assert!(quality.is_degraded());
        assert!(!quality.is_offline());
    }

    #[test]
    fn test_network_quality_offline() {
        let quality = NetworkQuality {
            latency_ms: 6000.0,
            packet_loss_pct: 60.0,
            jitter_ms: 100.0,
            bandwidth_mbps: 0.0,
            measured_at: 0,
        };
        assert!(quality.is_offline());
    }

    #[test]
    fn test_context_buffer_eviction() {
        let mut buffer = ContextBuffer::new(10, 1024);

        for i in 0..15 {
            let frame = TelemetryFrame {
                frame_id: i,
                source_ip: "192.168.1.1".to_string(),
                dest_ip: None,
                timestamp_ns: i * 1000,
                payload: vec![i as u8; 100],
                metadata: FrameMetadata::default(),
            };
            buffer.push(frame);
        }

        assert!(buffer.len() <= 10);
        assert!(buffer.total_bytes() <= 1024);
    }
}
