//! Recent-items list — persists last-opened folder/file paths to a small JSON
//! store under the OS app-data dir. The list is most-recent-first, deduplicated
//! by absolute path, and capped at MAX_RECENTS entries.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::error::{Error, Result};

const MAX_RECENTS: usize = 8;
const STORE_FILE: &str = "recents.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecentKind {
    Folder,
    JsonFile,
    YamlFile,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentItem {
    pub path: PathBuf,
    pub kind: RecentKind,
    /// Whether the path still exists on disk. Computed at read time so the
    /// frontend can render missing items as muted.
    #[serde(default)]
    pub exists: bool,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Store {
    items: Vec<StoredItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredItem {
    path: PathBuf,
    kind: RecentKind,
}

fn store_path(app: &AppHandle) -> Result<PathBuf> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| Error::Other(format!("app_data_dir: {e}")))?;
    if !dir.exists() {
        std::fs::create_dir_all(&dir).map_err(|e| Error::io(&dir, e))?;
    }
    Ok(dir.join(STORE_FILE))
}

fn read_store(app: &AppHandle) -> Result<Store> {
    let path = store_path(app)?;
    if !path.exists() {
        return Ok(Store::default());
    }
    let text = std::fs::read_to_string(&path).map_err(|e| Error::io(&path, e))?;
    serde_json::from_str::<Store>(&text)
        .map_err(|e| Error::Other(format!("recents store: {e}")))
}

fn write_store(app: &AppHandle, store: &Store) -> Result<()> {
    let path = store_path(app)?;
    let text = serde_json::to_string_pretty(store)
        .map_err(|e| Error::Other(format!("recents serialize: {e}")))?;
    std::fs::write(&path, text).map_err(|e| Error::io(&path, e))?;
    Ok(())
}

fn paths_eq(a: &Path, b: &Path) -> bool {
    a == b
}

#[tauri::command]
pub fn get_recents(app: AppHandle) -> Result<Vec<RecentItem>> {
    let store = read_store(&app)?;
    let items = store
        .items
        .into_iter()
        .map(|i| RecentItem {
            exists: i.path.exists(),
            path: i.path,
            kind: i.kind,
        })
        .collect();
    Ok(items)
}

#[tauri::command]
pub fn add_recent(app: AppHandle, path: String, kind: RecentKind) -> Result<()> {
    let new_path = PathBuf::from(&path);
    let mut store = read_store(&app).unwrap_or_default();
    store.items.retain(|i| !paths_eq(&i.path, &new_path));
    store.items.insert(
        0,
        StoredItem {
            path: new_path,
            kind,
        },
    );
    if store.items.len() > MAX_RECENTS {
        store.items.truncate(MAX_RECENTS);
    }
    write_store(&app, &store)
}

#[tauri::command]
pub fn remove_recent(app: AppHandle, path: String) -> Result<()> {
    let target = PathBuf::from(&path);
    let mut store = read_store(&app).unwrap_or_default();
    store.items.retain(|i| !paths_eq(&i.path, &target));
    write_store(&app, &store)
}
