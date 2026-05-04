use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use super::Value;

/// A single record in the table — an ordered map of field name → value.
///
/// Order is preserved from the source file. New fields are appended.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Record {
    pub fields: IndexMap<String, Value>,
}

impl Record {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, key: &str) -> Option<&Value> {
        self.fields.get(key)
    }

    pub fn insert(&mut self, key: impl Into<String>, value: Value) {
        self.fields.insert(key.into(), value);
    }

    /// Ensure every key in `schema_keys` is present, filling missing ones with `Value::Null`.
    /// Existing key order is preserved; new keys are appended in `schema_keys` order.
    pub fn fill_missing(&mut self, schema_keys: &[String]) {
        for k in schema_keys {
            if !self.fields.contains_key(k) {
                self.fields.insert(k.clone(), Value::Null);
            }
        }
    }

    /// Remove a key from the record entirely.
    pub fn remove(&mut self, key: &str) -> Option<Value> {
        self.fields.shift_remove(key)
    }
}
