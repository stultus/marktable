//! End-to-end round-trip tests on real files in a temp directory.
//!
//! These cover the markdown folder mode: open a folder, the schema is inferred,
//! we serialize each row back, and the on-disk text matches what we'd get from
//! reparsing.

use std::fs;
use std::path::PathBuf;

use marktable_lib::formats::{json, markdown, yaml, LineEnding};
use marktable_lib::model::{Record, Schema, Value};

fn temp_dir(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("marktable-test-{label}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn folder_round_trip_preserves_body_bytes_per_file() {
    let dir = temp_dir("folder-rt");

    let f1 = dir.join("a.md");
    let f1_text = "---\ntitle: A\nrating: 5\n---\nFirst body.\n";
    fs::write(&f1, f1_text).unwrap();

    let f2 = dir.join("b.md");
    let f2_text = "---\ntitle: B\nauthor: Bee\n---\n# Heading\n\nSecond body.\n";
    fs::write(&f2, f2_text).unwrap();

    // Parse both files.
    let p1 = markdown::parse(&f1, f1_text).unwrap();
    let p2 = markdown::parse(&f2, f2_text).unwrap();

    // Build schema across both records — should have title, rating, author.
    let schema = Schema::infer(&[p1.record.clone(), p2.record.clone()]);
    let keys = schema.keys();
    assert_eq!(keys.len(), 3);
    assert!(keys.contains(&"title".to_string()));
    assert!(keys.contains(&"rating".to_string()));
    assert!(keys.contains(&"author".to_string()));

    // Serialize each back. Body must be byte-identical.
    let out1 = markdown::serialize(&p1.record, &p1.body, &keys, LineEnding::Lf).unwrap();
    let out2 = markdown::serialize(&p2.record, &p2.body, &keys, LineEnding::Lf).unwrap();
    assert!(out1.ends_with("First body.\n"));
    assert!(out2.ends_with("# Heading\n\nSecond body.\n"));

    // Null preservation: file `a.md` originally had no `author`; on serialize,
    // schema_keys ensures `author:` (or `author: null`) is present.
    assert!(
        out1.contains("author:"),
        "missing 'author:' in {out1:?}"
    );
    // file `b.md` originally had no `rating`; same rule.
    assert!(
        out2.contains("rating:"),
        "missing 'rating:' in {out2:?}"
    );

    let _ = fs::remove_dir_all(&dir);
}

// ─── Key-order preservation (PRD: "Original field order is preserved per file. New fields
// are appended at the end of each record's block.") ──────────────────────────────────────

fn keys_in_order(record: &Record) -> Vec<String> {
    record.fields.keys().cloned().collect()
}

#[test]
fn json_preserves_key_order_on_round_trip() {
    let text = r#"[{"z": 1, "a": 2, "m": 3}]"#;
    let parsed = json::parse("k.json", text).unwrap();
    assert_eq!(
        keys_in_order(&parsed.records[0]),
        vec!["z", "a", "m"],
        "parse must keep original JSON key order"
    );

    let schema_keys = vec!["z".into(), "a".into(), "m".into()];
    let out = json::serialize(&parsed.records, &schema_keys, parsed.indent).unwrap();
    let reparsed = json::parse("k.json", &out).unwrap();
    assert_eq!(
        keys_in_order(&reparsed.records[0]),
        vec!["z", "a", "m"],
        "serialize → reparse must keep order"
    );
}

#[test]
fn yaml_preserves_key_order_on_round_trip() {
    let text = "- z: 1\n  a: 2\n  m: 3\n";
    let parsed = yaml::parse("k.yaml", text).unwrap();
    assert_eq!(keys_in_order(&parsed.records[0]), vec!["z", "a", "m"]);

    let schema_keys = vec!["z".into(), "a".into(), "m".into()];
    let out = yaml::serialize(&parsed.records, &schema_keys).unwrap();
    let reparsed = yaml::parse("k.yaml", &out).unwrap();
    assert_eq!(keys_in_order(&reparsed.records[0]), vec!["z", "a", "m"]);
}

#[test]
fn markdown_preserves_frontmatter_key_order_on_round_trip() {
    let text = "---\nz: 1\na: 2\nm: 3\n---\nbody\n";
    let parsed = markdown::parse("k.md", text).unwrap();
    assert_eq!(keys_in_order(&parsed.record), vec!["z", "a", "m"]);

    let schema_keys = vec!["z".into(), "a".into(), "m".into()];
    let out = markdown::serialize(&parsed.record, &parsed.body, &schema_keys, LineEnding::Lf).unwrap();
    let reparsed = markdown::parse("k.md", &out).unwrap();
    assert_eq!(keys_in_order(&reparsed.record), vec!["z", "a", "m"]);
}

#[test]
fn json_appends_new_field_after_existing_keys() {
    let text = r#"[{"z": 1, "a": 2, "m": 3}]"#;
    let parsed = json::parse("k.json", text).unwrap();
    let schema_keys: Vec<String> = vec!["z".into(), "a".into(), "m".into(), "x".into()];
    let out = json::serialize(&parsed.records, &schema_keys, parsed.indent).unwrap();
    let reparsed = json::parse("k.json", &out).unwrap();
    assert_eq!(
        keys_in_order(&reparsed.records[0]),
        vec!["z", "a", "m", "x"],
        "new column `x` must append after existing keys"
    );
    assert_eq!(reparsed.records[0].fields.get("x"), Some(&Value::Null));
}

#[test]
fn yaml_appends_new_field_after_existing_keys() {
    let text = "- z: 1\n  a: 2\n  m: 3\n";
    let parsed = yaml::parse("k.yaml", text).unwrap();
    let schema_keys: Vec<String> = vec!["z".into(), "a".into(), "m".into(), "x".into()];
    let out = yaml::serialize(&parsed.records, &schema_keys).unwrap();
    let reparsed = yaml::parse("k.yaml", &out).unwrap();
    assert_eq!(
        keys_in_order(&reparsed.records[0]),
        vec!["z", "a", "m", "x"]
    );
    assert_eq!(reparsed.records[0].fields.get("x"), Some(&Value::Null));
}

#[test]
fn markdown_appends_new_field_after_existing_keys() {
    let text = "---\nz: 1\na: 2\nm: 3\n---\nbody\n";
    let parsed = markdown::parse("k.md", text).unwrap();
    let schema_keys: Vec<String> = vec!["z".into(), "a".into(), "m".into(), "x".into()];
    let out = markdown::serialize(&parsed.record, &parsed.body, &schema_keys, LineEnding::Lf).unwrap();
    let reparsed = markdown::parse("k.md", &out).unwrap();
    assert_eq!(
        keys_in_order(&reparsed.record),
        vec!["z", "a", "m", "x"]
    );
}

#[test]
fn empty_columns_warning_lists_keys_with_no_values() {
    let dir = temp_dir("empty-cols");
    fs::write(
        dir.join("a.md"),
        "---\ntitle: A\nlegacy_field:\n---\nbody\n",
    )
    .unwrap();
    fs::write(
        dir.join("b.md"),
        "---\ntitle: B\nlegacy_field:\n---\nbody\n",
    )
    .unwrap();

    let p1 = markdown::parse(
        dir.join("a.md"),
        &fs::read_to_string(dir.join("a.md")).unwrap(),
    )
    .unwrap();
    let p2 = markdown::parse(
        dir.join("b.md"),
        &fs::read_to_string(dir.join("b.md")).unwrap(),
    )
    .unwrap();
    let records = [p1.record, p2.record];
    let schema = Schema::infer(&records);
    let empties = schema.empty_columns(&records);
    assert_eq!(empties, vec!["legacy_field".to_string()]);

    let _ = fs::remove_dir_all(&dir);
}
