//! A lightweight metrics registry.
//!
//! [`Metrics`] records named counters and can render them in the Prometheus
//! text exposition format. It is deliberately small — no external SDK — so the
//! runtime stays dependency-light; a richer backend can replace it behind the
//! same call sites.

use std::collections::BTreeMap;
use std::sync::Mutex;

/// Counter name for accepted connections.
pub const CONNECTIONS_ACCEPTED: &str = "rtd_edge_connections_accepted_total";
/// Counter name for connections shed by admission budgets.
pub const CONNECTIONS_SHED: &str = "rtd_edge_connections_shed_total";
/// Counter name for started handshakes.
pub const HANDSHAKES_STARTED: &str = "rtd_edge_handshakes_started_total";

/// A registry of named counters.
#[derive(Debug, Default)]
pub struct Metrics {
    counters: Mutex<BTreeMap<&'static str, u64>>,
}

impl Metrics {
    /// Creates an empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds one to the counter `name`.
    pub fn increment(&self, name: &'static str) {
        self.add(name, 1);
    }

    /// Adds `value` to the counter `name`.
    pub fn add(&self, name: &'static str, value: u64) {
        let mut counters = self
            .counters
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let entry = counters.entry(name).or_insert(0);
        *entry = entry.saturating_add(value);
    }

    /// Returns the current value of the counter `name`.
    #[must_use]
    pub fn get(&self, name: &str) -> u64 {
        self.counters
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(name)
            .copied()
            .unwrap_or(0)
    }

    /// Renders all counters in the Prometheus text exposition format.
    #[must_use]
    pub fn render(&self) -> String {
        let counters = self
            .counters
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut out = String::new();
        for (name, value) in counters.iter() {
            out.push_str("# TYPE ");
            out.push_str(name);
            out.push_str(" counter\n");
            out.push_str(name);
            out.push(' ');
            out.push_str(&value.to_string());
            out.push('\n');
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::{CONNECTIONS_ACCEPTED, Metrics};

    #[test]
    fn counters_increment_and_render() {
        let metrics = Metrics::new();
        metrics.increment(CONNECTIONS_ACCEPTED);
        metrics.add(CONNECTIONS_ACCEPTED, 2);
        assert_eq!(metrics.get(CONNECTIONS_ACCEPTED), 3);

        let rendered = metrics.render();
        assert!(rendered.contains("# TYPE rtd_edge_connections_accepted_total counter"));
        assert!(rendered.contains("rtd_edge_connections_accepted_total 3"));
    }

    #[test]
    fn unknown_counter_is_zero() {
        assert_eq!(Metrics::new().get("missing"), 0);
    }
}
