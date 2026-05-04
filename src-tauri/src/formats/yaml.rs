//! YAML list-of-mappings handling.
//!
//! ⚠️ MVP NOTE: full comment preservation is not yet implemented.
//!
//! Current strategy:
//! - If the source file contains no comments, we re-serialize via `serde_yaml`.
//! - If the source file contains comments, we keep the raw original text in
//!   memory (`ParsedYaml::had_comments == true`); on serialize we still emit a
//!   round-tripped version via `serde_yaml`, which DROPS comments. The frontend
//!   should warn the user when this is detected.
//!
//! TODO: implement a span-aware rewrite that:
//!   - emits original text for unchanged items
//!   - re-serializes only changed items
//!   - appends new fields after the last existing field per item
//!   - preserves all comment lines outside changed regions

use crate::error::{Error, Result};
use crate::model::{Record, Value};

pub struct ParsedYaml {
    pub records: Vec<Record>,
    pub had_comments: bool,
}

pub fn parse(path: impl AsRef<std::path::Path>, text: &str) -> Result<ParsedYaml> {
    let path = path.as_ref();
    let value: serde_yaml::Value = serde_yaml::from_str(text)
        .map_err(|e| Error::parse(path, format!("invalid YAML: {e}")))?;
    let serde_yaml::Value::Sequence(seq) = value else {
        return Err(Error::NotListOfMappings { path: path.into() });
    };
    let mut records = Vec::with_capacity(seq.len());
    for (i, item) in seq.iter().enumerate() {
        let serde_yaml::Value::Mapping(m) = item else {
            return Err(Error::parse(
                path,
                format!("list element {i} is not a mapping"),
            ));
        };
        let mut record = Record::new();
        for (k, v) in m {
            let key = match k {
                serde_yaml::Value::String(s) => s.clone(),
                other => serde_yaml::to_string(other)
                    .unwrap_or_default()
                    .trim()
                    .to_string(),
            };
            record.fields.insert(key, Value::from(v));
        }
        records.push(record);
    }
    Ok(ParsedYaml {
        records,
        had_comments: text_contains_comments(text),
    })
}

fn text_contains_comments(text: &str) -> bool {
    // A comment is a `#` that's not inside a string. This is a heuristic; we err
    // toward false positives so the user sees the warning.
    for line in text.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with('#') {
            return true;
        }
        // inline comments — " #"
        if line.contains(" #") {
            return true;
        }
    }
    false
}

pub fn serialize(records: &[Record], schema_keys: &[String]) -> Result<String> {
    let mut seq = serde_yaml::Sequence::with_capacity(records.len());
    for r in records {
        let mut full = r.clone();
        full.fill_missing(schema_keys);
        let mut mapping = serde_yaml::Mapping::new();
        for (k, v) in &full.fields {
            mapping.insert(serde_yaml::Value::String(k.clone()), v.to_yaml_value());
        }
        seq.push(serde_yaml::Value::Mapping(mapping));
    }
    let value = serde_yaml::Value::Sequence(seq);
    serde_yaml::to_string(&value).map_err(|e| Error::Other(format!("yaml serialize: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn p() -> std::path::PathBuf {
        "test.yaml".into()
    }

    #[test]
    fn parses_list_of_mappings() {
        let text = "- name: Alice\n  age: 30\n- name: Bob\n  age: 25\n";
        let parsed = parse(p(), text).unwrap();
        assert_eq!(parsed.records.len(), 2);
        assert_eq!(
            parsed.records[0].fields.get("name"),
            Some(&Value::String("Alice".into()))
        );
        assert!(!parsed.had_comments);
    }

    #[test]
    fn detects_comments() {
        let text = "# top comment\n- name: Alice\n  age: 30\n";
        let parsed = parse(p(), text).unwrap();
        assert!(parsed.had_comments);
    }

    #[test]
    fn rejects_top_level_mapping() {
        let text = "name: Alice\nage: 30\n";
        assert!(parse(p(), text).is_err());
    }

    #[test]
    fn round_trip_no_comments() {
        let text = "- name: Alice\n  age: 30\n- name: Bob\n  age: 25\n";
        let parsed = parse(p(), text).unwrap();
        let out = serialize(&parsed.records, &["name".into(), "age".into()]).unwrap();
        let reparsed = parse(p(), &out).unwrap();
        assert_eq!(reparsed.records.len(), 2);
        assert_eq!(
            reparsed.records[1].fields.get("age"),
            parsed.records[1].fields.get("age")
        );
    }

    #[test]
    fn null_preservation_writes_missing_key() {
        let text = "- name: Alice\n";
        let parsed = parse(p(), text).unwrap();
        let out = serialize(&parsed.records, &["name".into(), "age".into()]).unwrap();
        // serde_yaml writes Null as `null`. We accept that for MVP.
        assert!(out.contains("age:"));
    }
}
