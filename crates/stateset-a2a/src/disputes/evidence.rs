//! Evidence submission and SHA-256 content hashing.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::types::EvidenceType;

/// A piece of evidence submitted for a dispute.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Evidence {
    /// Unique evidence ID.
    pub id: Uuid,
    /// The dispute this evidence belongs to.
    pub dispute_id: Uuid,
    /// Who submitted this evidence.
    pub submitted_by: String,
    /// Type of evidence.
    pub evidence_type: EvidenceType,
    /// Description of the evidence.
    pub description: String,
    /// SHA-256 hash of the evidence content.
    pub content_hash: String,
    /// Submission timestamp.
    pub submitted_at: DateTime<Utc>,
}

impl Evidence {
    /// Create a new evidence record with content hashing.
    pub fn new(
        dispute_id: Uuid,
        submitted_by: impl Into<String>,
        evidence_type: EvidenceType,
        description: impl Into<String>,
        content: &[u8],
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            dispute_id,
            submitted_by: submitted_by.into(),
            evidence_type,
            description: description.into(),
            content_hash: hash_evidence(content),
            submitted_at: Utc::now(),
        }
    }
}

/// Compute the SHA-256 hash of evidence content as a hex string.
#[must_use]
pub fn hash_evidence(content: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content);
    hex::encode(hasher.finalize())
}

/// Verify that content matches a previously computed hash.
#[must_use]
pub fn verify_evidence_hash(content: &[u8], expected_hash: &str) -> bool {
    hash_evidence(content) == expected_hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_evidence_deterministic() {
        let h1 = hash_evidence(b"test content");
        let h2 = hash_evidence(b"test content");
        assert_eq!(h1, h2);
    }

    #[test]
    fn hash_evidence_is_64_hex_chars() {
        let hash = hash_evidence(b"hello");
        assert_eq!(hash.len(), 64);
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn hash_evidence_different_content() {
        let h1 = hash_evidence(b"content A");
        let h2 = hash_evidence(b"content B");
        assert_ne!(h1, h2);
    }

    #[test]
    fn hash_evidence_empty() {
        let hash = hash_evidence(b"");
        assert_eq!(hash.len(), 64);
        // SHA-256 of empty string is well-known
        assert_eq!(hash, "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
    }

    #[test]
    fn verify_evidence_hash_matches() {
        let content = b"important evidence";
        let hash = hash_evidence(content);
        assert!(verify_evidence_hash(content, &hash));
    }

    #[test]
    fn verify_evidence_hash_mismatch() {
        assert!(!verify_evidence_hash(b"original", "deadbeef"));
    }

    #[test]
    fn evidence_creation() {
        let dispute_id = Uuid::new_v4();
        let evidence = Evidence::new(
            dispute_id,
            "0xBuyer",
            EvidenceType::Screenshot,
            "Screenshot of missing item",
            b"<png data>",
        );
        assert_eq!(evidence.dispute_id, dispute_id);
        assert_eq!(evidence.evidence_type, EvidenceType::Screenshot);
        assert_eq!(evidence.content_hash, hash_evidence(b"<png data>"));
    }

    #[test]
    fn evidence_content_hash_integrity() {
        let content = b"transaction log: payment of 100 USDC";
        let evidence = Evidence::new(
            Uuid::new_v4(),
            "0xSeller",
            EvidenceType::TransactionLog,
            "Payment proof",
            content,
        );
        assert!(verify_evidence_hash(content, &evidence.content_hash));
    }

    #[test]
    fn evidence_tampered_content_fails_verification() {
        let evidence = Evidence::new(
            Uuid::new_v4(),
            "0xBuyer",
            EvidenceType::Communication,
            "Chat log",
            b"original chat log",
        );
        assert!(!verify_evidence_hash(b"tampered chat log", &evidence.content_hash));
    }
}
