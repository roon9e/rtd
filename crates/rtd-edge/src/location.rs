//! Client geolocation and edge location tracking.
//!
//! [`LocationProvider`] resolves a client IP to a coarse location, and
//! [`LocationRegistry`] tracks live edge instances with a TTL and heartbeat.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

/// A coarse client location.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Location {
    /// The ISO country code, when known.
    pub country: Option<String>,
}

impl Location {
    /// An unknown location.
    #[must_use]
    pub fn unknown() -> Self {
        Self { country: None }
    }
}

/// Resolves a client IP to a location.
pub trait LocationProvider: Send + Sync {
    /// Returns the location for `ip`, or `None` when unknown.
    fn lookup(&self, ip: IpAddr) -> Option<Location>;
}

/// A provider that returns a fixed country for any address (for tests/local use).
#[derive(Debug, Clone)]
pub struct StaticProvider {
    country: Option<String>,
}

impl StaticProvider {
    /// Creates a provider returning `country` for any address.
    #[must_use]
    pub fn new(country: impl Into<String>) -> Self {
        Self {
            country: Some(country.into()),
        }
    }

    /// Creates a provider that never resolves a location.
    #[must_use]
    pub fn unknown() -> Self {
        Self { country: None }
    }
}

impl LocationProvider for StaticProvider {
    fn lookup(&self, _ip: IpAddr) -> Option<Location> {
        self.country.clone().map(|country| Location {
            country: Some(country),
        })
    }
}

/// A registered edge instance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EdgeLocation {
    /// The instance identity.
    pub instance_id: String,
    /// The advertised address.
    pub advertise: String,
    /// The edge's own location.
    pub location: Location,
}

/// An entry with an expiry.
#[derive(Debug, Clone)]
struct Entry {
    value: EdgeLocation,
    expires_at: SystemTime,
}

/// A registry of live edge instances with TTL-based expiry.
#[derive(Debug, Default)]
pub struct LocationRegistry {
    entries: Mutex<HashMap<String, Entry>>,
}

impl LocationRegistry {
    /// Creates an empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers or refreshes an instance with `ttl`.
    pub fn register(&self, value: EdgeLocation, ttl: Duration, now: SystemTime) {
        let expires_at = now.checked_add(ttl).unwrap_or(now);
        self.entries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(value.instance_id.clone(), Entry { value, expires_at });
    }

    /// Refreshes an instance's expiry, returning whether it was registered.
    pub fn heartbeat(&self, instance_id: &str, ttl: Duration, now: SystemTime) -> bool {
        let expires_at = now.checked_add(ttl).unwrap_or(now);
        match self
            .entries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get_mut(instance_id)
        {
            Some(entry) => {
                entry.expires_at = expires_at;
                true
            }
            None => false,
        }
    }

    /// Returns a live instance, if present and unexpired.
    #[must_use]
    pub fn get(&self, instance_id: &str, now: SystemTime) -> Option<EdgeLocation> {
        let entries = self
            .entries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let entry = entries.get(instance_id)?;
        (entry.expires_at > now).then(|| entry.value.clone())
    }

    /// Returns every live instance at `now`.
    #[must_use]
    pub fn live(&self, now: SystemTime) -> Vec<EdgeLocation> {
        self.entries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .values()
            .filter(|entry| entry.expires_at > now)
            .map(|entry| entry.value.clone())
            .collect()
    }

    /// Evicts expired entries, returning how many were removed.
    pub fn evict_expired(&self, now: SystemTime) -> usize {
        let mut entries = self
            .entries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let before = entries.len();
        entries.retain(|_, entry| entry.expires_at > now);
        before - entries.len()
    }
}

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr};
    use std::time::{Duration, SystemTime};

    use super::{EdgeLocation, Location, LocationProvider, LocationRegistry, StaticProvider};

    #[test]
    fn static_provider_resolves() {
        let provider = StaticProvider::new("US");
        let location = provider
            .lookup(IpAddr::V4(Ipv4Addr::new(1, 2, 3, 4)))
            .unwrap();
        assert_eq!(location.country.as_deref(), Some("US"));
        assert!(
            StaticProvider::unknown()
                .lookup(IpAddr::V4(Ipv4Addr::LOCALHOST))
                .is_none()
        );
    }

    #[test]
    fn registry_expires_by_ttl() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1000);
        let registry = LocationRegistry::new();
        registry.register(
            EdgeLocation {
                instance_id: "edge-1".to_owned(),
                advertise: "1.2.3.4:443".to_owned(),
                location: Location::unknown(),
            },
            Duration::from_secs(30),
            now,
        );
        assert!(
            registry
                .get("edge-1", now + Duration::from_secs(10))
                .is_some()
        );
        assert!(
            registry
                .get("edge-1", now + Duration::from_secs(31))
                .is_none()
        );
        assert_eq!(registry.evict_expired(now + Duration::from_secs(31)), 1);
    }
}
