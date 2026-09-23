//! Contract-independent integrity checks for retained node artifacts.
//!
//! This verifier makes no claim about grant validity, receipt signatures,
//! aggregate acceptance, Move state, or payout completion. Those verdicts need
//! released schemas and a selected Aptos read path.

use sha2::{Digest as _, Sha256};

pub type Hash = [u8; 32];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArtifactVerdict {
    Match { digest: Hash },
    Mismatch { expected: Hash, actual: Hash },
}

#[must_use]
pub fn verify_bytes(expected: Hash, artifact: &[u8]) -> ArtifactVerdict {
    let actual: Hash = Sha256::digest(artifact).into();
    if actual == expected {
        ArtifactVerdict::Match { digest: actual }
    } else {
        ArtifactVerdict::Mismatch { expected, actual }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_only_artifact_integrity() {
        let expected: Hash = Sha256::digest(b"retained evidence").into();
        assert!(matches!(
            verify_bytes(expected, b"retained evidence"),
            ArtifactVerdict::Match { .. }
        ));
        assert!(matches!(
            verify_bytes(expected, b"changed"),
            ArtifactVerdict::Mismatch { .. }
        ));
    }
}
