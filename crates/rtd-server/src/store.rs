//! Durable storage traits for server state.
//!
//! The edge holds only transient state; a core persists authorization keys and
//! session-layer evidence through these traits. [`MemoryStore`] is an in-memory
//! implementation for single-process deployments and tests; a PostgreSQL
//! backend implements the same traits.

use std::collections::HashMap;
use std::sync::Mutex;

use crate::core::{AuthKeyId, AuthKeyRecord, SessionId};

/// An error raised by a storage backend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreError(pub String);

impl core::fmt::Display for StoreError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.0)
    }
}

impl core::error::Error for StoreError {}

/// Stores authorization keys durably.
pub trait AuthKeyStore: Send + Sync {
    /// Saves (or replaces) `record`.
    ///
    /// # Errors
    ///
    /// Returns a backend error.
    fn save(&self, record: AuthKeyRecord) -> Result<(), StoreError>;

    /// Returns the record for `id`, if present.
    ///
    /// # Errors
    ///
    /// Returns a backend error.
    fn get(&self, id: AuthKeyId) -> Result<Option<AuthKeyRecord>, StoreError>;

    /// Deletes the record for `id`.
    ///
    /// # Errors
    ///
    /// Returns a backend error.
    fn delete(&self, id: AuthKeyId) -> Result<(), StoreError>;
}

/// Stores session-layer evidence durably.
pub trait LayerEvidenceStore: Send + Sync {
    /// Returns the layer recorded for `session`, if any.
    ///
    /// # Errors
    ///
    /// Returns a backend error.
    fn resolve(&self, session: SessionId) -> Result<Option<i32>, StoreError>;

    /// Advances `session`'s layer to `layer` if it is higher.
    ///
    /// # Errors
    ///
    /// Returns a backend error.
    fn advance(&self, session: SessionId, layer: i32) -> Result<(), StoreError>;

    /// Publishes `session`'s layer as `layer`.
    ///
    /// # Errors
    ///
    /// Returns a backend error.
    fn publish(&self, session: SessionId, layer: i32) -> Result<(), StoreError>;
}

/// An in-memory store implementing both storage traits.
#[derive(Debug, Default)]
pub struct MemoryStore {
    keys: Mutex<HashMap<AuthKeyId, AuthKeyRecord>>,
    evidence: Mutex<HashMap<SessionId, i32>>,
}

impl MemoryStore {
    /// Creates an empty store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl AuthKeyStore for MemoryStore {
    fn save(&self, record: AuthKeyRecord) -> Result<(), StoreError> {
        self.keys
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(record.id, record);
        Ok(())
    }

    fn get(&self, id: AuthKeyId) -> Result<Option<AuthKeyRecord>, StoreError> {
        Ok(self
            .keys
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&id)
            .cloned())
    }

    fn delete(&self, id: AuthKeyId) -> Result<(), StoreError> {
        self.keys
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&id);
        Ok(())
    }
}

impl LayerEvidenceStore for MemoryStore {
    fn resolve(&self, session: SessionId) -> Result<Option<i32>, StoreError> {
        Ok(self
            .evidence
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&session)
            .copied())
    }

    fn advance(&self, session: SessionId, layer: i32) -> Result<(), StoreError> {
        let mut evidence = self
            .evidence
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let entry = evidence.entry(session).or_insert(layer);
        if layer > *entry {
            *entry = layer;
        }
        Ok(())
    }

    fn publish(&self, session: SessionId, layer: i32) -> Result<(), StoreError> {
        self.evidence
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(session, layer);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{AuthKeyStore, LayerEvidenceStore, MemoryStore};
    use crate::core::{AuthKeyId, AuthKeyRecord, SessionId};

    fn record(id: u8) -> AuthKeyRecord {
        AuthKeyRecord {
            id: AuthKeyId([id; 8]),
            dc: 2,
            key: vec![id; 256],
            expires_at: 0,
        }
    }

    #[test]
    fn auth_keys_round_trip() {
        let store = MemoryStore::new();
        assert!(store.get(AuthKeyId([1; 8])).unwrap().is_none());
        store.save(record(1)).unwrap();
        assert_eq!(store.get(AuthKeyId([1; 8])).unwrap(), Some(record(1)));
        store.delete(AuthKeyId([1; 8])).unwrap();
        assert!(store.get(AuthKeyId([1; 8])).unwrap().is_none());
    }

    #[test]
    fn layer_evidence_advances_monotonically() {
        let store = MemoryStore::new();
        let session = SessionId(7);
        assert!(store.resolve(session).unwrap().is_none());
        store.advance(session, 229).unwrap();
        store.advance(session, 225).unwrap();
        assert_eq!(store.resolve(session).unwrap(), Some(229));
        store.publish(session, 232).unwrap();
        assert_eq!(store.resolve(session).unwrap(), Some(232));
    }
}
