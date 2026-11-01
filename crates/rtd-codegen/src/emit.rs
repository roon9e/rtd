//! Shared naming helpers for the Rust emitters.

/// Returns `true` when `name` is a Rust keyword.
pub(crate) fn is_rust_keyword(name: &str) -> bool {
    matches!(
        name,
        "as" | "break"
            | "const"
            | "continue"
            | "crate"
            | "dyn"
            | "else"
            | "enum"
            | "extern"
            | "false"
            | "fn"
            | "for"
            | "if"
            | "impl"
            | "in"
            | "let"
            | "loop"
            | "match"
            | "mod"
            | "move"
            | "mut"
            | "pub"
            | "ref"
            | "return"
            | "self"
            | "static"
            | "struct"
            | "super"
            | "trait"
            | "true"
            | "type"
            | "unsafe"
            | "use"
            | "where"
            | "while"
            | "async"
            | "await"
            | "box"
            | "abstract"
            | "become"
            | "do"
            | "final"
            | "macro"
            | "override"
            | "priv"
            | "typeof"
            | "unsized"
            | "virtual"
            | "yield"
            | "try"
    )
}

/// Converts a TL field name into a snake_case Rust identifier.
///
/// A separator is inserted before an uppercase letter only when it follows a
/// lowercase letter or digit, so `srp_B` becomes `srp_b` and `M1` becomes `m1`
/// while acronym runs stay intact.
pub(crate) fn to_snake_case(value: &str) -> String {
    let mut out = String::new();
    let mut previous: Option<char> = None;
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            if ch.is_ascii_uppercase()
                && previous.is_some_and(|p| p.is_ascii_lowercase() || p.is_ascii_digit())
                && !out.ends_with('_')
            {
                out.push('_');
            }
            out.push(ch.to_ascii_lowercase());
            previous = Some(ch);
        } else {
            if !out.is_empty() && !out.ends_with('_') {
                out.push('_');
            }
            previous = Some('_');
        }
    }
    while out.ends_with('_') {
        out.pop();
    }
    out
}

/// Sanitizes a TL field name for use as a Rust binding.
pub(crate) fn field_ident(name: &str) -> String {
    let snake = to_snake_case(name);
    let candidate = if snake.is_empty() {
        "field".to_owned()
    } else {
        snake
    };
    if is_rust_keyword(&candidate) {
        format!("{candidate}_")
    } else {
        candidate
    }
}

/// Uppercases the first character of `value`.
pub(crate) fn capitalize(value: &str) -> String {
    let mut chars = value.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

/// Converts a dotted qualified name into a unique Rust identifier.
///
/// Only the dotted namespace separators are removed and each segment is
/// capitalized; underscores inside a name are preserved because distinct TL
/// names can differ only by case (`rpc_drop_answer` and `RpcDropAnswer`).
pub(crate) fn ident_for_qname(qname: &str) -> String {
    qname
        .split('.')
        .map(capitalize)
        .collect::<Vec<_>>()
        .join("")
}

/// Converts a constructor name into an enum-variant identifier.
pub(crate) fn variant_ident(name: &str) -> String {
    capitalize(name)
}

/// Converts a dotted qualified name into an UPPER_SNAKE identifier.
///
/// A separator is inserted before an uppercase letter only when it follows a
/// lowercase letter or digit, so acronym runs such as `DH` or `PQ` stay intact.
pub(crate) fn to_upper_snake(value: &str) -> String {
    let mut out = String::new();
    let mut previous: Option<char> = None;
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            if ch.is_ascii_uppercase()
                && previous.is_some_and(|p| p.is_ascii_lowercase() || p.is_ascii_digit())
                && !out.ends_with('_')
            {
                out.push('_');
            }
            out.push(ch.to_ascii_uppercase());
            previous = Some(ch);
        } else {
            if !out.is_empty() && !out.ends_with('_') {
                out.push('_');
            }
            previous = Some('_');
        }
    }
    while out.ends_with('_') {
        out.pop();
    }
    if out.is_empty() {
        out.push('_');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{ident_for_qname, to_upper_snake, variant_ident};

    #[test]
    fn qualifies_names() {
        assert_eq!(ident_for_qname("auth.Authorization"), "AuthAuthorization");
        assert_eq!(ident_for_qname("InputPeer"), "InputPeer");
        assert_eq!(variant_ident("inputPeerEmpty"), "InputPeerEmpty");
    }

    #[test]
    fn upper_snakes_names() {
        assert_eq!(to_upper_snake("auth.sendCode"), "AUTH_SEND_CODE");
        assert_eq!(to_upper_snake("inputPeerEmpty"), "INPUT_PEER_EMPTY");
        assert_eq!(to_upper_snake("req_DH_params"), "REQ_DH_PARAMS");
        assert_eq!(to_upper_snake("resPQ"), "RES_PQ");
    }
}
