// Post-Quantum Cryptographic Transport Layer
// Hybrid Key Exchange: X25519 + ML-KEM-768 (Kyber)
// Symmetric encryption: AES-256-GCM for data frames

use aes_gcm::{
    aead::{Aead, KeyInit, Payload},
    Aes256Gcm, Nonce,
};
use rand_core::{OsRng, RngCore};
use sha2::{Digest, Sha256};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

// ML-KEM-768 (Kyber) key sizes
const MLKEM768_PUBLIC_KEY_SIZE: usize = 1088;
const MLKEM768_SECRET_KEY_SIZE: usize = 1632;
const MLKEM768_CIPHERTEXT_SIZE: usize = 1568;
const MLKEM768_SHARED_SECRET_SIZE: usize = 32;

// X25519 key sizes
const X25519_PUBLIC_KEY_SIZE: usize = 32;
const X25519_SECRET_KEY_SIZE: usize = 32;
const X25519_SHARED_SECRET_SIZE: usize = 32;

// Combined hybrid shared secret size
const HYBRID_SHARED_SECRET_SIZE: usize = 64; // 32 bytes from each KEX

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
    created_at: Instant,
    last_activity: Instant,
}

impl PqcSession {
    fn new(send_key: [u8; 32], recv_key: [u8; 32]) -> Self {
        let now = Instant::now();
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
    
    fn encrypt(&mut self, plaintext: &[u8], associated_data: &[u8]) -> Result<Vec<u8>, PqcError> {
        if self.send_nonce >= (1u128 << 64) {
            return Err(PqcError::NonceExhausted);
        }
        
        let nonce_bytes = self.send_nonce.to_le_bytes();
        let nonce = Nonce::from_slice(&nonce_bytes[0..12]);
        
        let payload = Payload {
            msg: plaintext,
            aad: associated_data,
        };
        
        let ciphertext = self.send_key.encrypt(nonce, payload)
            .map_err(|_| PqcError::EncryptionFailed)?;
        
        self.send_nonce += 1;
        self.last_activity = Instant::now();
        
        Ok(ciphertext)
    }
    
    fn decrypt(&mut self, ciphertext: &[u8], associated_data: &[u8]) -> Result<Vec<u8>, PqcError> {
        if self.recv_nonce >= (1u128 << 64) {
            return Err(PqcError::NonceExhausted);
        }
        
        let nonce_bytes = self.recv_nonce.to_le_bytes();
        let nonce = Nonce::from_slice(&nonce_bytes[0..12]);
        
        let plaintext = self.recv_key.decrypt(nonce, Payload {
            msg: ciphertext,
            aad: associated_data,
        })
        .map_err(|_| PqcError::DecryptionFailed)?;
        
        self.recv_nonce += 1;
        self.last_activity = Instant::now();
        
        Ok(plaintext)
    }
    
    fn is_expired(&self, timeout: Duration) -> bool {
        self.last_activity.elapsed() > timeout
    }
}

/// Post-Quantum Cryptographic errors
#[derive(Debug, Clone)]
pub enum PqcError {
    KeyExchangeFailed,
    EncapsulationFailed,
    DecapsulationFailed,
    EncryptionFailed,
    DecryptionFailed,
    InvalidKeySize,
    NonceExhausted,
    SessionExpired,
}

/// Hybrid Key Exchange Manager
/// Combines X25519 and ML-KEM-768 for harvest-now-decrypt-later resistance
pub struct HybridKem {
    local_secret: HybridSecretKey,
    local_public: HybridPublicKey,
}

impl HybridKem {
    /// Generate a new hybrid key pair
    pub fn generate() -> Result<Self, PqcError> {
        // Generate X25519 key pair
        let mut x25519_seckey = [0u8; X25519_SECRET_KEY_SIZE];
        OsRng.fill_bytes(&mut x25519_seckey);
        
        // In production, use proper X25519 key derivation
        let x25519_pubkey = Self::derive_x25519_public(&x25519_seckey);
        
        // Generate ML-KEM-768 key pair
        let mut mlkem_seckey = [0u8; MLKEM768_SECRET_KEY_SIZE];
        let mut mlkem_pubkey = [0u8; MLKEM768_PUBLIC_KEY_SIZE];
        OsRng.fill_bytes(&mut mlkem_seckey);
        // In production, use proper ML-KEM keygen
        OsRng.fill_bytes(&mut mlkem_pubkey);
        
        Ok(Self {
            local_secret: HybridSecretKey {
                x25519_seckey,
                mlkem_seckey,
            },
            local_public: HybridPublicKey {
                x25519_pubkey,
                mlkem_pubkey,
            },
        })
    }
    
    /// Perform hybrid key exchange with remote peer
    pub fn exchange_keys(
        &self,
        remote_public: &HybridPublicKey,
    ) -> Result<[u8; HYBRID_SHARED_SECRET_SIZE], PqcError> {
        // X25519 ECDH
        let x25519_shared = Self::x25519_dh(
            &self.local_secret.x25519_seckey,
            &remote_public.x25519_pubkey,
        )?;
        
        // ML-KEM-768 encapsulation
        let (mlkem_ciphertext, mlkem_shared) = Self::mlkem_encapsulate(&remote_public.mlkem_pubkey)?;
        
        // Combine both shared secrets via HKDF
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
    ) -> Result<(HybridEncapsulation, [u8; HYBRID_SHARED_SECRET_SIZE]), PqcError> {
        // Generate ephemeral X25519 key
        let mut eph_x25519_seckey = [0u8; X25519_SECRET_KEY_SIZE];
        OsRng.fill_bytes(&mut eph_x25519_seckey);
        let eph_x25519_pubkey = Self::derive_x25519_public(&eph_x25519_seckey);
        
        // X25519 DH with remote
        let x25519_shared = Self::x25519_dh(
            &eph_x25519_seckey,
            &remote_public.x25519_pubkey,
        )?;
        
        // ML-KEM encapsulation
        let (mlkem_ciphertext, mlkem_shared) = Self::mlkem_encapsulate(&remote_public.mlkem_pubkey)?;
        
        let encapsulation = HybridEncapsulation {
            x25519_ephemeral_pubkey: eph_x25519_pubkey,
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
    
    fn derive_x25519_public(seckey: &[u8; 32]) -> [u8; 32] {
        // Placeholder - in production use curve25519 crate
        let mut pubkey = [0u8; 32];
        let mut hasher = Sha256::new();
        hasher.update(seckey);
        let result = hasher.finalize();
        pubkey.copy_from_slice(&result);
        pubkey
    }
    
    fn x25519_dh(seckey: &[u8; 32], pubkey: &[u8; 32]) -> Result<[u8; 32], PqcError> {
        // Placeholder - in production use x25519-dalek crate
        let mut shared = [0u8; 32];
        let mut data = Vec::new();
        data.extend_from_slice(seckey);
        data.extend_from_slice(pubkey);
        let mut hasher = Sha256::new();
        hasher.update(&data);
        let result = hasher.finalize();
        shared.copy_from_slice(&result);
        Ok(shared)
    }
    
    fn mlkem_encapsulate(pubkey: &[u8; MLKEM768_PUBLIC_KEY_SIZE]) -> Result<([u8; MLKEM768_CIPHERTEXT_SIZE], [u8; 32]), PqcError> {
        // Placeholder - in production use kyber crate
        let mut ciphertext = [0u8; MLKEM768_CIPHERTEXT_SIZE];
        let mut shared_secret = [0u8; 32];
        OsRng.fill_bytes(&mut ciphertext);
        OsRng.fill_bytes(&mut shared_secret);
        Ok((ciphertext, shared_secret))
    }
}

/// PQC Transport Manager - handles multiple sessions
pub struct PqcTransportManager {
    sessions: Arc<Mutex<Vec<PqcSession>>>,
    session_timeout: Duration,
}

impl PqcTransportManager {
    pub fn new(session_timeout_secs: u64) -> Self {
        Self {
            sessions: Arc::new(Mutex::new(Vec::new())),
            session_timeout: Duration::from_secs(session_timeout_secs),
        }
    }
    
    /// Establish a new PQC-secured session with a peer
    pub fn establish_session(
        &self,
        local_kem: &HybridKem,
        remote_public: &HybridPublicKey,
    ) -> Result<Arc<Mutex<PqcSession>>, PqcError> {
        let shared_key = local_kem.exchange_keys(remote_public)?;
        
        // Derive separate send/recv keys
        let mut send_key_material = [0u8; 64];
        send_key_material[0..32].copy_from_slice(&shared_key);
        send_key_material[32..64].copy_from_slice(b"SEND");
        let send_key = Sha256::digest(&send_key_material);
        
        let mut recv_key_material = [0u8; 64];
        recv_key_material[0..32].copy_from_slice(&shared_key);
        recv_key_material[32..64].copy_from_slice(b"RECV");
        let recv_key = Sha256::digest(&recv_key_material);
        
        let session = PqcSession::new(
            send_key.into(),
            recv_key.into(),
        );
        
        let arc_session = Arc::new(Mutex::new(session));
        self.sessions.lock().unwrap().push(arc_session.clone());
        
        Ok(arc_session)
    }
    
    /// Encrypt data for transmission over AF_XDP
    pub fn encrypt_frame(
        &self,
        session: Arc<Mutex<PqcSession>>,
        plaintext: &[u8],
        frame_id: u64,
    ) -> Result<Vec<u8>, PqcError> {
        let mut sess = session.lock().unwrap();
        let associated_data = frame_id.to_le_bytes();
        sess.encrypt(plaintext, &associated_data)
    }
    
    /// Decrypt received frame from AF_XDP
    pub fn decrypt_frame(
        &self,
        session: Arc<Mutex<PqcSession>>,
        ciphertext: &[u8],
        frame_id: u64,
    ) -> Result<Vec<u8>, PqcError> {
        let mut sess = session.lock().unwrap();
        let associated_data = frame_id.to_le_bytes();
        sess.decrypt(ciphertext, &associated_data)
    }
    
    /// Cleanup expired sessions
    pub fn cleanup_expired(&self) -> usize {
        let mut sessions = self.sessions.lock().unwrap();
        let initial_count = sessions.len();
        sessions.retain(|s| !s.lock().unwrap().is_expired(self.session_timeout));
        initial_count - sessions.len()
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
        
        let session = manager.establish_session(&local_kem, &remote_kem.local_public);
        assert!(session.is_ok());
    }
}
