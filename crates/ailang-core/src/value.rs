/// Core Value type — everything in ailang pipelines is a Value.
/// Designed to be cheap to clone for pipeline streaming.
use serde::{Deserialize, Serialize};
use indexmap::IndexMap;

/// Insertion-ordered object map (deterministic output for agents).
pub type Map = IndexMap<String, Value>;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    List(Vec<Value>),
    Object(Map),
    /// A file entry produced by glob/walk
    FileEntry(FileEntry),
    /// A text line with source metadata
    Line(LineEntry),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileEntry {
    pub path: String,
    pub name: String,
    pub ext: String,
    pub size: u64,
    pub is_dir: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LineEntry {
    pub path: String,
    pub line: String,
    pub num: usize,
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Null => write!(f, "null"),
            Value::Bool(b) => write!(f, "{b}"),
            Value::Int(i) => write!(f, "{i}"),
            Value::Float(fl) => write!(f, "{fl}"),
            Value::Str(s) => write!(f, "{s}"),
            Value::FileEntry(fe) => write!(f, "{}", fe.path),
            Value::Line(le) => write!(f, "{}", le.line),
            other => write!(
                f,
                "{}",
                serde_json::to_string(&value_to_json(other)).unwrap_or_default()
            ),
        }
    }
}

impl Value {
    /// Build an object from key/value pairs.
    pub fn obj(pairs: Vec<(&str, Value)>) -> Value {
        Value::Object(pairs.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
    }

    /// Get a single field (no dots).
    pub fn get_field(&self, field: &str) -> Value {
        match self {
            Value::Object(map) => map.get(field).cloned().unwrap_or(Value::Null),
            Value::List(items) => field
                .parse::<usize>()
                .ok()
                .and_then(|i| items.get(i).cloned())
                .unwrap_or(Value::Null),
            Value::FileEntry(fe) => match field {
                "path" => Value::Str(fe.path.clone()),
                "name" => Value::Str(fe.name.clone()),
                "ext" => Value::Str(fe.ext.clone()),
                "size" => Value::Int(fe.size as i64),
                "is_dir" => Value::Bool(fe.is_dir),
                _ => Value::Null,
            },
            Value::Line(le) => match field {
                "path" => Value::Str(le.path.clone()),
                "line" => Value::Str(le.line.clone()),
                "num" => Value::Int(le.num as i64),
                _ => Value::Null,
            },
            _ => Value::Null,
        }
    }

    /// Get a dotted path: "a.b.c"
    pub fn get_path(&self, path: &str) -> Value {
        path.split('.')
            .filter(|s| !s.is_empty())
            .fold(self.clone(), |v, p| v.get_field(p))
    }

    /// Coerce to boolean for filter expressions
    pub fn is_truthy(&self) -> bool {
        match self {
            Value::Null => false,
            Value::Bool(b) => *b,
            Value::Int(i) => *i != 0,
            Value::Float(f) => *f != 0.0,
            Value::Str(s) => !s.is_empty(),
            Value::List(v) => !v.is_empty(),
            Value::Object(m) => !m.is_empty(),
            Value::FileEntry(_) => true,
            Value::Line(le) => !le.line.is_empty(),
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s.as_str()),
            _ => None,
        }
    }

    pub fn as_int(&self) -> Option<i64> {
        match self {
            Value::Int(i) => Some(*i),
            Value::Float(f) => Some(*f as i64),
            Value::Str(s) => s.parse().ok(),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Int(i) => Some(*i as f64),
            Value::Float(f) => Some(*f),
            Value::Str(s) => s.parse().ok(),
            _ => None,
        }
    }
}

// ── JSON ↔ Value conversion ─────────────────────────────────────────────────

pub fn value_to_json(v: &Value) -> serde_json::Value {
    match v {
        Value::Null => serde_json::Value::Null,
        Value::Bool(b) => serde_json::Value::Bool(*b),
        Value::Int(i) => serde_json::json!(i),
        Value::Float(f) => serde_json::json!(f),
        Value::Str(s) => serde_json::Value::String(s.clone()),
        Value::List(v) => serde_json::Value::Array(v.iter().map(value_to_json).collect()),
        Value::Object(m) => {
            let map: serde_json::Map<_, _> = m
                .iter()
                .map(|(k, v)| (k.clone(), value_to_json(v)))
                .collect();
            serde_json::Value::Object(map)
        }
        Value::FileEntry(fe) => serde_json::json!({
            "path": fe.path,
            "name": fe.name,
            "ext": fe.ext,
            "size": fe.size,
            "is_dir": fe.is_dir,
        }),
        Value::Line(le) => serde_json::json!({
            "path": le.path,
            "line": le.line,
            "num": le.num,
        }),
    }
}

pub fn json_to_value(v: serde_json::Value) -> Value {
    match v {
        serde_json::Value::Null => Value::Null,
        serde_json::Value::Bool(b) => Value::Bool(b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Value::Int(i)
            } else {
                Value::Float(n.as_f64().unwrap_or(0.0))
            }
        }
        serde_json::Value::String(s) => Value::Str(s),
        serde_json::Value::Array(arr) => Value::List(arr.into_iter().map(json_to_value).collect()),
        serde_json::Value::Object(map) => {
            Value::Object(map.into_iter().map(|(k, v)| (k, json_to_value(v))).collect())
        }
    }
}
