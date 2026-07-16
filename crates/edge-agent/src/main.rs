//! Edge Agent Binary - Main entry point
//!
//! Runs the edge agent with network monitoring and frame processing

use anyhow::Result;
use clap::Parser;
use common::{FrameMetadata, NetworkQuality, TelemetryFrame};
use edge_agent::{hardware, EdgeAgent};
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

#[derive(Debug, Parser)]
#[command(name = "edge_agent")]
#[command(about = "SovereignEdge Edge Agent with graceful degradation", long_about = None)]
struct Args {
    /// Network quality probe interval in seconds
    #[arg(short, long, default_value_t = 5)]
    probe_interval: u64,

    /// Initial agent mode (online, degraded, offline)
    #[arg(long, default_value = "online")]
    mode: String,

    /// UDP port for telemetry
    #[arg(long, default_value_t = 47821)]
    port: u16,

    /// Path to a GGUF model for real local inference
    /// (requires building with --features llama)
    #[arg(long)]
    model: Option<std::path::PathBuf>,

    /// Number of transformer layers to offload to GPU (0 = CPU only)
    #[arg(long, default_value_t = 0)]
    gpu_layers: u32,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let args = Args::parse();

    info!("SovereignEdge Edge Agent starting...");
    info!(
        "Probe interval: {}s, Port: {}",
        args.probe_interval, args.port
    );

    // Detect local compute accelerators
    let accelerators = hardware::detect_accelerators();
    if accelerators.is_empty() {
        info!("No hardware accelerators detected - CPU-only inference");
    }
    for accel in &accelerators {
        info!("Detected accelerator: {} [{}]", accel, accel.read_sensors());
    }
    let gpu_available = accelerators.iter().any(|a| a.supports_llama_offload());

    // Create edge agent
    let mut agent = EdgeAgent::new(args.probe_interval);

    // Optionally load a real llama.cpp inference backend
    if let Some(ref model_path) = args.model {
        let gpu_layers = if gpu_available { args.gpu_layers } else { 0 };
        load_llama_backend(&mut agent, model_path, gpu_layers);
    }

    // Set mode change callback
    agent.set_mode_change_callback(|old_mode, new_mode| {
        info!("Mode changed: {:?} -> {:?}", old_mode, new_mode);
    });

    // Force initial mode via synthetic network quality if requested
    match args.mode.as_str() {
        "offline" => agent.update_network_quality(NetworkQuality {
            latency_ms: 10_000.0,
            packet_loss_pct: 100.0,
            jitter_ms: 0.0,
            bandwidth_mbps: 0.0,
            measured_at: helpers::time::now_ns(),
        }),
        "degraded" => agent.update_network_quality(NetworkQuality {
            latency_ms: 300.0,
            packet_loss_pct: 10.0,
            jitter_ms: 60.0,
            bandwidth_mbps: 5.0,
            measured_at: helpers::time::now_ns(),
        }),
        _ => {}
    }

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

#[cfg(feature = "llama")]
fn load_llama_backend(agent: &mut EdgeAgent, model_path: &std::path::Path, gpu_layers: u32) {
    match edge_agent::inference::LlamaCppBackend::load(model_path, gpu_layers) {
        Ok(backend) => agent.set_local_backend(Box::new(backend)),
        Err(e) => warn!(
            "Failed to load llama.cpp model {:?}: {} - using simulated backend",
            model_path, e
        ),
    }
}

#[cfg(not(feature = "llama"))]
fn load_llama_backend(_agent: &mut EdgeAgent, model_path: &std::path::Path, _gpu_layers: u32) {
    warn!(
        "--model {:?} given but binary was built without llama.cpp support; \
         rebuild with `cargo build -p edge-agent --features llama`",
        model_path
    );
}
