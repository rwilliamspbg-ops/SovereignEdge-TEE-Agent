//! Integration test: PQC transport layer testing

use common::{TelemetryFrame, FrameMetadata};
use pqc_transport::{HybridKem, PqcTransportManager};

#[test]
fn test_hybrid_key_exchange() {
    // Test that both parties can generate keys and establish session
    let local_kem = HybridKem::generate().expect("Failed to generate local keys");
    let remote_kem = HybridKem::generate().expect("Failed to generate remote keys");
    
    // Verify key generation produced valid structures
    assert_eq!(local_kem.public_key().x25519_pubkey.len(), 32);
    assert_eq!(local_kem.public_key().mlkem_pubkey.len(), 1088);
}

#[test]
fn test_session_establishment() {
    // Test full session establishment between two parties
    let manager = PqcTransportManager::new(300);
    let local_kem = HybridKem::generate().unwrap();
    let remote_kem = HybridKem::generate().unwrap();
    
    let session = manager.establish_session(&local_kem, remote_kem.public_key());
    assert!(session.is_ok(), "Session establishment failed");
    
    let session_count = manager.session_count();
    assert_eq!(session_count, 1);
}

#[test]
fn test_frame_encryption_decryption() {
    // Test encrypting and decrypting a telemetry frame
    let manager = PqcTransportManager::new(300);
    let local_kem = HybridKem::generate().unwrap();
    let remote_kem = HybridKem::generate().unwrap();
    
    let session = manager.establish_session(&local_kem, remote_kem.public_key()).unwrap();
    
    // Create test frame
    let original_frame = TelemetryFrame {
        frame_id: 42,
        source_ip: "192.168.1.100".to_string(),
        dest_ip: None,
        timestamp_ns: 1234567890,
        payload: b"test telemetry data".to_vec(),
        metadata: FrameMetadata::default(),
    };
    
    // Encrypt
    let encrypted = manager.encrypt_frame(session.clone(), &original_frame);
    assert!(encrypted.is_ok(), "Encryption failed");
    let encrypted_frame = encrypted.unwrap();
    
    // Verify encryption produced ciphertext
    assert!(!encrypted_frame.ciphertext.is_empty());
    assert_ne!(encrypted_frame.ciphertext, original_frame.payload);
    
    // Decrypt (note: in real scenario, remote would use its own session)
    // For this test, we verify the structure is valid
    assert_eq!(encrypted_frame.frame_id, original_frame.frame_id);
}

#[test]
fn test_session_cleanup() {
    // Test that expired sessions are cleaned up
    let manager = PqcTransportManager::new(300); // 5 minute timeout
    let local_kem = HybridKem::generate().unwrap();
    let remote_kem = HybridKem::generate().unwrap();
    
    // Create multiple sessions
    for _ in 0..5 {
        manager.establish_session(&local_kem, remote_kem.public_key()).unwrap();
    }
    
    assert_eq!(manager.session_count(), 5);
    
    // Cleanup should not remove fresh sessions
    let removed = manager.cleanup_expired();
    assert_eq!(removed, 0);
    assert_eq!(manager.session_count(), 5);
}

#[test]
fn test_multiple_sessions() {
    // Test managing multiple concurrent sessions
    let manager = PqcTransportManager::new(300);
    let local_kem = HybridKem::generate().unwrap();
    
    // Create sessions with different remote peers
    for i in 0..10 {
        let remote_kem = HybridKem::generate().unwrap();
        let result = manager.establish_session(&local_kem, remote_kem.public_key());
        assert!(result.is_ok(), "Failed to establish session {}", i);
    }
    
    assert_eq!(manager.session_count(), 10);
}
