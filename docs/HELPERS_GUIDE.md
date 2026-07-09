# Helper Utilities Guide

The `helpers` crate provides developer-friendly utilities to improve code quality and testing experience.

## Overview

```rust
use helpers::{builders, fixtures, time, metrics};
```

## Time Utilities

Fix the common timestamp calculation bugs found in the codebase:

```rust
use helpers::time;

// Get current time in nanoseconds
let now = time::now_ns();

// Calculate elapsed time correctly
let start = Instant::now();
// ... do work ...
let elapsed = time::elapsed_ms(start);

// Format duration for display
let formatted = time::format_duration(Duration::from_millis(1500));
// Returns: "1.50s"
```

**Problem this solves**: The original codebase had多处 instances of:
```rust
// BUG: Always returns ~0!
timestamp_ns: Instant::now().duration_since(Instant::now()).as_nanos()
```

## Builder Patterns

Construct complex types fluently:

```rust
use helpers::builders::{TelemetryFrameBuilder, NetworkQualityBuilder, FrameMetadataBuilder};

// Build a telemetry frame
let frame = TelemetryFrameBuilder::new()
    .with_frame_id(42)
    .with_source_ip("192.168.1.100")
    .with_payload_str("sensor data")
    .build()?;

// Build network quality scenarios
let excellent = NetworkQualityBuilder::excellent();
let degraded = NetworkQualityBuilder::degraded();
let offline = NetworkQualityBuilder::offline();

// Custom network quality
let custom = NetworkQualityBuilder::new()
    .with_latency_ms(75.0)
    .with_packet_loss_pct(2.5)
    .with_bandwidth_mbps(500.0)
    .build();
```

## Test Fixtures

Pre-built sample data for testing:

```rust
use helpers::fixtures;

// Single sample frame
let frame = fixtures::sample_frame();

// Multiple frames with sequential IDs
let frames = fixtures::sample_frames(10);

// Network quality for different scenarios
let good_network = fixtures::excellent_network();
let bad_network = fixtures::degraded_network();
let no_network = fixtures::offline_network();

// Frame with quality metadata attached
let frame_with_qoS = fixtures::frame_with_quality(100, &bad_network);
```

## Metrics Helpers

Placeholder functions for Prometheus integration:

```rust
use helpers::metrics;

// Record events (stubs - implement with actual Prometheus)
metrics::observe_latency(45.2);
metrics::record_mode_transition("online", "degraded");
metrics::record_frame_processed("cloud", 2048);
metrics::record_pqc_event("session_created");
metrics::record_tee_event("api_call", 120.5);
metrics::record_zk_event("proof_verified", true);

// Export metrics in Prometheus format
let prometheus_text = metrics::export_metrics();
```

## Zero-Copy Buffer

Efficient buffer management for zero-copy pipelines:

```rust
use helpers::ZeroCopyBuffer;

let mut buffer = ZeroCopyBuffer::with_capacity(65536);

// Write data
buffer.advance_write(1024)?;

// Read data
let available = buffer.read_available();
buffer.advance_read(512)?;

// Reset for reuse
buffer.reset();
```

## Adding Helpers to Your Crate

1. Add dependency to `Cargo.toml`:
```toml
[dependencies]
helpers = { path = "../helpers" }
```

2. Import what you need:
```rust
use helpers::{time, builders, fixtures};
```

3. Use the utilities in your code!

## Contributing New Helpers

When adding new helper functions:

1. Place in appropriate module (`time.rs`, `builders.rs`, etc.)
2. Add comprehensive documentation
3. Include unit tests
4. Consider if it belongs in `common` instead (if used by external consumers)
