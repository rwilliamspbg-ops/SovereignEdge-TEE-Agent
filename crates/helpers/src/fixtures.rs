//! Test fixtures and sample data for testing

use crate::builders::{NetworkQualityBuilder, TelemetryFrameBuilder};
use common::{AgentMode, FrameMetadata, NetworkQuality, TelemetryFrame};

/// Generate a sample telemetry frame for testing
pub fn sample_frame() -> TelemetryFrame {
    TelemetryFrameBuilder::new()
        .with_frame_id(1)
        .with_source_ip("192.168.1.100")
        .with_payload_str("sample telemetry data")
        .build()
        .unwrap()
}

/// Generate a sample telemetry frame with custom ID
pub fn sample_frame_with_id(id: u64) -> TelemetryFrame {
    TelemetryFrameBuilder::new()
        .with_frame_id(id)
        .with_source_ip("192.168.1.100")
        .with_payload_str(format!("telemetry frame {}", id))
        .build()
        .unwrap()
}

/// Generate multiple sample frames
pub fn sample_frames(count: u64) -> Vec<TelemetryFrame> {
    (1..=count).map(sample_frame_with_id).collect()
}

/// Generate excellent network quality for testing
pub fn excellent_network() -> NetworkQuality {
    NetworkQualityBuilder::excellent()
}

/// Generate degraded network quality for testing
pub fn degraded_network() -> NetworkQuality {
    NetworkQualityBuilder::degraded()
}

/// Generate offline network quality for testing
pub fn offline_network() -> NetworkQuality {
    NetworkQualityBuilder::offline()
}

/// Generate sample frame metadata
pub fn sample_metadata() -> FrameMetadata {
    FrameMetadata {
        latency_ms: Some(45.0),
        packet_loss_pct: Some(0.5),
        jitter_ms: Some(5.0),
        agent_mode: Some("online".to_string()),
        session_id: Some("test-session-123".to_string()),
    }
}

/// Generate sample metadata for specific agent mode
pub fn metadata_for_mode(mode: AgentMode) -> FrameMetadata {
    FrameMetadata {
        latency_ms: Some(50.0),
        packet_loss_pct: Some(1.0),
        jitter_ms: Some(10.0),
        agent_mode: Some(format!("{:?}", mode)),
        session_id: Some("test-session".to_string()),
    }
}

/// Create a frame with network quality metadata
pub fn frame_with_quality(frame_id: u64, quality: &NetworkQuality) -> TelemetryFrame {
    let metadata = FrameMetadata {
        latency_ms: Some(quality.latency_ms),
        packet_loss_pct: Some(quality.packet_loss_pct),
        jitter_ms: Some(quality.jitter_ms),
        agent_mode: None,
        session_id: Some(format!("session-{}", frame_id)),
    };

    TelemetryFrameBuilder::new()
        .with_frame_id(frame_id)
        .with_source_ip("192.168.1.100")
        .with_payload_str(format!("frame {} data", frame_id))
        .with_metadata(metadata)
        .build()
        .unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sample_frame_generation() {
        let frame = sample_frame();
        assert_eq!(frame.frame_id, 1);
        assert!(!frame.payload.is_empty());
    }

    #[test]
    fn test_multiple_frames() {
        let frames = sample_frames(5);
        assert_eq!(frames.len(), 5);
        assert_eq!(frames[0].frame_id, 1);
        assert_eq!(frames[4].frame_id, 5);
    }

    #[test]
    fn test_network_quality_fixtures() {
        let excellent = excellent_network();
        assert!(!excellent.is_degraded());

        let degraded = degraded_network();
        assert!(degraded.is_degraded());

        let offline = offline_network();
        assert!(offline.is_offline());
    }

    #[test]
    fn test_frame_with_quality() {
        let quality = degraded_network();
        let frame = frame_with_quality(42, &quality);

        assert_eq!(frame.frame_id, 42);
        assert_eq!(frame.metadata.latency_ms, Some(quality.latency_ms));
    }
}
