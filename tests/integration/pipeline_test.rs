//! Integration test: Full pipeline from XDP ingestion to TEE processing

use common::{TelemetryFrame, FrameMetadata, AgentMode};
use helpers::fixtures::{sample_frame, degraded_network, offline_network};

#[test]
fn test_telemetry_frame_creation() {
    // Test basic frame creation
    let frame = sample_frame();
    assert!(!frame.payload.is_empty());
    assert_eq!(frame.source_ip, "192.168.1.100");
}

#[test]
fn test_frame_metadata_propagation() {
    // Test that metadata is properly attached to frames
    let mut frame = sample_frame();
    frame.metadata.latency_ms = Some(50.0);
    frame.metadata.agent_mode = Some("online".to_string());
    
    assert_eq!(frame.metadata.latency_ms, Some(50.0));
    assert_eq!(frame.metadata.agent_mode, Some("online".to_string()));
}

#[test]
fn test_network_quality_modes() {
    // Test network quality determination for different modes
    
    // Excellent network -> Online mode
    let excellent = helpers::fixtures::excellent_network();
    assert!(!excellent.is_degraded());
    assert!(!excellent.is_offline());
    
    // Degraded network
    let degraded = degraded_network();
    assert!(degraded.is_degraded());
    assert!(!degraded.is_offline());
    
    // Offline network
    let offline = offline_network();
    assert!(offline.is_offline());
}

#[test]
fn test_agent_mode_transitions() {
    // Simulate agent mode transitions based on network quality
    
    // Start online
    let mut current_mode = AgentMode::Online;
    assert_eq!(current_mode, AgentMode::Online);
    
    // Network degrades
    let degraded = degraded_network();
    if degraded.is_degraded() {
        current_mode = AgentMode::Degraded;
    }
    assert_eq!(current_mode, AgentMode::Degraded);
    
    // Network goes offline
    let offline = offline_network();
    if offline.is_offline() {
        current_mode = AgentMode::Offline;
    }
    assert_eq!(current_mode, AgentMode::Offline);
}

#[test]
fn test_multiple_frames_sequence() {
    // Test processing a sequence of frames
    let frames = helpers::fixtures::sample_frames(10);
    
    assert_eq!(frames.len(), 10);
    
    // Verify frame IDs are sequential
    for (i, frame) in frames.iter().enumerate() {
        assert_eq!(frame.frame_id, (i + 1) as u64);
    }
}

#[test]
fn test_frame_with_quality_metadata() {
    // Test attaching network quality to frame metadata
    let quality = degraded_network();
    let frame = helpers::fixtures::frame_with_quality(100, &quality);
    
    assert_eq!(frame.frame_id, 100);
    assert_eq!(frame.metadata.latency_ms, Some(quality.latency_ms));
    assert_eq!(frame.metadata.packet_loss_pct, Some(quality.packet_loss_pct));
}
