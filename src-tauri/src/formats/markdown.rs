//! Markdown + YAML frontmatter handling.
//!
//! Invariants:
//! - The body bytes after the closing `---` are NEVER modified.
//! - If a file has no frontmatter, parsing produces an empty record and the
//!   entire file is treated as body.
//! - On serialize, the frontmatter block is rewritten via `serde_yaml`; the
//!   line ending of the original file is preserved.

use crate::error::{Error, Result};
use crate::model::{Record, Value};

use super::LineEnding;

/// The result of parsing a markdown file.
pub struct ParsedMarkdown {
    pub record: Record,
    pub body: String,
    pub had_frontmatter: bool,
    pub line_ending: LineEnding,
}

/// Parse a markdown file into (frontmatter record, body).
pub fn parse(path: impl AsRef<std::path::Path>, text: &str) -> Result<ParsedMarkdown> {
    let path = path.as_ref();
    let line_ending = LineEnding::detect(text);

    // Look for an opening `---` on the very first line. Allow a leading BOM.
    let trimmed_start = text.strip_prefix('\u{feff}').unwrap_or(text);
    let after_bom_offset = text.len() - trimmed_start.len();

    let opens_with_fence = trimmed_start.starts_with("---\n") || trimmed_start.starts_with("---\r\n");
    if !opens_with_fence {
        return Ok(ParsedMarkdown {
            record: Record::new(),
            body: text.to_string(),
            had_frontmatter: false,
            line_ending,
        });
    }

    // Skip past the opening fence + its trailing newline.
    let after_open = if trimmed_start.starts_with("---\r\n") {
        &trimmed_start[5..]
    } else {
        &trimmed_start[4..]
    };

    // Find the closing fence: a line that is exactly `---`.
    let closing = find_closing_fence(after_open);
    let Some((fm_text, body_start_in_after_open)) = closing else {
        return Err(Error::parse(
            path,
            "frontmatter opened with `---` but never closed",
        ));
    };

    let body_offset = after_bom_offset + (trimmed_start.len() - after_open.len()) + body_start_in_after_open;
    let body = text[body_offset..].to_string();

    let record = parse_frontmatter_yaml(path, fm_text)?;

    Ok(ParsedMarkdown {
        record,
        body,
        had_frontmatter: true,
        line_ending,
    })
}

/// Returns (frontmatter text excluding the closing fence, byte offset in `after_open`
/// at which the body starts — i.e., just past the closing fence's trailing newline,
/// or at end-of-string if the file ends with the fence).
fn find_closing_fence(after_open: &str) -> Option<(&str, usize)> {
    let mut cursor = 0usize;
    while cursor < after_open.len() {
        let rest = &after_open[cursor..];
        let line_end = rest.find('\n').map(|i| cursor + i).unwrap_or(after_open.len());
        let line = &after_open[cursor..line_end];
        let line_trimmed_cr = line.strip_suffix('\r').unwrap_or(line);
        if line_trimmed_cr == "---" {
            let fm_text = &after_open[..cursor];
            // Strip the trailing newline on the fm_text if any (the line BEFORE the fence).
            // Actually fm_text already excludes the fence line; the newline before the fence
            // stays in fm_text — that's fine for YAML parsing.
            let body_start = if line_end < after_open.len() {
                line_end + 1
            } else {
                after_open.len()
            };
            return Some((fm_text, body_start));
        }
        if line_end >= after_open.len() {
            break;
        }
        cursor = line_end + 1;
    }
    None
}

fn parse_frontmatter_yaml(path: &std::path::Path, text: &str) -> Result<Record> {
    if text.trim().is_empty() {
        return Ok(Record::new());
    }
    let parsed: serde_yaml::Value = serde_yaml::from_str(text)
        .map_err(|e| Error::parse(path, format!("frontmatter YAML: {e}")))?;
    let mapping = match parsed {
        serde_yaml::Value::Mapping(m) => m,
        serde_yaml::Value::Null => return Ok(Record::new()),
        _ => return Err(Error::parse(path, "frontmatter must be a YAML mapping")),
    };
    let mut record = Record::new();
    for (k, v) in mapping {
        let key = match k {
            serde_yaml::Value::String(s) => s,
            other => serde_yaml::to_string(&other)
                .unwrap_or_default()
                .trim()
                .to_string(),
        };
        record.fields.insert(key, Value::from(&v));
    }
    Ok(record)
}

/// Serialize a record as a markdown frontmatter block + the original body, verbatim.
///
/// `schema_keys` enforces null preservation: every key in the schema is written for
/// every record, even if the value is `Null` (emitted as `key:` in YAML).
pub fn serialize(record: &Record, body: &str, schema_keys: &[String], line_ending: LineEnding) -> Result<String> {
    let mut full = record.clone();
    full.fill_missing(schema_keys);

    // Build a YAML mapping in the record's current key order, but reorder so that
    // schema_keys-known keys appear in record's existing order, and any keys that
    // weren't originally present are appended in schema order (already done by fill_missing
    // — IndexMap.insert appends new keys at the end).
    let mut mapping = serde_yaml::Mapping::new();
    for (k, v) in &full.fields {
        mapping.insert(serde_yaml::Value::String(k.clone()), v.to_yaml_value());
    }
    let yaml_value = serde_yaml::Value::Mapping(mapping);
    let yaml_text = serialize_mapping_yaml(&yaml_value)?;
    let yaml_text = normalize_null_keys(&yaml_text);
    let yaml_text = ensure_trailing_newline(&yaml_text);

    let nl = line_ending.as_str();
    let yaml_text = if line_ending == LineEnding::Crlf {
        yaml_text.replace('\n', "\r\n")
    } else {
        yaml_text
    };

    Ok(format!("---{nl}{yaml_text}---{nl}{body}"))
}

fn serialize_mapping_yaml(value: &serde_yaml::Value) -> Result<String> {
    serde_yaml::to_string(value).map_err(|e| Error::Other(format!("yaml serialize: {e}")))
}

fn ensure_trailing_newline(s: &str) -> String {
    if s.ends_with('\n') {
        s.to_string()
    } else {
        format!("{s}\n")
    }
}

/// Build a record from an explicit list of (key, value) pairs (test helper).
#[allow(dead_code)]
pub fn record_from(pairs: Vec<(&str, Value)>) -> Record {
    let mut r = Record::new();
    for (k, v) in pairs {
        r.fields.insert(k.to_string(), v);
    }
    r
}

/// Suggest writing `key: ` (empty) for null values, matching PRD §7.4 YAML rule.
/// `serde_yaml` already emits `key: null` for `Value::Null`. We post-process to convert
/// `^key: null$` → `key:` only at the top level of the frontmatter mapping.
pub fn normalize_null_keys(yaml_text: &str) -> String {
    yaml_text
        .lines()
        .map(|line| {
            // Only touch lines that are top-level keys (no leading whitespace).
            if !line.starts_with(' ') && !line.starts_with('\t') {
                if let Some(idx) = line.find(": null") {
                    let key_part = &line[..idx];
                    // Validate it's "key: null" with no trailing tokens.
                    if line[idx + 6..].trim().is_empty() {
                        return format!("{key_part}:");
                    }
                }
            }
            line.to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use indexmap::indexmap;
    use pretty_assertions::assert_eq;

    fn p() -> std::path::PathBuf {
        "test.md".into()
    }

    #[test]
    fn no_frontmatter_keeps_body_verbatim() {
        let text = "# Just a heading\n\nSome body content.\n";
        let parsed = parse(p(), text).unwrap();
        assert!(!parsed.had_frontmatter);
        assert!(parsed.record.fields.is_empty());
        assert_eq!(parsed.body, text);
    }

    #[test]
    fn parses_simple_frontmatter() {
        let text = "---\ntitle: Hello\nrating: 5\n---\nBody text here.\n";
        let parsed = parse(p(), text).unwrap();
        assert!(parsed.had_frontmatter);
        assert_eq!(
            parsed.record.fields.get("title"),
            Some(&Value::String("Hello".into()))
        );
        assert_eq!(parsed.body, "Body text here.\n");
    }

    #[test]
    fn body_after_frontmatter_is_byte_identical() {
        let body = "# Title\n\n```rust\nfn main() {}\n```\n\n> a quote\n";
        let text = format!("---\nfoo: bar\n---\n{body}");
        let parsed = parse(p(), &text).unwrap();
        assert_eq!(parsed.body, body);
    }

    #[test]
    fn round_trip_preserves_body_bytes() {
        let body = "Line 1\n\nLine 3 with **bold**.\n";
        let text = format!("---\ntitle: T\n---\n{body}");
        let parsed = parse(p(), &text).unwrap();
        let out = serialize(
            &parsed.record,
            &parsed.body,
            &["title".into()],
            parsed.line_ending,
        )
        .unwrap();
        // Body must reappear verbatim.
        assert!(out.ends_with(body), "round-tripped body changed: {out:?}");
    }

    #[test]
    fn empty_frontmatter_block_is_empty_record() {
        let text = "---\n---\nbody\n";
        let parsed = parse(p(), text).unwrap();
        assert!(parsed.record.fields.is_empty());
        assert_eq!(parsed.body, "body\n");
    }

    #[test]
    fn unclosed_frontmatter_is_an_error() {
        let text = "---\ntitle: T\nno closing fence here\n";
        assert!(parse(p(), text).is_err());
    }

    #[test]
    fn null_value_writes_as_bare_key() {
        let mut r = Record::new();
        r.fields = indexmap! {
            "title".to_string() => Value::String("T".into()),
            "rating".to_string() => Value::Null,
        };
        let out = serialize(&r, "", &["title".into(), "rating".into()], LineEnding::Lf).unwrap();
        // PRD §7.4: empty YAML cell must serialize as bare `key:`, not `key: null`.
        assert!(
            out.contains("\nrating:\n") || out.contains("rating:\n---"),
            "expected bare 'rating:' in {out:?}"
        );
        assert!(
            !out.contains("rating: null"),
            "must not emit 'rating: null' in {out:?}"
        );
        // Round-trip back to Null.
        let reparsed = parse(p(), &out).unwrap();
        assert_eq!(reparsed.record.fields.get("rating"), Some(&Value::Null));
    }

    #[test]
    fn crlf_line_endings_are_preserved_in_fences() {
        let text = "---\r\ntitle: T\r\n---\r\nbody\r\n";
        let parsed = parse(p(), text).unwrap();
        assert_eq!(parsed.line_ending, LineEnding::Crlf);
        assert_eq!(parsed.body, "body\r\n");
        let out = serialize(&parsed.record, &parsed.body, &["title".into()], parsed.line_ending).unwrap();
        assert!(out.starts_with("---\r\n"));
        assert!(out.contains("---\r\nbody\r\n"));
    }
}
