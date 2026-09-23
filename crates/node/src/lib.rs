//! Participant-node foundations for London 0.1.0.
//!
//! This crate deliberately has no grant issuer, accounting rules, transaction
//! payloads, or Aptos client. It persists opaque evidence and submission
//! intents so a later, versioned coordinator contract can be attached without
//! making the node a financial authority.

use sha2::{Digest as _, Sha256};
use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
};

pub type Hash = [u8; 32];

#[must_use]
pub fn sha256(bytes: &[u8]) -> Hash {
    Sha256::digest(bytes).into()
}

#[must_use]
pub fn hex(hash: &Hash) -> String {
    hash.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// A content-addressed, locally durable cache. Cache admission always verifies
/// the caller-provided digest before bytes become visible.
#[derive(Debug, Clone)]
pub struct VerifiedCache {
    root: PathBuf,
}

impl VerifiedCache {
    pub fn open(root: impl Into<PathBuf>) -> io::Result<Self> {
        let root = root.into();
        fs::create_dir_all(&root)?;
        Ok(Self { root })
    }

    pub fn admit(&self, expected: Hash, bytes: &[u8]) -> io::Result<CacheAdmission> {
        let actual = sha256(bytes);
        if actual != expected {
            return Ok(CacheAdmission::DigestMismatch { expected, actual });
        }
        let destination = self.root.join(hex(&expected));
        if destination.exists() {
            return Ok(CacheAdmission::AlreadyPresent);
        }
        write_new_atomically(&destination, bytes)?;
        Ok(CacheAdmission::Stored)
    }

    pub fn read_verified(&self, expected: Hash) -> io::Result<Option<Vec<u8>>> {
        let path = self.root.join(hex(&expected));
        let Ok(bytes) = fs::read(path) else {
            return Ok(None);
        };
        if sha256(&bytes) == expected {
            Ok(Some(bytes))
        } else {
            Ok(None)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheAdmission {
    Stored,
    AlreadyPresent,
    DigestMismatch { expected: Hash, actual: Hash },
}

/// Restart-safe consumption of an externally issued opaque capability. This
/// records only that a capability was consumed: it never validates or issues
/// the capability itself.
#[derive(Debug, Clone)]
pub struct ConsumeJournal {
    root: PathBuf,
}

impl ConsumeJournal {
    pub fn open(root: impl Into<PathBuf>) -> io::Result<Self> {
        let root = root.into();
        fs::create_dir_all(&root)?;
        Ok(Self { root })
    }

    pub fn consume_once(&self, opaque_capability: &str) -> io::Result<ConsumeResult> {
        let id = hex(&sha256(opaque_capability.as_bytes()));
        let path = self.root.join(id);
        match OpenOptions::new().write(true).create_new(true).open(path) {
            Ok(mut file) => {
                file.write_all(b"consumed\n")?;
                file.sync_all()?;
                Ok(ConsumeResult::Consumed)
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                Ok(ConsumeResult::AlreadyConsumed)
            }
            Err(error) => Err(error),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsumeResult {
    Consumed,
    AlreadyConsumed,
}

/// A private spool for signed receipt bytes. The receipt structure and
/// signature algorithm remain owned by the future shared contract.
#[derive(Debug, Clone)]
pub struct ReceiptSpool {
    root: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpoolReceipt {
    pub id: String,
    pub receipt: Vec<u8>,
    pub signature: Vec<u8>,
}

impl ReceiptSpool {
    pub fn open(root: impl Into<PathBuf>) -> io::Result<Self> {
        let root = root.into();
        fs::create_dir_all(&root)?;
        Ok(Self { root })
    }

    pub fn append(&self, receipt: &SpoolReceipt) -> io::Result<()> {
        let id = safe_id(&receipt.id)?;
        let directory = self.root.join(id);
        if directory.exists() {
            return Ok(());
        }
        let staging = self
            .root
            .join(format!(".{}.staging", hex(&sha256(receipt.id.as_bytes()))));
        if staging.exists() {
            fs::remove_dir_all(&staging)?;
        }
        fs::create_dir(&staging)?;
        write_file_sync(&staging.join("receipt"), &receipt.receipt)?;
        write_file_sync(&staging.join("signature"), &receipt.signature)?;
        fs::rename(staging, directory)?;
        Ok(())
    }

    pub fn recover(&self) -> io::Result<Vec<SpoolReceipt>> {
        let mut receipts = Vec::new();
        for entry in fs::read_dir(&self.root)? {
            let entry = entry?;
            if entry.file_name().to_string_lossy().starts_with('.') {
                continue;
            }
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let Some(id) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            let (Ok(receipt), Ok(signature)) = (
                fs::read(path.join("receipt")),
                fs::read(path.join("signature")),
            ) else {
                continue;
            };
            receipts.push(SpoolReceipt {
                id: id.to_owned(),
                receipt,
                signature,
            });
        }
        receipts.sort_by(|left, right| left.id.cmp(&right.id));
        Ok(receipts)
    }
}

/// A deterministic, non-financial local counter. It is suitable for batching
/// diagnostics or opaque evidence references, never accepted usage or payouts.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct DeterministicAggregate(BTreeMap<String, u64>);

impl DeterministicAggregate {
    pub fn add(&mut self, key: impl Into<String>, units: u64) -> Result<(), AggregateError> {
        let entry = self.0.entry(key.into()).or_default();
        *entry = entry.checked_add(units).ok_or(AggregateError::Overflow)?;
        Ok(())
    }

    #[must_use]
    pub fn ordered(&self) -> Vec<(&str, u64)> {
        self.0
            .iter()
            .map(|(key, units)| (key.as_str(), *units))
            .collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AggregateError {
    Overflow,
}

/// A persisted exactly-identical retry boundary. `body` is opaque to the node,
/// and an acknowledgement means only that a future transport reported receipt,
/// not Move acceptance, settlement, or listening proof.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubmissionIntent {
    pub id: String,
    pub body: Vec<u8>,
    pub body_digest: Hash,
}

#[derive(Debug, Clone)]
pub struct SubmissionOutbox {
    root: PathBuf,
}

impl SubmissionOutbox {
    pub fn open(root: impl Into<PathBuf>) -> io::Result<Self> {
        let root = root.into();
        fs::create_dir_all(&root)?;
        Ok(Self { root })
    }

    pub fn prepare(&self, id: &str, body: &[u8]) -> io::Result<PrepareResult> {
        let id = safe_id(id)?;
        let path = self.root.join(&id);
        let digest = sha256(body);
        if path.exists() {
            let existing = fs::read(&path)?;
            return Ok(if sha256(&existing) == digest {
                PrepareResult::Existing
            } else {
                PrepareResult::Conflict
            });
        }
        write_new_atomically(&path, body)?;
        Ok(PrepareResult::Prepared(SubmissionIntent {
            id,
            body: body.to_vec(),
            body_digest: digest,
        }))
    }

    pub fn pending(&self) -> io::Result<Vec<SubmissionIntent>> {
        let mut intents = Vec::new();
        for entry in fs::read_dir(&self.root)? {
            let entry = entry?;
            if !entry.file_type()?.is_file() {
                continue;
            }
            let name = entry.file_name();
            let Some(id) = name.to_str() else {
                continue;
            };
            let body = fs::read(entry.path())?;
            intents.push(SubmissionIntent {
                id: id.to_owned(),
                body_digest: sha256(&body),
                body,
            });
        }
        intents.sort_by(|left, right| left.id.cmp(&right.id));
        Ok(intents)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrepareResult {
    Prepared(SubmissionIntent),
    Existing,
    Conflict,
}

/// Redacts values before diagnostics leave the node. Keep structured receipt
/// content and credentials out of diagnostic messages entirely.
#[must_use]
pub fn redact_diagnostic(message: &str, secrets: &[&str]) -> String {
    secrets.iter().fold(message.to_owned(), |redacted, secret| {
        if secret.is_empty() {
            redacted
        } else {
            redacted.replace(secret, "[REDACTED]")
        }
    })
}

fn safe_id(id: &str) -> io::Result<String> {
    if id.is_empty()
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "opaque id must be non-empty ASCII alphanumeric, '-' or '_'",
        ));
    }
    Ok(id.to_owned())
}

fn write_file_sync(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

fn write_new_atomically(destination: &Path, bytes: &[u8]) -> io::Result<()> {
    let temporary = destination.with_extension("staging");
    if temporary.exists() {
        fs::remove_file(&temporary)?;
    }
    write_file_sync(&temporary, bytes)?;
    match fs::rename(&temporary, destination) {
        Ok(()) => Ok(()),
        Err(error) if destination.exists() => {
            fs::remove_file(temporary)?;
            Ok(())
        }
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temporary(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "porto-node-{name}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        fs::create_dir_all(&path).expect("create temporary directory");
        path
    }

    #[test]
    fn cache_rejects_mismatched_bytes_and_recovers_verified_bytes() {
        let path = temporary("cache");
        let cache = VerifiedCache::open(&path).expect("cache");
        let digest = sha256(b"music");
        assert!(matches!(
            cache.admit(digest, b"other").expect("admit"),
            CacheAdmission::DigestMismatch { .. }
        ));
        assert_eq!(
            cache.admit(digest, b"music").expect("admit"),
            CacheAdmission::Stored
        );
        drop(cache);
        assert_eq!(
            VerifiedCache::open(path)
                .expect("reopen")
                .read_verified(digest)
                .expect("read"),
            Some(b"music".to_vec())
        );
    }

    #[test]
    fn consumption_and_receipts_survive_restart() {
        let path = temporary("evidence");
        let journal = ConsumeJournal::open(path.join("consumed")).expect("journal");
        assert_eq!(
            journal.consume_once("opaque-grant").expect("consume"),
            ConsumeResult::Consumed
        );
        drop(journal);
        assert_eq!(
            ConsumeJournal::open(path.join("consumed"))
                .expect("reopen")
                .consume_once("opaque-grant")
                .expect("consume"),
            ConsumeResult::AlreadyConsumed
        );
        let spool = ReceiptSpool::open(path.join("spool")).expect("spool");
        spool
            .append(&SpoolReceipt {
                id: "r_1".into(),
                receipt: b"opaque receipt".to_vec(),
                signature: b"signature".to_vec(),
            })
            .expect("append");
        assert_eq!(
            ReceiptSpool::open(path.join("spool"))
                .expect("reopen")
                .recover()
                .expect("recover")
                .len(),
            1
        );
    }

    #[test]
    fn outbox_never_changes_a_retry_body_and_aggregates_are_ordered() {
        let path = temporary("outbox");
        let outbox = SubmissionOutbox::open(&path).expect("outbox");
        assert!(matches!(
            outbox.prepare("batch_1", b"opaque").expect("prepare"),
            PrepareResult::Prepared(_)
        ));
        assert_eq!(
            outbox.prepare("batch_1", b"opaque").expect("retry"),
            PrepareResult::Existing
        );
        assert_eq!(
            outbox.prepare("batch_1", b"different").expect("conflict"),
            PrepareResult::Conflict
        );
        assert_eq!(
            SubmissionOutbox::open(path)
                .expect("reopen")
                .pending()
                .expect("pending")
                .len(),
            1
        );
        let mut aggregate = DeterministicAggregate::default();
        aggregate.add("z", 1).expect("add");
        aggregate.add("a", 2).expect("add");
        assert_eq!(aggregate.ordered(), vec![("a", 2), ("z", 1)]);
    }

    #[test]
    fn diagnostics_do_not_expose_supplied_secrets() {
        assert_eq!(
            redact_diagnostic("failed with bearer abc", &["abc", "bearer"]),
            "failed with [REDACTED] [REDACTED]"
        );
    }
}
