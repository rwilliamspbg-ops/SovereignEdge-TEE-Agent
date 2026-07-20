//! TEE Gateway binary — relays edge telemetry to Qwen Cloud (DashScope).
//!
//! This is the Alibaba Cloud integration point: it makes real API calls to
//! Qwen models on Qwen Cloud via the DashScope OpenAI-compatible endpoint.
//!
//! Usage:
//!   export QWEN_API_KEY=sk-...           # DashScope API key
//!   tee_gateway --prompt "engine temp 92C, vibration rising"
//!
//!   # Offline demo without an API key:
//!   tee_gateway --simulate --prompt "..."
//!
//! Mainland-China accounts: pass
//!   --endpoint https://dashscope.aliyuncs.com/compatible-mode/v1

use anyhow::{bail, Result};
use clap::Parser;
use common::{FrameMetadata, TelemetryFrame};
use tee_gateway::{TeeGateway, DASHSCOPE_INTL_ENDPOINT};
use tracing::info;
use tracing_subscriber::EnvFilter;

#[derive(Debug, Parser)]
#[command(name = "tee_gateway")]
#[command(about = "TEE gateway relaying edge telemetry to Qwen Cloud", long_about = None)]
struct Args {
    /// Telemetry text to analyze (stands in for a decrypted edge frame)
    #[arg(
        long,
        default_value = "temp=41C vib=0.2g rpm=1450 battery=87% gps_drift=0.4m"
    )]
    prompt: String,

    /// DashScope OpenAI-compatible endpoint
    #[arg(long, default_value = DASHSCOPE_INTL_ENDPOINT)]
    endpoint: String,

    /// Qwen model to use
    #[arg(long, default_value = "qwen-max")]
    model: String,

    /// Use the canned offline response instead of calling the API
    #[arg(long)]
    simulate: bool,
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let args = Args::parse();

    let (mut gateway, api_key) = if args.simulate {
        info!("[TEE] Simulated mode - no real API call");
        (TeeGateway::new(&args.endpoint), "simulated-key".to_string())
    } else {
        let key = std::env::var("QWEN_API_KEY").unwrap_or_default();
        if key.is_empty() {
            bail!(
                "QWEN_API_KEY is not set. Get a DashScope key from the Alibaba Cloud \
                 Model Studio console, or run with --simulate for the offline demo."
            );
        }
        (TeeGateway::new_live(&args.endpoint, &args.model), key)
    };

    // Attest + seal the API token, mirroring the TEE flow
    gateway.initialize(api_key.as_bytes())?;

    let frame = TelemetryFrame {
        frame_id: 1,
        source_ip: "edge-node-01".to_string(),
        dest_ip: None,
        timestamp_ns: helpers_now_ns(),
        payload: args.prompt.clone().into_bytes(),
        metadata: FrameMetadata::default(),
    };

    info!("[TEE] Processing frame: {}", args.prompt);
    let response = gateway.process_frame(&frame)?;

    println!("\n=== Qwen Cloud response ===");
    println!("request_id : {}", response.request_id);
    println!("model      : {}", response.model);
    println!(
        "tokens     : {} in / {} out",
        response.usage.prompt_tokens, response.usage.completion_tokens
    );
    println!("content    :\n{}", response.choices[0].message.content);

    let log = gateway.generate_execution_log(frame.frame_id, &response);
    println!("\n=== Execution log (hash-chained, for verification) ===");
    println!("{}", String::from_utf8_lossy(&log));

    Ok(())
}

fn helpers_now_ns() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}
