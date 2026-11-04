//! Datacenter routing with `*_MIGRATE_*` handling.
//!
//! [`DcRouter`] keeps one [`Client`] per datacenter and, whenever a call fails
//! with a `PHONE_MIGRATE_*`/`FILE_MIGRATE_*`/… error, reconnects to the target
//! datacenter and retries the call there once.

use std::collections::BTreeMap;

use rtd_tg::{Method, Object};

use crate::client::Client;
use crate::dcs::{Resolver, StaticResolver, migrate_target};
use crate::error::{Error, Result};
use crate::keepalive::{Keepalive, Reconnect};
use crate::session::SessionData;
use crate::transport::Transport;

/// Routes calls across datacenters, following `*_MIGRATE_*` errors.
pub struct DcRouter<T: Transport> {
    clients: BTreeMap<i32, Client<T>>,
    current: i32,
    resolver: StaticResolver,
    connect: Box<dyn FnMut(i32) -> Result<Client<T>>>,
}

impl<T: Transport> DcRouter<T> {
    /// Creates a router whose active datacenter is `current`.
    ///
    /// `connect` establishes a client for a datacenter on first use; it is
    /// called immediately for `current`.
    ///
    /// # Errors
    ///
    /// Returns the connection error for `current`.
    pub fn new(
        current: i32,
        resolver: StaticResolver,
        mut connect: impl FnMut(i32) -> Result<Client<T>> + 'static,
    ) -> Result<Self> {
        let client = connect(current)?;
        let mut clients = BTreeMap::new();
        clients.insert(current, client);
        Ok(Self {
            clients,
            current,
            resolver,
            connect: Box::new(connect),
        })
    }

    /// Returns the active datacenter.
    #[must_use]
    pub fn current(&self) -> i32 {
        self.current
    }

    /// Returns the client for `dc`, connecting on first use.
    ///
    /// # Errors
    ///
    /// Returns the connection error when `dc` has not been connected yet.
    pub fn client(&mut self, dc: i32) -> Result<&mut Client<T>> {
        if !self.clients.contains_key(&dc) {
            let client = (self.connect)(dc)?;
            self.clients.insert(dc, client);
        }
        self.clients.get_mut(&dc).ok_or(Error::Closed)
    }

    /// Invokes `method`, following a migrate error to the target datacenter and
    /// retrying once there.
    ///
    /// # Errors
    ///
    /// Returns the transport, crypto, decode or RPC error; a non-migrate RPC
    /// error is returned unchanged.
    pub fn invoke(&mut self, method: Method) -> Result<Object> {
        loop {
            let current = self.current;
            let result = self.client(current)?.invoke(method.clone());
            match result {
                Ok(object) => return Ok(object),
                Err(Error::Rpc(error)) => {
                    let Some(dc) = migrate_target(&error) else {
                        return Err(Error::Rpc(error));
                    };
                    if self.resolver.resolve(dc).is_none() {
                        return Err(Error::Rpc(error));
                    }
                    self.current = dc;
                }
                Err(other) => return Err(other),
            }
        }
    }
}

/// A client that reconnects (with capped backoff) when its keepalive lapses.
pub struct ReconnectingClient<T: Transport> {
    client: Option<Client<T>>,
    connect: Box<dyn FnMut() -> Result<Client<T>>>,
    reconnect: Reconnect,
    keepalive: Keepalive,
}

impl<T: Transport> ReconnectingClient<T> {
    /// Creates a reconnectable client resuming `session`, reconnecting through
    /// `connect`.
    pub fn new(
        client: Client<T>,
        session: SessionData,
        connect: impl FnMut() -> Result<Client<T>> + 'static,
        keepalive: Keepalive,
        initial_backoff: std::time::Duration,
        max_backoff: std::time::Duration,
    ) -> Self {
        Self {
            client: Some(client),
            connect: Box::new(connect),
            reconnect: Reconnect::new(session, initial_backoff, max_backoff),
            keepalive,
        }
    }

    /// Returns the current client, if connected.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Closed`] while disconnected.
    pub fn client(&mut self) -> Result<&mut Client<T>> {
        self.client.as_mut().ok_or(Error::Closed)
    }

    /// Sends a `ping` if one is due, returning its id.
    pub fn ping_due(&self, now_ms: u64, ping_id: i64) -> Option<i64> {
        self.keepalive.due(now_ms, ping_id)
    }

    /// Records that a ping was sent.
    pub fn ping_sent(&mut self, ping_id: i64, now_ms: u64) {
        self.keepalive.sent(ping_id, now_ms);
    }

    /// Records a `pong`. Returns whether it matched.
    pub fn on_pong(&mut self, ping_id: i64, now_ms: u64) -> bool {
        self.keepalive.on_pong(ping_id, now_ms)
    }

    /// Reconnects if the pending ping has lapsed, returning whether it did.
    ///
    /// # Errors
    ///
    /// Returns the connection error when reconnecting fails.
    pub fn maintain(&mut self, now_ms: u64) -> Result<bool> {
        if !self.keepalive.check(now_ms) {
            return Ok(false);
        }
        self.client = None;
        let fresh = (self.connect)()?;
        self.client = Some(fresh);
        self.keepalive.reconnect(now_ms);
        self.reconnect.reset();
        Ok(true)
    }

    /// Returns the next reconnect backoff delay.
    pub fn next_backoff(&mut self) -> std::time::Duration {
        self.reconnect.next_delay()
    }

    /// Invokes `method` on the current client.
    ///
    /// # Errors
    ///
    /// Returns a transport, crypto, decode or RPC error, or [`Error::Closed`]
    /// while disconnected.
    pub fn invoke(&mut self, method: Method) -> Result<Object> {
        self.client()?.invoke(method)
    }
}
