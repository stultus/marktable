use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::formats::{json as json_fmt, markdown, yaml as yaml_fmt, LineEnding};
use crate::model::{Record, Row, RowSource, Schema, TableMode, TableModel, Warning};

#[tauri::command]
pub fn open_folder(path: String) -> Result<TableModel> {
    let path: PathBuf = PathBuf::from(&path);
    if !path.is_dir() {
        return Err(Error::Other(format!("{} is not a directory", path.display())));
    }

    let mut rows: Vec<Row> = Vec::new();
    let mut warnings: Vec<Warning> = Vec::new();
    let mut entries: Vec<PathBuf> = std::fs::read_dir(&path)
        .map_err(|e| Error::io(&path, e))?
        .filter_map(|r| r.ok())
        .map(|e| e.path())
        .filter(|p| p.is_file() && p.extension().is_some_and(|x| x == "md"))
        .collect();
    entries.sort();

    for file_path in entries {
        let text = match std::fs::read_to_string(&file_path) {
            Ok(t) => t,
            Err(e) => {
                warnings.push(Warning::UnparseableFile {
                    path: file_path.clone(),
                    message: format!("read error: {e}"),
                });
                continue;
            }
        };
        match markdown::parse(&file_path, &text) {
            Ok(parsed) => rows.push(Row {
                id: file_path.to_string_lossy().to_string(),
                source: RowSource::File {
                    path: file_path.clone(),
                    original_text: text,
                },
                record: parsed.record,
                parse_error: None,
                pending_delete: false,
            }),
            Err(e) => {
                let msg = e.to_string();
                warnings.push(Warning::UnparseableFile {
                    path: file_path.clone(),
                    message: msg.clone(),
                });
                rows.push(Row {
                    id: file_path.to_string_lossy().to_string(),
                    source: RowSource::File {
                        path: file_path.clone(),
                        original_text: text,
                    },
                    record: Record::new(),
                    parse_error: Some(msg),
                    pending_delete: false,
                });
            }
        }
    }

    let parseable: Vec<&Record> = rows
        .iter()
        .filter(|r| r.parse_error.is_none())
        .map(|r| &r.record)
        .collect();
    let owned: Vec<Record> = parseable.into_iter().cloned().collect();
    let schema = Schema::infer(&owned);
    let empty_cols = schema.empty_columns(&owned);
    if !empty_cols.is_empty() {
        warnings.push(Warning::EmptyColumns { names: empty_cols });
    }

    Ok(TableModel {
        mode: TableMode::Folder { path },
        schema,
        rows,
        warnings,
    })
}

#[tauri::command]
pub fn open_file(path: String) -> Result<TableModel> {
    let path: PathBuf = PathBuf::from(&path);
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();
    let text = std::fs::read_to_string(&path).map_err(|e| Error::io(&path, e))?;

    match ext.as_str() {
        "json" => {
            let parsed = json_fmt::parse(&path, &text)?;
            let rows = parsed
                .records
                .into_iter()
                .enumerate()
                .map(|(i, record)| Row {
                    id: format!("row-{i}"),
                    source: RowSource::Inline { index: i },
                    record,
                    parse_error: None,
                    pending_delete: false,
                })
                .collect::<Vec<_>>();
            let records: Vec<Record> = rows.iter().map(|r| r.record.clone()).collect();
            let schema = Schema::infer(&records);
            let mut warnings = Vec::new();
            let empty_cols = schema.empty_columns(&records);
            if !empty_cols.is_empty() {
                warnings.push(Warning::EmptyColumns { names: empty_cols });
            }
            Ok(TableModel {
                mode: TableMode::JsonFile {
                    path,
                    original_text: text,
                },
                schema,
                rows,
                warnings,
            })
        }
        "yaml" | "yml" => {
            let parsed = yaml_fmt::parse(&path, &text)?;
            let rows = parsed
                .records
                .into_iter()
                .enumerate()
                .map(|(i, record)| Row {
                    id: format!("row-{i}"),
                    source: RowSource::Inline { index: i },
                    record,
                    parse_error: None,
                    pending_delete: false,
                })
                .collect::<Vec<_>>();
            let records: Vec<Record> = rows.iter().map(|r| r.record.clone()).collect();
            let schema = Schema::infer(&records);
            let mut warnings = Vec::new();
            let empty_cols = schema.empty_columns(&records);
            if !empty_cols.is_empty() {
                warnings.push(Warning::EmptyColumns { names: empty_cols });
            }
            if parsed.had_comments {
                warnings.push(Warning::UnparseableFile {
                    path: path.clone(),
                    message:
                        "this YAML file contains comments; comment preservation is not yet implemented and Save All will drop them"
                            .to_string(),
                });
            }
            Ok(TableModel {
                mode: TableMode::YamlFile {
                    path,
                    original_text: text,
                },
                schema,
                rows,
                warnings,
            })
        }
        other => Err(Error::UnsupportedExtension(other.to_string())),
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SaveResult {
    pub written: Vec<PathBuf>,
    pub failures: Vec<SaveFailure>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SaveFailure {
    pub path: PathBuf,
    pub message: String,
}

#[tauri::command]
pub fn save_all(table: TableModel) -> Result<SaveResult> {
    let schema_keys = table.schema.keys();
    let mut written = Vec::new();
    let mut failures = Vec::new();

    match &table.mode {
        TableMode::Folder { .. } => {
            for row in &table.rows {
                let RowSource::File {
                    path,
                    original_text,
                } = &row.source
                else {
                    continue;
                };
                if row.pending_delete {
                    if path.exists() {
                        match std::fs::remove_file(path) {
                            Ok(()) => written.push(path.clone()),
                            Err(e) => failures.push(SaveFailure {
                                path: path.clone(),
                                message: format!("delete failed: {e}"),
                            }),
                        }
                    } else {
                        // New row that was never saved — nothing to do on disk.
                        written.push(path.clone());
                    }
                    continue;
                }
                if row.parse_error.is_some() {
                    continue;
                }
                let body = extract_body(original_text);
                let line_ending = LineEnding::detect(original_text);
                match markdown::serialize(&row.record, &body, &schema_keys, line_ending) {
                    Ok(out) => match write_file(path, &out) {
                        Ok(()) => written.push(path.clone()),
                        Err(e) => failures.push(SaveFailure {
                            path: path.clone(),
                            message: e.to_string(),
                        }),
                    },
                    Err(e) => failures.push(SaveFailure {
                        path: path.clone(),
                        message: e.to_string(),
                    }),
                }
            }
        }
        TableMode::JsonFile { path, .. } => {
            let records: Vec<Record> = table
                .rows
                .iter()
                .filter(|r| !r.pending_delete)
                .map(|r| r.record.clone())
                .collect();
            let indent = json_fmt::Indent::detect(
                &std::fs::read_to_string(path).unwrap_or_default(),
            );
            match json_fmt::serialize(&records, &schema_keys, indent) {
                Ok(out) => match write_file(path, &out) {
                    Ok(()) => written.push(path.clone()),
                    Err(e) => failures.push(SaveFailure {
                        path: path.clone(),
                        message: e.to_string(),
                    }),
                },
                Err(e) => failures.push(SaveFailure {
                    path: path.clone(),
                    message: e.to_string(),
                }),
            }
        }
        TableMode::YamlFile { path, .. } => {
            let records: Vec<Record> = table
                .rows
                .iter()
                .filter(|r| !r.pending_delete)
                .map(|r| r.record.clone())
                .collect();
            match yaml_fmt::serialize(&records, &schema_keys) {
                Ok(out) => match write_file(path, &out) {
                    Ok(()) => written.push(path.clone()),
                    Err(e) => failures.push(SaveFailure {
                        path: path.clone(),
                        message: e.to_string(),
                    }),
                },
                Err(e) => failures.push(SaveFailure {
                    path: path.clone(),
                    message: e.to_string(),
                }),
            }
        }
    }

    Ok(SaveResult { written, failures })
}

fn extract_body(original: &str) -> String {
    let trimmed = original.strip_prefix('\u{feff}').unwrap_or(original);
    if !(trimmed.starts_with("---\n") || trimmed.starts_with("---\r\n")) {
        return original.to_string();
    }
    let after_open = if trimmed.starts_with("---\r\n") {
        &trimmed[5..]
    } else {
        &trimmed[4..]
    };
    let mut cursor = 0usize;
    while cursor < after_open.len() {
        let line_end = after_open[cursor..]
            .find('\n')
            .map(|i| cursor + i)
            .unwrap_or(after_open.len());
        let line = &after_open[cursor..line_end];
        let line_no_cr = line.strip_suffix('\r').unwrap_or(line);
        if line_no_cr == "---" {
            let body_start = if line_end < after_open.len() {
                line_end + 1
            } else {
                after_open.len()
            };
            return after_open[body_start..].to_string();
        }
        if line_end >= after_open.len() {
            break;
        }
        cursor = line_end + 1;
    }
    String::new()
}

fn write_file(path: &Path, contents: &str) -> std::io::Result<()> {
    std::fs::write(path, contents)
}
