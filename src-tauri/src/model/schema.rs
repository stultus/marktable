use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use super::{FieldType, Record};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Column {
    pub name: String,
    #[serde(rename = "type")]
    pub field_type: FieldType,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Schema {
    pub columns: Vec<Column>,
}

impl Schema {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn keys(&self) -> Vec<String> {
        self.columns.iter().map(|c| c.name.clone()).collect()
    }

    /// Build a unified schema across `records`.
    ///
    /// Column order: the first time a key is seen, it is appended.
    /// This means the union of keys is in first-seen order across all records.
    /// Type per column: inferred from the majority of non-empty values.
    pub fn infer(records: &[Record]) -> Self {
        let mut order: Vec<String> = Vec::new();
        let mut by_key: IndexMap<String, Vec<&super::Value>> = IndexMap::new();
        for record in records {
            for (k, v) in &record.fields {
                if !by_key.contains_key(k) {
                    order.push(k.clone());
                    by_key.insert(k.clone(), Vec::new());
                }
                by_key.get_mut(k).unwrap().push(v);
            }
        }
        let columns = order
            .into_iter()
            .map(|name| {
                let values = by_key.get(&name).cloned().unwrap_or_default();
                let field_type = FieldType::infer(&values);
                Column { name, field_type }
            })
            .collect();
        Self { columns }
    }

    /// Names of columns where every value across all records is empty.
    pub fn empty_columns(&self, records: &[Record]) -> Vec<String> {
        self.columns
            .iter()
            .filter(|c| {
                records.iter().all(|r| match r.fields.get(&c.name) {
                    None => true,
                    Some(v) => v.is_empty(),
                })
            })
            .map(|c| c.name.clone())
            .collect()
    }
}
