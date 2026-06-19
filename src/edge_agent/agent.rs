// Edge Agent with Local Inference and Graceful Degradation
// Integrates Mohawk-Inference-Engine primitives for local decision-making

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use std::collections::VecDeque;
use std::thread;

/// Network quality metrics
#[derive(Debug, Clone)]
pub struct NetworkQuality {
    pub latency_ms: f64,
    pub packet_loss_pct: f64,
    pub jitter_ms: f64,
    pub bandwidth_mbps: f64,
    pub last_measured: Instant,
}

impl NetworkQuality {
    fn new() -> Self {
        Self {
            latency_ms: 0.0,
            packet_loss_pct: 0.0,
            jitter_ms: 0.0,
            bandwidth_mbps: 0.0,
            last_measured: Instant::now(),
        }
    }
    
    fn is_degraded(&self) -> bool {
        self.latency_ms > 200.0 || 
        self.packet_loss_pct > 5.0 ||
        self.jitter_ms > 50.0
    }
    
    fn is_offline(&self) -> bool {
        self.latency_ms > 5000.0 || 
        self.packet_loss_pct > 50.0
    }
}

/// Rolling frame buffer for situational context
pub struct ContextBuffer {
    frames: VecDeque<Vec<u8>>,
    max_frames: usize,
    total_bytes: usize,
    max_bytes: usize,
}

impl ContextBuffer {
    fn new(max_frames: usize, max_bytes: usize) -> Self {
        Self {
            frames: VecDeque::with_capacity(max_frames),
            max_frames,
            total_bytes: 0,
            max_bytes,
        }
    }
    
    fn push(&mut self, frame: Vec<u8>) {
        if self.total_bytes + frame.len() > self.max_bytes {
            // Remove oldest frames until we have space
            while let Some(oldest) = self.frames.pop_front() {
                self.total_bytes -= oldest.len();
                if self.total_bytes + frame.len() <= self.max_bytes {
                    break;
                }
            }
        }
        
        self.total_bytes += frame.len();
        self.frames.push_back(frame);
        
        // Enforce max frames limit
        while self.frames.len() > self.max_frames {
            if let Some(oldest) = self.frames.pop_front() {
                self.total_bytes -= oldest.len();
            }
        }
    }
    
    fn get_recent(&self, count: usize) -> Vec<&[u8]> {
        self.frames.iter()
            .rev()
            .take(count)
            .map(|f| f.as_slice())
            .collect()
    }
    
    fn clear(&mut self) {
        self.frames.clear();
        self.total_bytes = 0;
    }
}

/// Local inference result
#[derive(Debug, Clone)]
pub struct InferenceResult {
    pub action: String,
    pub confidence: f32,
    pub metadata: std::collections::HashMap<String, String>,
    pub timestamp: Instant,
}

/// Edge agent operating modes
#[derive(Debug, Clone, PartialEq)]
pub enum AgentMode {
    Online,           // Full cloud offload to Qwen
    Degraded,         // High latency, partial local processing
    Offline,          // Complete local execution
    Transitioning,    // Switching between modes
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
}

impl EdgeAgent {
    pub fn new(probe_interval_secs: u64) -> Self {
        Self {
            mode: AgentMode::Online,
            network_quality: NetworkQuality::new(),
            context_buffer: ContextBuffer::new(100, 10 * 1024 * 1024), // 100 frames, 10MB
            mode_change_callback: None,
            last_mode: AgentMode::Online,
            probe_interval: Duration::from_secs(probe_interval_secs),
            running: Arc::new(Mutex::new(false)),
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
        
        thread::spawn(move || {
            println!("[EdgeAgent] Starting network quality monitoring...");
            
            while *running.lock().unwrap() {
                // Simulate network probe
                let quality = Self::probe_network_quality();
                println!(
                    "[NET_PROBE] latency={:.1}ms, loss={:.2}%, jitter={:.1}ms",
                    quality.latency_ms,
                    quality.packet_loss_pct,
                    quality.jitter_ms
                );
                
                thread::sleep(probe_interval);
            }
        });
        
        self.running.clone()
    }
    
    /// Probe network quality (simulated - in production use actual RTT measurements)
    fn probe_network_quality() -> NetworkQuality {
        // In production: measure actual RTT to cloud endpoint
        // Use ICMP ping, TCP handshake timing, or application-level probes
        
        NetworkQuality {
            latency_ms: 45.0 + (rand::random::<f64>() * 20.0),
            packet_loss_pct: rand::random::<f64>() * 2.0,
            jitter_ms: rand::random::<f64>() * 10.0,
            bandwidth_mbps: 100.0,
            last_measured: Instant::now(),
        }
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
        
        println!(
            "[EdgeAgent] Mode transition: {:?} -> {:?}",
            old_mode, new_mode
        );
        
        match new_mode {
            AgentMode::Online => {
                // Flush spooled state to cloud
                println!("[EdgeAgent] Flushing spooled state to cloud backend...");
                self.context_buffer.clear();
            }
            AgentMode::Degraded => {
                // Enable hybrid processing
                println!("[EdgeAgent] Entering degraded mode - hybrid processing enabled");
            }
            AgentMode::Offline => {
                // Spool all state locally
                println!("[EdgeAgent] OFFLINE MODE - All processing local, spooling state");
            }
            AgentMode::Transitioning => {}
        }
        
        self.mode = new_mode;
        self.last_mode = old_mode;
        
        if let Some(ref callback) = self.mode_change_callback {
            callback(old_mode, new_mode);
        }
    }
    
    /// Process incoming telemetry frame
    pub fn process_frame(&mut self, frame: Vec<u8>) -> InferenceResult {
        // Store in context buffer
        self.context_buffer.push(frame);
        
        match self.mode {
            AgentMode::Online => {
                // Stream to cloud for Qwen processing
                self.process_cloud_offload(&frame)
            }
            AgentMode::Degraded => {
                // Attempt cloud, fallback to local
                if self.can_reach_cloud() {
                    self.process_cloud_offload(&frame)
                } else {
                    self.process_local_inference(&frame)
                }
            }
            AgentMode::Offline => {
                // Pure local inference
                self.process_local_inference(&frame)
            }
            AgentMode::Transitioning => {
                // Buffer during transition
                InferenceResult {
                    action: "BUFFERED".to_string(),
                    confidence: 0.0,
                    metadata: std::collections::HashMap::new(),
                    timestamp: Instant::now(),
                }
            }
        }
    }
    
    fn can_reach_cloud(&self) -> bool {
        !self.network_quality.is_offline()
    }
    
    /// Cloud offload processing (Qwen API integration)
    fn process_cloud_offload(&self, frame: &[u8]) -> InferenceResult {
        // In production: send frame to TEE gateway which calls Qwen Cloud API
        println!("[EdgeAgent] Cloud offload: {} bytes", frame.len());
        
        // Simulated response
        InferenceResult {
            action: "CLOUD_ANALYSIS_COMPLETE".to_string(),
            confidence: 0.95,
            metadata: [("model".to_string(), "qwen-max".to_string())]
                .into_iter()
                .collect(),
            timestamp: Instant::now(),
        }
    }
    
    /// Local inference using embedded model
    fn process_local_inference(&self, frame: &[u8]) -> InferenceResult {
        // In production: run ONNX model via Mohawk-Inference-Engine
        println!("[EdgeAgent] Local inference: {} bytes", frame.len());
        
        // Simulated lightweight inference
        InferenceResult {
            action: "LOCAL_SAFETY_CHECK".to_string(),
            confidence: 0.75,
            metadata: [("model".to_string(), "qwen2.5-0.5b".to_string())]
                .into_iter()
                .collect(),
            timestamp: Instant::now(),
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
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_context_buffer() {
        let mut buffer = ContextBuffer::new(10, 1024);
        
        for i in 0..15 {
            buffer.push(vec![i; 100]);
        }
        
        assert_eq!(buffer.frames.len(), 10);
        assert!(buffer.total_bytes <= 1024);
    }
    
    #[test]
    fn test_network_quality_degraded() {
        let mut quality = NetworkQuality::new();
        quality.latency_ms = 250.0;
        assert!(quality.is_degraded());
    }
    
    #[test]
    fn test_network_quality_offline() {
        let mut quality = NetworkQuality::new();
        quality.packet_loss_pct = 60.0;
        assert!(quality.is_offline());
    }
}
