//! An NTP-calibrated [`Clock`] (available with the `ntp` feature).
//!
//! [`NtpClock::sync`] performs a single SNTP exchange and records the measured
//! offset, which [`NtpClock::now`] applies to the system clock.

use std::io;
use std::net::UdpSocket;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::clock::{Clock, SystemClock};

/// Seconds between the NTP epoch (1900) and the Unix epoch (1970).
const NTP_UNIX_EPOCH_DELTA: i128 = 2_208_988_800;
/// An SNTP request header: leap 0, version 4, mode 3 (client).
const SNTP_REQUEST: [u8; 48] = {
    let mut request = [0u8; 48];
    request[0] = 0x23;
    request
};

/// A measured clock offset and round-trip delay, in nanoseconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NtpOffset {
    /// The server clock minus the local clock, in nanoseconds.
    pub offset_nanos: i64,
    /// The round-trip delay, in nanoseconds.
    pub delay_nanos: i64,
}

/// Measures the offset from an SNTP response.
///
/// `t1` and `t4` are the local send and receive times; `response` is the raw
/// 48-byte SNTP reply. Returns `None` when the timestamps are unusable.
#[must_use]
pub fn measure_offset(t1: SystemTime, t4: SystemTime, response: &[u8]) -> Option<NtpOffset> {
    if response.len() < 48 {
        return None;
    }
    let t1 = to_nanos(t1);
    let t4 = to_nanos(t4);
    let t2 = ntp_timestamp(&response[32..40])?;
    let t3 = ntp_timestamp(&response[40..48])?;
    let offset = (t2 - t1 + (t3 - t4)) / 2;
    let delay = (t4 - t1) - (t3 - t2);
    Some(NtpOffset {
        offset_nanos: i64::try_from(offset).ok()?,
        delay_nanos: i64::try_from(delay.max(0)).unwrap_or(i64::MAX),
    })
}

/// A [`Clock`] calibrated against an NTP server.
#[derive(Debug, Clone, Copy)]
pub struct NtpClock {
    offset_nanos: i64,
    base: SystemClock,
}

impl NtpClock {
    /// Creates a clock that applies `offset_nanos` to the system clock.
    #[must_use]
    pub fn from_offset(offset_nanos: i64) -> Self {
        Self {
            offset_nanos,
            base: SystemClock,
        }
    }

    /// Performs one SNTP exchange with `server` and returns the calibrated clock.
    ///
    /// # Errors
    ///
    /// Returns an I/O error when the socket cannot be created or the exchange
    /// times out, or an invalid-data error when the reply is unusable.
    pub fn sync(server: &str, timeout: Duration) -> io::Result<Self> {
        let socket = UdpSocket::bind("0.0.0.0:0")?;
        socket.set_read_timeout(Some(timeout))?;
        socket.connect(server)?;
        let t1 = SystemTime::now();
        socket.send(&SNTP_REQUEST)?;
        let mut response = [0u8; 48];
        let read = socket.recv(&mut response)?;
        let t4 = SystemTime::now();
        let measured = measure_offset(t1, t4, &response[..read])
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "invalid SNTP reply"))?;
        Ok(Self::from_offset(measured.offset_nanos))
    }

    /// Returns the measured offset, in nanoseconds.
    #[must_use]
    pub fn offset_nanos(&self) -> i64 {
        self.offset_nanos
    }
}

impl Clock for NtpClock {
    fn now(&self) -> SystemTime {
        let adjusted = to_nanos(self.base.now()) + i128::from(self.offset_nanos);
        from_nanos(adjusted)
    }
}

/// Converts a [`SystemTime`] to nanoseconds since the Unix epoch.
fn to_nanos(time: SystemTime) -> i128 {
    match time.duration_since(UNIX_EPOCH) {
        Ok(since) => since.as_nanos() as i128,
        Err(before) => -(before.duration().as_nanos() as i128),
    }
}

/// Converts nanoseconds since the Unix epoch back to a [`SystemTime`].
fn from_nanos(nanos: i128) -> SystemTime {
    if nanos >= 0 {
        UNIX_EPOCH + Duration::from_nanos(u64::try_from(nanos).unwrap_or(u64::MAX))
    } else {
        UNIX_EPOCH - Duration::from_nanos(u64::try_from(-nanos).unwrap_or(u64::MAX))
    }
}

/// Converts an 8-byte NTP timestamp (seconds since 1900 + fraction) to
/// nanoseconds since the Unix epoch.
fn ntp_timestamp(bytes: &[u8]) -> Option<i128> {
    let seconds = u32::from_be_bytes(bytes.get(0..4)?.try_into().ok()?);
    let fraction = u32::from_be_bytes(bytes.get(4..8)?.try_into().ok()?);
    if seconds == 0 {
        return None;
    }
    let seconds = i128::from(seconds) - NTP_UNIX_EPOCH_DELTA;
    let frac_nanos = i128::from(fraction) * 1_000_000_000 / (1_i128 << 32);
    Some(seconds * 1_000_000_000 + frac_nanos)
}

#[cfg(test)]
mod tests {
    use super::{NTP_UNIX_EPOCH_DELTA, NtpClock, measure_offset};
    use crate::clock::Clock;
    use std::time::{Duration, UNIX_EPOCH};

    /// Builds an SNTP reply with the given server receive/transmit timestamps,
    /// expressed as (seconds since the Unix epoch, fractional nanos).
    fn reply(receive: (i64, i64), transmit: (i64, i64)) -> [u8; 48] {
        let mut bytes = [0u8; 48];
        for (offset, (secs, nanos)) in [(32, receive), (40, transmit)] {
            let ntp_secs = (i128::from(secs) + NTP_UNIX_EPOCH_DELTA) as u32;
            let frac = (i128::from(nanos) * (1_i128 << 32) / 1_000_000_000) as u32;
            bytes[offset..offset + 4].copy_from_slice(&ntp_secs.to_be_bytes());
            bytes[offset + 4..offset + 8].copy_from_slice(&frac.to_be_bytes());
        }
        bytes
    }

    #[test]
    fn measures_zero_offset_for_an_echo() {
        let t1 = UNIX_EPOCH + Duration::from_secs(1_000_000);
        let t4 = t1 + Duration::from_millis(250);
        // Server receive == t1, transmit == t4 (a perfect, symmetric link).
        let response = reply((1_000_000, 0), (1_000_000, 250_000_000));
        let measured = measure_offset(t1, t4, &response).expect("offset");
        assert_eq!(measured.offset_nanos, 0);
        assert_eq!(measured.delay_nanos, 0);
    }

    #[test]
    fn measures_a_positive_offset() {
        let t1 = UNIX_EPOCH + Duration::from_secs(2_000_000);
        let t4 = t1 + Duration::from_millis(250);
        // The server clock is 5 seconds ahead of ours.
        let response = reply((2_000_005, 0), (2_000_005, 250_000_000));
        let measured = measure_offset(t1, t4, &response).expect("offset");
        assert_eq!(measured.offset_nanos, 5_000_000_000);
    }

    #[test]
    fn clock_applies_the_offset() {
        let clock = NtpClock::from_offset(Duration::from_secs(3).as_nanos() as i64);
        assert_eq!(clock.offset_nanos(), 3_000_000_000);
        let now = clock.now();
        assert!(now > UNIX_EPOCH);
    }
}
