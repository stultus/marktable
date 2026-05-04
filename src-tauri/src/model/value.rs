use serde::{Deserialize, Serialize};

/// A single cell value in the unified table model.
///
/// `Null` represents an empty cell — distinct from a missing key.
/// `Raw` is for complex nested values (objects, arrays of mixed types) that
/// MarkTable does not understand structurally; they round-trip as text.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "lowercase")]
pub enum Value {
    Null,
    Bool(bool),
    Number(serde_json::Number),
    String(String),
    Date(String),
    List(Vec<Value>),
    Raw(String),
}

impl Value {
    pub fn is_empty(&self) -> bool {
        match self {
            Value::Null => true,
            Value::String(s) => s.is_empty(),
            Value::List(items) => items.is_empty(),
            Value::Raw(s) => s.is_empty(),
            _ => false,
        }
    }
}

impl From<&serde_json::Value> for Value {
    fn from(v: &serde_json::Value) -> Self {
        match v {
            serde_json::Value::Null => Value::Null,
            serde_json::Value::Bool(b) => Value::Bool(*b),
            serde_json::Value::Number(n) => Value::Number(n.clone()),
            serde_json::Value::String(s) => {
                if crate::model::field_type::looks_like_iso_date(s) {
                    Value::Date(s.clone())
                } else {
                    Value::String(s.clone())
                }
            }
            serde_json::Value::Array(arr) => {
                if arr.iter().all(is_scalar_json) {
                    Value::List(arr.iter().map(Value::from).collect())
                } else {
                    Value::Raw(serde_json::to_string(v).unwrap_or_default())
                }
            }
            serde_json::Value::Object(_) => {
                Value::Raw(serde_json::to_string(v).unwrap_or_default())
            }
        }
    }
}

impl From<&serde_yaml::Value> for Value {
    fn from(v: &serde_yaml::Value) -> Self {
        match v {
            serde_yaml::Value::Null => Value::Null,
            serde_yaml::Value::Bool(b) => Value::Bool(*b),
            serde_yaml::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Value::Number(serde_json::Number::from(i))
                } else if let Some(u) = n.as_u64() {
                    Value::Number(serde_json::Number::from(u))
                } else if let Some(f) = n.as_f64() {
                    serde_json::Number::from_f64(f)
                        .map(Value::Number)
                        .unwrap_or(Value::Null)
                } else {
                    Value::Null
                }
            }
            serde_yaml::Value::String(s) => {
                if crate::model::field_type::looks_like_iso_date(s) {
                    Value::Date(s.clone())
                } else {
                    Value::String(s.clone())
                }
            }
            serde_yaml::Value::Sequence(seq) => {
                if seq.iter().all(is_scalar_yaml) {
                    Value::List(seq.iter().map(Value::from).collect())
                } else {
                    Value::Raw(serde_yaml::to_string(v).unwrap_or_default().trim_end().to_string())
                }
            }
            serde_yaml::Value::Mapping(_) | serde_yaml::Value::Tagged(_) => {
                Value::Raw(serde_yaml::to_string(v).unwrap_or_default().trim_end().to_string())
            }
        }
    }
}

fn is_scalar_json(v: &serde_json::Value) -> bool {
    matches!(
        v,
        serde_json::Value::Null
            | serde_json::Value::Bool(_)
            | serde_json::Value::Number(_)
            | serde_json::Value::String(_)
    )
}

fn is_scalar_yaml(v: &serde_yaml::Value) -> bool {
    matches!(
        v,
        serde_yaml::Value::Null
            | serde_yaml::Value::Bool(_)
            | serde_yaml::Value::Number(_)
            | serde_yaml::Value::String(_)
    )
}

impl Value {
    pub fn to_json_value(&self) -> serde_json::Value {
        match self {
            Value::Null => serde_json::Value::Null,
            Value::Bool(b) => serde_json::Value::Bool(*b),
            Value::Number(n) => serde_json::Value::Number(n.clone()),
            Value::String(s) | Value::Date(s) => serde_json::Value::String(s.clone()),
            Value::List(items) => {
                serde_json::Value::Array(items.iter().map(Value::to_json_value).collect())
            }
            Value::Raw(s) => serde_json::from_str(s)
                .unwrap_or_else(|_| serde_json::Value::String(s.clone())),
        }
    }

    pub fn to_yaml_value(&self) -> serde_yaml::Value {
        match self {
            Value::Null => serde_yaml::Value::Null,
            Value::Bool(b) => serde_yaml::Value::Bool(*b),
            Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    serde_yaml::Value::Number(i.into())
                } else if let Some(u) = n.as_u64() {
                    serde_yaml::Value::Number(u.into())
                } else if let Some(f) = n.as_f64() {
                    serde_yaml::Value::Number(f.into())
                } else {
                    serde_yaml::Value::Null
                }
            }
            Value::String(s) | Value::Date(s) => serde_yaml::Value::String(s.clone()),
            Value::List(items) => {
                serde_yaml::Value::Sequence(items.iter().map(Value::to_yaml_value).collect())
            }
            Value::Raw(s) => serde_yaml::from_str(s)
                .unwrap_or_else(|_| serde_yaml::Value::String(s.clone())),
        }
    }
}
