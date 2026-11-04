//! An asynchronous façade over the synchronous client core.
//!
//! The authorization-key exchange and the steady-state loop are synchronous by
//! design, so the asynchronous client serves them from a dedicated actor thread
//! (`std::thread`) and exposes an `async` command API. A tokio runtime worker
//! submitting a call never blocks; it awaits a `oneshot` reply.

use std::sync::Arc;

use rtd_exchange::ClientConfig;
use rtd_proto::CodecKind;
use rtd_tg::{Method, Object, Updates};
use rtd_util::Clock;
use tokio::sync::{broadcast, mpsc, oneshot};

use crate::client::{Client, InitParams};
use crate::error::{Error, Result};
use crate::transport::{Transport, connect_tcp};

/// A command for the actor thread.
enum Command {
    /// Establish the session.
    Initialize(InitParams, oneshot::Sender<Result<Object>>),
    /// Invoke a canonical-layer method.
    Invoke(Method, oneshot::Sender<Result<Object>>),
}

/// An asynchronous MTProto 2.0 client.
///
/// A dedicated actor thread owns the synchronous [`Client`] and serves commands,
/// so [`AsyncClient::initialize`] and [`AsyncClient::invoke`] never block the
/// tokio runtime.
pub struct AsyncClient {
    tx: Option<mpsc::Sender<Command>>,
    handle: Option<std::thread::JoinHandle<()>>,
    updates: broadcast::Sender<Updates>,
}

impl AsyncClient {
    /// Spawns the actor over an established transport, completing the
    /// authorization-key exchange before returning.
    ///
    /// # Errors
    ///
    /// Returns a transport or exchange error when the exchange cannot complete.
    pub fn spawn<T: Transport + Send + 'static>(
        transport: T,
        config: ClientConfig,
        clock: Arc<dyn Clock + Send + Sync>,
    ) -> Result<Self> {
        let (tx, mut rx) = mpsc::channel::<Command>(1);
        let (updates_tx, _) = broadcast::channel::<Updates>(64);
        let (ready_tx, ready_rx) = std::sync::mpsc::channel::<Result<()>>();
        let handler_tx = updates_tx.clone();
        let handle = std::thread::spawn(move || {
            let mut client = match Client::connect(transport, &config, clock) {
                Ok(client) => {
                    let _ = ready_tx.send(Ok(()));
                    client
                }
                Err(error) => {
                    let _ = ready_tx.send(Err(error));
                    return;
                }
            };
            client.set_update_handler(move |updates| {
                let _ = handler_tx.send(updates);
            });
            while let Some(command) = rx.blocking_recv() {
                match command {
                    Command::Initialize(params, reply) => {
                        let _ = reply.send(client.initialize(params));
                    }
                    Command::Invoke(method, reply) => {
                        let _ = reply.send(client.invoke(method));
                    }
                }
            }
        });
        match ready_rx.recv() {
            Ok(Ok(())) => Ok(Self {
                tx: Some(tx),
                handle: Some(handle),
                updates: updates_tx,
            }),
            Ok(Err(error)) => {
                drop(tx);
                let _ = handle.join();
                Err(error)
            }
            Err(_) => Err(Error::Closed),
        }
    }

    /// Dials `address` and spawns the actor over a TCP transport.
    ///
    /// # Errors
    ///
    /// As [`AsyncClient::spawn`], plus dialing and obfuscation errors.
    pub fn connect_tcp(
        address: &str,
        dc: u16,
        codec: CodecKind,
        obfuscation: Option<&[u8]>,
        config: ClientConfig,
        clock: Arc<dyn Clock + Send + Sync>,
    ) -> Result<Self> {
        let transport = connect_tcp(address, dc, codec, obfuscation)?;
        Self::spawn(transport, config, clock)
    }

    /// Establishes the session; see [`Client::initialize`].
    ///
    /// # Errors
    ///
    /// Returns a transport, crypto, decode or RPC error.
    pub async fn initialize(&self, params: InitParams) -> Result<Object> {
        let (reply, rx) = oneshot::channel();
        self.sender()?
            .send(Command::Initialize(params, reply))
            .await
            .map_err(|_| Error::Closed)?;
        rx.await.map_err(|_| Error::Closed)?
    }

    /// Invokes one canonical-layer method; see [`Client::invoke`].
    ///
    /// # Errors
    ///
    /// Returns a transport, crypto, decode or RPC error.
    pub async fn invoke(&self, method: Method) -> Result<Object> {
        let (reply, rx) = oneshot::channel();
        self.sender()?
            .send(Command::Invoke(method, reply))
            .await
            .map_err(|_| Error::Closed)?;
        rx.await.map_err(|_| Error::Closed)?
    }

    /// Subscribes to server updates received while calls await their replies.
    ///
    /// Each returned receiver observes a copy of every `Updates` envelope the
    /// actor thread dispatches.
    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<Updates> {
        self.updates.subscribe()
    }

    fn sender(&self) -> Result<&mpsc::Sender<Command>> {
        self.tx.as_ref().ok_or(Error::Closed)
    }
}

impl Drop for AsyncClient {
    fn drop(&mut self) {
        // Dropping the sender ends the actor's `blocking_recv`, so the join
        // cannot deadlock even if a call was in flight.
        self.tx.take();
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}
