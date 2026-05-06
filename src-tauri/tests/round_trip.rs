//! End-to-end round-trip tests on real files in a temp directory.
//!
//! These cover the markdown folder mode: open a folder, the schema is inferred,
//! we serialize each row back, and the on-disk text matches what we'd get from
//! reparsing.

use std::fs;
use std::path::PathBuf;

use marktable_lib::commands::save_all;
use marktable_lib::formats::{json, markdown, yaml, LineEnding};
use marktable_lib::model::{
    Record, Row, RowSource, Schema, TableMode, TableModel, Value,
};

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

// ─── End-to-end round-trip on real files ─────────────────────────────────────────────────

#[test]
fn json_round_trip_byte_identity_no_user_edits() {
    // No edits — just open → serialize. With detected indent reproduced and
    // preserved key order, we expect byte-identical output for clean input.
    let text = "[\n  {\n    \"title\": \"A\",\n    \"rating\": 5\n  },\n  {\n    \"title\": \"B\",\n    \"rating\": 3\n  }\n]\n";
    let parsed = json::parse("rt.json", text).unwrap();
    let schema = Schema::infer(&parsed.records);
    let out = json::serialize(&parsed.records, &schema.keys(), parsed.indent).unwrap();
    assert_eq!(out, text, "untouched JSON must round-trip byte-for-byte");
}

#[test]
fn json_round_trip_preserves_4space_indent() {
    let text = "[\n    {\n        \"a\": 1,\n        \"b\": \"x\"\n    }\n]\n";
    let parsed = json::parse("rt.json", text).unwrap();
    let schema = Schema::infer(&parsed.records);
    let out = json::serialize(&parsed.records, &schema.keys(), parsed.indent).unwrap();
    assert_eq!(out, text);
}

#[test]
fn json_round_trip_preserves_tab_indent() {
    let text = "[\n\t{\n\t\t\"a\": 1,\n\t\t\"b\": \"x\"\n\t}\n]\n";
    let parsed = json::parse("rt.json", text).unwrap();
    let schema = Schema::infer(&parsed.records);
    let out = json::serialize(&parsed.records, &schema.keys(), parsed.indent).unwrap();
    assert_eq!(out, text);
}

#[test]
fn yaml_round_trip_byte_identity_no_comments() {
    // For files without comments and using serde_yaml's canonical layout, an
    // open → serialize round-trip should be byte-identical.
    let text = "- name: Alice\n  age: 30\n- name: Bob\n  age: 25\n";
    let parsed = yaml::parse("rt.yaml", text).unwrap();
    assert!(!parsed.had_comments);
    let schema = Schema::infer(&parsed.records);
    let out = yaml::serialize(&parsed.records, &schema.keys()).unwrap();
    assert_eq!(out, text, "untouched comment-free YAML must round-trip byte-for-byte");
}

#[test]
#[ignore = "blocked on #1: YAML comment preservation"]
fn yaml_round_trip_preserves_comments() {
    // Until span-aware YAML rewrite lands, comments are dropped. This test
    // codifies the post-#1 acceptance criterion.
    let text = "# header comment\n- name: Alice  # inline\n  age: 30\n";
    let parsed = yaml::parse("rt.yaml", text).unwrap();
    assert!(parsed.had_comments);
    let schema = Schema::infer(&parsed.records);
    let out = yaml::serialize(&parsed.records, &schema.keys()).unwrap();
    assert!(out.contains("# header comment"), "header comment dropped");
    assert!(out.contains("# inline"), "inline comment dropped");
}

// ─── Filename rename + Save All ordering (#4) ────────────────────────────────────────────

#[test]
fn folder_save_renames_after_writing_data() {
    let dir = temp_dir("rename-after-write");

    let f = dir.join("draft.md");
    let original_text = "---\ntitle: Draft\n---\nBody.\n";
    fs::write(&f, original_text).unwrap();

    let parsed = markdown::parse(&f, original_text).unwrap();
    let mut record = parsed.record.clone();
    record
        .fields
        .insert("title".into(), Value::String("Final".into()));
    let schema = Schema::infer(&[record.clone()]);

    let model = TableModel {
        mode: TableMode::Folder { path: dir.clone() },
        schema,
        rows: vec![Row {
            id: f.to_string_lossy().to_string(),
            source: RowSource::File {
                path: f.clone(),
                original_text: original_text.into(),
            },
            record,
            parse_error: None,
            pending_delete: false,
            dirty: true, // edited title above
            pending_rename: Some("final.md".into()),
        }],
        warnings: vec![],
    };

    let result = save_all(model).unwrap();
    assert!(result.failures.is_empty(), "no failures expected: {:?}", result.failures);

    // Old name is gone, new name has the new content.
    assert!(!f.exists(), "draft.md should have been renamed away");
    let new_path = dir.join("final.md");
    assert!(new_path.exists(), "final.md should exist after rename");
    let after = fs::read_to_string(&new_path).unwrap();
    assert!(after.contains("title: Final"), "new file must reflect data write; got:\n{after}");

    // SaveResult tracks the rename for the frontend.
    assert_eq!(result.renamed.len(), 1);
    assert_eq!(result.renamed[0].from, f);
    assert_eq!(result.renamed[0].to, new_path);

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn folder_save_rename_collision_fails_per_file() {
    let dir = temp_dir("rename-collision");

    let a = dir.join("a.md");
    let b = dir.join("b.md");
    fs::write(&a, "---\nx: 1\n---\nA\n").unwrap();
    fs::write(&b, "---\nx: 2\n---\nB\n").unwrap();

    let pa = markdown::parse(&a, "---\nx: 1\n---\nA\n").unwrap();
    let pb = markdown::parse(&b, "---\nx: 2\n---\nB\n").unwrap();
    let schema = Schema::infer(&[pa.record.clone(), pb.record.clone()]);

    // Try to rename a.md → b.md (already exists). The rename must fail
    // with a per-file failure; b.md must remain intact.
    let model = TableModel {
        mode: TableMode::Folder { path: dir.clone() },
        schema,
        rows: vec![
            Row {
                id: a.to_string_lossy().to_string(),
                source: RowSource::File {
                    path: a.clone(),
                    original_text: "---\nx: 1\n---\nA\n".into(),
                },
                record: pa.record,
                parse_error: None,
                pending_delete: false,
                dirty: false,
                pending_rename: Some("b.md".into()),
            },
            Row {
                id: b.to_string_lossy().to_string(),
                source: RowSource::File {
                    path: b.clone(),
                    original_text: "---\nx: 2\n---\nB\n".into(),
                },
                record: pb.record,
                parse_error: None,
                pending_delete: false,
                dirty: false,
                pending_rename: None,
            },
        ],
        warnings: vec![],
    };

    let result = save_all(model).unwrap();
    assert_eq!(result.failures.len(), 1);
    assert!(
        result.failures[0].message.contains("already exists"),
        "expected collision failure; got: {:?}",
        result.failures[0]
    );
    assert!(result.renamed.is_empty());
    assert!(a.exists(), "a.md must still exist after failed rename");
    assert!(b.exists(), "b.md must still exist after failed rename");

    let _ = fs::remove_dir_all(&dir);
}

// ─── Save All only writes dirty files in folder mode (#17) ───────────────────────────────

#[test]
fn folder_save_skips_clean_files() {
    let dir = temp_dir("dirty-flag");

    // Two files. We'll edit one and leave the other untouched, then assert
    // that only the edited file's bytes change.
    let clean_path = dir.join("clean.md");
    let clean_text = "---\ntitle: Clean\nrating: 5\n---\nThis body must not change.\n";
    fs::write(&clean_path, clean_text).unwrap();

    let dirty_path = dir.join("dirty.md");
    let dirty_text = "---\ntitle: Old\nrating: 3\n---\nBody.\n";
    fs::write(&dirty_path, dirty_text).unwrap();

    // Build the model the way open_folder would have, then mutate one row.
    let p_clean = markdown::parse(&clean_path, clean_text).unwrap();
    let p_dirty = markdown::parse(&dirty_path, dirty_text).unwrap();
    let schema = Schema::infer(&[p_clean.record.clone(), p_dirty.record.clone()]);

    let mut dirty_record = p_dirty.record.clone();
    dirty_record
        .fields
        .insert("title".into(), Value::String("New".into()));

    let model = TableModel {
        mode: TableMode::Folder { path: dir.clone() },
        schema,
        rows: vec![
            Row {
                id: clean_path.to_string_lossy().to_string(),
                source: RowSource::File {
                    path: clean_path.clone(),
                    original_text: clean_text.into(),
                },
                record: p_clean.record,
                parse_error: None,
                pending_delete: false,
                dirty: false,
                pending_rename: None,
            },
            Row {
                id: dirty_path.to_string_lossy().to_string(),
                source: RowSource::File {
                    path: dirty_path.clone(),
                    original_text: dirty_text.into(),
                },
                record: dirty_record,
                parse_error: None,
                pending_delete: false,
                dirty: true,
                pending_rename: None,
            },
        ],
        warnings: vec![],
    };

    let result = save_all(model).unwrap();
    assert!(result.failures.is_empty(), "no save failures expected");

    // The clean file's bytes must be byte-identical to the original.
    let after_clean = fs::read_to_string(&clean_path).unwrap();
    assert_eq!(
        after_clean, clean_text,
        "clean.md must not be touched on Save All"
    );
    assert_eq!(
        result.written.iter().any(|p| p == &clean_path),
        false,
        "clean.md must not appear in written list"
    );

    // The dirty file's bytes must reflect the edit (title: New).
    let after_dirty = fs::read_to_string(&dirty_path).unwrap();
    assert!(
        after_dirty.contains("title: New"),
        "dirty.md should have new title; got:\n{after_dirty}"
    );
    assert!(
        result.written.iter().any(|p| p == &dirty_path),
        "dirty.md must appear in written list"
    );

    let _ = fs::remove_dir_all(&dir);
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
