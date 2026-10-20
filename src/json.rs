// Copyright (C) 2026 Bel Esprit D'Accord Irrevocable Trust / SnapKitty Collective Limited
// Dual-licensed: AGPL-3.0-only plus SnapKitty Opaque License.
// SPDX-License-Identifier: AGPL-3.0-only AND LicenseRef-SnapKitty-Opaque

//! Minimal JSON support for the front-end wire formats (WebSocket
//! messages and the WASM result channel). The protocol uses flat
//! objects with string values; this module escapes strings for
//! output and extracts string fields from input. It is not a
//! general JSON parser and does not pretend to be one.

/// Escape a string for inclusion inside a JSON string literal
/// (the surrounding quotes are not added).
pub fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out
}

/// Extract the string value of `key` from a flat JSON object.
/// Returns `None` if the key is absent or its value is not a
/// string. Handles the standard string escapes.
pub fn get_string(json: &str, key: &str) -> Option<String> {
    let needle = format!("\"{key}\"");
    let mut rest = json;
    loop {
        let at = rest.find(&needle)?;
        let after_key = &rest[at + needle.len()..];
        rest = after_key;
        let Some(after_colon) = after_key.trim_start().strip_prefix(':') else {
            continue;
        };
        if let Some(value) = parse_string(after_colon.trim_start()) {
            return Some(value);
        }
    }
}

/// Parse a JSON string literal at the start of `s`, returning its
/// unescaped contents.
fn parse_string(s: &str) -> Option<String> {
    let mut chars = s.chars();
    if chars.next() != Some('"') {
        return None;
    }
    let mut out = String::new();
    loop {
        match chars.next()? {
            '"' => return Some(out),
            '\\' => match chars.next()? {
                '"' => out.push('"'),
                '\\' => out.push('\\'),
                '/' => out.push('/'),
                'n' => out.push('\n'),
                'r' => out.push('\r'),
                't' => out.push('\t'),
                'b' => out.push('\u{0008}'),
                'f' => out.push('\u{000C}'),
                'u' => {
                    let hex: String = chars.by_ref().take(4).collect();
                    let code = u32::from_str_radix(&hex, 16).ok()?;
                    out.push(char::from_u32(code)?);
                }
                _ => return None,
            },
            c => out.push(c),
        }
    }
}

/// Extract an array of u16 values stored under `key`
/// (`"key":[1,2,3]`). Returns `None` if absent or malformed.
pub fn get_u16_array(json: &str, key: &str) -> Option<Vec<u16>> {
    let needle = format!("\"{key}\"");
    let at = json.find(&needle)?;
    let after = json[at + needle.len()..].trim_start().strip_prefix(':')?;
    let after = after.trim_start().strip_prefix('[')?;
    let end = after.find(']')?;
    let body = after[..end].trim();
    if body.is_empty() {
        return Some(Vec::new());
    }
    body.split(',')
        .map(|part| part.trim().parse::<u16>().ok())
        .collect()
}

/// Extract the numeric value of `key` from a flat JSON object.
/// Returns `None` if the key is absent or not a number.
pub fn get_number(json: &str, key: &str) -> Option<f64> {
    let needle = format!("\"{key}\"");
    let at = json.find(&needle)?;
    let after = json[at + needle.len()..].trim_start().strip_prefix(':')?;
    let digits: String = after
        .trim_start()
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.' || *c == '-')
        .collect();
    digits.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escape_covers_quotes_backslashes_and_controls() {
        assert_eq!(escape("a\"b\\c\nd\te"), "a\\\"b\\\\c\\nd\\te");
        assert_eq!(escape("plain"), "plain");
    }

    #[test]
    fn get_string_extracts_fields() {
        let json = r#"{"type":"compile","bf":"+++>---","note":"x"}"#;
        assert_eq!(get_string(json, "type").as_deref(), Some("compile"));
        assert_eq!(get_string(json, "bf").as_deref(), Some("+++>---"));
        assert_eq!(get_string(json, "missing"), None);
    }

    #[test]
    fn get_string_unescapes() {
        let json = r#"{"grid":"+ +\n  -","u":"A"}"#;
        assert_eq!(get_string(json, "grid").as_deref(), Some("+ +\n  -"));
        assert_eq!(get_string(json, "u").as_deref(), Some("A"));
    }

    #[test]
    fn get_string_ignores_non_string_values_and_key_prefixes() {
        let json = r#"{"cells":42,"cell":"+"}"#;
        assert_eq!(get_string(json, "cells"), None);
        assert_eq!(get_string(json, "cell").as_deref(), Some("+"));
    }

    #[test]
    fn escape_then_extract_roundtrips() {
        let original = "line1\n\"quoted\" \\ tab\tend";
        let json = format!("{{\"v\":\"{}\"}}", escape(original));
        assert_eq!(get_string(&json, "v").as_deref(), Some(original));
    }
}
