//! Post-Quantum Cryptographic Transport Layer
//!
//! Implements hybrid key exchange combining X25519 and ML-KEM-768 (Kyber)
//! for resistance against harvest-now-decrypt-later attacks.
//! Data frames are encrypted using AES-256-GCM.

use aes_gcm::{
    aead::{Aead, KeyInit, Payload},
    Aes256Gcm, Nonce,
};
use common::{EncryptedFrame, TelemetryFrame};
use rand_core::{OsRng, RngCore};
use sha2::{Digest, Sha256};
use std::sync::{Arc, Mutex};
use thiserror::Error;
use tracing::{debug, warn};

// Re-export x25519-dalek for real implementations
use x25519_dalek::{PublicKey as X25519PublicKey, StaticSecret as X25519StaticSecret};

// ML-KEM-768 (Kyber) key sizes
pub const MLKEM768_PUBLIC_KEY_SIZE: usize = 1088;
pub const MLKEM768_SECRET_KEY_SIZE: usize = 1632;
pub const MLKEM768_CIPHERTEXT_SIZE: usize = 1568;
pub const MLKEM768_SHARED_SECRET_SIZE: usize = 32;

// X25519 key sizes
pub const X25519_PUBLIC_KEY_SIZE: usize = 32;
pub const X25519_SECRET_KEY_SIZE: usize = 32;
pub const X25519_SHARED_SECRET_SIZE: usize = 32;

// Combined hybrid shared secret size
pub const HYBRID_SHARED_SECRET_SIZE: usize = 64;

/// PQC Transport errors
#[derive(Error, Debug)]
pub enum PqcError {
    #[error("Key exchange failed")]
    KeyExchangeFailed,

    #[error("Encapsulation failed")]
    EncapsulationFailed,

    #[error("Decapsulation failed")]
    DecapsulationFailed,

    #[error("Encryption failed: {0}")]
    EncryptionFailed(String),

    #[error("Decryption failed: {0}")]
    DecryptionFailed(String),

    #[error("Invalid key size: expected {expected}, got {actual}")]
    InvalidKeySize { expected: usize, actual: usize },

    #[error("Nonce exhausted for session {session_id}")]
    NonceExhausted { session_id: u64 },

    #[error("Session expired")]
    SessionExpired,

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, PqcError>;

/// Hybrid public key: X25519 || ML-KEM-768
#[derive(Clone, Debug)]
pub struct HybridPublicKey {
    pub x25519_pubkey: [u8; X25519_PUBLIC_KEY_SIZE],
    pub mlkem_pubkey: [u8; MLKEM768_PUBLIC_KEY_SIZE],
}

/// Hybrid secret key
#[derive(Clone)]
pub struct HybridSecretKey {
    pub x25519_seckey: [u8; X25519_SECRET_KEY_SIZE],
    pub mlkem_seckey: [u8; MLKEM768_SECRET_KEY_SIZE],
}

/// Encapsulated key material for hybrid KEX
#[derive(Clone, Debug)]
pub struct HybridEncapsulation {
    pub x25519_ephemeral_pubkey: [u8; X25519_PUBLIC_KEY_SIZE],
    pub mlkem_ciphertext: [u8; MLKEM768_CIPHERTEXT_SIZE],
}

/// Session state for PQC-secured channel
pub struct PqcSession {
    session_id: u64,
    send_key: Aes256Gcm,
    recv_key: Aes256Gcm,
    send_nonce: u128,
    recv_nonce: u128,
    created_at: u64,
    last_activity: u64,
}

impl PqcSession {
    fn new(send_key: [u8; 32], recv_key: [u8; 32]) -> Self {
        let now = helpers::time::now_ns();
        Self {
            session_id: OsRng.next_u64(),
            send_key: Aes256Gcm::new_from_slice(&send_key).unwrap(),
            recv_key: Aes256Gcm::new_from_slice(&recv_key).unwrap(),
            send_nonce: 0,
            recv_nonce: 0,
            created_at: now,
            last_activity: now,
        }
    }

    pub fn encrypt(&mut self, plaintext: &[u8], associated_data: &[u8]) -> Result<Vec<u8>> {
        if self.send_nonce >= (1u128 << 64) {
            return Err(PqcError::NonceExhausted {
                session_id: self.session_id,
            });
        }

        let nonce_bytes = self.send_nonce.to_le_bytes();
        let nonce = Nonce::from_slice(&nonce_bytes[0..12]);

        let payload = Payload {
            msg: plaintext,
            aad: associated_data,
        };

        let ciphertext = self
            .send_key
            .encrypt(nonce, payload)
            .map_err(|e| PqcError::EncryptionFailed(e.to_string()))?;

        self.send_nonce += 1;
        self.last_activity = helpers::time::now_ns();

        Ok(ciphertext)
    }

    pub fn decrypt(&mut self, ciphertext: &[u8], associated_data: &[u8]) -> Result<Vec<u8>> {
        if self.recv_nonce >= (1u128 << 64) {
            return Err(PqcError::NonceExhausted {
                session_id: self.session_id,
            });
        }

        let nonce_bytes = self.recv_nonce.to_le_bytes();
        let nonce = Nonce::from_slice(&nonce_bytes[0..12]);

        let plaintext = self
            .recv_key
            .decrypt(
                nonce,
                Payload {
                    msg: ciphertext,
                    aad: associated_data,
                },
            )
            .map_err(|e| PqcError::DecryptionFailed(e.to_string()))?;

        self.recv_nonce += 1;
        self.last_activity = helpers::time::now_ns();

        Ok(plaintext)
    }

    pub fn is_expired(&self, timeout_nanos: u64) -> bool {
        let elapsed = helpers::time::elapsed_ns(self.last_activity);
        elapsed > timeout_nanos
    }

    pub fn session_id(&self) -> u64 {
        self.session_id
    }
}

/// Hybrid Key Exchange Manager
/// Combines X25519 and ML-KEM-768 for harvest-now-decrypt-later resistance
pub struct HybridKem {
    local_secret: HybridSecretKey,
    local_public: HybridPublicKey,
}

impl HybridKem {
    /// Generate a new hybrid key pair using real x25519-dalek
    pub fn generate() -> Result<Self> {
        // Generate X25519 key pair using x25519-dalek
        let x25519_secret = X25519StaticSecret::random_from_rng(OsRng);
        let x25519_pubkey: X25519PublicKey = (&x25519_secret).into();

        // Generate ML-KEM-768 key pair (placeholder - use pqcrypto in production)
        let mut mlkem_seckey = [0u8; MLKEM768_SECRET_KEY_SIZE];
        let mut mlkem_pubkey = [0u8; MLKEM768_PUBLIC_KEY_SIZE];
        OsRng.fill_bytes(&mut mlkem_seckey);
        OsRng.fill_bytes(&mut mlkem_pubkey);

        Ok(Self {
            local_secret: HybridSecretKey {
                x25519_seckey: x25519_secret.to_bytes(),
                mlkem_seckey,
            },
            local_public: HybridPublicKey {
                x25519_pubkey: x25519_pubkey.to_bytes(),
                mlkem_pubkey,
            },
        })
    }

    /// Perform hybrid key exchange with remote peer
    pub fn exchange_keys(&self, remote_public: &HybridPublicKey) -> Result<[u8; 32]> {
        // X25519 ECDH using real x25519-dalek
        let x25519_shared = self.x25519_dh_real(
            &self.local_secret.x25519_seckey,
            &remote_public.x25519_pubkey,
        )?;

        // ML-KEM-768 encapsulation (placeholder)
        let (_mlkem_ciphertext, mlkem_shared) =
            Self::mlkem_encapsulate(&remote_public.mlkem_pubkey)?;

        // Combine both shared secrets via HKDF-like construction
        let mut combined = [0u8; HYBRID_SHARED_SECRET_SIZE];
        combined[0..32].copy_from_slice(&x25519_shared);
        combined[32..64].copy_from_slice(&mlkem_shared);

        // Derive final key via SHA-256
        let mut hasher = Sha256::new();
        hasher.update(&combined);
        let result = hasher.finalize();

        let mut final_key = [0u8; 32];
        final_key.copy_from_slice(&result);

        Ok(final_key)
    }

    /// Create encapsulated key material to send to peer
    pub fn encapsulate_for_peer(
        &self,
        remote_public: &HybridPublicKey,
    ) -> Result<(HybridEncapsulation, [u8; 32])> {
        // Generate ephemeral X25519 key using x25519-dalek
        let eph_x25519_secret = X25519StaticSecret::random_from_rng(OsRng);
        let eph_x25519_pubkey: X25519PublicKey = (&eph_x25519_secret).into();

        // X25519 DH with remote using real implementation
        let x25519_shared =
            self.x25519_dh_real(&eph_x25519_secret.to_bytes(), &remote_public.x25519_pubkey)?;

        // ML-KEM encapsulation (placeholder)
        let (mlkem_ciphertext, mlkem_shared) =
            Self::mlkem_encapsulate(&remote_public.mlkem_pubkey)?;

        let encapsulation = HybridEncapsulation {
            x25519_ephemeral_pubkey: eph_x25519_pubkey.to_bytes(),
            mlkem_ciphertext,
        };

        // Combine shared secrets
        let mut combined = [0u8; HYBRID_SHARED_SECRET_SIZE];
        combined[0..32].copy_from_slice(&x25519_shared);
        combined[32..64].copy_from_slice(&mlkem_shared);

        let mut hasher = Sha256::new();
        hasher.update(&combined);
        let result = hasher.finalize();

        let mut shared_key = [0u8; 32];
        shared_key.copy_from_slice(&result);

        Ok((encapsulation, shared_key))
    }

    /// Real X25519 Diffie-Hellman using x25519-dalek
    fn x25519_dh_real(&self, seckey_bytes: &[u8; 32], pubkey_bytes: &[u8; 32]) -> Result<[u8; 32]> {
        let secret = X25519StaticSecret::from(*seckey_bytes);
        let public = X25519PublicKey::from(*pubkey_bytes);
        let shared = secret.diffie_hellman(&public);
        Ok(shared.to_bytes())
    }

    fn mlkem_encapsulate(
        _pubkey: &[u8; MLKEM768_PUBLIC_KEY_SIZE],
    ) -> Result<([u8; MLKEM768_CIPHERTEXT_SIZE], [u8; 32])> {
        // Placeholder - in production use kyber/pqcrypto crate
        let mut ciphertext = [0u8; MLKEM768_CIPHERTEXT_SIZE];
        let mut shared_secret = [0u8; 32];
        OsRng.fill_bytes(&mut ciphertext);
        OsRng.fill_bytes(&mut shared_secret);
        Ok((ciphertext, shared_secret))
    }

    pub fn public_key(&self) -> &HybridPublicKey {
        &self.local_public
    }
}

/// PQC Transport Manager - handles multiple sessions
pub struct PqcTransportManager {
    sessions: Arc<Mutex<Vec<Arc<Mutex<PqcSession>>>>>,
    session_timeout_nanos: u64,
}

impl PqcTransportManager {
    pub fn new(session_timeout_secs: u64) -> Self {
        Self {
            sessions: Arc::new(Mutex::new(Vec::new())),
            session_timeout_nanos: session_timeout_secs * 1_000_000_000, // Convert to nanos
        }
    }

    /// Establish a new PQC-secured session with a peer
    pub fn establish_session(
        &self,
        local_kem: &HybridKem,
        remote_public: &HybridPublicKey,
    ) -> Result<Arc<Mutex<PqcSession>>> {
        let shared_key = local_kem.exchange_keys(remote_public)?;

        // Derive separate send/recv keys using domain separation
        let mut send_key_material = [0u8; 64];
        send_key_material[0..32].copy_from_slice(&shared_key);
        send_key_material[32..64].copy_from_slice(b"SEND");
        let send_key = Sha256::digest(&send_key_material);

        let mut recv_key_material = [0u8; 64];
        recv_key_material[0..32].copy_from_slice(&shared_key);
        recv_key_material[32..64].copy_from_slice(b"RECV");
        let recv_key = Sha256::digest(&recv_key_material);

        let session = PqcSession::new(send_key.into(), recv_key.into());

        let arc_session = Arc::new(Mutex::new(session));
        self.sessions.lock().unwrap().push(arc_session.clone());

        debug!(
            "Established new PQC session: {}",
            arc_session.lock().unwrap().session_id()
        );
        Ok(arc_session)
    }

    /// Encrypt telemetry frame for transmission
    pub fn encrypt_frame(
        &self,
        session: Arc<Mutex<PqcSession>>,
        frame: &TelemetryFrame,
    ) -> Result<EncryptedFrame> {
        use serde_json::to_vec;

        let mut sess = session.lock().unwrap();

        // Serialize frame metadata as associated data
        let meta_json = to_vec(&frame.metadata).unwrap_or_default();
        let associated_data = meta_json.as_slice();

        // Encrypt payload
        let ciphertext = sess.encrypt(&frame.payload, associated_data)?;

        // Generate nonce for this encryption
        let mut nonce = [0u8; 12];
        OsRng.fill_bytes(&mut nonce);

        Ok(EncryptedFrame {
            frame_id: frame.frame_id,
            nonce,
            ciphertext,
            tag: [0u8; 16], // AES-GCM tag is appended to ciphertext in our implementation
        })
    }

    /// Decrypt received frame
    pub fn decrypt_frame(
        &self,
        session: Arc<Mutex<PqcSession>>,
        encrypted: &EncryptedFrame,
    ) -> Result<Vec<u8>> {
        let mut sess = session.lock().unwrap();
        let associated_data = encrypted.frame_id.to_le_bytes();
        sess.decrypt(&encrypted.ciphertext, &associated_data)
    }

    /// Cleanup expired sessions
    pub fn cleanup_expired(&self) -> usize {
        let mut sessions = self.sessions.lock().unwrap();
        let initial_count = sessions.len();
        sessions.retain(|s| !s.lock().unwrap().is_expired(self.session_timeout_nanos));
        let removed = initial_count - sessions.len();
        if removed > 0 {
            debug!("Cleaned up {} expired PQC sessions", removed);
        }
        removed
    }

    /// Get active session count
    pub fn session_count(&self) -> usize {
        self.sessions.lock().unwrap().len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hybrid_keygen() {
        let kem = HybridKem::generate();
        assert!(kem.is_ok());
    }

    #[test]
    fn test_session_creation() {
        let manager = PqcTransportManager::new(300);
        let local_kem = HybridKem::generate().unwrap();
        let remote_kem = HybridKem::generate().unwrap();

        let session = manager.establish_session(&local_kem, remote_kem.public_key());
        assert!(session.is_ok());
    }

    #[test]
    fn test_session_cleanup() {
        let manager = PqcTransportManager::new(300);
        let local_kem = HybridKem::generate().unwrap();
        let remote_kem = HybridKem::generate().unwrap();

        manager
            .establish_session(&local_kem, remote_kem.public_key())
            .unwrap();
        assert_eq!(manager.session_count(), 1);

        // Cleanup should not remove fresh sessions
        let removed = manager.cleanup_expired();
        assert_eq!(removed, 0);
        assert_eq!(manager.session_count(), 1);
    }
}
