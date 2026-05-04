use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};

use super::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FieldType {
    Text,
    Number,
    Boolean,
    Date,
    List,
    Raw,
}

static ISO_DATE: Lazy<Regex> = Lazy::new(|| {
    // YYYY-MM-DD or YYYY-MM-DDTHH:MM:SS(...)
    Regex::new(r"^\d{4}-\d{2}-\d{2}([T ]\d{2}:\d{2}(:\d{2})?(\.\d+)?(Z|[+-]\d{2}:?\d{2})?)?$")
        .unwrap()
});

pub fn looks_like_iso_date(s: &str) -> bool {
    ISO_DATE.is_match(s)
}

impl FieldType {
    /// Infer the field type for a column from a slice of values.
    /// Strategy: take the majority type among non-empty values; ties go to `Text`.
    pub fn infer(values: &[&Value]) -> FieldType {
        let mut counts = TypeCounts::default();
        for v in values {
            if v.is_empty() {
                continue;
            }
            counts.observe(v);
        }
        counts.dominant()
    }
}

#[derive(Default)]
struct TypeCounts {
    boolean: usize,
    number: usize,
    date: usize,
    list: usize,
    raw: usize,
    text: usize,
}

impl TypeCounts {
    fn observe(&mut self, v: &Value) {
        match v {
            Value::Bool(_) => self.boolean += 1,
            Value::Number(_) => self.number += 1,
            Value::Date(_) => self.date += 1,
            Value::List(_) => self.list += 1,
            Value::Raw(_) => self.raw += 1,
            Value::String(s) => {
                if looks_like_iso_date(s) {
                    self.date += 1;
                } else {
                    self.text += 1;
                }
            }
            Value::Null => {}
        }
    }

    fn dominant(&self) -> FieldType {
        let scored = [
            (FieldType::Boolean, self.boolean),
            (FieldType::Number, self.number),
            (FieldType::Date, self.date),
            (FieldType::List, self.list),
            (FieldType::Raw, self.raw),
            (FieldType::Text, self.text),
        ];
        scored
            .iter()
            .max_by_key(|(_, c)| *c)
            .filter(|(_, c)| *c > 0)
            .map(|(t, _)| *t)
            .unwrap_or(FieldType::Text)
    }
}
