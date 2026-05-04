use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::{Record, Schema};

/// How a row maps back to its source.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RowSource {
    /// Folder mode: each row is one file.
    File {
        path: PathBuf,
        /// Original on-disk text for the file. Used for round-trip preservation
        /// (markdown body, YAML comments, etc.).
        original_text: String,
    },
    /// Single-file mode: each row is one item in the array/list.
    Inline {
        index: usize,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Row {
    pub id: String,
    pub source: RowSource,
    pub record: Record,
    /// If parsing failed, the row is shown read-only with this error.
    pub parse_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TableMode {
    /// Folder of `.md` files.
    Folder { path: PathBuf },
    /// Single `.json` array file.
    JsonFile {
        path: PathBuf,
        original_text: String,
    },
    /// Single `.yaml` / `.yml` list file.
    YamlFile {
        path: PathBuf,
        original_text: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Warning {
    EmptyColumns { names: Vec<String> },
    UnparseableFile { path: PathBuf, message: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableModel {
    pub mode: TableMode,
    pub schema: Schema,
    pub rows: Vec<Row>,
    pub warnings: Vec<Warning>,
}
