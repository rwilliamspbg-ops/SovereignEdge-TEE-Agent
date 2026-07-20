//! Post-Quantum Cryptographic Transport Layer
//!
//! Implements hybrid key exchange combining X25519 and ML-KEM-768 (Kyber, FIPS 203)
//! for resistance against harvest-now-decrypt-later attacks.
//! Data frames are encrypted using AES-256-GCM.

use aes_gcm::{
    aead::{Aead, KeyInit, Payload},
    Aes256Gcm, Nonce,
};
use common::{EncryptedFrame, TelemetryFrame};
use ml_kem::{
    kem::{Decapsulate, Encapsulate, Kem},
    DecapsulationKey, EncapsulationKey, Key, KeyExport, MlKem768, Seed, SharedKey,
};
use rand_core::{OsRng, RngCore};
use sha2::{Digest, Sha256};
use std::sync::{Arc, Mutex};
use thiserror::Error;
use tracing::debug;

// Re-export x25519-dalek for real implementations
use x25519_dalek::{PublicKey as X25519PublicKey, StaticSecret as X25519StaticSecret};

// ML-KEM-768 (Kyber) key sizes (FIPS 203)
pub const MLKEM768_PUBLIC_KEY_SIZE: usize = 1088;
pub const MLKEM768_SEED_SIZE: usize = 64;
pub const MLKEM768_CIPHERTEXT_SIZE: usize = 1088;
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

    #[error("Invalid ML-KEM key")]
    InvalidMlKemKey,

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
    pub mlkem_pubkey: Vec<u8>,
}

/// Hybrid secret key (stores ML-KEM seed for compact serialization)
#[derive(Clone)]
pub struct HybridSecretKey {
    pub x25519_seckey: [u8; X25519_SECRET_KEY_SIZE],
    pub mlkem_seed: Seed,
}

/// Encapsulated key material for hybrid KEX
#[derive(Clone, Debug)]
pub struct HybridEncapsulation {
    pub x25519_ephemeral_pubkey: [u8; X25519_PUBLIC_KEY_SIZE],
    pub mlkem_ciphertext: Vec<u8>,
}

/// Session state for PQC-secured channel
pub struct PqcSession {
    session_id: u64,
    send_key: Aes256Gcm,
    recv_key: Aes256Gcm,
    send_nonce: u128,
    recv_nonce: u128,
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
            last_activity: now,
        }
    }

    /// Nonce discipline is machine-verified in `verification/SovereignEdge/Nonce.lean`:
    /// `run_noDup` proves no (key, nonce) pair is ever reused within a session and
    /// `run_stops_at_limit` proves the counter hard-stops instead of wrapping.
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
        let elapsed = helpers::time::now_ns().saturating_sub(self.last_activity);
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
    /// Generate a new hybrid key pair using real x25519-dalek and real ML-KEM-768 (FIPS 203)
    pub fn generate() -> Result<Self> {
        // Generate X25519 key pair using x25519-dalek
        let x25519_secret = X25519StaticSecret::random_from_rng(OsRng);
        let x25519_pubkey: X25519PublicKey = (&x25519_secret).into();

        // Generate real ML-KEM-768 key pair (FIPS 203)
        let (dk, ek) = MlKem768::generate_keypair();

        // Serialize: decapsulation key as 64-byte seed, encapsulation key as bytes
        let mlkem_seed: Seed = dk.to_bytes();
        let mlkem_pubkey: Vec<u8> = ek.to_bytes().as_slice().to_vec();

        Ok(Self {
            local_secret: HybridSecretKey {
                x25519_seckey: x25519_secret.to_bytes(),
                mlkem_seed,
            },
            local_public: HybridPublicKey {
                x25519_pubkey: x25519_pubkey.to_bytes(),
                mlkem_pubkey,
            },
        })
    }

    /// Perform the receiver side of hybrid key exchange.
    /// Given an encapsulation from a peer, derive the same shared secret.
    pub fn decapsulate_from_peer(&self, encapsulation: &HybridEncapsulation) -> Result<[u8; 32]> {
        // X25519 ECDH: our static secret + peer's ephemeral public
        let x25519_shared = self.x25519_dh_real(
            &self.local_secret.x25519_seckey,
            &encapsulation.x25519_ephemeral_pubkey,
        )?;

        // ML-KEM-768 decapsulation: use our decapsulation key + peer's ciphertext
        let mlkem_shared = self.mlkem_decapsulate(&encapsulation.mlkem_ciphertext)?;

        // Combine both shared secrets via HKDF-like construction
        let mut combined = [0u8; HYBRID_SHARED_SECRET_SIZE];
        combined[0..32].copy_from_slice(&x25519_shared);
        combined[32..64].copy_from_slice(&mlkem_shared);

        // Derive final key via SHA-256
        let mut hasher = Sha256::new();
        hasher.update(combined);
        let result = hasher.finalize();

        let mut final_key = [0u8; 32];
        final_key.copy_from_slice(&result);

        Ok(final_key)
    }

    /// Perform hybrid key exchange (sender side).
    /// Encapsulates to the remote's public key and returns both the shared secret
    /// and the encapsulation material that must be sent to the peer.
    pub fn encapsulate_to_peer(
        &self,
        remote_public: &HybridPublicKey,
    ) -> Result<(HybridEncapsulation, [u8; 32])> {
        // Generate ephemeral X25519 key using x25519-dalek
        let eph_x25519_secret = X25519StaticSecret::random_from_rng(OsRng);
        let eph_x25519_pubkey: X25519PublicKey = (&eph_x25519_secret).into();

        // X25519 DH: our ephemeral secret + remote's static public
        let x25519_shared =
            self.x25519_dh_real(&eph_x25519_secret.to_bytes(), &remote_public.x25519_pubkey)?;

        // ML-KEM-768 encapsulation (real, FIPS 203)
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
        hasher.update(combined);
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

    /// Real ML-KEM-768 encapsulation (FIPS 203)
    fn mlkem_encapsulate(pubkey_bytes: &[u8]) -> Result<(Vec<u8>, [u8; 32])> {
        let ek_bytes: Key<EncapsulationKey<MlKem768>> = pubkey_bytes
            .try_into()
            .map_err(|_| PqcError::InvalidMlKemKey)?;
        let ek =
            EncapsulationKey::<MlKem768>::new(&ek_bytes).map_err(|_| PqcError::InvalidMlKemKey)?;

        let (ciphertext, shared_secret) = ek.encapsulate();

        let ct: Vec<u8> = ciphertext.as_slice().to_vec();
        let ss: [u8; 32] = shared_secret
            .as_slice()
            .try_into()
            .map_err(|_| PqcError::EncapsulationFailed)?;

        Ok((ct, ss))
    }

    /// Real ML-KEM-768 decapsulation (FIPS 203)
    fn mlkem_decapsulate(&self, ciphertext_bytes: &[u8]) -> Result<[u8; 32]> {
        let dk = DecapsulationKey::<MlKem768>::from_seed(self.local_secret.mlkem_seed);

        let ct: ml_kem::kem::Ciphertext<MlKem768> = ciphertext_bytes
            .try_into()
            .map_err(|_| PqcError::InvalidMlKemKey)?;
        let shared_secret: SharedKey = dk.decapsulate(&ct);

        let ss: [u8; 32] = shared_secret
            .as_slice()
            .try_into()
            .map_err(|_| PqcError::DecapsulationFailed)?;

        Ok(ss)
    }

    pub fn public_key(&self) -> &HybridPublicKey {
        &self.local_public
    }

    pub fn seed(&self) -> &Seed {
        &self.local_secret.mlkem_seed
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
            session_timeout_nanos: session_timeout_secs * 1_000_000_000,
        }
    }

    /// Establish a new PQC-secured session (sender side).
    /// The returned encapsulation must be sent to the peer so they can
    /// derive the same shared secret via `accept_session()`.
    pub fn establish_session(
        &self,
        local_kem: &HybridKem,
        remote_public: &HybridPublicKey,
    ) -> Result<(Arc<Mutex<PqcSession>>, HybridEncapsulation)> {
        let (encapsulation, shared_key) = local_kem.encapsulate_to_peer(remote_public)?;

        // Derive separate send/recv keys using domain separation
        let mut send_key_material = [0u8; 64];
        send_key_material[0..32].copy_from_slice(&shared_key);
        send_key_material[32..36].copy_from_slice(b"SEND");
        let send_key = Sha256::digest(send_key_material);

        let mut recv_key_material = [0u8; 64];
        recv_key_material[0..32].copy_from_slice(&shared_key);
        recv_key_material[32..36].copy_from_slice(b"RECV");
        let recv_key = Sha256::digest(recv_key_material);

        let session = PqcSession::new(send_key.into(), recv_key.into());

        let arc_session = Arc::new(Mutex::new(session));
        self.sessions.lock().unwrap().push(arc_session.clone());

        debug!(
            "Established new PQC session: {}",
            arc_session.lock().unwrap().session_id()
        );
        Ok((arc_session, encapsulation))
    }

    /// Accept a session from the receiver's perspective.
    /// Given an encapsulation from the sender, derive the same shared secret.
    pub fn accept_session(
        &self,
        local_kem: &HybridKem,
        encapsulation: &HybridEncapsulation,
    ) -> Result<Arc<Mutex<PqcSession>>> {
        let shared_key = local_kem.decapsulate_from_peer(encapsulation)?;

        // Keys are swapped relative to the sender:
        // sender's SEND = receiver's RECV, sender's RECV = receiver's SEND
        let mut recv_key_material = [0u8; 64];
        recv_key_material[0..32].copy_from_slice(&shared_key);
        recv_key_material[32..36].copy_from_slice(b"SEND");
        let recv_key = Sha256::digest(recv_key_material);

        let mut send_key_material = [0u8; 64];
        send_key_material[0..32].copy_from_slice(&shared_key);
        send_key_material[32..36].copy_from_slice(b"RECV");
        let send_key = Sha256::digest(send_key_material);

        let session = PqcSession::new(send_key.into(), recv_key.into());

        let arc_session = Arc::new(Mutex::new(session));
        self.sessions.lock().unwrap().push(arc_session.clone());

        debug!(
            "Accepted PQC session: {}",
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
        let mut sess = session.lock().unwrap();

        let associated_data = frame.frame_id.to_le_bytes();
        let ciphertext = sess.encrypt(&frame.payload, &associated_data)?;

        Ok(EncryptedFrame {
            frame_id: frame.frame_id,
            nonce: [0u8; 12],
            ciphertext,
            tag: [0u8; 16],
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
        let kem = kem.unwrap();
        assert!(!kem.public_key().mlkem_pubkey.is_empty());
        assert_eq!(kem.seed().len(), MLKEM768_SEED_SIZE);
    }

    #[test]
    fn test_hybrid_key_exchange_roundtrip() {
        let alice = HybridKem::generate().unwrap();
        let bob = HybridKem::generate().unwrap();

        // Alice encapsulates to Bob
        let (encapsulation, alice_shared) = alice.encapsulate_to_peer(bob.public_key()).unwrap();

        // Bob decapsulates from Alice
        let bob_shared = bob.decapsulate_from_peer(&encapsulation).unwrap();

        // Both derive the same shared secret
        assert_eq!(alice_shared, bob_shared);
    }

    #[test]
    fn test_session_creation() {
        let manager = PqcTransportManager::new(300);
        let local_kem = HybridKem::generate().unwrap();
        let remote_kem = HybridKem::generate().unwrap();

        let (_session, _encapsulation) = manager
            .establish_session(&local_kem, remote_kem.public_key())
            .unwrap();
    }

    #[test]
    fn test_session_roundtrip() {
        let manager = PqcTransportManager::new(300);
        let alice = HybridKem::generate().unwrap();
        let bob = HybridKem::generate().unwrap();

        // Alice establishes session
        let (alice_session, encapsulation) =
            manager.establish_session(&alice, bob.public_key()).unwrap();

        // Bob accepts session
        let bob_session = manager.accept_session(&bob, &encapsulation).unwrap();

        // Encrypt with Alice, decrypt with Bob
        let plaintext = b"hello from the edge";
        let encrypted = manager
            .encrypt_frame(
                alice_session,
                &TelemetryFrame {
                    frame_id: 1,
                    source_ip: "10.0.0.1".to_string(),
                    dest_ip: None,
                    timestamp_ns: 0,
                    payload: plaintext.to_vec(),
                    metadata: common::FrameMetadata::default(),
                },
            )
            .unwrap();

        let decrypted = manager.decrypt_frame(bob_session, &encrypted).unwrap();
        assert_eq!(decrypted, plaintext);
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

        let removed = manager.cleanup_expired();
        assert_eq!(removed, 0);
        assert_eq!(manager.session_count(), 1);
    }

    #[test]
    fn bench_mlkem_keygen() {
        let iters = 100;
        let start = std::time::Instant::now();
        for _ in 0..iters {
            let _ = HybridKem::generate().unwrap();
        }
        let elapsed = start.elapsed();
        eprintln!(
            "ML-KEM-768 keygen: {:?} per iter ({}/100)",
            elapsed / iters,
            iters
        );
    }

    #[test]
    fn bench_mlkem_roundtrip() {
        let iters = 50;
        let start = std::time::Instant::now();
        for _ in 0..iters {
            let alice = HybridKem::generate().unwrap();
            let bob = HybridKem::generate().unwrap();
            let (enc, alice_ss) = alice.encapsulate_to_peer(bob.public_key()).unwrap();
            let bob_ss = bob.decapsulate_from_peer(&enc).unwrap();
            assert_eq!(alice_ss, bob_ss);
        }
        let elapsed = start.elapsed();
        eprintln!(
            "ML-KEM-768 roundtrip: {:?} per iter ({}/50)",
            elapsed / iters,
            iters
        );
    }

    #[test]
    fn bench_aes_encrypt_decrypt() {
        let manager = PqcTransportManager::new(300);
        let alice = HybridKem::generate().unwrap();
        let bob = HybridKem::generate().unwrap();
        let (alice_session, encapsulation) =
            manager.establish_session(&alice, bob.public_key()).unwrap();
        let bob_session = manager.accept_session(&bob, &encapsulation).unwrap();

        let frame = common::TelemetryFrame {
            frame_id: 1,
            source_ip: "10.0.0.1".to_string(),
            dest_ip: None,
            timestamp_ns: 0,
            payload: vec![0u8; 1024],
            metadata: common::FrameMetadata::default(),
        };

        let iters = 1000;
        let start = std::time::Instant::now();
        for _ in 0..iters {
            let enc = manager
                .encrypt_frame(alice_session.clone(), &frame)
                .unwrap();
            let _ = manager.decrypt_frame(bob_session.clone(), &enc).unwrap();
        }
        let elapsed = start.elapsed();
        eprintln!(
            "AES-256-GCM encrypt+decrypt (1KB): {:?} per iter ({}/1000)",
            elapsed / iters,
            iters
        );
    }
}
