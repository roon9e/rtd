//! Datacenter discovery and resolution.
//!
//! [`StaticResolver`] holds a fixed DC list (production or test) and resolves a
//! datacenter identifier to an ordered list of endpoints and the transport each
//! uses.

/// The transport used to reach an endpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protocol {
    /// Plain TCP with the codec's framing.
    Plain,
    /// Obfuscated2 (MTProxy transport obfuscation).
    Obfuscated2,
    /// FakeTLS with obfuscated2 inside.
    FakeTls,
    /// MTProto over WebSocket.
    WebSocket,
}

/// A dialable endpoint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Endpoint {
    /// The `host:port` address.
    pub address: String,
    /// The transport to use.
    pub protocol: Protocol,
}

impl Endpoint {
    /// Creates an endpoint.
    #[must_use]
    pub fn new(address: impl Into<String>, protocol: Protocol) -> Self {
        Self {
            address: address.into(),
            protocol,
        }
    }
}

/// A datacenter and its endpoints.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataCenter {
    /// The datacenter identifier.
    pub id: i32,
    /// The ordered endpoints to try.
    pub endpoints: Vec<Endpoint>,
}

impl DataCenter {
    /// Creates a datacenter with a single obfuscated2 endpoint.
    #[must_use]
    pub fn new(id: i32, address: impl Into<String>) -> Self {
        Self {
            id,
            endpoints: vec![Endpoint::new(address, Protocol::Obfuscated2)],
        }
    }
}

/// Resolves a datacenter identifier to its endpoints.
pub trait Resolver {
    /// Returns the datacenter for `id`, if known.
    fn resolve(&self, id: i32) -> Option<&DataCenter>;

    /// Returns every known datacenter.
    fn datacenters(&self) -> &[DataCenter];
}

/// A resolver backed by a fixed DC list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StaticResolver {
    datacenters: Vec<DataCenter>,
}

impl StaticResolver {
    /// Creates a resolver from an explicit DC list.
    #[must_use]
    pub fn new(datacenters: Vec<DataCenter>) -> Self {
        Self { datacenters }
    }

    /// Returns the Telegram production DC list.
    #[must_use]
    pub fn production() -> Self {
        Self::new(vec![
            DataCenter::new(1, "149.154.175.53:443"),
            DataCenter::new(2, "149.154.167.51:443"),
            DataCenter::new(3, "149.154.175.100:443"),
            DataCenter::new(4, "149.154.167.91:443"),
            DataCenter::new(5, "91.108.56.130:443"),
        ])
    }

    /// Returns the Telegram test DC list.
    #[must_use]
    pub fn test() -> Self {
        Self::new(vec![
            DataCenter::new(1, "149.154.175.10:443"),
            DataCenter::new(2, "149.154.167.40:443"),
            DataCenter::new(3, "149.154.175.117:443"),
        ])
    }
}

impl Resolver for StaticResolver {
    fn resolve(&self, id: i32) -> Option<&DataCenter> {
        self.datacenters.iter().find(|dc| dc.id == id)
    }

    fn datacenters(&self) -> &[DataCenter] {
        &self.datacenters
    }
}

#[cfg(test)]
mod tests {
    use super::{Protocol, Resolver, StaticResolver};

    #[test]
    fn production_list_resolves() {
        let resolver = StaticResolver::production();
        let dc = resolver.resolve(2).unwrap();
        assert_eq!(dc.id, 2);
        assert_eq!(dc.endpoints.len(), 1);
        assert_eq!(dc.endpoints[0].protocol, Protocol::Obfuscated2);
        assert!(resolver.resolve(99).is_none());
        assert_eq!(resolver.datacenters().len(), 5);
    }
}
