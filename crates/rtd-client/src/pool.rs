//! A bounded connection pool.
//!
//! [`Pool`] reuses idle connections and opens new ones through a factory up to
//! a maximum, so a caller can spread load across several connections to a
//! datacenter and replace dead ones on demand.

use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::error::{Error, Result};

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
}
