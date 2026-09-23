//! Contract-independent domain boundaries for the London coordinator.
//!
//! Move remains the authority for allocation, obligations, and payout state.
//! This crate intentionally contains no asset amounts, payout calculations,
//! transaction payloads, or shared Move schema types.

use std::fmt;

/// Testnet is an integration environment. It must never be presented as a
/// production settlement environment.
pub const TESTNET_DISCLAIMER: &str = "Testnet integration only";

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct OpaqueId(String);

impl OpaqueId {
    pub fn new(value: impl Into<String>) -> Result<Self, DomainError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(DomainError::EmptyIdentifier);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DomainError {
    EmptyIdentifier,
    InvalidRange,
    ReplayConflict,
    InvalidStateTransition {
        from: SubmissionState,
        to: SubmissionState,
    },
    ConflictingIntent,
    MissingIntent,
    InvalidConfiguration(&'static str),
}

impl fmt::Display for DomainError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyIdentifier => write!(formatter, "identifiers must not be empty"),
            Self::InvalidRange => write!(formatter, "usage range must be ordered and non-empty"),
            Self::ReplayConflict => write!(
                formatter,
                "usage batch conflicts with an existing replay guard"
            ),
            Self::InvalidStateTransition { from, to } => {
                write!(
                    formatter,
                    "cannot transition submission from {from:?} to {to:?}"
                )
            }
            Self::ConflictingIntent => {
                write!(formatter, "a conflicting durable intent already exists")
            }
            Self::MissingIntent => write!(formatter, "submission intent was not found"),
            Self::InvalidConfiguration(message) => {
                write!(formatter, "invalid service configuration: {message}")
            }
        }
    }
}

impl std::error::Error for DomainError {}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct ParticipantId(pub OpaqueId);
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct CatalogueItemId(pub OpaqueId);
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct EvidenceReference(pub OpaqueId);
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct SubmissionId(pub OpaqueId);
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct BillingAuthorisationReference(pub OpaqueId);
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct MoveTransactionReference(pub OpaqueId);
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct ProjectionVersion(pub OpaqueId);

/// A reference to the released `london.v1` usage-batch schema. The canonical
/// shared definition remains `packages/contracts/schemas/london.v1.json`; this
/// is coordinator persistence metadata, not a replacement Rust schema.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UsageBatchReference {
    pub period: OpaqueId,
    pub bucket: OpaqueId,
    pub range: UsageRange,
    pub payload_digest: OpaqueId,
}

impl UsageBatchReference {
    /// This is the released schema's replay rule: an exact range and digest is
    /// idempotent, while an overlapping range in the same period and lane is a
    /// conflict. Aggregate usage remains inside the released batch payload.
    pub fn replay_relation(&self, other: &Self) -> ReplayRelation {
        if self.period != other.period || self.range.lane != other.range.lane {
            return ReplayRelation::Distinct;
        }
        if self.range.first == other.range.first
            && self.range.last == other.range.last
            && self.payload_digest == other.payload_digest
        {
            return ReplayRelation::Exact;
        }
        if self.range.first <= other.range.last && other.range.first <= self.range.last {
            return ReplayRelation::Conflict;
        }
        ReplayRelation::Distinct
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReplayRelation {
    Exact,
    Conflict,
    Distinct,
}

/// A bounded source range supplied by the private evidence pipeline. Its
/// interpretation and aggregate contents are defined by the Move package once
/// its schema is released; services retain only this replay boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UsageRange {
    pub lane: OpaqueId,
    pub first: u64,
    pub last: u64,
}

impl UsageRange {
    pub fn new(lane: OpaqueId, first: u64, last: u64) -> Result<Self, DomainError> {
        if first > last {
            return Err(DomainError::InvalidRange);
        }
        Ok(Self { lane, first, last })
    }
}

/// The durable, idempotent instruction to submit a private-evidence-backed,
/// bounded usage range. It deliberately has no Move entry-function payload.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SubmissionIntent {
    pub id: SubmissionId,
    pub usage_batch: UsageBatchReference,
    pub evidence: EvidenceReference,
    pub state: SubmissionState,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SubmissionState {
    Prepared,
    Submitted {
        transaction: MoveTransactionReference,
    },
    Confirmed {
        transaction: MoveTransactionReference,
    },
    Indexed {
        transaction: MoveTransactionReference,
        projection: ProjectionVersion,
    },
}

impl SubmissionIntent {
    pub fn prepared(
        id: SubmissionId,
        usage_batch: UsageBatchReference,
        evidence: EvidenceReference,
    ) -> Self {
        Self {
            id,
            usage_batch,
            evidence,
            state: SubmissionState::Prepared,
        }
    }

    pub fn transition(self, next: SubmissionState) -> Result<Self, DomainError> {
        if !self.state.can_transition_to(&next) {
            return Err(DomainError::InvalidStateTransition {
                from: self.state,
                to: next,
            });
        }
        Ok(Self {
            state: next,
            ..self
        })
    }
}

impl SubmissionState {
    pub fn can_transition_to(&self, next: &Self) -> bool {
        matches!(
            (self, next),
            (Self::Prepared, Self::Submitted { .. })
                | (Self::Submitted { .. }, Self::Confirmed { .. })
                | (Self::Confirmed { .. }, Self::Indexed { .. })
        )
    }

    pub fn transaction(&self) -> Option<&MoveTransactionReference> {
        match self {
            Self::Prepared => None,
            Self::Submitted { transaction }
            | Self::Confirmed { transaction }
            | Self::Indexed { transaction, .. } => Some(transaction),
        }
    }
}

/// A cursor describes what a projection has observed. It does not attest to
/// settlement or payout completion, even when it follows a confirmed action.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectionCursor {
    pub projection: ProjectionVersion,
    pub last_indexed_submission: Option<SubmissionId>,
}

pub trait IdentityAdapter {
    type Error;
    fn admit(&self, participant: &ParticipantId) -> Result<Admission, Self::Error>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Admission {
    Allowed,
    Denied,
}

/// This boundary verifies a provider-side authorisation reference. It neither
/// computes a balance nor records financial state.
pub trait BillingAdapter {
    type Error;
    fn verify_authorisation(
        &self,
        authorisation: &BillingAuthorisationReference,
    ) -> Result<BillingAuthorisation, Self::Error>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BillingAuthorisation {
    Valid,
    Rejected,
}

pub trait Catalogue {
    type Error;
    fn is_available(&self, item: &CatalogueItemId) -> Result<CatalogueAvailability, Self::Error>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CatalogueAvailability {
    Available,
    Unavailable,
}

pub trait PrivateEvidenceStore {
    type Error;
    fn retain(&mut self, reference: EvidenceReference) -> Result<(), Self::Error>;
    fn exists(&self, reference: &EvidenceReference) -> Result<bool, Self::Error>;
}

/// A Move adapter may inspect an opaque operation reference, but financial
/// completion remains solely observable from Move state and events.
pub trait MoveLedger {
    type Error;
    fn transaction_status(
        &self,
        transaction: &MoveTransactionReference,
    ) -> Result<MoveTransactionStatus, Self::Error>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MoveTransactionStatus {
    Submitted,
    Confirmed,
    Rejected,
}

pub trait SubmissionIntentStore {
    type Error;
    fn create(&mut self, intent: SubmissionIntent) -> Result<(), Self::Error>;
    fn load(&self, id: &SubmissionId) -> Result<Option<SubmissionIntent>, Self::Error>;
    fn replace(&mut self, intent: SubmissionIntent) -> Result<(), Self::Error>;
    fn find_replay(&self, batch: &UsageBatchReference) -> Result<ReplayRecord, Self::Error>;
    fn load_cursor(&self) -> Result<Option<ProjectionCursor>, Self::Error>;
    fn save_cursor(&mut self, cursor: ProjectionCursor) -> Result<(), Self::Error>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReplayRecord {
    Exact(Box<SubmissionIntent>),
    Conflict,
    Absent,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(value: &str) -> OpaqueId {
        OpaqueId::new(value).unwrap()
    }

    #[test]
    fn submission_states_distinguish_submitted_confirmed_and_indexed() {
        let prepared = SubmissionIntent::prepared(
            SubmissionId(id("submission-1")),
            UsageBatchReference {
                period: id("period-1"),
                bucket: id("bucket-1"),
                range: UsageRange::new(id("lane-1"), 5, 8).unwrap(),
                payload_digest: id("digest-1"),
            },
            EvidenceReference(id("evidence-1")),
        );
        let transaction = MoveTransactionReference(id("tx-1"));
        let submitted = prepared
            .transition(SubmissionState::Submitted {
                transaction: transaction.clone(),
            })
            .unwrap();
        let confirmed = submitted
            .transition(SubmissionState::Confirmed {
                transaction: transaction.clone(),
            })
            .unwrap();
        let indexed = confirmed
            .transition(SubmissionState::Indexed {
                transaction,
                projection: ProjectionVersion(id("projection-7")),
            })
            .unwrap();

        assert!(matches!(indexed.state, SubmissionState::Indexed { .. }));
    }

    #[test]
    fn submission_cannot_skip_confirmation() {
        let prepared = SubmissionIntent::prepared(
            SubmissionId(id("submission-1")),
            UsageBatchReference {
                period: id("period-1"),
                bucket: id("bucket-1"),
                range: UsageRange::new(id("lane-1"), 5, 8).unwrap(),
                payload_digest: id("digest-1"),
            },
            EvidenceReference(id("evidence-1")),
        );
        let result = prepared.transition(SubmissionState::Confirmed {
            transaction: MoveTransactionReference(id("tx-1")),
        });
        assert!(matches!(
            result,
            Err(DomainError::InvalidStateTransition { .. })
        ));
    }

    #[test]
    fn released_schema_replay_rules_preserve_exact_retry_and_reject_overlap() {
        let first = UsageBatchReference {
            period: id("period-1"),
            bucket: id("bucket-1"),
            range: UsageRange::new(id("lane-1"), 1, 2).unwrap(),
            payload_digest: id("digest-1"),
        };
        let exact = first.clone();
        let overlap = UsageBatchReference {
            period: id("period-1"),
            bucket: id("bucket-2"),
            range: UsageRange::new(id("lane-1"), 2, 3).unwrap(),
            payload_digest: id("digest-2"),
        };
        assert_eq!(first.replay_relation(&exact), ReplayRelation::Exact);
        assert_eq!(first.replay_relation(&overlap), ReplayRelation::Conflict);
    }
}
