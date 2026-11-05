//! ASCII classification helpers.
//!
//! Mirrors `td/ascii`: predicates over the ASCII range used when parsing text
//! and identifiers. Unlike the Unicode-aware `char` methods, these are
//! deliberately ASCII-only.

/// Returns whether `c` is an ASCII digit (`0`–`9`).
#[must_use]
pub fn is_digit(c: char) -> bool {
    c.is_ascii_digit()
}

/// Returns whether `c` is an ASCII letter (`a`–`z`, `A`–`Z`).
#[must_use]
pub fn is_letter(c: char) -> bool {
    c.is_ascii_alphabetic()
}

/// Returns whether `c` is a lowercase ASCII letter.
#[must_use]
pub fn is_lower(c: char) -> bool {
    c.is_ascii_lowercase()
}

/// Returns whether `c` is an uppercase ASCII letter.
#[must_use]
pub fn is_upper(c: char) -> bool {
    c.is_ascii_uppercase()
}

/// Returns whether `c` is an ASCII hexadecimal digit (`0`–`9`, `a`–`f`, `A`–`F`).
#[must_use]
pub fn is_hex_digit(c: char) -> bool {
    c.is_ascii_hexdigit()
}

/// Returns whether `c` is an ASCII whitespace character.
#[must_use]
pub fn is_space(c: char) -> bool {
    c.is_ascii_whitespace()
}

#[cfg(test)]
mod tests {
    use super::{is_digit, is_hex_digit, is_letter, is_lower, is_upper};

    #[test]
    fn classifies_ascii() {
        assert!(is_digit('7'));
        assert!(!is_digit('a'));
        assert!(is_letter('a') && is_letter('Z'));
        assert!(!is_letter('1'));
        assert!(is_lower('a') && is_upper('A'));
        assert!(is_hex_digit('f') && is_hex_digit('0'));
        assert!(!is_hex_digit('g'));
    }
}
