//! A bounded connection pool.
//!
//! [`Pool`] reuses idle connections and opens new ones through a factory up to
//! a maximum, so a caller can spread load across several connections to a
//! datacenter and replace dead ones on demand.

use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use rtd_crypto::Key;

use crate::error::{Error, Result};

/// Options describing how to establish (or resume) a session for a datacenter.
///
/// A caller selects the authorization key with [`SessionOptions::select_key`]:
/// when PFS is requested and a temporary (bound) key is available it is
/// preferred, otherwise the permanent key is used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionOptions {
    /// The target datacenter.
    pub dc: i32,
    /// The permanent authorization key, if known.
    pub auth_key: Option<[u8; Key::LEN]>,
    /// The temporary (PFS) authorization key, if bound.
    pub temp_auth_key: Option<[u8; Key::LEN]>,
    /// The initial server salt.
    pub salt: i64,
    /// The session id to reuse, if any.
    pub session_id: Option<i64>,
    /// Whether to prefer the temporary key when present.
    pub pfs: bool,
}

impl SessionOptions {
    /// Creates options for `dc` with no stored keys.
    #[must_use]
    pub fn new(dc: i32) -> Self {
        Self {
            dc,
            auth_key: None,
            temp_auth_key: None,
            salt: 0,
            session_id: None,
            pfs: false,
        }
    }

    /// Sets the permanent authorization key.
    #[must_use]
    pub fn with_auth_key(mut self, key: [u8; Key::LEN]) -> Self {
        self.auth_key = Some(key);
        self
    }

    /// Sets the temporary (PFS) authorization key.
    #[must_use]
    pub fn with_temp_auth_key(mut self, key: [u8; Key::LEN]) -> Self {
        self.temp_auth_key = Some(key);
        self
    }

    /// Enables or disables PFS key preference.
    #[must_use]
    pub fn with_pfs(mut self, pfs: bool) -> Self {
        self.pfs = pfs;
        self
    }

    /// Selects the authorization key to use for this session.
    #[must_use]
    pub fn select_key(&self) -> Option<SelectedKey<'_>> {
        if self.pfs
            && let Some(key) = &self.temp_auth_key
        {
            return Some(SelectedKey::Temporary(key));
        }
        if let Some(key) = &self.auth_key {
            return Some(SelectedKey::Permanent(key));
        }
        self.temp_auth_key.as_ref().map(SelectedKey::Temporary)
    }
}

/// The authorization key [`SessionOptions::select_key`] chose.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectedKey<'a> {
    /// The permanent authorization key.
    Permanent(&'a [u8; Key::LEN]),
    /// The temporary (PFS) authorization key.
    Temporary(&'a [u8; Key::LEN]),
}

impl SelectedKey<'_> {
    /// Returns the raw key bytes.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8; Key::LEN] {
        match self {
            Self::Permanent(key) | Self::Temporary(key) => key,
        }
    }

    /// Returns whether this is the temporary (PFS) key.
    #[must_use]
    pub fn is_temporary(&self) -> bool {
        matches!(self, Self::Temporary(_))
    }
}

/// A bounded pool of connections produced by a factory.
pub struct Pool<C> {
    max: usize,
    factory: Box<dyn Fn() -> Result<C> + Send + Sync>,
    idle: Mutex<Vec<C>>,
    in_use: AtomicUsize,
}

impl<C> Pool<C> {
    /// Creates a pool holding at most `max` connections.
    #[must_use]
    pub fn new(max: usize, factory: impl Fn() -> Result<C> + Send + Sync + 'static) -> Self {
        Self {
            max: max.max(1),
            factory: Box::new(factory),
            idle: Mutex::new(Vec::new()),
            in_use: AtomicUsize::new(0),
        }
    }

    /// Acquires a connection, reusing an idle one or opening a new one.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Unexpected`] when the pool is at capacity and an
    /// idle connection is unavailable.
    pub fn acquire(&self) -> Result<C> {
        if let Some(connection) = self
            .idle
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .pop()
        {
            self.in_use.fetch_add(1, Ordering::SeqCst);
            return Ok(connection);
        }
        if self.in_use.load(Ordering::SeqCst) >= self.max {
            return Err(Error::Unexpected("connection pool is exhausted".to_owned()));
        }
        self.in_use.fetch_add(1, Ordering::SeqCst);
        match (self.factory)() {
            Ok(connection) => Ok(connection),
            Err(error) => {
                self.in_use.fetch_sub(1, Ordering::SeqCst);
                Err(error)
            }
        }
    }

    /// Returns a connection to the pool for reuse.
    pub fn release(&self, connection: C) {
        self.in_use.fetch_sub(1, Ordering::SeqCst);
        self.idle
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(connection);
    }

    /// Returns the number of idle connections.
    #[must_use]
    pub fn idle(&self) -> usize {
        self.idle
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .len()
    }

    /// Returns the number of checked-out connections.
    #[must_use]
    pub fn in_use(&self) -> usize {
        self.in_use.load(Ordering::SeqCst)
    }
}

#[cfg(test)]
mod tests {
    use super::Pool;
    use crate::transport::DuplexStream;

    #[test]
    fn reuses_idle_connections_up_to_capacity() {
        let pool = Pool::new(1, || Ok(DuplexStream::pair().0));
        let first = pool.acquire().unwrap();
        assert_eq!(pool.in_use(), 1);
        // At capacity with nothing idle: acquiring fails.
        assert!(pool.acquire().is_err());
        pool.release(first);
        assert_eq!(pool.idle(), 1);
        assert_eq!(pool.in_use(), 0);
        // The released connection is reused.
        let _second = pool.acquire().unwrap();
        assert_eq!(pool.idle(), 0);
    }

    #[test]
    fn factory_error_does_not_leak_capacity() {
        let pool: Pool<DuplexStream> = Pool::new(1, || {
            Err(crate::error::Error::Unexpected("no connection".to_owned()))
        });
        assert!(pool.acquire().is_err());
        assert_eq!(pool.in_use(), 0);
    }

    #[test]
    fn session_options_select_pfs_key() {
        use super::{SelectedKey, SessionOptions};

        let permanent = [1u8; rtd_crypto::Key::LEN];
        let temporary = [2u8; rtd_crypto::Key::LEN];

        let options = SessionOptions::new(2)
            .with_auth_key(permanent)
            .with_temp_auth_key(temporary);
        assert!(options.select_key().is_some());
        assert!(!options.select_key().unwrap().is_temporary());
        assert_eq!(options.select_key().unwrap().as_bytes(), &permanent);

        let pfs = options.clone().with_pfs(true);
        assert!(pfs.select_key().unwrap().is_temporary());
        assert!(matches!(pfs.select_key(), Some(SelectedKey::Temporary(_))));

        // Without a permanent key the temporary key is used regardless of PFS.
        let only_temp = SessionOptions::new(2).with_temp_auth_key(temporary);
        assert!(only_temp.select_key().unwrap().is_temporary());
        // With no key at all, nothing is selected.
        assert!(SessionOptions::new(2).select_key().is_none());
    }
}
