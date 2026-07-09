//! Builder patterns for constructing complex types

use common::{AgentMode, FrameMetadata, NetworkQuality, TelemetryFrame};

/// Builder for TelemetryFrame
#[derive(Default)]
pub struct TelemetryFrameBuilder {
    frame_id: Option<u64>,
    source_ip: Option<String>,
    dest_ip: Option<String>,
    timestamp_ns: Option<u64>,
    payload: Option<Vec<u8>>,
    metadata: Option<FrameMetadata>,
}

impl TelemetryFrameBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_frame_id(mut self, id: u64) -> Self {
        self.frame_id = Some(id);
        self
    }

    pub fn with_source_ip(mut self, ip: &str) -> Self {
        self.source_ip = Some(ip.to_string());
        self
    }

    pub fn with_dest_ip(mut self, ip: &str) -> Self {
        self.dest_ip = Some(ip.to_string());
        self
    }

    pub fn with_timestamp_ns(mut self, ts: u64) -> Self {
        self.timestamp_ns = Some(ts);
        self
    }

    pub fn with_payload(mut self, data: Vec<u8>) -> Self {
        self.payload = Some(data);
        self
    }

    pub fn with_payload_str(mut self, data: &str) -> Self {
        self.payload = Some(data.as_bytes().to_vec());
        self
    }

    pub fn with_metadata(mut self, meta: FrameMetadata) -> Self {
        self.metadata = Some(meta);
        self
    }

    pub fn build(self) -> Result<TelemetryFrame, &'static str> {
        Ok(TelemetryFrame {
            frame_id: self.frame_id.unwrap_or(0),
            source_ip: self.source_ip.unwrap_or_else(|| "0.0.0.0".to_string()),
            dest_ip: self.dest_ip,
            timestamp_ns: self.timestamp_ns.unwrap_or(0),
            payload: self.payload.unwrap_or_default(),
            metadata: self.metadata.unwrap_or_default(),
        })
    }
}

/// Builder for NetworkQuality
#[derive(Default)]
pub struct NetworkQualityBuilder {
    latency_ms: Option<f64>,
    packet_loss_pct: Option<f64>,
    jitter_ms: Option<f64>,
    bandwidth_mbps: Option<f64>,
    measured_at: Option<u64>,
}

impl NetworkQualityBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_latency_ms(mut self, latency: f64) -> Self {
        self.latency_ms = Some(latency);
        self
    }

    pub fn with_packet_loss_pct(mut self, loss: f64) -> Self {
        self.packet_loss_pct = Some(loss);
        self
    }

    pub fn with_jitter_ms(mut self, jitter: f64) -> Self {
        self.jitter_ms = Some(jitter);
        self
    }

    pub fn with_bandwidth_mbps(mut self, bw: f64) -> Self {
        self.bandwidth_mbps = Some(bw);
        self
    }

    pub fn with_measured_at(mut self, ts: u64) -> Self {
        self.measured_at = Some(ts);
        self
    }

    pub fn build(self) -> NetworkQuality {
        NetworkQuality {
            latency_ms: self.latency_ms.unwrap_or(0.0),
            packet_loss_pct: self.packet_loss_pct.unwrap_or(0.0),
            jitter_ms: self.jitter_ms.unwrap_or(0.0),
            bandwidth_mbps: self.bandwidth_mbps.unwrap_or(0.0),
            measured_at: self.measured_at.unwrap_or(0),
        }
    }

    /// Build a network quality representing excellent conditions
    pub fn excellent() -> NetworkQuality {
        Self::new()
            .with_latency_ms(10.0)
            .with_packet_loss_pct(0.1)
            .with_jitter_ms(2.0)
            .with_bandwidth_mbps(1000.0)
            .build()
    }

    /// Build a network quality representing degraded conditions
    pub fn degraded() -> NetworkQuality {
        Self::new()
            .with_latency_ms(250.0)
            .with_packet_loss_pct(6.0)
            .with_jitter_ms(60.0)
            .with_bandwidth_mbps(50.0)
            .build()
    }

    /// Build a network quality representing offline conditions
    pub fn offline() -> NetworkQuality {
        Self::new()
            .with_latency_ms(10000.0)
            .with_packet_loss_pct(100.0)
            .with_jitter_ms(0.0)
            .with_bandwidth_mbps(0.0)
            .build()
    }
}

/// Builder for FrameMetadata
#[derive(Default)]
pub struct FrameMetadataBuilder {
    latency_ms: Option<f64>,
    packet_loss_pct: Option<f64>,
    jitter_ms: Option<f64>,
    agent_mode: Option<String>,
    session_id: Option<String>,
}

impl FrameMetadataBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_latency_ms(mut self, latency: f64) -> Self {
        self.latency_ms = Some(latency);
        self
    }

    pub fn with_packet_loss_pct(mut self, loss: f64) -> Self {
        self.packet_loss_pct = Some(loss);
        self
    }

    pub fn with_jitter_ms(mut self, jitter: f64) -> Self {
        self.jitter_ms = Some(jitter);
        self
    }

    pub fn with_agent_mode(mut self, mode: AgentMode) -> Self {
        self.agent_mode = Some(format!("{:?}", mode));
        self
    }

    pub fn with_session_id(mut self, id: &str) -> Self {
        self.session_id = Some(id.to_string());
        self
    }

    pub fn build(self) -> FrameMetadata {
        FrameMetadata {
            latency_ms: self.latency_ms,
            packet_loss_pct: self.packet_loss_pct,
            jitter_ms: self.jitter_ms,
            agent_mode: self.agent_mode,
            session_id: self.session_id,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_telemetry_frame_builder() {
        let frame = TelemetryFrameBuilder::new()
            .with_frame_id(42)
            .with_source_ip("192.168.1.100")
            .with_payload_str("test data")
            .build()
            .unwrap();

        assert_eq!(frame.frame_id, 42);
        assert_eq!(frame.source_ip, "192.168.1.100");
        assert_eq!(frame.payload, b"test data");
    }

    #[test]
    fn test_network_quality_builders() {
        let excellent = NetworkQualityBuilder::excellent();
        assert!(!excellent.is_degraded());
        assert!(!excellent.is_offline());

        let degraded = NetworkQualityBuilder::degraded();
        assert!(degraded.is_degraded());
        assert!(!degraded.is_offline());

        let offline = NetworkQualityBuilder::offline();
        assert!(offline.is_offline());
    }

    #[test]
    fn test_frame_metadata_builder() {
        let meta = FrameMetadataBuilder::new()
            .with_latency_ms(50.0)
            .with_session_id("session-123")
            .with_agent_mode(AgentMode::Online)
            .build();

        assert_eq!(meta.latency_ms, Some(50.0));
        assert_eq!(meta.session_id, Some("session-123".to_string()));
    }
}
