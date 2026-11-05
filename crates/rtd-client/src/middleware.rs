//! Invoker middleware for the client.
//!
//! A [`Middleware`] wraps the invocation of any [`Method`], so behaviour such as
//! `FLOOD_WAIT` retries, rate limiting or tracing can be composed without
//! changing call sites. The chain is applied outermost-first.

use std::sync::Arc;
use std::time::Duration;

use rtd_tg::Method;

use crate::client::InvokeResult;
use crate::error::{Error, Result};

/// A decorator around one method invocation.
pub trait Middleware: Send {
    /// Invokes `method`, delegating to `next` for the actual send.
    ///
    /// # Errors
    ///
    /// Returns whatever `next` (or the middleware) reports.
    fn invoke(
        &self,
        method: Method,
        next: &mut dyn FnMut(Method) -> Result<InvokeResult>,
    ) -> Result<InvokeResult>;
}

/// A blocking delay, injectable so `FLOOD_WAIT` retries are testable.
pub trait Delay: Send + Sync {
    /// Blocks for `duration`.
    fn sleep(&self, duration: Duration);
}

/// A [`Delay`] that sleeps the calling thread.
#[derive(Debug, Default, Clone, Copy)]
pub struct ThreadDelay;

impl Delay for ThreadDelay {
    fn sleep(&self, duration: Duration) {
        std::thread::sleep(duration);
    }
}

/// Retries a call that the server answered with `FLOOD_WAIT_<n>`.
pub struct FloodWait {
    max_retries: u32,
    delay: Arc<dyn Delay>,
}

impl FloodWait {
    /// Creates a middleware that waits out a flood and retries at most
    /// `max_retries` times.
    #[must_use]
    pub fn new(max_retries: u32, delay: Arc<dyn Delay>) -> Self {
        Self { max_retries, delay }
    }
}

impl Middleware for FloodWait {
    fn invoke(
        &self,
        method: Method,
        next: &mut dyn FnMut(Method) -> Result<InvokeResult>,
    ) -> Result<InvokeResult> {
        let mut attempt = 0;
        loop {
            match next(method.clone()) {
                Ok(object) => return Ok(object),
                Err(Error::Rpc(error)) => {
                    let Some(wait) = rtd_error::as_flood_wait(&error) else {
                        return Err(Error::Rpc(error));
                    };
                    if attempt >= self.max_retries {
                        return Err(Error::Rpc(error));
                    }
                    self.delay.sleep(wait);
                    attempt += 1;
                }
                Err(other) => return Err(other),
            }
        }
    }
}

/// Records method invocations and outcomes (a tracing hook point).
pub trait Tracer: Send + Sync {
    /// Records one invocation of `method` (its constructor name) and success.
    fn record(&self, method: &str, ok: bool);
}

/// A [`Middleware`] forwarding every invocation to a [`Tracer`].
pub struct Trace {
    tracer: Arc<dyn Tracer>,
}

impl Trace {
    /// Creates a tracing middleware over `tracer`.
    #[must_use]
    pub fn new(tracer: Arc<dyn Tracer>) -> Self {
        Self { tracer }
    }
}

impl Middleware for Trace {
    fn invoke(
        &self,
        method: Method,
        next: &mut dyn FnMut(Method) -> Result<InvokeResult>,
    ) -> Result<InvokeResult> {
        let name = crate::json::type_name(method.type_id()).unwrap_or("unknown");
        let result = next(method);
        self.tracer.record(name, result.is_ok());
        result
    }
}

/// A [`Tracer`] that records one OpenTelemetry span per invocation.
///
/// Available with the `otel` feature. It uses the process-global OpenTelemetry
/// tracer provider, so an application configures the provider; without one the
/// global tracer is a no-op.
#[cfg(feature = "otel")]
pub struct OtelTracer {
    name: String,
}

#[cfg(feature = "otel")]
impl OtelTracer {
    /// Creates a tracer that records spans under instrumentation `name`.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
}

#[cfg(feature = "otel")]
impl Tracer for OtelTracer {
    fn record(&self, method: &str, ok: bool) {
        use opentelemetry::trace::{Span as _, Tracer as _};
        use opentelemetry::{KeyValue, global};

        let tracer = global::tracer(self.name.clone());
        let mut span = tracer.start(method.to_owned());
        span.set_attribute(KeyValue::new("rpc.ok", ok));
        span.end();
    }
}

/// Enforces a minimum interval between invocations.
pub struct RateLimit {
    interval: Duration,
    delay: Arc<dyn Delay>,
    last: std::sync::Mutex<Option<std::time::Instant>>,
}

impl RateLimit {
    /// Creates a middleware spacing calls by at least `interval`.
    #[must_use]
    pub fn new(interval: Duration, delay: Arc<dyn Delay>) -> Self {
        Self {
            interval,
            delay,
            last: std::sync::Mutex::new(None),
        }
    }
}

impl Middleware for RateLimit {
    fn invoke(
        &self,
        method: Method,
        next: &mut dyn FnMut(Method) -> Result<InvokeResult>,
    ) -> Result<InvokeResult> {
        {
            let mut last = self
                .last
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if let Some(previous) = *last {
                let elapsed = previous.elapsed();
                if elapsed < self.interval {
                    self.delay.sleep(self.interval - elapsed);
                }
            }
            *last = Some(std::time::Instant::now());
        }
        next(method)
    }
}

/// Invokes `method` through `chain`, delegating to `next` at the innermost
/// level.
pub(crate) fn run_chain(
    chain: &[Box<dyn Middleware + Send>],
    method: Method,
    next: &mut dyn FnMut(Method) -> Result<InvokeResult>,
) -> Result<InvokeResult> {
    match chain.split_first() {
        None => next(method),
        Some((head, tail)) => {
            let mut call_tail = |inner: Method| run_chain(tail, inner, next);
            head.invoke(method, &mut call_tail)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use rtd_tg::{HelpGetConfig, Method};

    use super::{Delay, FloodWait, Middleware, run_chain};
    use crate::error::Error;

    #[derive(Default)]
    struct Recording {
        waits: Mutex<Vec<Duration>>,
    }

    impl Delay for Recording {
        fn sleep(&self, duration: Duration) {
            self.waits
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(duration);
        }
    }

    fn a_method() -> Method {
        Method::HelpGetConfig(Box::new(HelpGetConfig))
    }

    #[test]
    fn tracer_records_calls() {
        use super::{Trace, Tracer};
        use crate::error::Error;
        use std::sync::Mutex;

        #[derive(Default)]
        struct Recording {
            calls: Mutex<Vec<(String, bool)>>,
        }
        impl Tracer for Recording {
            fn record(&self, method: &str, ok: bool) {
                self.calls
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .push((method.to_owned(), ok));
            }
        }

        let recording = Arc::new(Recording::default());
        let chain: Vec<Box<dyn Middleware + Send>> = vec![Box::new(Trace::new(recording.clone()))];
        let _ = run_chain(&chain, a_method(), &mut |_m| {
            Err(Error::Unexpected("x".to_owned()))
        });
        let calls = recording
            .calls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        assert_eq!(calls.len(), 1);
        assert!(!calls[0].1);
    }

    #[test]
    fn flood_wait_is_retried_then_surfaced() {
        let recording = Arc::new(Recording::default());
        let chain: Vec<Box<dyn Middleware + Send>> =
            vec![Box::new(FloodWait::new(1, recording.clone()))];

        let mut calls = 0;
        let result = run_chain(&chain, a_method(), &mut |_method| {
            calls += 1;
            Err(Error::Rpc(rtd_error::Error::new(420, "FLOOD_WAIT_2")))
        });

        assert!(matches!(result, Err(Error::Rpc(_))));
        assert_eq!(calls, 2);
        let waits = recording
            .waits
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        assert_eq!(waits.as_slice(), &[Duration::from_secs(2)]);
    }

    #[cfg(feature = "otel")]
    #[test]
    fn otel_tracer_records_without_a_provider() {
        use super::{OtelTracer, Tracer};

        // With no global provider installed the tracer is a no-op; this asserts
        // the call path is safe.
        OtelTracer::new("rtd-test").record("help.getConfig", true);
        OtelTracer::new("rtd-test").record("help.getConfig", false);
    }
}
