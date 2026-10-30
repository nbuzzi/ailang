/// Core Value type — everything in ailang pipelines is a Value.
/// Designed to be cheap to clone for pipeline streaming.
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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
    Object(HashMap<String, Value>),
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
            Value::List(v) => {
                write!(f, "{}", serde_json::to_string(v).unwrap_or_default())
            }
            Value::Object(m) => {
                write!(f, "{}", serde_json::to_string(m).unwrap_or_default())
            }
            Value::FileEntry(fe) => write!(f, "{}", fe.path),
            Value::Line(le) => write!(f, "{}", le.line),
        }
    }
}

impl Value {
    /// Get a field by dotted path (.field or .nested.field)
    pub fn get_field(&self, field: &str) -> Value {
        match self {
            Value::Object(map) => map.get(field).cloned().unwrap_or(Value::Null),
            Value::FileEntry(fe) => match field {
                "path" => Value::Str(fe.path.clone()),
                "name" => Value::Str(fe.name.clone()),
                "ext"  => Value::Str(fe.ext.clone()),
                "size" => Value::Int(fe.size as i64),
                "is_dir" => Value::Bool(fe.is_dir),
                _ => Value::Null,
            },
            Value::Line(le) => match field {
                "path" => Value::Str(le.path.clone()),
                "line" => Value::Str(le.line.clone()),
                "num"  => Value::Int(le.num as i64),
                _ => Value::Null,
            },
            _ => Value::Null,
        }
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

    /// Try to coerce to string
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s.as_str()),
            _ => None,
        }
    }

    /// Try to coerce to i64
    pub fn as_int(&self) -> Option<i64> {
        match self {
            Value::Int(i) => Some(*i),
            Value::Float(f) => Some(*f as i64),
            Value::Str(s) => s.parse().ok(),
            _ => None,
        }
    }
}
