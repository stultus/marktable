//! JSON array-of-objects handling.
//!
//! - Detects indentation style from the source (tab / 2 / 4 spaces, default 2).
//! - Reproduces detected indentation on write.
//! - Preserves original key order per object via `serde_json` `preserve_order` feature.
//! - Null preservation: every schema key is written for every record. Empty cells
//!   are written as JSON `null`.

use crate::error::{Error, Result};
use crate::model::{Record, Value};

/// What indentation does this JSON file use?
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Indent {
    Tab,
    Spaces(u8),
}

impl Indent {
    pub fn detect(text: &str) -> Self {
        for line in text.lines() {
            if line.starts_with('\t') {
                return Indent::Tab;
            }
            let leading = line.chars().take_while(|c| *c == ' ').count();
            if leading > 0 {
                return match leading {
                    4 => Indent::Spaces(4),
                    _ => Indent::Spaces(2),
                };
            }
        }
        Indent::Spaces(2)
    }

    fn unit(self) -> Vec<u8> {
        match self {
            Indent::Tab => b"\t".to_vec(),
            Indent::Spaces(n) => vec![b' '; n as usize],
        }
    }
}

pub struct ParsedJson {
    pub records: Vec<Record>,
    pub indent: Indent,
}

pub fn parse(path: impl AsRef<std::path::Path>, text: &str) -> Result<ParsedJson> {
    let path = path.as_ref();
    let value: serde_json::Value = serde_json::from_str(text)
        .map_err(|e| Error::parse(path, format!("invalid JSON: {e}")))?;
    let serde_json::Value::Array(arr) = value else {
        return Err(Error::NotArrayOfObjects { path: path.into() });
    };
    let mut records = Vec::with_capacity(arr.len());
    for (i, item) in arr.iter().enumerate() {
        let obj = match item {
            serde_json::Value::Object(o) => o,
            _ => {
                return Err(Error::parse(
                    path,
                    format!("array element {i} is not an object"),
                ))
            }
        };
        let mut record = Record::new();
        for (k, v) in obj {
            record.fields.insert(k.clone(), Value::from(v));
        }
        records.push(record);
    }
    Ok(ParsedJson {
        records,
        indent: Indent::detect(text),
    })
}

pub fn serialize(records: &[Record], schema_keys: &[String], indent: Indent) -> Result<String> {
    let mut arr = Vec::with_capacity(records.len());
    for r in records {
        let mut full = r.clone();
        full.fill_missing(schema_keys);
        let mut obj = serde_json::Map::new();
        for (k, v) in &full.fields {
            obj.insert(k.clone(), v.to_json_value());
        }
        arr.push(serde_json::Value::Object(obj));
    }
    let value = serde_json::Value::Array(arr);
    let mut buf = Vec::new();
    let unit = indent.unit();
    let formatter = serde_json::ser::PrettyFormatter::with_indent(&unit);
    let mut ser = serde_json::Serializer::with_formatter(&mut buf, formatter);
    use serde::Serialize;
    value
        .serialize(&mut ser)
        .map_err(|e| Error::Other(format!("json serialize: {e}")))?;
    let mut out = String::from_utf8(buf)
        .map_err(|e| Error::Other(format!("json utf8: {e}")))?;
    out.push('\n');
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn p() -> std::path::PathBuf {
        "test.json".into()
    }

    #[test]
    fn parses_array_of_objects() {
        let text = r#"[
  {"title": "A", "rating": 5},
  {"title": "B", "rating": 3}
]"#;
        let parsed = parse(p(), text).unwrap();
        assert_eq!(parsed.records.len(), 2);
        assert_eq!(parsed.indent, Indent::Spaces(2));
        assert_eq!(
            parsed.records[0].fields.get("title"),
            Some(&Value::String("A".into()))
        );
    }

    #[test]
    fn rejects_non_array_top_level() {
        let text = r#"{"not": "an array"}"#;
        assert!(parse(p(), text).is_err());
    }

    #[test]
    fn rejects_array_of_non_objects() {
        let text = r#"[1, 2, 3]"#;
        assert!(parse(p(), text).is_err());
    }

    #[test]
    fn detects_tab_indent() {
        let text = "[\n\t{\n\t\t\"x\": 1\n\t}\n]\n";
        assert_eq!(Indent::detect(text), Indent::Tab);
    }

    #[test]
    fn detects_4_space_indent() {
        let text = "[\n    {\n        \"x\": 1\n    }\n]\n";
        assert_eq!(Indent::detect(text), Indent::Spaces(4));
    }

    #[test]
    fn round_trip_preserves_2space_indent() {
        let text = "[\n  {\n    \"a\": 1,\n    \"b\": \"x\"\n  }\n]\n";
        let parsed = parse(p(), text).unwrap();
        let out = serialize(
            &parsed.records,
            &["a".into(), "b".into()],
            parsed.indent,
        )
        .unwrap();
        assert!(out.contains("\n  {"));
        assert!(out.contains("\n    \"a\""));
    }

    #[test]
    fn null_preservation_writes_missing_keys_as_null() {
        let text = r#"[{"a": 1}]"#;
        let parsed = parse(p(), text).unwrap();
        let out = serialize(
            &parsed.records,
            &["a".into(), "b".into()],
            parsed.indent,
        )
        .unwrap();
        assert!(out.contains("\"b\": null"), "missing 'b: null' in {out}");
    }
}
