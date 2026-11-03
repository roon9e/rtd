//! A minimal config-file loader.
//!
//! Parses a gramsrv-shaped YAML subset (`key: value` with indentation-based
//! nesting, comments and `${ENV}` references) into a flat map of dotted keys,
//! then maps known keys onto the edge's command-line flags. This is deliberately
//! small: the flags remain the single source of validation.

use std::collections::BTreeMap;

/// Parses a YAML subset into a map of dotted keys.
///
/// Values keep any `${ENV}` reference verbatim; the caller expands them from the
/// environment.
#[must_use]
pub fn parse(text: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let mut stack: Vec<(usize, String)> = Vec::new();
    for raw in text.lines() {
        let line = strip_comment(raw);
        if line.trim().is_empty() {
            continue;
        }
        let indent = line.len() - line.trim_start().len();
        let content = line.trim();
        let Some((key, value)) = content.split_once(':') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();
        while let Some((level, _)) = stack.last() {
            if *level >= indent {
                stack.pop();
            } else {
                break;
            }
        }
        let path = match stack.last() {
            Some((_, prefix)) => format!("{prefix}.{key}"),
            None => key.to_owned(),
        };
        if value.is_empty() {
            stack.push((indent, path));
        } else {
            out.insert(path, unquote(value));
        }
    }
    out
}

/// Maps a parsed configuration to edge flags (`--flag value`).
///
/// `${VAR}` references are expanded from `env`.
#[must_use]
pub fn to_args(
    config: &BTreeMap<String, String>,
    env: &dyn Fn(&str) -> Option<String>,
) -> Vec<String> {
    let mut args = Vec::new();
    let mut push = |flag: &str, value: &str| {
        let value = expand(value, env);
        if !value.is_empty() {
            args.push(flag.to_owned());
            args.push(value);
        }
    };
    for (key, value) in config {
        match key.as_str() {
            "edge.listen" | "public.listen" => push("--bind", value),
            "edge.rsa_key" | "public.rsa_key" => push("--rsa-key", value),
            "public.dc" | "dc" | "edge.dc" => push("--dc", value),
            "edge.websocket.enabled" => push("--websocket", value),
            "core_exec.token" => push("--core-token", value),
            "core_exec.targets" => push("--core", value),
            "core_exec.timeout" => push("--core-timeout", value),
            "instance.id" => push("--instance-id", value),
            _ => {}
        }
    }
    args
}

/// Expands a leading `${VAR}` reference from `env` (else returns the value).
fn expand(value: &str, env: &dyn Fn(&str) -> Option<String>) -> String {
    let trimmed = value.trim();
    if let Some(name) = trimmed
        .strip_prefix("${")
        .and_then(|rest| rest.strip_suffix('}'))
    {
        return env(name).unwrap_or_default();
    }
    trimmed.to_owned()
}

/// Removes a trailing ` # comment`, respecting simple quotes.
fn strip_comment(line: &str) -> String {
    let mut in_quote = false;
    let mut previous = '\0';
    for (index, ch) in line.char_indices() {
        match ch {
            '\'' | '"' if previous != '\\' => in_quote = !in_quote,
            '#' if !in_quote => return line[..index].to_owned(),
            _ => {}
        }
        previous = ch;
    }
    line.to_owned()
}

/// Removes surrounding single or double quotes.
fn unquote(value: &str) -> String {
    let bytes = value.as_bytes();
    if bytes.len() >= 2
        && (bytes[0] == b'"' && bytes[bytes.len() - 1] == b'"'
            || bytes[0] == b'\'' && bytes[bytes.len() - 1] == b'\'')
    {
        return value[1..value.len() - 1].to_owned();
    }
    value.to_owned()
}

#[cfg(test)]
mod tests {
    use super::{parse, to_args};

    #[test]
    fn parses_nested_keys_and_comments() {
        let config = parse(
            "public:\n  dc: 2 # the serving dc\nedge:\n  listen: 0.0.0.0:2398\n  rsa_key: /key.pem\ncore_exec:\n  token: ${TELESRV_CORE_EXEC_TOKEN}\n  targets: core:2440\n",
        );
        assert_eq!(config.get("public.dc").map(String::as_str), Some("2"));
        assert_eq!(
            config.get("edge.listen").map(String::as_str),
            Some("0.0.0.0:2398")
        );
        assert_eq!(
            config.get("core_exec.token").map(String::as_str),
            Some("${TELESRV_CORE_EXEC_TOKEN}")
        );
    }

    #[test]
    fn maps_to_flags_and_expands_env() {
        let config = parse(
            "edge:\n  listen: 0.0.0.0:2398\n  rsa_key: /key.pem\npublic:\n  dc: 2\ncore_exec:\n  token: ${TOKEN}\n",
        );
        let args = to_args(&config, &|name| {
            (name == "TOKEN").then(|| "secret".to_owned())
        });
        assert!(args.windows(2).any(|w| w == ["--bind", "0.0.0.0:2398"]));
        assert!(args.windows(2).any(|w| w == ["--dc", "2"]));
        assert!(args.windows(2).any(|w| w == ["--core-token", "secret"]));
    }
}
