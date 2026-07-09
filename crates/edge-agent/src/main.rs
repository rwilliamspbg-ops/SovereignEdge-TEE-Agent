//! Edge Agent Binary - Main entry point
//!
//! Runs the edge agent with network monitoring and frame processing

use anyhow::Result;
use clap::Parser;
use common::{AgentMode, TelemetryFrame, FrameMetadata};
use edge_agent::EdgeAgent;
use std::sync::Arc;
use tracing::{info, error};
use tracing_subscriber::{fmt, EnvFilter};

#[derive(Debug, Parser)]
#[command(name = "edge_agent")]
#[command(about = "SovereignEdge Edge Agent with graceful degradation", long_about = None)]
struct Args {
    /// Network quality probe interval in seconds
    #[arg(short, long, default_value_t = 5)]
    probe_interval: u64,

    /// Initial agent mode
    #[arg(long, default_value = "online")]
    mode: String,

    /// UDP port for telemetry
    #[arg(short, long, default_value_t = 47821)]
    port: u16,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let args = Args::parse();

    info!("SovereignEdge Edge Agent starting...");
    info!("Probe interval: {}s, Port: {}", args.probe_interval, args.port);

    // Create edge agent
    let mut agent = EdgeAgent::new(args.probe_interval);

    // Set mode change callback
    agent.set_mode_change_callback(|old_mode, new_mode| {
        info!("Mode changed: {:?} -> {:?}", old_mode, new_mode);
    });

    // Start network monitoring
    let _running = agent.start_monitoring();

    // Simulate some telemetry frames for demo
    for i in 0..5 {
        let frame = TelemetryFrame {
            frame_id: 0,
            source_ip: "192.168.1.1".to_string(),
            dest_ip: None,
            timestamp_ns: 0,
            payload: format!("telemetry-data-{}", i).into_bytes(),
            metadata: FrameMetadata::default(),
        };

        let result = agent.process_frame(frame);
        info!(
            "Frame {}: action={}, confidence={:.2}, source={:?}",
            i + 1,
            result.action,
            result.confidence,
            result.source
        );

        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }

    // Print final stats
    let stats = agent.stats();
    info!(
        "Final stats: {} frames, {} cloud offloads, {} local inferences, {} mode transitions",
        stats.frames_processed,
        stats.cloud_offloads,
        stats.local_inferences,
        stats.mode_transitions
    );

    info!("Edge agent shutting down...");
    agent.stop();

    Ok(())
}
