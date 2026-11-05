//! A Go `strconv.Quote`-compatible string quoter.
//!
//! `langpackfetch` writes `.strings` files where every key and value is wrapped
//! by Go's `strconv.Quote`. To keep those files byte-identical the rules are
//! reproduced here: `"` and `\` are backslash-escaped, the C0 controls use the
//! short `\a\b\f\n\r\t\v` forms, other non-printable runes use `\xNN`/`\uNNNN`/
//! `\UNNNNNNNN`, and printable characters pass through unchanged.

/// Quotes `value` with Go `strconv.Quote` semantics (including the quotes).
#[must_use]
pub fn quote(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{0007}' => out.push_str("\\a"),
            '\u{0008}' => out.push_str("\\b"),
            '\u{000c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{000b}' => out.push_str("\\v"),
            ch if (ch as u32) < 0x20 || ch as u32 == 0x7f => {
                push_hex_escape(&mut out, ch as u32, 2);
            }
            ch if !is_print(ch) => {
                let code = ch as u32;
                let width = if code <= 0xff {
                    2
                } else if code <= 0xffff {
                    4
                } else {
                    8
                };
                push_hex_escape(&mut out, code, width);
            }
            ch => out.push(ch),
        }
    }
    out.push('"');
    out
}

/// Writes a `\x`/`\u`/`\U` escape for `code` padded to `width` hex digits.
fn push_hex_escape(out: &mut String, code: u32, width: usize) {
    let prefix = match width {
        2 => "\\x",
        4 => "\\u",
        _ => "\\U",
    };
    out.push_str(prefix);
    out.push_str(&format!("{code:0width$x}"));
}

/// Approximates Go's `unicode.IsPrint`.
///
/// Go treats letters, marks, numbers, punctuation, symbols and the ASCII space
/// as printable. Rust's `char` API has no direct equivalent, so control and
/// format/separator code points are treated as non-printable and everything else
/// as printable; langpack text never carries anything subtler.
fn is_print(ch: char) -> bool {
    if ch == ' ' {
        return true;
    }
    if ch.is_control() {
        return false;
    }
    // Format (Cf), line/paragraph separators (Zl/Zp) and unassigned planes.
    !matches!(
        ch,
        '\u{00ad}' | '\u{200b}'..='\u{200f}' | '\u{2028}' | '\u{2029}' | '\u{feff}'
    )
}

#[cfg(test)]
mod tests {
    use super::quote;

    #[test]
    fn matches_strconv_quote() {
        // Expectations were produced by Go's strconv.Quote.
        let cases: &[(&str, &str)] = &[
            ("plain", "\"plain\""),
            ("a\"b\\c", "\"a\\\"b\\\\c\""),
            ("line\nbreak", "\"line\\nbreak\""),
            ("tab\there", "\"tab\\there\""),
            ("emoji 😀", "\"emoji 😀\""),
            ("<tag> & 'q'", "\"<tag> & 'q'\""),
            ("héllo", "\"héllo\""),
            ("ctrl\u{1}\u{7f}", "\"ctrl\\x01\\x7f\""),
            ("", "\"\""),
        ];
        for (input, expected) in cases {
            assert_eq!(&quote(input), expected, "input {input:?}");
        }
    }
}
