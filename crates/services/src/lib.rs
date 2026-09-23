//! Coordinator persistence semantics. This crate stores intent and projection
//! progress only; it does not calculate, allocate, or complete financial work.

use std::collections::BTreeMap;

use porto_core_domain::{
    DomainError, ProjectionCursor, ProjectionVersion, ReplayRecord, ReplayRelation, SubmissionId,
    SubmissionIntent, SubmissionIntentStore, SubmissionState, UsageBatchReference,
};

/// A test adapter for the durable-store boundary. Production persistence is an
/// infrastructure choice and must implement `SubmissionIntentStore` with the
/// same create-before-submit and cursor-save semantics.
#[derive(Default)]
pub struct InMemoryIntentStore {
    intents: BTreeMap<SubmissionId, SubmissionIntent>,
    cursor: Option<ProjectionCursor>,
}

impl SubmissionIntentStore for InMemoryIntentStore {
    type Error = DomainError;

    fn create(&mut self, intent: SubmissionIntent) -> Result<(), Self::Error> {
        if self.intents.contains_key(&intent.id) {
            return Err(DomainError::ConflictingIntent);
        }
        self.intents.insert(intent.id.clone(), intent);
        Ok(())
    }

    fn load(&self, id: &SubmissionId) -> Result<Option<SubmissionIntent>, Self::Error> {
        Ok(self.intents.get(id).cloned())
    }

    fn replace(&mut self, intent: SubmissionIntent) -> Result<(), Self::Error> {
        if !self.intents.contains_key(&intent.id) {
            return Err(DomainError::MissingIntent);
        }
        self.intents.insert(intent.id.clone(), intent);
        Ok(())
    }

    fn find_replay(&self, batch: &UsageBatchReference) -> Result<ReplayRecord, Self::Error> {
        for intent in self.intents.values() {
            match intent.usage_batch.replay_relation(batch) {
                ReplayRelation::Exact => return Ok(ReplayRecord::Exact(Box::new(intent.clone()))),
                ReplayRelation::Conflict => return Ok(ReplayRecord::Conflict),
                ReplayRelation::Distinct => {}
            }
        }
        Ok(ReplayRecord::Absent)
    }

    fn load_cursor(&self) -> Result<Option<ProjectionCursor>, Self::Error> {
        Ok(self.cursor.clone())
    }

    fn save_cursor(&mut self, cursor: ProjectionCursor) -> Result<(), Self::Error> {
        self.cursor = Some(cursor);
        Ok(())
    }
}

/// Persists the exact intent before any Move adapter may submit it. Repeating
/// the same request returns its original record; conflicting reuse is rejected.
pub fn record_submission<S>(
    store: &mut S,
    intent: SubmissionIntent,
) -> Result<SubmissionIntent, S::Error>
where
    S: SubmissionIntentStore,
    S::Error: From<DomainError>,
{
    match store.find_replay(&intent.usage_batch)? {
        ReplayRecord::Exact(existing) => Ok(*existing),
        ReplayRecord::Conflict => Err(S::Error::from(DomainError::ReplayConflict)),
        ReplayRecord::Absent => match store.load(&intent.id)? {
            Some(existing) if existing == intent => Ok(existing),
            Some(_) => Err(S::Error::from(DomainError::ConflictingIntent)),
            None => {
                store.create(intent.clone())?;
                Ok(intent)
            }
        },
    }
}

/// Advances an already persisted intent. A caller must retain the updated
/// record before exposing the state change to the next service step.
pub fn advance_submission<S>(
    store: &mut S,
    id: &SubmissionId,
    next: SubmissionState,
) -> Result<SubmissionIntent, S::Error>
where
    S: SubmissionIntentStore,
    S::Error: From<DomainError>,
{
    let existing = store
        .load(id)?
        .ok_or_else(|| S::Error::from(DomainError::MissingIntent))?;
    let updated = existing.transition(next).map_err(S::Error::from)?;
    store.replace(updated.clone())?;
    Ok(updated)
}

/// Projection movement is explicit and separate from Move confirmation. The
/// cursor contains no conclusion about allocation or payout completion.
pub fn advance_projection_cursor<S>(
    store: &mut S,
    projection: ProjectionVersion,
    last_indexed_submission: Option<SubmissionId>,
) -> Result<ProjectionCursor, S::Error>
where
    S: SubmissionIntentStore,
{
    let cursor = ProjectionCursor {
        projection,
        last_indexed_submission,
    };
    store.save_cursor(cursor.clone())?;
    Ok(cursor)
}

#[cfg(test)]
mod tests {
    use porto_core_domain::{
        EvidenceReference, MoveTransactionReference, OpaqueId, UsageBatchReference, UsageRange,
    };

    use super::*;

    fn id(value: &str) -> OpaqueId {
        OpaqueId::new(value).unwrap()
    }

    fn prepared() -> SubmissionIntent {
        SubmissionIntent::prepared(
            SubmissionId(id("submission-1")),
            UsageBatchReference {
                period: id("period-1"),
                bucket: id("bucket-1"),
                range: UsageRange::new(id("lane-1"), 1, 2).unwrap(),
                payload_digest: id("digest-1"),
            },
            EvidenceReference(id("evidence-1")),
        )
    }

    #[test]
    fn records_before_submission_and_preserves_exact_retry() {
        let mut store = InMemoryIntentStore::default();
        let first = record_submission(&mut store, prepared()).unwrap();
        let retry = record_submission(&mut store, prepared()).unwrap();
        assert_eq!(first, retry);
        assert!(matches!(first.state, SubmissionState::Prepared));
    }

    #[test]
    fn persists_submitted_then_confirmed_then_indexed_as_distinct_states() {
        let mut store = InMemoryIntentStore::default();
        let intent = record_submission(&mut store, prepared()).unwrap();
        let transaction = MoveTransactionReference(id("tx-1"));
        let submitted = advance_submission(
            &mut store,
            &intent.id,
            SubmissionState::Submitted {
                transaction: transaction.clone(),
            },
        )
        .unwrap();
        let confirmed = advance_submission(
            &mut store,
            &intent.id,
            SubmissionState::Confirmed {
                transaction: transaction.clone(),
            },
        )
        .unwrap();
        let indexed = advance_submission(
            &mut store,
            &intent.id,
            SubmissionState::Indexed {
                transaction,
                projection: ProjectionVersion(id("projection-1")),
            },
        )
        .unwrap();

        assert!(matches!(submitted.state, SubmissionState::Submitted { .. }));
        assert!(matches!(confirmed.state, SubmissionState::Confirmed { .. }));
        assert!(matches!(indexed.state, SubmissionState::Indexed { .. }));
    }

    #[test]
    fn cursor_is_persisted_independently_from_confirmation() {
        let mut store = InMemoryIntentStore::default();
        let cursor =
            advance_projection_cursor(&mut store, ProjectionVersion(id("projection-1")), None)
                .unwrap();
        assert_eq!(store.load_cursor().unwrap(), Some(cursor));
    }

    #[test]
    fn conflicting_replay_range_is_rejected() {
        let mut store = InMemoryIntentStore::default();
        record_submission(&mut store, prepared()).unwrap();
        let mut conflicting = prepared();
        conflicting.id = SubmissionId(id("submission-2"));
        conflicting.usage_batch.range = UsageRange::new(id("lane-1"), 2, 3).unwrap();
        conflicting.usage_batch.payload_digest = id("digest-2");

        assert_eq!(
            record_submission(&mut store, conflicting),
            Err(DomainError::ReplayConflict)
        );
    }
}
