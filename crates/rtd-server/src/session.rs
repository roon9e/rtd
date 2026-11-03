//! The transient session registry and auth-key cache.
//!
//! Owned by task group 6. Implemented in this file.
//!
//! The edge holds only *transient* state for the life of a connection; the core
//! is the durable owner of authorization keys and session-layer evidence (design
//! D22). This module provides:
//!
//! - [`SessionRegistry`] — a transient registry keyed by `(auth_key_id,
//!   session_id)` with clock-driven eviction on disconnect; and
//! - [`AuthKeyCache`] — a transient cache keyed by authorization-key id.
//!
//! It also provides the core-backed durable accessors. A newly created key is
//! saved *before* the exchange is confirmed ([`save_auth_key_before_confirm`]),
//! and a key the edge does not hold is fetched from the core
//! ([`resolve_auth_key`]); a core failure fails the operation closed rather than
//! assuming state was written.
//!
//! The registry and cache are shared across connection tasks, so they use
//! `tokio` synchronization; every operation is short and never awaits while
//! holding the lock.

use std::collections::HashMap;
use std::time::{Duration, SystemTime};

use rtd_crypto::{AuthKey, Key};
use rtd_error::Error as RtdError;
use rtd_util::{Clock, SystemClock};
use tokio::sync::Mutex;

use crate::core::{AuthKeyId, AuthKeyRecord, Core, SessionId};

/// Default idle timeout after which an unused transient session is evicted.
pub const DEFAULT_REGISTRY_IDLE_TIMEOUT: Duration = Duration::from_secs(300);

/// The composite key identifying a transient session at the edge.
///
/// A session is scoped to the authorization key that owns it, so two clients
/// that happen to pick the same session identifier do not collide.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SessionKey {
    /// The authorization key the session belongs to.
    pub auth_key_id: AuthKeyId,
    /// The session identifier carried in the encrypted-message inner header.
    pub session_id: SessionId,
}

impl SessionKey {
    /// Creates a session key from its parts.
    #[must_use]
    pub const fn new(auth_key_id: AuthKeyId, session_id: SessionId) -> Self {
        Self {
            auth_key_id,
            session_id,
        }
    }
}

/// A snapshot of a registered transient session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionRecord {
    /// The composite registry key.
    pub key: SessionKey,
    /// When the session was first registered.
    pub registered_at: SystemTime,
    /// When the session last saw activity.
    pub last_activity: SystemTime,
}

impl SessionRecord {
    /// Reports whether the record has been idle for at least `idle_timeout`.
    #[must_use]
    pub fn is_expired(&self, now: SystemTime, idle_timeout: Duration) -> bool {
        now.duration_since(self.last_activity)
            .map(|idle| idle >= idle_timeout)
            .unwrap_or(false)
    }
}

/// A transient registry of active sessions, shared across connection tasks.
///
/// Registration happens when a connection adopts a session identifier; the
/// record is touched on activity and evicted when the connection disconnects or
/// the session has been idle for longer than the configured timeout.
pub struct SessionRegistry {
    clock: std::sync::Arc<dyn Clock + Send + Sync>,
    idle_timeout: Duration,
    sessions: Mutex<HashMap<SessionKey, SessionRecord>>,
}

impl SessionRegistry {
    /// Creates an empty registry driven by `clock`.
    #[must_use]
    pub fn new(clock: std::sync::Arc<dyn Clock + Send + Sync>, idle_timeout: Duration) -> Self {
        Self {
            clock,
            idle_timeout,
            sessions: Mutex::new(HashMap::new()),
        }
    }

    /// Returns the configured idle timeout.
    #[must_use]
    pub const fn idle_timeout(&self) -> Duration {
        self.idle_timeout
    }

    /// Registers `key`, returning the (possibly pre-existing) record.
    pub async fn register(&self, key: SessionKey) -> SessionRecord {
        let now = self.clock.now();
        let mut sessions = self.sessions.lock().await;
        let record = sessions.entry(key).or_insert(SessionRecord {
            key,
            registered_at: now,
            last_activity: now,
        });
        record.last_activity = now;
        *record
    }

    /// Looks up `key`, returning a copy of its record if present.
    pub async fn lookup(&self, key: SessionKey) -> Option<SessionRecord> {
        self.sessions.lock().await.get(&key).copied()
    }

    /// Marks `key` as active at the current clock time, if registered.
    pub async fn touch(&self, key: SessionKey) -> bool {
        let now = self.clock.now();
        let mut sessions = self.sessions.lock().await;
        match sessions.get_mut(&key) {
            Some(record) => {
                record.last_activity = now;
                true
            }
            None => false,
        }
    }

    /// Evicts `key`, returning whether it was registered.
    pub async fn evict(&self, key: SessionKey) -> bool {
        self.sessions.lock().await.remove(&key).is_some()
    }

    /// Evicts every record idle for at least the configured timeout.
    ///
    /// Returns the evicted keys. Eviction is clock-driven, so it is also
    /// available to a housekeeping task independent of any disconnect.
    pub async fn evict_expired(&self) -> Vec<SessionKey> {
        let now = self.clock.now();
        let idle_timeout = self.idle_timeout;
        let mut sessions = self.sessions.lock().await;
        let mut evicted = Vec::new();
        sessions.retain(|key, record| {
            if record.is_expired(now, idle_timeout) {
                evicted.push(*key);
                false
            } else {
                true
            }
        });
        evicted
    }

    /// Returns the number of registered sessions.
    pub async fn len(&self) -> usize {
        self.sessions.lock().await.len()
    }

    /// Reports whether the registry is empty.
    pub async fn is_empty(&self) -> bool {
        self.sessions.lock().await.is_empty()
    }
}

/// Default maximum number of cached authorization keys.
pub const DEFAULT_AUTH_KEY_CACHE_MAX_ENTRIES: usize = 262_144;
/// Default time-to-live for a cached authorization key.
pub const DEFAULT_AUTH_KEY_CACHE_TTL: Duration = Duration::from_secs(30 * 60);
/// Default maximum number of cached temporary keys.
pub const DEFAULT_TEMP_KEY_CACHE_MAX_ENTRIES: usize = 262_144;
/// Default time-to-live for a cached temporary key.
pub const DEFAULT_TEMP_KEY_CACHE_TTL: Duration = Duration::from_secs(30 * 60);

/// Limits for a bounded cache: a maximum entry count and a time-to-live.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CacheLimits {
    /// The maximum number of entries kept before the least-recently-used is
    /// evicted.
    pub max_entries: usize,
    /// The time-to-live of a cached entry.
    pub ttl: Duration,
}

impl CacheLimits {
    /// Creates limits from a maximum and a TTL.
    #[must_use]
    pub const fn new(max_entries: usize, ttl: Duration) -> Self {
        Self { max_entries, ttl }
    }
}

impl Default for CacheLimits {
    fn default() -> Self {
        Self::new(
            DEFAULT_AUTH_KEY_CACHE_MAX_ENTRIES,
            DEFAULT_AUTH_KEY_CACHE_TTL,
        )
    }
}

/// One cached authorization key with its insertion and last-use times.
#[derive(Debug, Clone, Copy)]
struct CachedKey {
    key: AuthKey,
    inserted_at: SystemTime,
    last_used: SystemTime,
}

/// Returns whether an entry inserted at `inserted_at` has expired at `now`.
fn expires(inserted_at: SystemTime, now: SystemTime, ttl: Duration) -> bool {
    now.duration_since(inserted_at)
        .map(|age| age >= ttl)
        .unwrap_or(false)
}

/// A bounded, read-through cache of authorization keys.
///
/// The cache is a read-through front for [`Core::get_auth_key`]; the core
/// remains the source of truth (design D22). Entries expire after the configured
/// TTL, and the least-recently-used entry is evicted once the maximum is
/// reached. A key can be invalidated (on `auth_key_switch` or
/// `destroy_auth_key`) with [`AuthKeyCache::remove`].
pub struct AuthKeyCache {
    clock: std::sync::Arc<dyn Clock + Send + Sync>,
    limits: CacheLimits,
    keys: Mutex<HashMap<AuthKeyId, CachedKey>>,
}

impl AuthKeyCache {
    /// Creates a cache with the default limits.
    #[must_use]
    pub fn new() -> Self {
        Self::with_limits(CacheLimits::default(), std::sync::Arc::new(SystemClock))
    }

    /// Creates a cache with explicit `limits`, driven by `clock`.
    #[must_use]
    pub fn with_limits(
        limits: CacheLimits,
        clock: std::sync::Arc<dyn Clock + Send + Sync>,
    ) -> Self {
        Self {
            clock,
            limits,
            keys: Mutex::new(HashMap::new()),
        }
    }

    /// Returns the cached key for `id`, if present and unexpired.
    ///
    /// An expired entry is removed and reported as a miss.
    pub async fn get(&self, id: AuthKeyId) -> Option<AuthKey> {
        let now = self.clock.now();
        let mut keys = self.keys.lock().await;
        let entry = *keys.get(&id)?;
        if expires(entry.inserted_at, now, self.limits.ttl) {
            keys.remove(&id);
            return None;
        }
        if let Some(stored) = keys.get_mut(&id) {
            stored.last_used = now;
        }
        Some(entry.key)
    }

    /// Inserts `key`, evicting the least-recently-used entry when full.
    pub async fn insert(&self, key: AuthKey) -> AuthKeyId {
        let now = self.clock.now();
        let id = AuthKeyId(key.id);
        let mut keys = self.keys.lock().await;
        keys.insert(
            id,
            CachedKey {
                key,
                inserted_at: now,
                last_used: now,
            },
        );
        while keys.len() > self.limits.max_entries {
            let Some(oldest) = keys
                .iter()
                .min_by_key(|(_, entry)| entry.last_used)
                .map(|(id, _)| *id)
            else {
                break;
            };
            keys.remove(&oldest);
        }
        id
    }

    /// Removes and returns the cached key for `id`.
    pub async fn remove(&self, id: AuthKeyId) -> Option<AuthKey> {
        self.keys.lock().await.remove(&id).map(|entry| entry.key)
    }

    /// Evicts every entry older than the TTL, returning how many were removed.
    pub async fn evict_expired(&self) -> usize {
        let now = self.clock.now();
        let ttl = self.limits.ttl;
        let mut keys = self.keys.lock().await;
        let before = keys.len();
        keys.retain(|_, entry| !expires(entry.inserted_at, now, ttl));
        before - keys.len()
    }

    /// Returns the number of cached keys.
    pub async fn len(&self) -> usize {
        self.keys.lock().await.len()
    }

    /// Reports whether the cache is empty.
    pub async fn is_empty(&self) -> bool {
        self.keys.lock().await.is_empty()
    }
}

impl Default for AuthKeyCache {
    fn default() -> Self {
        Self::new()
    }
}

/// A bounded cache of temporary (`bindTempAuthKey`) authorization keys.
///
/// It shares [`AuthKeyCache`]'s bounded, TTL-driven eviction but is a distinct
/// type so temporary keys can be sized independently (see `temp_key_cache` in
/// the edge configuration).
pub struct TempKeyCache(AuthKeyCache);

impl TempKeyCache {
    /// Creates a cache with the default temporary-key limits.
    #[must_use]
    pub fn new() -> Self {
        Self::with_limits(
            CacheLimits::new(
                DEFAULT_TEMP_KEY_CACHE_MAX_ENTRIES,
                DEFAULT_TEMP_KEY_CACHE_TTL,
            ),
            std::sync::Arc::new(SystemClock),
        )
    }

    /// Creates a cache with explicit `limits`, driven by `clock`.
    #[must_use]
    pub fn with_limits(
        limits: CacheLimits,
        clock: std::sync::Arc<dyn Clock + Send + Sync>,
    ) -> Self {
        Self(AuthKeyCache::with_limits(limits, clock))
    }

    /// Returns the cached key for `id`, if present and unexpired.
    pub async fn get(&self, id: AuthKeyId) -> Option<AuthKey> {
        self.0.get(id).await
    }

    /// Inserts `key`, evicting the least-recently-used entry when full.
    pub async fn insert(&self, key: AuthKey) -> AuthKeyId {
        self.0.insert(key).await
    }

    /// Removes and returns the cached key for `id`.
    pub async fn remove(&self, id: AuthKeyId) -> Option<AuthKey> {
        self.0.remove(id).await
    }

    /// Returns the number of cached keys.
    pub async fn len(&self) -> usize {
        self.0.len().await
    }

    /// Reports whether the cache is empty.
    pub async fn is_empty(&self) -> bool {
        self.0.is_empty().await
    }
}

impl Default for TempKeyCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Reconstructs an [`AuthKey`] from a durable core record.
///
/// # Errors
///
/// Fails closed with a core-failure error when the record does not carry a
/// well-formed 2048-bit key.
fn auth_key_from_record(record: &AuthKeyRecord) -> Result<AuthKey, RtdError> {
    let bytes: [u8; Key::LEN] = record
        .key
        .as_slice()
        .try_into()
        .map_err(|_| crate::core::core_failure_error())?;
    Ok(AuthKey::new(Key::from_array(bytes)))
}

/// Returns the durable record for an authorization key.
#[must_use]
fn record_for(key: &AuthKey, dc: i32, expires_at: i32) -> AuthKeyRecord {
    AuthKeyRecord {
        id: AuthKeyId(key.id),
        dc,
        key: key.value.as_bytes().to_vec(),
        expires_at,
    }
}

/// Saves a newly created authorization key to the core *before* the exchange is
/// confirmed, and caches it on success.
///
/// A core failure is propagated without caching the key, so the caller must not
/// confirm the exchange.
///
/// # Errors
///
/// Returns the core's error (or a core-failure error) when the key could not be
/// persisted.
pub async fn save_auth_key_before_confirm(
    core: &dyn Core,
    cache: &AuthKeyCache,
    key: &AuthKey,
    dc: i32,
    expires_at: i32,
) -> Result<AuthKeyRecord, RtdError> {
    let record = record_for(key, dc, expires_at);
    core.save_auth_key(record.clone()).await?;
    cache.insert(*key).await;
    Ok(record)
}

/// Resolves an authorization key from the cache, falling back to the core.
///
/// A key the edge does not hold is fetched from the core and cached for the life
/// of the connection. A missing key is `Ok(None)`; a core failure is `Err`.
///
/// # Errors
///
/// Returns the core's error when the lookup failed, or a core-failure error when
/// the core returned a malformed record.
pub async fn resolve_auth_key(
    core: &dyn Core,
    cache: &AuthKeyCache,
    id: AuthKeyId,
) -> Result<Option<AuthKey>, RtdError> {
    if let Some(key) = cache.get(id).await {
        return Ok(Some(key));
    }
    match core.get_auth_key(id).await? {
        Some(record) => {
            let key = auth_key_from_record(&record)?;
            cache.insert(key).await;
            Ok(Some(key))
        }
        None => Ok(None),
    }
}

/// Resolves durable session-layer evidence the edge does not hold.
///
/// # Errors
///
/// Returns the core's error when the lookup failed.
pub async fn resolve_layer_evidence(
    core: &dyn Core,
    session: SessionId,
) -> Result<Option<i32>, RtdError> {
    core.resolve_layer_evidence(session).await
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};

    use rtd_util::ManualClock;

    use super::*;
    use crate::core::{
        AdmittedCall, CORE_PROTOCOL_VERSION, CallResult, CoreFuture, CoreInfo, LayerEvidence,
        core_failure_error,
    };

    const BASE_SECS: u64 = 1_700_000_000;

    fn clock_at(secs: u64) -> ManualClock {
        ManualClock::new(std::time::UNIX_EPOCH + Duration::from_secs(secs))
    }

    fn key(seed: u8) -> AuthKey {
        let mut bytes = [0u8; 256];
        for (i, byte) in bytes.iter_mut().enumerate() {
            *byte = seed.wrapping_add(i as u8);
        }
        AuthKey::new(Key::from_array(bytes))
    }

    /// A fake core whose durable store is an in-memory map and whose save can be
    /// made to fail, to prove the fail-closed path.
    struct FakeCore {
        keys: std::sync::Mutex<HashMap<AuthKeyId, AuthKeyRecord>>,
        evidence: std::sync::Mutex<HashMap<SessionId, i32>>,
        fail_save: AtomicBool,
    }

    impl FakeCore {
        fn new() -> Self {
            Self {
                keys: std::sync::Mutex::new(HashMap::new()),
                evidence: std::sync::Mutex::new(HashMap::new()),
                fail_save: AtomicBool::new(false),
            }
        }

        fn with_key(record: AuthKeyRecord) -> Self {
            let core = Self::new();
            core.keys
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .insert(record.id, record);
            core
        }
    }

    impl Core for FakeCore {
        fn info(&self) -> CoreFuture<'_, Result<CoreInfo, RtdError>> {
            Box::pin(async move {
                Ok(CoreInfo {
                    protocol_version: CORE_PROTOCOL_VERSION,
                    dc: 2,
                    capabilities: Vec::new(),
                })
            })
        }

        fn dispatch(&self, _call: AdmittedCall) -> CoreFuture<'_, Result<CallResult, RtdError>> {
            Box::pin(async move { Err(core_failure_error()) })
        }

        fn save_auth_key(&self, key: AuthKeyRecord) -> CoreFuture<'_, Result<(), RtdError>> {
            let fail = self.fail_save.load(Ordering::Relaxed);
            Box::pin(async move {
                if fail {
                    return Err(core_failure_error());
                }
                self.keys
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .insert(key.id, key);
                Ok(())
            })
        }

        fn get_auth_key(
            &self,
            id: AuthKeyId,
        ) -> CoreFuture<'_, Result<Option<AuthKeyRecord>, RtdError>> {
            Box::pin(async move {
                let key = self
                    .keys
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .get(&id)
                    .cloned();
                Ok(key)
            })
        }

        fn resolve_layer_evidence(
            &self,
            session: SessionId,
        ) -> CoreFuture<'_, Result<Option<i32>, RtdError>> {
            Box::pin(async move {
                let layer = self
                    .evidence
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .get(&session)
                    .copied();
                Ok(layer)
            })
        }

        fn advance_layer_evidence(
            &self,
            session: SessionId,
            layer: i32,
        ) -> CoreFuture<'_, Result<(), RtdError>> {
            Box::pin(async move {
                let mut evidence = self
                    .evidence
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                let entry = evidence.entry(session).or_insert(layer);
                if layer > *entry {
                    *entry = layer;
                }
                Ok(())
            })
        }

        fn publish_layer_evidence(
            &self,
            evidence: LayerEvidence,
        ) -> CoreFuture<'_, Result<(), RtdError>> {
            Box::pin(async move {
                self.evidence
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .insert(evidence.session, evidence.layer);
                Ok(())
            })
        }

        fn report_session_offline(
            &self,
            _auth_key_id: AuthKeyId,
            session: SessionId,
        ) -> CoreFuture<'_, Result<(), RtdError>> {
            Box::pin(async move {
                self.evidence
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .remove(&session);
                Ok(())
            })
        }
    }

    #[tokio::test]
    async fn registry_register_lookup_evict() {
        let registry = SessionRegistry::new(
            std::sync::Arc::new(clock_at(BASE_SECS)),
            DEFAULT_REGISTRY_IDLE_TIMEOUT,
        );
        let key = SessionKey::new(AuthKeyId([1, 2, 3, 4, 5, 6, 7, 8]), SessionId(9));
        assert!(registry.lookup(key).await.is_none());

        let record = registry.register(key).await;
        assert_eq!(record.key, key);
        assert_eq!(registry.len().await, 1);
        assert_eq!(registry.lookup(key).await, Some(record));

        assert!(registry.evict(key).await);
        assert!(registry.is_empty().await);
        assert!(!registry.evict(key).await);
    }

    #[tokio::test]
    async fn registry_evicts_on_idle_timeout() {
        let clock = std::sync::Arc::new(clock_at(BASE_SECS));
        let registry = SessionRegistry::new(clock.clone(), Duration::from_secs(60));
        let key = SessionKey::new(AuthKeyId([0; 8]), SessionId(1));
        registry.register(key).await;

        clock.advance(Duration::from_secs(59));
        assert!(registry.evict_expired().await.is_empty());
        clock.advance(Duration::from_secs(2));
        assert_eq!(registry.evict_expired().await, vec![key]);

        // Activity defers eviction.
        registry.register(key).await;
        clock.advance(Duration::from_secs(50));
        assert!(registry.touch(key).await);
        clock.advance(Duration::from_secs(50));
        assert!(registry.evict_expired().await.is_empty());
        assert!(registry.lookup(key).await.is_some());
    }

    #[tokio::test]
    async fn auth_key_cache_round_trip() {
        let cache = AuthKeyCache::new();
        let key = key(7);
        let id = AuthKeyId(key.id);
        assert!(cache.get(id).await.is_none());
        assert_eq!(cache.insert(key).await, id);
        assert_eq!(cache.get(id).await, Some(key));
        assert_eq!(cache.remove(id).await, Some(key));
        assert!(cache.is_empty().await);
    }

    #[tokio::test]
    async fn cache_evicts_least_recently_used() {
        let clock = std::sync::Arc::new(clock_at(BASE_SECS));
        let cache = AuthKeyCache::with_limits(
            CacheLimits::new(2, Duration::from_secs(3600)),
            clock.clone(),
        );
        let a = key(1);
        let b = key(2);
        let c = key(3);
        cache.insert(a).await;
        clock.advance(Duration::from_secs(1));
        cache.insert(b).await;
        // Touch `a` so `b` becomes the least recently used.
        clock.advance(Duration::from_secs(1));
        assert!(cache.get(AuthKeyId(a.id)).await.is_some());
        clock.advance(Duration::from_secs(1));
        cache.insert(c).await;

        assert_eq!(cache.len().await, 2);
        assert!(cache.get(AuthKeyId(b.id)).await.is_none());
        assert!(cache.get(AuthKeyId(a.id)).await.is_some());
        assert!(cache.get(AuthKeyId(c.id)).await.is_some());
    }

    #[tokio::test]
    async fn cache_expires_by_ttl() {
        let clock = std::sync::Arc::new(clock_at(BASE_SECS));
        let cache =
            AuthKeyCache::with_limits(CacheLimits::new(10, Duration::from_secs(60)), clock.clone());
        let a = key(4);
        cache.insert(a).await;
        clock.advance(Duration::from_secs(59));
        assert!(cache.get(AuthKeyId(a.id)).await.is_some());
        clock.advance(Duration::from_secs(2));
        assert!(cache.get(AuthKeyId(a.id)).await.is_none());
        assert!(cache.is_empty().await);
        assert_eq!(cache.evict_expired().await, 0);
    }

    #[tokio::test]
    async fn temp_key_cache_round_trips() {
        let cache = TempKeyCache::new();
        let key = key(6);
        let id = AuthKeyId(key.id);
        assert!(cache.is_empty().await);
        cache.insert(key).await;
        assert_eq!(cache.get(id).await, Some(key));
        assert_eq!(cache.remove(id).await, Some(key));
        assert!(cache.is_empty().await);
    }

    #[tokio::test]
    async fn save_before_confirm_persists_to_core() {
        let core = FakeCore::new();
        let cache = AuthKeyCache::new();
        let key = key(3);
        let record = save_auth_key_before_confirm(&core, &cache, &key, 2, 0)
            .await
            .expect("save");
        assert_eq!(record.id, AuthKeyId(key.id));
        assert_eq!(record.dc, 2);
        assert_eq!(record.key, key.value.as_bytes().to_vec());
        // The key is now in both the durable core and the transient cache.
        assert_eq!(
            core.get_auth_key(AuthKeyId(key.id))
                .await
                .expect("core get"),
            Some(record)
        );
        assert_eq!(cache.get(AuthKeyId(key.id)).await, Some(key));
    }

    #[tokio::test]
    async fn save_before_confirm_fails_closed() {
        let core = FakeCore::new();
        core.fail_save.store(true, Ordering::Relaxed);
        let cache = AuthKeyCache::new();
        let key = key(5);
        let error = save_auth_key_before_confirm(&core, &cache, &key, 2, 0)
            .await
            .expect_err("save fails");
        assert_eq!(error.message(), "CORE_FAILURE");
        // The key must not be cached when the core refused to persist it.
        assert!(cache.get(AuthKeyId(key.id)).await.is_none());
        assert!(
            core.get_auth_key(AuthKeyId(key.id))
                .await
                .expect("get")
                .is_none()
        );
    }

    #[tokio::test]
    async fn fetch_on_unknown_resolves_from_core_and_caches() {
        let key = key(9);
        let record = record_for(&key, 2, 0);
        let core = FakeCore::with_key(record.clone());
        let cache = AuthKeyCache::new();

        let resolved = resolve_auth_key(&core, &cache, AuthKeyId(key.id))
            .await
            .expect("resolve");
        assert_eq!(resolved, Some(key));
        // The second lookup is served from the cache; the core record is gone.
        core.keys
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clear();
        assert_eq!(
            resolve_auth_key(&core, &cache, AuthKeyId(key.id))
                .await
                .expect("cached resolve"),
            Some(key)
        );
    }

    #[tokio::test]
    async fn resolve_unknown_key_is_none() {
        let core = FakeCore::new();
        let cache = AuthKeyCache::new();
        assert_eq!(
            resolve_auth_key(&core, &cache, AuthKeyId([0; 8]))
                .await
                .expect("resolve"),
            None
        );
    }

    #[tokio::test]
    async fn resolve_layer_evidence_is_core_backed() {
        let core = FakeCore::new();
        let session = SessionId(4);
        assert_eq!(
            resolve_layer_evidence(&core, session)
                .await
                .expect("resolve"),
            None
        );
        core.publish_layer_evidence(LayerEvidence {
            session,
            layer: 232,
        })
        .await
        .expect("publish");
        assert_eq!(
            resolve_layer_evidence(&core, session)
                .await
                .expect("resolve"),
            Some(232)
        );
    }
}
