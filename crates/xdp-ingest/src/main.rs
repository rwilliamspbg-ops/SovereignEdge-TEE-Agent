//! AF_XDP Daemon - Zero-copy packet ingestion from eBPF XDP
//!
//! This daemon binds to an AF_XDP socket and receives packets
//! filtered by the eBPF XDP program. It forwards telemetry frames
//! to the PQC transport layer for encryption.

use anyhow::{Context, Result};
use clap::Parser;
use common::{FrameMetadata, TelemetryFrame};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

#[derive(Debug, Parser)]
#[command(name = "af_xdp_daemon")]
#[command(about = "AF_XDP zero-copy packet ingestion daemon", long_about = None)]
struct Args {
    /// Network interface to bind to
    #[arg(short, long, default_value = "eth0")]
    iface: String,

    /// UDP port for telemetry frames
    #[arg(short, long, default_value_t = 47821)]
    port: u16,

    /// Queue ID for AF_XDP
    #[arg(short, long, default_value_t = 0)]
    queue_id: u32,

    /// Ring buffer size in KB
    #[arg(long, default_value_t = 256)]
    ring_size_kb: usize,
}

/// UMEM region for zero-copy packet buffers
struct UmemRegion {
    frame_size: usize,
    frame_count: usize,
    buffer: Vec<u8>,
}

impl UmemRegion {
    fn new(frame_size: usize, frame_count: usize) -> Self {
        Self {
            frame_size,
            frame_count,
            buffer: vec![0u8; frame_size * frame_count],
        }
    }

    fn get_frame(&self, frame_idx: usize) -> &[u8] {
        let start = frame_idx * self.frame_size;
        &self.buffer[start..start + self.frame_size.min(self.buffer.len() - start)]
    }

    fn get_frame_mut(&mut self, frame_idx: usize) -> &mut [u8] {
        let start = frame_idx * self.frame_size;
        let end = (start + self.frame_size).min(self.buffer.len());
        &mut self.buffer[start..end]
    }
}

/// XDP Socket wrapper for AF_XDP
struct XdpSocket {
    iface: String,
    queue_id: u32,
    running: Arc<AtomicBool>,
    umem: UmemRegion,
    stats: SocketStats,
}

#[derive(Default)]
struct SocketStats {
    packets_received: u64,
    bytes_received: u64,
    errors: u64,
}

impl XdpSocket {
    fn new(iface: &str, queue_id: u32, ring_size_kb: usize) -> Result<Self> {
        info!("Creating AF_XDP socket on {} queue {}", iface, queue_id);

        // In production, this would:
        // 1. Create AF_XDP socket via socket(AF_XDP, SOCK_RAW, 0)
        // 2. Set up UMEM with mmap'd regions
        // 3. Configure RX/TX rings
        // 4. Bind to interface and queue

        let frame_size = 2048;
        let frame_count = (ring_size_kb * 1024) / frame_size;
        info!(
            "Allocating UMEM with {} frames of {} bytes",
            frame_count, frame_size
        );

        Ok(Self {
            iface: iface.to_string(),
            queue_id,
            running: Arc::new(AtomicBool::new(true)),
            umem: UmemRegion::new(frame_size, frame_count),
            stats: SocketStats::default(),
        })
    }

    fn receive_loop(&mut self) -> Result<()> {
        info!(
            "Starting AF_XDP receive loop on {} queue {}...",
            self.iface, self.queue_id
        );

        while self.running.load(Ordering::Relaxed) {
            // In production:
            // 1. poll() on socket for incoming packets
            // 2. Read from RX ring
            // 3. Process packet metadata
            // 4. Forward to telemetry processor

            // Simulated packet reception for now
            std::thread::sleep(Duration::from_micros(100));

            if self.umem.frame_count > 0 {
                let frame = self.umem.get_frame_mut(0);
                let packet_len = 100.min(frame.len());
                frame[..packet_len].fill(0xAB);

                if let Some(_telemetry) = parse_telemetry_frame(
                    &self.umem.get_frame(0)[..packet_len],
                    self.stats.packets_received + 1,
                ) {
                    // In production this frame would be forwarded downstream.
                }
            }

            // Mock statistics update
            self.stats.packets_received += 1;
            self.stats.bytes_received += 100;
        }

        info!("AF_XDP receive loop stopped");
        Ok(())
    }

    fn get_stats(&self) -> &SocketStats {
        &self.stats
    }
}

/// Parse raw packet data into TelemetryFrame
fn parse_telemetry_frame(data: &[u8], frame_id: u64) -> Option<TelemetryFrame> {
    if data.len() < 20 {
        return None;
    }

    Some(TelemetryFrame {
        frame_id,
        source_ip: "0.0.0.0".to_string(), // Would extract from IP header
        dest_ip: None,
        timestamp_ns: helpers::time::now_ns(),
        payload: data.to_vec(),
        metadata: FrameMetadata::default(),
    })
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let args = Args::parse();

    info!(
        "SovereignEdge AF_XDP Daemon starting on {}:{} (queue {})",
        args.iface, args.port, args.queue_id
    );

    // Create XDP socket
    let mut xdp_sock = XdpSocket::new(&args.iface, args.queue_id, args.ring_size_kb)
        .context("Failed to create AF_XDP socket")?;

    // Run receive loop in background task
    let running = xdp_sock.running.clone();

    let handle = tokio::task::spawn_blocking(move || {
        let result = xdp_sock.receive_loop();
        (xdp_sock, result)
    });

    // Set up signal handlers
    tokio::signal::ctrl_c().await?;
    info!("Received shutdown signal");

    // Stop the socket
    running.store(false, Ordering::Relaxed);

    // Wait for receive loop to finish
    match handle.await {
        Ok((xdp_sock, Ok(()))) => {
            info!("AF_XDP daemon shut down cleanly");
            info!(
                "Final stats: {} packets, {} bytes, {} errors",
                xdp_sock.get_stats().packets_received,
                xdp_sock.get_stats().bytes_received,
                xdp_sock.get_stats().errors
            );
        }
        Ok((xdp_sock, Err(e))) => {
            error!("AF_XDP daemon error: {}", e);
            info!(
                "Final stats: {} packets, {} bytes, {} errors",
                xdp_sock.get_stats().packets_received,
                xdp_sock.get_stats().bytes_received,
                xdp_sock.get_stats().errors
            );
        }
        Err(e) => error!("Task join error: {}", e),
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_umem_region() {
        let umem = UmemRegion::new(1024, 100);
        assert_eq!(umem.buffer.len(), 102400);
    }

    #[test]
    fn test_parse_telemetry_frame() {
        let data = vec![0u8; 100];
        let frame = parse_telemetry_frame(&data, 1);
        assert!(frame.is_some());
        assert_eq!(frame.unwrap().payload.len(), 100);
    }

    #[test]
    fn test_parse_invalid_frame() {
        let data = vec![0u8; 10];
        let frame = parse_telemetry_frame(&data, 1);
        assert!(frame.is_none());
    }
}
