//! The TL `flags` bitfield.
//!
//! A `flags` field is a 32-bit word in which individual bits mark the presence
//! of optional fields that follow in the constructor.

/// A TL `flags` bitfield.
#[derive(Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Fields(u32);

impl Fields {
    /// Creates an empty bitfield.
    #[must_use]
    pub const fn new() -> Self {
        Self(0)
    }

    /// Wraps a raw 32-bit value.
    #[must_use]
    pub const fn from_value(value: u32) -> Self {
        Self(value)
    }

    /// Returns the raw 32-bit value.
    #[must_use]
    pub const fn value(self) -> u32 {
        self.0
    }

    /// Returns `true` if bit `bit` is set.
    ///
    /// # Panics
    ///
    /// Panics if `bit` is not less than 32.
    #[must_use]
    pub const fn has(self, bit: u8) -> bool {
        assert!(bit < 32, "flags bit out of range");
        self.0 & (1 << bit) != 0
    }

    /// Sets bit `bit`.
    ///
    /// # Panics
    ///
    /// Panics if `bit` is not less than 32.
    pub fn set(&mut self, bit: u8) {
        assert!(bit < 32, "flags bit out of range");
        self.0 |= 1 << bit;
    }

    /// Clears bit `bit`.
    ///
    /// # Panics
    ///
    /// Panics if `bit` is not less than 32.
    pub fn clear(&mut self, bit: u8) {
        assert!(bit < 32, "flags bit out of range");
        self.0 &= !(1 << bit);
    }

    /// Sets bit `bit` when `value` is `true`, clears it otherwise.
    ///
    /// # Panics
    ///
    /// Panics if `bit` is not less than 32.
    pub fn set_if(&mut self, bit: u8, value: bool) {
        if value {
            self.set(bit);
        } else {
            self.clear(bit);
        }
    }
}

impl From<u32> for Fields {
    fn from(value: u32) -> Self {
        Self(value)
    }
}

impl From<Fields> for u32 {
    fn from(value: Fields) -> Self {
        value.0
    }
}

impl core::fmt::Debug for Fields {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Fields({:#010x})", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::Fields;

    #[test]
    fn set_has_clear() {
        let mut f = Fields::new();
        assert!(!f.has(3));
        f.set(3);
        assert!(f.has(3));
        assert_eq!(f.value(), 1 << 3);
        f.clear(3);
        assert!(!f.has(3));
        assert_eq!(f.value(), 0);
    }

    #[test]
    fn set_if_tracks_value() {
        let mut f = Fields::new();
        f.set_if(1, true);
        assert!(f.has(1));
        f.set_if(1, false);
        assert!(!f.has(1));
    }

    #[test]
    fn round_trip_value() {
        let f = Fields::from_value(0x8000_0001);
        assert!(f.has(0));
        assert!(f.has(31));
        assert!(!f.has(1));
        assert_eq!(u32::from(f), 0x8000_0001);
    }
}
