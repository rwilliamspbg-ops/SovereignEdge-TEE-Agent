//! Edge Agent with Local Inference and Graceful Degradation
//!
//! This module implements the edge agent that:
//! - Monitors network quality
//! - Manages mode transitions (Online/Degraded/Offline)
//! - Processes telemetry frames with local or cloud inference
//! - Maintains rolling context buffer

use common::{AgentMode, ContextBuffer, NetworkQuality, TelemetryFrame};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use thiserror::Error;
use tracing::{debug, info, warn};

/// Edge Agent errors
#[derive(Error, Debug)]
pub enum AgentError {
    #[error("Network probe failed")]
    ProbeFailed,

    #[error("Mode transition failed: {0}")]
    ModeTransitionFailed(String),

    #[error("Inference failed: {0}")]
    InferenceFailed(String),
}

pub type Result<T> = std::result::Result<T, AgentError>;

/// Inference result from local or cloud processing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceResult {
    pub action: String,
    pub confidence: f32,
    pub metadata: HashMap<String, String>,
    pub timestamp: u64,
    pub source: InferenceSource,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InferenceSource {
    Cloud,
    Local,
    Hybrid,
}

/// Edge Agent state machine
pub struct EdgeAgent {
    mode: AgentMode,
    network_quality: NetworkQuality,
    context_buffer: ContextBuffer,
    mode_change_callback: Option<Box<dyn Fn(AgentMode, AgentMode) + Send>>,
    last_mode: AgentMode,
    probe_interval: Duration,
    running: Arc<Mutex<bool>>,
    frame_counter: u64,
    stats: AgentStats,
}

#[derive(Default)]
pub struct AgentStats {
    pub frames_processed: u64,
    pub cloud_offloads: u64,
    pub local_inferences: u64,
    pub mode_transitions: u64,
    pub probe_count: u64,
}

impl EdgeAgent {
    pub fn new(probe_interval_secs: u64) -> Self {
        Self {
            mode: AgentMode::Online,
            network_quality: NetworkQuality {
                latency_ms: 0.0,
                packet_loss_pct: 0.0,
                jitter_ms: 0.0,
                bandwidth_mbps: 0.0,
                measured_at: 0,
            },
            context_buffer: ContextBuffer::new(100, 10 * 1024 * 1024), // 100 frames, 10MB
            mode_change_callback: None,
            last_mode: AgentMode::Online,
            probe_interval: Duration::from_secs(probe_interval_secs),
            running: Arc::new(Mutex::new(false)),
            frame_counter: 0,
            stats: AgentStats::default(),
        }
    }

    pub fn set_mode_change_callback<F>(&mut self, callback: F)
    where
        F: Fn(AgentMode, AgentMode) + Send + 'static,
    {
        self.mode_change_callback = Some(Box::new(callback));
    }

    /// Start continuous network monitoring
    pub fn start_monitoring(&self) -> Arc<Mutex<bool>> {
        let running = self.running.clone();
        let probe_interval = self.probe_interval;

        info!(
            "Starting network quality monitoring with {:?} interval",
            probe_interval
        );

        let handle = running.clone();
        std::thread::spawn(move || {
            while *handle.lock().unwrap() {
                // Simulate network probe - in production use actual RTT measurements
                std::thread::sleep(probe_interval);
            }
        });

        self.running.clone()
    }

    /// Probe network quality (simulated - in production use actual RTT measurements)
    pub fn probe_network_quality(&mut self) -> NetworkQuality {
        self.stats.probe_count += 1;

        // In production: measure actual RTT to cloud endpoint
        // Use ICMP ping, TCP handshake timing, or application-level probes
        let quality = NetworkQuality {
            latency_ms: 45.0 + (rand::random::<f64>() * 20.0),
            packet_loss_pct: rand::random::<f64>() * 2.0,
            jitter_ms: rand::random::<f64>() * 10.0,
            bandwidth_mbps: 100.0,
            measured_at: helpers::time::now_ns(),
        };

        debug!(
            "[NET_PROBE] latency={:.1}ms, loss={:.2}%, jitter={:.1}ms",
            quality.latency_ms, quality.packet_loss_pct, quality.jitter_ms
        );

        quality
    }

    /// Update network quality and trigger mode transitions
    pub fn update_network_quality(&mut self, quality: NetworkQuality) {
        self.network_quality = quality;

        let new_mode = self.determine_mode();

        if new_mode != self.mode {
            self.transition_mode(new_mode);
        }
    }

    fn determine_mode(&self) -> AgentMode {
        if self.network_quality.is_offline() {
            AgentMode::Offline
        } else if self.network_quality.is_degraded() {
            AgentMode::Degraded
        } else {
            AgentMode::Online
        }
    }

    fn transition_mode(&mut self, new_mode: AgentMode) {
        let old_mode = self.mode.clone();
        self.mode = AgentMode::Transitioning;

        info!(
            "[EdgeAgent] Mode transition: {:?} -> {:?}",
            old_mode, new_mode
        );

        match new_mode {
            AgentMode::Online => {
                info!("[EdgeAgent] Flushing spooled state to cloud backend...");
                self.context_buffer.clear();
            }
            AgentMode::Degraded => {
                info!("[EdgeAgent] Entering degraded mode - hybrid processing enabled");
            }
            AgentMode::Offline => {
                warn!("[EdgeAgent] OFFLINE MODE - All processing local, spooling state");
            }
            AgentMode::Transitioning => {}
        }

        self.mode = new_mode.clone();
        self.last_mode = old_mode.clone();
        self.stats.mode_transitions += 1;

        if let Some(ref callback) = self.mode_change_callback {
            callback(old_mode, new_mode);
        }
    }

    /// Process incoming telemetry frame
    pub fn process_frame(&mut self, mut frame: TelemetryFrame) -> InferenceResult {
        self.frame_counter += 1;
        frame.frame_id = self.frame_counter;
        self.stats.frames_processed += 1;

        // Store in context buffer
        self.context_buffer.push(frame.clone());

        match self.mode {
            AgentMode::Online => {
                self.stats.cloud_offloads += 1;
                self.process_cloud_offload(&frame)
            }
            AgentMode::Degraded => {
                // Attempt cloud, fallback to local
                if self.can_reach_cloud() {
                    self.stats.cloud_offloads += 1;
                    self.process_cloud_offload(&frame)
                } else {
                    self.stats.local_inferences += 1;
                    self.process_local_inference(&frame)
                }
            }
            AgentMode::Offline => {
                self.stats.local_inferences += 1;
                self.process_local_inference(&frame)
            }
            AgentMode::Transitioning => {
                // Buffer during transition
                InferenceResult {
                    action: "BUFFERED".to_string(),
                    confidence: 0.0,
                    metadata: HashMap::new(),
                    timestamp: helpers::time::now_ns(),
                    source: InferenceSource::Local,
                }
            }
        }
    }

    fn can_reach_cloud(&self) -> bool {
        !self.network_quality.is_offline()
    }

    /// Cloud offload processing (Qwen API integration)
    fn process_cloud_offload(&self, frame: &TelemetryFrame) -> InferenceResult {
        info!("[EdgeAgent] Cloud offload: {} bytes", frame.payload.len());

        // In production: send frame to TEE gateway which calls Qwen Cloud API
        InferenceResult {
            action: "CLOUD_ANALYSIS_COMPLETE".to_string(),
            confidence: 0.95,
            metadata: [("model".to_string(), "qwen-max".to_string())]
                .into_iter()
                .collect(),
            timestamp: helpers::time::now_ns(),
            source: InferenceSource::Cloud,
        }
    }

    /// Local inference using embedded model
    fn process_local_inference(&self, frame: &TelemetryFrame) -> InferenceResult {
        info!("[EdgeAgent] Local inference: {} bytes", frame.payload.len());

        // In production: run ONNX model via candle or llama.cpp bindings
        InferenceResult {
            action: "LOCAL_SAFETY_CHECK".to_string(),
            confidence: 0.75,
            metadata: [("model".to_string(), "qwen2.5-0.5b".to_string())]
                .into_iter()
                .collect(),
            timestamp: helpers::time::now_ns(),
            source: InferenceSource::Local,
        }
    }

    /// Get current operating mode
    pub fn mode(&self) -> &AgentMode {
        &self.mode
    }

    /// Get network quality metrics
    pub fn network_quality(&self) -> &NetworkQuality {
        &self.network_quality
    }

    /// Get agent statistics
    pub fn stats(&self) -> &AgentStats {
        &self.stats
    }

    /// Stop the agent
    pub fn stop(&self) {
        *self.running.lock().unwrap() = false;
        info!("Edge agent stopped");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_creation() {
        let agent = EdgeAgent::new(5);
        assert_eq!(*agent.mode(), AgentMode::Online);
    }

    #[test]
    fn test_mode_determination() {
        let mut agent = EdgeAgent::new(5);

        // Good network -> Online
        agent.network_quality = NetworkQuality {
            latency_ms: 50.0,
            packet_loss_pct: 0.5,
            jitter_ms: 5.0,
            bandwidth_mbps: 100.0,
            measured_at: 0,
        };
        assert_eq!(agent.determine_mode(), AgentMode::Online);

        // Degraded network
        agent.network_quality.latency_ms = 250.0;
        assert_eq!(agent.determine_mode(), AgentMode::Degraded);

        // Offline
        agent.network_quality.packet_loss_pct = 60.0;
        assert_eq!(agent.determine_mode(), AgentMode::Offline);
    }

    #[test]
    fn test_frame_processing() {
        let mut agent = EdgeAgent::new(5);

        let frame = TelemetryFrame {
            frame_id: 0,
            source_ip: "192.168.1.1".to_string(),
            dest_ip: None,
            timestamp_ns: 0,
            payload: vec![1, 2, 3, 4],
            metadata: Default::default(),
        };

        let result = agent.process_frame(frame);
        assert_eq!(result.source, InferenceSource::Cloud);
        assert_eq!(agent.stats().frames_processed, 1);
    }
}
