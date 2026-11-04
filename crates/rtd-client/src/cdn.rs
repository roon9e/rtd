//! Telegram CDN redirect support.
//!
//! When `upload.getFile` is answered with `upload.fileCdnRedirect`, the file is
//! fetched from a CDN datacenter. CDN chunks are AES-256-CTR encrypted and come
//! with per-window SHA-256 hashes. This module implements the decryption, the
//! hash-window lookup and a bounded cache of verified windows, leaving the
//! network wiring to the downloader.

use std::collections::VecDeque;

use sha2::{Digest, Sha256};

use crate::error::{Error, Result};

/// The CDN hash window size used by Telegram, in bytes.
pub const CDN_HASH_WINDOW: i64 = 128 * 1024;
/// Maximum number of verified windows kept in memory.
pub const MAX_VERIFIED_WINDOWS: usize = 16;

/// One CDN hash window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HashWindow {
    /// The window's file offset in bytes.
    pub offset: i64,
    /// The window's length in bytes.
    pub limit: i32,
    /// The SHA-256 digest of the (decrypted) window.
    pub hash: Vec<u8>,
}

impl HashWindow {
    /// Creates a window.
    #[must_use]
    pub fn new(offset: i64, limit: i32, hash: Vec<u8>) -> Self {
        Self {
            offset,
            limit,
            hash,
        }
    }

    /// Returns the exclusive end offset.
    #[must_use]
    pub fn end(&self) -> i64 {
        self.offset + i64::from(self.limit)
    }

    /// Returns whether `offset` falls within the window.
    #[must_use]
    pub fn contains(&self, offset: i64) -> bool {
        offset >= self.offset && offset < self.end()
    }
}

/// A sorted, deduplicated set of hash windows with predecessor lookup.
#[derive(Debug, Default, Clone)]
pub struct HashCache {
    windows: Vec<HashWindow>,
}

impl HashCache {
    /// Creates an empty cache.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Inserts a batch of windows, ignoring those with a non-positive limit.
    pub fn extend(&mut self, windows: impl IntoIterator<Item = HashWindow>) {
        for window in windows {
            self.insert(window);
        }
    }

    /// Inserts one window, replacing an existing entry at the same offset.
    pub fn insert(&mut self, window: HashWindow) {
        if window.limit <= 0 {
            return;
        }
        match self
            .windows
            .binary_search_by_key(&window.offset, |w| w.offset)
        {
            Ok(index) => self.windows[index] = window,
            Err(index) => self.windows.insert(index, window),
        }
    }

    /// Returns the window containing `offset`, if any.
    #[must_use]
    pub fn get(&self, offset: i64) -> Option<&HashWindow> {
        let index = self
            .windows
            .partition_point(|w| w.offset <= offset)
            .checked_sub(1)?;
        let window = &self.windows[index];
        window.contains(offset).then_some(window)
    }

    /// Returns the number of cached windows.
    #[must_use]
    pub fn len(&self) -> usize {
        self.windows.len()
    }

    /// Returns whether the cache is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.windows.is_empty()
    }

    /// Clears the cache.
    pub fn clear(&mut self) {
        self.windows.clear();
    }
}

/// Verifies a CDN window against its SHA-256 hash.
///
/// # Errors
///
/// Returns [`Error::Unexpected`] when the digest does not match.
pub fn verify_window(window: &HashWindow, data: &[u8]) -> Result<()> {
    if data.is_empty() || data.len() > usize::try_from(window.limit).unwrap_or(usize::MAX) {
        return Err(Error::Unexpected(format!(
            "invalid CDN window length at offset={} limit={} got={}",
            window.offset,
            window.limit,
            data.len()
        )));
    }
    let digest = Sha256::digest(data);
    if digest.as_slice() != window.hash.as_slice() {
        return Err(Error::Unexpected(format!(
            "CDN hash mismatch at offset={}",
            window.offset
        )));
    }
    Ok(())
}

/// Decrypts a CDN chunk in place.
///
/// The AES-256-CTR counter starts at `iv` with its last four bytes replaced by
/// `offset / 16`, matching the Telegram CDN scheme.
///
/// # Errors
///
/// Returns [`Error::Unexpected`] when the key is not 32 bytes or the IV is not
/// 16 bytes.
pub fn decrypt_in_place(key: &[u8], iv: &[u8], offset: i64, data: &mut [u8]) -> Result<()> {
    let key: [u8; 32] = key
        .try_into()
        .map_err(|_| Error::Unexpected(format!("invalid CDN key length {}", key.len())))?;
    let mut counter: [u8; 16] = iv
        .try_into()
        .map_err(|_| Error::Unexpected(format!("invalid CDN iv length {}", iv.len())))?;
    let block_index = u32::try_from(offset / 16).unwrap_or(0);
    counter[12..].copy_from_slice(&block_index.to_be_bytes());
    rtd_crypto::ctr_xor(&key, &counter, data);
    Ok(())
}

/// A parsed `upload.fileCdnRedirect`.
#[derive(Debug, Clone)]
pub struct CdnRedirect {
    /// The CDN datacenter to fetch from.
    pub dc_id: i32,
    /// The opaque file token, refreshed when it becomes invalid.
    pub file_token: Vec<u8>,
    /// The AES-256 encryption key.
    pub encryption_key: Vec<u8>,
    /// The 16-byte CTR initialization vector.
    pub encryption_iv: Vec<u8>,
    /// The hash windows for the redirected file.
    pub hashes: HashCache,
}

impl CdnRedirect {
    /// Creates a redirect from its wire fields and hash windows.
    #[must_use]
    pub fn new(
        dc_id: i32,
        file_token: Vec<u8>,
        encryption_key: Vec<u8>,
        encryption_iv: Vec<u8>,
        hashes: impl IntoIterator<Item = HashWindow>,
    ) -> Self {
        let mut cache = HashCache::new();
        cache.extend(hashes);
        Self {
            dc_id,
            file_token,
            encryption_key,
            encryption_iv,
            hashes: cache,
        }
    }

    /// Decrypts a CDN chunk in place, selecting the counter from `offset`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Unexpected`] when the key or IV are the wrong size.
    pub fn decrypt(&self, offset: i64, data: &mut [u8]) -> Result<()> {
        decrypt_in_place(&self.encryption_key, &self.encryption_iv, offset, data)
    }

    /// Verifies a chunk that exactly covers one hash window.
    ///
    /// Chunks that only partially cover a window (custom part sizes) are not
    /// checked here; the downloader loads and verifies the full window first.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Unexpected`] when the chunk covers a whole window whose
    /// digest does not match.
    pub fn verify_chunk(&self, offset: i64, data: &[u8]) -> Result<()> {
        if let Some(window) = self.hashes.get(offset) {
            let exact = window.offset == offset
                && usize::try_from(window.limit).unwrap_or(usize::MAX) == data.len();
            if exact {
                return verify_window(window, data);
            }
        }
        Ok(())
    }

    /// Refreshes the file token and hash windows in place.
    pub fn refresh(&mut self, file_token: Vec<u8>, hashes: impl IntoIterator<Item = HashWindow>) {
        self.file_token = file_token;
        self.hashes.clear();
        self.hashes.extend(hashes);
    }
}

/// The master/CDN mode state machine, including token refresh.
#[derive(Debug, Default)]
pub struct CdnState {
    redirect: Option<CdnRedirect>,
}

impl CdnState {
    /// Creates a state in master mode.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Enters CDN mode with `redirect`.
    pub fn on_redirect(&mut self, redirect: CdnRedirect) {
        self.redirect = Some(redirect);
    }

    /// Handles a `FILE_TOKEN_INVALID` response by returning to master mode so
    /// the downloader re-requests the redirect.
    pub fn on_token_invalid(&mut self) {
        self.redirect = None;
    }

    /// Returns whether the state is in CDN mode.
    #[must_use]
    pub fn is_cdn(&self) -> bool {
        self.redirect.is_some()
    }

    /// Returns the active redirect, if in CDN mode.
    #[must_use]
    pub fn redirect(&self) -> Option<&CdnRedirect> {
        self.redirect.as_ref()
    }

    /// Returns the active redirect mutably, if in CDN mode.
    pub fn redirect_mut(&mut self) -> Option<&mut CdnRedirect> {
        self.redirect.as_mut()
    }
}

/// A bounded FIFO cache of verified CDN windows.
#[derive(Debug)]
pub struct VerifiedWindows {
    windows: std::collections::HashMap<i64, Vec<u8>>,
    order: VecDeque<i64>,
    max: usize,
}

impl VerifiedWindows {
    /// Creates a cache holding at most [`MAX_VERIFIED_WINDOWS`] windows.
    #[must_use]
    pub fn new() -> Self {
        Self::with_capacity(MAX_VERIFIED_WINDOWS)
    }

    /// Creates a cache holding at most `max` windows.
    #[must_use]
    pub fn with_capacity(max: usize) -> Self {
        Self {
            windows: std::collections::HashMap::new(),
            order: VecDeque::new(),
            max: max.max(1),
        }
    }

    /// Returns a copy of the verified bytes for the window at `offset`.
    #[must_use]
    pub fn get(&self, offset: i64) -> Option<Vec<u8>> {
        self.windows.get(&offset).cloned()
    }

    /// Returns the number of cached windows.
    #[must_use]
    pub fn len(&self) -> usize {
        self.windows.len()
    }

    /// Returns whether the cache is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.windows.is_empty()
    }

    /// Verifies `data` against `window` and caches it on success.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Unexpected`] when verification fails.
    pub fn verify_and_cache(&mut self, window: HashWindow, data: &[u8]) -> Result<()> {
        verify_window(&window, data)?;
        if self.windows.insert(window.offset, data.to_vec()).is_none() {
            self.order.push_back(window.offset);
        }
        while self.order.len() > self.max {
            if let Some(evicted) = self.order.pop_front() {
                self.windows.remove(&evicted);
            }
        }
        Ok(())
    }

    /// Clears the cache.
    pub fn clear(&mut self) {
        self.windows.clear();
        self.order.clear();
    }
}

impl Default for VerifiedWindows {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use sha2::{Digest, Sha256};

    use super::{HashCache, HashWindow, decrypt_in_place, verify_window};

    #[test]
    fn hash_cache_looks_up_containing_window() {
        let mut cache = HashCache::new();
        cache.extend([
            HashWindow::new(0, 100, vec![1]),
            HashWindow::new(200, 100, vec![2]),
        ]);
        assert!(cache.get(50).is_some());
        assert!(cache.get(150).is_none());
        assert_eq!(cache.get(250).map(|w| w.offset), Some(200));
        assert!(cache.get(-1).is_none());
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn verify_rejects_mismatch() {
        let data = b"hello world".to_vec();
        let hash = Sha256::digest(&data).to_vec();
        let window = HashWindow::new(0, 11, hash);
        assert!(verify_window(&window, &data).is_ok());
        assert!(verify_window(&window, b"tampered!!!").is_err());
        assert!(verify_window(&HashWindow::new(0, 1, vec![0]), &data).is_err());
    }

    #[test]
    fn decrypt_uses_offset_counter() {
        let key = [0x42u8; 32];
        let iv = [0u8; 16];
        let mut at_zero = vec![0u8; 32];
        decrypt_in_place(&key, &iv, 0, &mut at_zero).unwrap();
        // A different offset selects a different counter block, so the
        // keystream differs.
        let mut at_block = vec![0u8; 32];
        decrypt_in_place(&key, &iv, 16, &mut at_block).unwrap();
        assert_ne!(at_zero[..16], at_block[..16]);
        // Decrypting twice restores the input.
        decrypt_in_place(&key, &iv, 0, &mut at_zero).unwrap();
        assert_eq!(at_zero, vec![0u8; 32]);
    }

    #[test]
    fn verified_windows_evict_fifo() {
        use super::VerifiedWindows;

        let mut cache = VerifiedWindows::with_capacity(2);
        for offset in [0i64, 100, 200] {
            let data = offset.to_le_bytes().to_vec();
            let window = HashWindow::new(offset, 8, Sha256::digest(&data).to_vec());
            cache.verify_and_cache(window, &data).unwrap();
        }
        assert_eq!(cache.len(), 2);
        assert!(cache.get(0).is_none());
        assert_eq!(cache.get(100).unwrap(), 100i64.to_le_bytes().to_vec());
        assert_eq!(cache.get(200).unwrap(), 200i64.to_le_bytes().to_vec());
        cache.clear();
        assert!(cache.is_empty());
    }

    #[test]
    fn cdn_state_switches_and_refreshes() {
        use super::{CdnRedirect, CdnState};

        let mut state = CdnState::new();
        assert!(!state.is_cdn());
        assert!(state.redirect().is_none());

        let data = b"cdn chunk".to_vec();
        let window = HashWindow::new(0, data.len() as i32, Sha256::digest(&data).to_vec());
        state.on_redirect(CdnRedirect::new(
            4,
            b"token".to_vec(),
            [0x11u8; 32].to_vec(),
            [0x22u8; 16].to_vec(),
            [window],
        ));
        assert!(state.is_cdn());

        // A window-aligned chunk verifies; a tampered one does not.
        assert!(state.redirect().unwrap().verify_chunk(0, &data).is_ok());
        let tampered = b"bad chunk".to_vec();
        assert!(
            state
                .redirect()
                .unwrap()
                .verify_chunk(0, &tampered)
                .is_err()
        );

        // Decryption is reversible.
        let mut encrypted = data.clone();
        state
            .redirect()
            .unwrap()
            .decrypt(0, &mut encrypted)
            .unwrap();
        assert_ne!(encrypted, data);
        state
            .redirect()
            .unwrap()
            .decrypt(0, &mut encrypted)
            .unwrap();
        assert_eq!(encrypted, data);

        // A token refresh keeps CDN mode but replaces the token/hashes.
        let window = HashWindow::new(0, data.len() as i32, Sha256::digest(&data).to_vec());
        state
            .redirect_mut()
            .unwrap()
            .refresh(b"token2".to_vec(), [window]);
        assert!(state.is_cdn());
        assert_eq!(state.redirect().unwrap().file_token, b"token2".to_vec());

        state.on_token_invalid();
        assert!(!state.is_cdn());
    }
}
