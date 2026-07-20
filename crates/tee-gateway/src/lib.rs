//! TEE Gateway - Secure Enclave Runtime for Qwen Cloud Integration
//!
//! This module provides:
//! - TEE backend abstraction (simulated, SGX, SEV-SNP, Alibaba Cloud)
//! - Qwen Cloud API integration via reqwest
//! - Execution log generation for ZK verification

use common::TelemetryFrame;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use thiserror::Error;
use tracing::{debug, info};

/// TEE Gateway errors
#[derive(Error, Debug)]
pub enum GatewayError {
    #[error("Sealing failed: {0}")]
    SealingFailed(String),

    #[error("Unsealing failed: {0}")]
    UnsealingFailed(String),

    #[error("Attestation failed")]
    AttestationFailed,

    #[error("API key not initialized")]
    ApiKeyNotInitialized,

    #[error("Qwen API error: {0}")]
    QwenApiError(String),

    #[error("HTTP error: {0}")]
    HttpError(#[from] reqwest::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, GatewayError>;

/// Attestation report from a TEE backend
#[derive(Debug, Clone)]
pub struct AttestationReport {
    pub quote: Vec<u8>,
    pub measurement: Vec<u8>,
}

/// TEE backend trait for pluggable attestation and sealing
pub trait TeeBackend: Send + Sync {
    /// Seal sensitive data (API tokens, credentials)
    fn seal(&self, plaintext: &[u8]) -> Result<Vec<u8>>;

    /// Unseal previously sealed data
    fn unseal(&self, sealed: &[u8]) -> Result<Vec<u8>>;

    /// Verify TEE attestation and return a report
    fn attest(&self) -> Result<AttestationReport>;
}

/// Simulated TEE backend (default, for development/testing)
pub struct SimulatedTee {
    encryption_key: [u8; 32],
}

impl SimulatedTee {
    pub fn new() -> Self {
        let mut key = [0u8; 32];
        for (i, byte) in key.iter_mut().enumerate() {
            *byte = (i * 7 + 13) as u8;
        }
        Self {
            encryption_key: key,
        }
    }
}

impl Default for SimulatedTee {
    fn default() -> Self {
        Self::new()
    }
}

impl TeeBackend for SimulatedTee {
    fn seal(&self, plaintext: &[u8]) -> Result<Vec<u8>> {
        // Simulated sealing: XOR with key (NOT cryptographically secure)
        let mut sealed = plaintext.to_vec();
        for (i, byte) in sealed.iter_mut().enumerate() {
            *byte ^= self.encryption_key[i % 32];
        }
        Ok(sealed)
    }

    fn unseal(&self, sealed: &[u8]) -> Result<Vec<u8>> {
        // Simulated unsealing: XOR with key (symmetric)
        let mut plaintext = sealed.to_vec();
        for (i, byte) in plaintext.iter_mut().enumerate() {
            *byte ^= self.encryption_key[i % 32];
        }
        Ok(plaintext)
    }

    fn attest(&self) -> Result<AttestationReport> {
        let quote = b"simulated-tee-quote-v1".to_vec();
        let measurement = Sha256::digest(b"simulated-tee-measurement").to_vec();
        info!("Simulated TEE attestation verified");
        Ok(AttestationReport { quote, measurement })
    }
}

/// Sealed storage for API tokens (TEE-specific)
pub struct SealedStorage<T: TeeBackend> {
    tee: T,
    sealed_data: Vec<u8>,
    attested: bool,
}

impl<T: TeeBackend> SealedStorage<T> {
    pub fn new(tee: T) -> Self {
        Self {
            tee,
            sealed_data: Vec::new(),
            attested: false,
        }
    }

    /// Seal sensitive data (API tokens, credentials)
    pub fn seal(&mut self, plaintext: &[u8]) -> Result<()> {
        self.sealed_data = self.tee.seal(plaintext)?;
        debug!("Sealed {} bytes of sensitive data", plaintext.len());
        Ok(())
    }

    /// Unseal previously sealed data
    pub fn unseal(&self) -> Result<Vec<u8>> {
        if !self.attested {
            return Err(GatewayError::AttestationFailed);
        }

        if self.sealed_data.is_empty() {
            return Err(GatewayError::UnsealingFailed(
                "No sealed data available".to_string(),
            ));
        }
        self.tee.unseal(&self.sealed_data)
    }

    /// Verify TEE attestation
    pub fn verify_attestation(&mut self) -> Result<AttestationReport> {
        let report = self.tee.attest()?;
        self.attested = true;
        Ok(report)
    }

    pub fn is_attested(&self) -> bool {
        self.attested
    }
}

/// Qwen API response structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QwenResponse {
    pub request_id: String,
    pub model: String,
    pub choices: Vec<QwenChoice>,
    pub usage: QwenUsage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QwenChoice {
    pub index: usize,
    pub message: QwenMessage,
    pub finish_reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QwenMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QwenUsage {
    pub prompt_tokens: usize,
    pub completion_tokens: usize,
    pub total_tokens: usize,
}

/// Session state for caching
struct SessionState {
    last_activity: Instant,
    frame_count: u64,
}

/// Gateway statistics
#[derive(Debug, Default)]
pub struct GatewayStats {
    pub processed_frames: u64,
    pub active_sessions: usize,
    pub api_calls: u64,
}

const SYSTEM_PROMPT: &str = "You are an AI agent analyzing real-time edge telemetry.
Your task is to provide actionable insights and decisions based on sensor data.
Respond with structured JSON containing: action, confidence, reasoning.";

/// Default DashScope OpenAI-compatible endpoint (international).
/// Mainland China accounts use `https://dashscope.aliyuncs.com/compatible-mode/v1`.
pub const DASHSCOPE_INTL_ENDPOINT: &str = "https://dashscope-intl.aliyuncs.com/compatible-mode/v1";

/// TEE Gateway relay engine
pub struct TeeGateway<T: TeeBackend> {
    storage: SealedStorage<T>,
    qwen_api_key: Option<String>,
    qwen_endpoint: String,
    qwen_model: String,
    /// When true, `call_qwen_api` performs a real HTTPS call to DashScope;
    /// when false it returns a canned response (offline demo / tests).
    live: bool,
    processed_frames: u64,
    session_cache: Arc<Mutex<HashMap<String, SessionState>>>,
    http_client: Option<Client>,
}

impl<T: TeeBackend> TeeGateway<T> {
    pub fn new(tee: T, qwen_endpoint: &str) -> Self {
        Self {
            storage: SealedStorage::new(tee),
            qwen_api_key: None,
            qwen_endpoint: qwen_endpoint.to_string(),
            qwen_model: "qwen-max".to_string(),
            live: false,
            processed_frames: 0,
            session_cache: Arc::new(Mutex::new(HashMap::new())),
            http_client: None,
        }
    }

    /// Gateway that makes real DashScope (Qwen Cloud) API calls.
    pub fn new_live(tee: T, qwen_endpoint: &str, model: &str) -> Self {
        let mut gw = Self::new(tee, qwen_endpoint);
        gw.qwen_model = model.to_string();
        gw.live = true;
        gw
    }

    /// Initialize gateway with sealed API token
    pub fn initialize(&mut self, api_token: &[u8]) -> Result<()> {
        // First verify attestation
        self.storage.verify_attestation()?;

        // Seal the API token in TEE storage
        self.storage.seal(api_token)?;

        // Store in memory for current session
        self.qwen_api_key = Some(String::from_utf8_lossy(api_token).to_string());

        // Initialize HTTP client
        self.http_client = Some(
            Client::builder()
                .build()
                .expect("failed to build HTTP client"),
        );

        info!("[TEE] Gateway initialized with sealed storage");
        Ok(())
    }

    /// Process incoming PQC-decrypted telemetry frame
    pub async fn process_frame(&mut self, frame: &TelemetryFrame) -> Result<QwenResponse> {
        self.processed_frames += 1;

        // Update session cache
        {
            let mut cache = self.session_cache.lock().unwrap();
            let session = cache
                .entry(frame.source_ip.clone())
                .or_insert(SessionState {
                    last_activity: Instant::now(),
                    frame_count: 0,
                });
            session.last_activity = Instant::now();
            session.frame_count += 1;
        }

        // Build structured prompt for Qwen
        let prompt = self.build_prompt(frame)?;

        // Call Qwen Cloud API from within TEE
        let response = self.call_qwen_api(&prompt).await?;

        Ok(response)
    }

    /// Build structured prompt from edge state frames
    fn build_prompt(&self, frame: &TelemetryFrame) -> Result<String> {
        let context = String::from_utf8_lossy(&frame.payload);

        let user_prompt = format!(
            "Telemetry Frame #{} from {}\nData: {}\n\nAnalyze and provide decision:",
            frame.frame_id, frame.source_ip, context
        );

        Ok(format!("{}\n\n{}", SYSTEM_PROMPT, user_prompt))
    }

    /// Call Qwen Cloud API: real DashScope HTTPS call in live mode,
    /// canned response otherwise.
    async fn call_qwen_api(&self, prompt: &str) -> Result<QwenResponse> {
        let api_key = self
            .qwen_api_key
            .as_ref()
            .ok_or(GatewayError::ApiKeyNotInitialized)?;

        info!(
            "[TEE] Calling Qwen API at {} (live: {}, key length: {})",
            self.qwen_endpoint,
            self.live,
            api_key.len()
        );
        debug!("[TEE] Prompt length: {} chars", prompt.len());

        if self.live {
            let client = self.http_client.as_ref().ok_or_else(|| {
                GatewayError::QwenApiError("HTTP client not initialized".to_string())
            })?;

            let url = format!(
                "{}/chat/completions",
                self.qwen_endpoint.trim_end_matches('/')
            );
            let body = serde_json::json!({
                "model": self.qwen_model,
                "messages": [
                    {"role": "user", "content": prompt}
                ]
            });

            let resp = client
                .post(&url)
                .bearer_auth(api_key)
                .json(&body)
                .send()
                .await
                .map_err(|e| GatewayError::QwenApiError(format!("request failed: {e}")))?;

            let status = resp.status();
            let text = resp
                .text()
                .await
                .map_err(|e| GatewayError::QwenApiError(e.to_string()))?;
            if !status.is_success() {
                return Err(GatewayError::QwenApiError(format!(
                    "HTTP {}: {}",
                    status, text
                )));
            }

            let v: serde_json::Value = serde_json::from_str(&text)
                .map_err(|e| GatewayError::QwenApiError(format!("bad JSON: {}", e)))?;

            let choices = v["choices"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .enumerate()
                        .map(|(i, c)| QwenChoice {
                            index: i,
                            message: QwenMessage {
                                role: c["message"]["role"]
                                    .as_str()
                                    .unwrap_or("assistant")
                                    .to_string(),
                                content: c["message"]["content"].as_str().unwrap_or("").to_string(),
                            },
                            finish_reason: c["finish_reason"]
                                .as_str()
                                .unwrap_or("stop")
                                .to_string(),
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            if choices.is_empty() {
                return Err(GatewayError::QwenApiError(
                    "response contained no choices".to_string(),
                ));
            }

            return Ok(QwenResponse {
                request_id: v["id"].as_str().unwrap_or("unknown").to_string(),
                model: v["model"].as_str().unwrap_or(&self.qwen_model).to_string(),
                choices,
                usage: QwenUsage {
                    prompt_tokens: v["usage"]["prompt_tokens"].as_u64().unwrap_or(0) as usize,
                    completion_tokens: v["usage"]["completion_tokens"].as_u64().unwrap_or(0)
                        as usize,
                    total_tokens: v["usage"]["total_tokens"].as_u64().unwrap_or(0) as usize,
                },
            });
        }

        // Simulated response for offline demos and tests
        Ok(QwenResponse {
            request_id: format!("tee-req-{}", self.processed_frames),
            model: self.qwen_model.clone(),
            choices: vec![QwenChoice {
                index: 0,
                message: QwenMessage {
                    role: "assistant".to_string(),
                    content: r#"{"action": "MAINTAIN_COURSE", "confidence": 0.92, "reasoning": "Sensor readings nominal"}"#.to_string(),
                },
                finish_reason: "stop".to_string(),
            }],
            usage: QwenUsage {
                prompt_tokens: prompt.len() / 4,
                completion_tokens: 50,
                total_tokens: (prompt.len() / 4) + 50,
            },
        })
    }

    /// Generate execution log for ZK verification
    pub fn generate_execution_log(&self, frame_id: u64, response: &QwenResponse) -> Vec<u8> {
        let log = format!(
            "FRAME:{}|MODEL:{}|ACTION:{}|TOKENS:{}",
            frame_id,
            response.model,
            response.choices[0].message.content,
            response.usage.total_tokens
        );

        let hash = Sha256::digest(log.as_bytes());

        format!("{}|HASH:{:x}", log, hash).into_bytes()
    }

    /// Get statistics
    pub fn stats(&self) -> GatewayStats {
        let cache = self.session_cache.lock().unwrap();
        GatewayStats {
            processed_frames: self.processed_frames,
            active_sessions: cache.len(),
            api_calls: self.processed_frames,
        }
    }

    /// Check if gateway is properly initialized
    pub fn is_initialized(&self) -> bool {
        self.qwen_api_key.is_some() && self.storage.is_attested()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::FrameMetadata;

    #[test]
    fn test_sealed_storage() {
        let tee = SimulatedTee::new();
        let mut storage = SealedStorage::new(tee);
        let secret = b"sk-qwen-test-token-12345";

        assert!(storage.seal(secret).is_ok());

        // Must attest before unsealing
        assert!(storage.unseal().is_err());

        storage.verify_attestation().unwrap();
        let unsealed = storage.unseal();
        assert!(unsealed.is_ok());
        assert_eq!(unsealed.unwrap(), secret);
    }

    #[test]
    fn test_gateway_initialization() {
        let tee = SimulatedTee::new();
        let mut gateway = TeeGateway::new(tee, "https://dashscope.aliyuncs.com/api/v1");
        let token = b"test-api-token";

        assert!(gateway.initialize(token).is_ok());
        assert!(gateway.is_initialized());
    }

    #[test]
    fn test_frame_processing_builds_prompt() {
        let tee = SimulatedTee::new();
        let mut gateway = TeeGateway::new(tee, "https://dashscope.aliyuncs.com/api/v1");
        gateway.initialize(b"test-token").unwrap();

        let frame = TelemetryFrame {
            frame_id: 1,
            source_ip: "192.168.1.1".to_string(),
            dest_ip: None,
            timestamp_ns: 0,
            payload: b"test telemetry data".to_vec(),
            metadata: FrameMetadata::default(),
        };

        let prompt = gateway.build_prompt(&frame);
        assert!(prompt.is_ok());
        let prompt = prompt.unwrap();
        assert!(prompt.contains("Telemetry Frame #1"));
        assert!(prompt.contains(SYSTEM_PROMPT));
    }
}
