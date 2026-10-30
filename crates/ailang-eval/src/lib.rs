use ailang_core::{
    ast::*,
    error::{AilangError, AilangResult},
    Map, Value,
};
use ailang_stdlib as stdlib;
use std::collections::HashMap;

/// Execution context — holds variables set by `let` bindings
#[derive(Default, Clone)]
pub struct Context {
    pub vars: HashMap<String, Vec<Value>>,
}

impl Context {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(&mut self, name: &str, values: Vec<Value>) {
        self.vars.insert(name.to_string(), values);
    }

    pub fn get(&self, name: &str) -> Option<&Vec<Value>> {
        self.vars.get(name)
    }
}

/// Execute a parsed pipeline, returning the final output values.
pub fn eval_pipeline(
    pipeline: &Pipeline,
    ctx: &mut Context,
    rt: &tokio::runtime::Handle,
) -> AilangResult<Vec<Value>> {
    let mut stream: Vec<Value> = vec![];

    for (i, stage) in pipeline.stages.iter().enumerate() {
        let input = if i == 0 { None } else { collapse_stream(stream.clone()) };
        stream = eval_stage(stage, input, ctx, rt)?;
    }

    // Apply sink if present
    if let Some(sink) = &pipeline.sink {
        let content = match stream.as_slice() {
            [Value::Str(s)] => s.clone(),
            values => {
                let serialized: Vec<serde_json::Value> = values.iter()
                    .map(value_to_json)
                    .collect();
                serde_json::to_string(&serialized).unwrap_or_default()
            }
        };
        match sink {
            Sink::Write(path) => std::fs::write(path, content)?,
            Sink::Append(path) => {
                use std::io::Write;
                let mut f = std::fs::OpenOptions::new()
                    .create(true).append(true).open(path)?;
                writeln!(f, "{}", content)?;
            }
        }
        return Ok(vec![]);
    }

    Ok(stream)
}

fn eval_stage(
    stage: &Stage,
    input: Option<Value>,
    ctx: &mut Context,
    rt: &tokio::runtime::Handle,
) -> AilangResult<Vec<Value>> {
    match stage {
        Stage::Call(call) => eval_call(call, input, ctx, rt),

        Stage::Keyword(kw) => eval_keyword(kw.as_str(), input, rt),

        Stage::Filter(expr) => {
            let items = input_to_list(input);
            let mut results = Vec::new();
            for item in items {
                if eval_expr(expr, &item, ctx)?.is_truthy() {
                    results.push(item);
                }
            }
            Ok(results)
        }

        Stage::Map(map_expr) => {
            let items = input_to_list(input);
            let mut results = Vec::new();
            for item in items {
                let mut obj = Map::new();
                for (key, expr) in &map_expr.fields {
                    obj.insert(key.clone(), eval_expr(expr, &item, ctx)?);
                }
                results.push(Value::Object(obj));
            }
            Ok(results)
        }

        Stage::FieldAccess(field) => {
            let items = input_to_list(input);
            Ok(items.into_iter().map(|v| v.get_field(field)).collect())
        }

        Stage::Let(name, sub_pipeline) => {
            let values = eval_pipeline(sub_pipeline, ctx, rt)?;
            ctx.set(name, values);
            Ok(vec![])
        }

        Stage::Var(name) => {
            ctx.get(name)
                .cloned()
                .ok_or_else(|| AilangError::UnknownVariable(name.clone()))
        }
    }
}

fn eval_call(
    call: &Call,
    input: Option<Value>,
    ctx: &mut Context,
    rt: &tokio::runtime::Handle,
) -> AilangResult<Vec<Value>> {
    let arg_input = input.clone().unwrap_or(Value::Null);
    let args: Vec<Value> = call.args.iter()
        .map(|expr| eval_expr(expr, &arg_input, ctx))
        .collect::<AilangResult<Vec<_>>>()?;

    stdlib::dispatch(&call.name, &args, input, rt)
}

fn eval_keyword(
    kw: &str,
    input: Option<Value>,
    _rt: &tokio::runtime::Handle,
) -> AilangResult<Vec<Value>> {
    match kw {
        "json" => {
            match &input {
                Some(Value::Str(s)) => {
                    // Try to parse as JSON
                    let v: serde_json::Value = serde_json::from_str(s)
                        .map_err(|e| AilangError::RuntimeError {
                            stage: "json".into(),
                            msg: e.to_string(),
                        })?;
                    Ok(vec![json_to_value(v)])
                }
                Some(other) => {
                    // Serialize to JSON string
                    let json = serde_json::to_string(&value_to_json(other))
                        .map_err(|e| AilangError::RuntimeError {
                            stage: "json".into(),
                            msg: e.to_string(),
                        })?;
                    Ok(vec![Value::Str(json)])
                }
                None => Err(AilangError::RuntimeError {
                    stage: "json".into(),
                    msg: "json requires piped input".into(),
                }),
            }
        }

        "yaml" => {
            match &input {
                Some(Value::Str(s)) => {
                    let v: serde_json::Value = serde_yaml::from_str(s)
                        .map_err(|e| AilangError::RuntimeError {
                            stage: "yaml".into(),
                            msg: e.to_string(),
                        })?;
                    Ok(vec![json_to_value(v)])
                }
                Some(other) => {
                    let yaml = serde_yaml::to_string(&value_to_json(other))
                        .map_err(|e| AilangError::RuntimeError {
                            stage: "yaml".into(),
                            msg: e.to_string(),
                        })?;
                    Ok(vec![Value::Str(yaml)])
                }
                None => Err(AilangError::RuntimeError {
                    stage: "yaml".into(),
                    msg: "yaml requires piped input".into(),
                }),
            }
        }

        "csv" => {
            let content = match &input {
                Some(Value::Str(s)) => s.clone(),
                _ => return Err(AilangError::RuntimeError {
                    stage: "csv".into(),
                    msg: "csv requires string input".into(),
                }),
            };
            let mut reader = csv::Reader::from_reader(content.as_bytes());
            let headers: Vec<String> = reader.headers()
                .map_err(|e| AilangError::RuntimeError { stage: "csv".into(), msg: e.to_string() })?
                .iter().map(|s| s.to_string()).collect();

            let mut records = Vec::new();
            for result in reader.records() {
                let record = result.map_err(|e| AilangError::RuntimeError {
                    stage: "csv".into(), msg: e.to_string(),
                })?;
                let mut obj = Map::new();
                for (i, field) in record.iter().enumerate() {
                    if let Some(header) = headers.get(i) {
                        // Try to parse as number
                        let val = if let Ok(n) = field.parse::<i64>() {
                            Value::Int(n)
                        } else if let Ok(f) = field.parse::<f64>() {
                            Value::Float(f)
                        } else {
                            Value::Str(field.to_string())
                        };
                        obj.insert(header.clone(), val);
                    }
                }
                records.push(Value::Object(obj));
            }
            Ok(records)
        }

        "lines" => {
            let s = match input {
                Some(Value::Str(s)) => s,
                Some(other) => other.to_string(),
                None => return Ok(vec![]),
            };
            Ok(s.lines().map(|l| Value::Str(l.to_string())).collect())
        }

        "count" => {
            let n = match input {
                Some(Value::List(v)) => v.len(),
                Some(Value::Str(s)) => s.lines().count(),
                Some(_) => 1,
                None => 0,
            };
            Ok(vec![Value::Int(n as i64)])
        }

        "trim" => {
            match input {
                Some(Value::Str(s)) => Ok(vec![Value::Str(s.trim().to_string())]),
                Some(Value::List(v)) => Ok(v.into_iter().map(|item| {
                    if let Value::Str(s) = item { Value::Str(s.trim().to_string()) } else { item }
                }).collect()),
                other => Ok(other.into_iter().collect()),
            }
        }

        "upper" => {
            match input {
                Some(Value::Str(s)) => Ok(vec![Value::Str(s.to_uppercase())]),
                other => Ok(other.into_iter().collect()),
            }
        }

        "lower" => {
            match input {
                Some(Value::Str(s)) => Ok(vec![Value::Str(s.to_lowercase())]),
                other => Ok(other.into_iter().collect()),
            }
        }

        "sha256" => ailang_stdlib::crypto::sha256(input),
        "sha512" => ailang_stdlib::crypto::sha512(input),
        "md5"    => ailang_stdlib::crypto::md5(input),
        "unique" => ailang_stdlib::data::unique(input),

        "write" => {
            match input {
                Some(Value::FileEntry(fe)) => Ok(vec![Value::FileEntry(fe)]),
                Some(Value::Str(s)) => Ok(vec![Value::Str(s)]),
                other => Ok(other.into_iter().collect()),
            }
        }

        other => Err(AilangError::UnknownFunction(other.to_string())),
    }
}

/// Evaluate an expression against a current item value
pub fn eval_expr(expr: &Expr, current: &Value, ctx: &Context) -> AilangResult<Value> {
    match expr {
        Expr::Null       => Ok(Value::Null),
        Expr::Bool(b)    => Ok(Value::Bool(*b)),
        Expr::Int(i)     => Ok(Value::Int(*i)),
        Expr::Float(f)   => Ok(Value::Float(*f)),
        Expr::Str(s)     => Ok(Value::Str(s.clone())),

        Expr::List(items) => {
            let evaluated = items.iter()
                .map(|e| eval_expr(e, current, ctx))
                .collect::<AilangResult<Vec<_>>>()?;
            Ok(Value::List(evaluated))
        }

        Expr::Var(name) => {
            if let Some(vals) = ctx.get(name) {
                match vals.as_slice() {
                    [single] => Ok(single.clone()),
                    _ => Ok(Value::List(vals.clone())),
                }
            } else {
                Err(AilangError::UnknownVariable(name.clone()))
            }
        }

        Expr::Field(parts) => {
            let mut val = current.clone();
            for part in parts {
                val = val.get_field(part);
            }
            Ok(val)
        }

        Expr::FieldMethod(parts, method, args) => {
            let mut val = current.clone();
            for part in parts {
                val = val.get_field(part);
            }
            let val_str = val.to_string();
            match method.as_str() {
                "contains" => {
                    if let Some(Expr::Str(s)) = args.first() {
                        Ok(Value::Bool(val_str.contains(s.as_str())))
                    } else {
                        Ok(Value::Bool(false))
                    }
                }
                "starts_with" => {
                    if let Some(Expr::Str(s)) = args.first() {
                        Ok(Value::Bool(val_str.starts_with(s.as_str())))
                    } else {
                        Ok(Value::Bool(false))
                    }
                }
                "ends_with" => {
                    if let Some(Expr::Str(s)) = args.first() {
                        Ok(Value::Bool(val_str.ends_with(s.as_str())))
                    } else {
                        Ok(Value::Bool(false))
                    }
                }
                other => Err(AilangError::UnknownFunction(format!(".{}", other))),
            }
        }

        Expr::Not(inner) => {
            let val = eval_expr(inner, current, ctx)?;
            Ok(Value::Bool(!val.is_truthy()))
        }

        Expr::BinOp(lhs, op, rhs) => {
            let lv = eval_expr(lhs, current, ctx)?;
            let rv = eval_expr(rhs, current, ctx)?;
            eval_binop(&lv, op, &rv)
        }

        Expr::Pipeline(_) => Err(AilangError::RuntimeError {
            stage: "expr".into(),
            msg: "Nested pipelines in expressions not yet supported".into(),
        }),
    }
}

fn eval_binop(lhs: &Value, op: &BinOp, rhs: &Value) -> AilangResult<Value> {
    match op {
        BinOp::Eq  => Ok(Value::Bool(lhs.to_string() == rhs.to_string())),
        BinOp::Ne  => Ok(Value::Bool(lhs.to_string() != rhs.to_string())),
        BinOp::And => Ok(Value::Bool(lhs.is_truthy() && rhs.is_truthy())),
        BinOp::Or  => Ok(Value::Bool(lhs.is_truthy() || rhs.is_truthy())),
        BinOp::Gt  => compare_values(lhs, rhs, |a, b| a > b),
        BinOp::Ge  => compare_values(lhs, rhs, |a, b| a >= b),
        BinOp::Lt  => compare_values(lhs, rhs, |a, b| a < b),
        BinOp::Le  => compare_values(lhs, rhs, |a, b| a <= b),
        BinOp::Add => arithmetic(lhs, rhs, |a, b| a + b),
        BinOp::Sub => arithmetic(lhs, rhs, |a, b| a - b),
        BinOp::Mul => arithmetic(lhs, rhs, |a, b| a * b),
        BinOp::Div => arithmetic(lhs, rhs, |a, b| a / b),
    }
}

fn compare_values(lhs: &Value, rhs: &Value, cmp: impl Fn(f64, f64) -> bool) -> AilangResult<Value> {
    let l = value_to_f64(lhs);
    let r = value_to_f64(rhs);
    match (l, r) {
        (Some(a), Some(b)) => Ok(Value::Bool(cmp(a, b))),
        // Fallback: lexicographic string comparison
        _ => {
            let ls = lhs.to_string();
            let rs = rhs.to_string();
            let ord = ls.cmp(&rs);
            let (a, b): (f64, f64) = match ord {
                std::cmp::Ordering::Less    => (-1.0, 0.0),
                std::cmp::Ordering::Equal   => (0.0, 0.0),
                std::cmp::Ordering::Greater => (1.0, 0.0),
            };
            Ok(Value::Bool(cmp(a, b)))
        }
    }
}

fn arithmetic(lhs: &Value, rhs: &Value, op: impl Fn(f64, f64) -> f64) -> AilangResult<Value> {
    let l = value_to_f64(lhs).unwrap_or(0.0);
    let r = value_to_f64(rhs).unwrap_or(0.0);
    let result = op(l, r);
    if result.fract() == 0.0 {
        Ok(Value::Int(result as i64))
    } else {
        Ok(Value::Float(result))
    }
}

fn value_to_f64(v: &Value) -> Option<f64> {
    match v {
        Value::Int(i) => Some(*i as f64),
        Value::Float(f) => Some(*f),
        Value::Str(s) => s.parse().ok(),
        _ => None,
    }
}

fn input_to_list(input: Option<Value>) -> Vec<Value> {
    match input {
        Some(Value::List(v)) => v,
        Some(other) => vec![other],
        None => vec![],
    }
}

fn collapse_stream(stream: Vec<Value>) -> Option<Value> {
    match stream.len() {
        0 => None,
        1 => Some(stream.into_iter().next().unwrap()),
        _ => Some(Value::List(stream)),
    }
}

// ── JSON ↔ Value conversion ─────────────────────────────────────────────────

pub fn value_to_json(v: &Value) -> serde_json::Value {
    match v {
        Value::Null       => serde_json::Value::Null,
        Value::Bool(b)    => serde_json::Value::Bool(*b),
        Value::Int(i)     => serde_json::json!(i),
        Value::Float(f)   => serde_json::json!(f),
        Value::Str(s)     => serde_json::Value::String(s.clone()),
        Value::List(v)    => serde_json::Value::Array(v.iter().map(value_to_json).collect()),
        Value::Object(m)  => {
            let map: serde_json::Map<_, _> = m.iter()
                .map(|(k, v)| (k.clone(), value_to_json(v)))
                .collect();
            serde_json::Value::Object(map)
        }
        Value::FileEntry(fe) => serde_json::json!({
            "path": fe.path,
            "name": fe.name,
            "ext":  fe.ext,
            "size": fe.size,
            "is_dir": fe.is_dir,
        }),
        Value::Line(le) => serde_json::json!({
            "path": le.path,
            "line": le.line,
            "num":  le.num,
        }),
    }
}

pub fn json_to_value(v: serde_json::Value) -> Value {
    match v {
        serde_json::Value::Null        => Value::Null,
        serde_json::Value::Bool(b)     => Value::Bool(b),
        serde_json::Value::Number(n)   => {
            if let Some(i) = n.as_i64() { Value::Int(i) }
            else { Value::Float(n.as_f64().unwrap_or(0.0)) }
        }
        serde_json::Value::String(s)   => Value::Str(s),
        serde_json::Value::Array(arr)  => Value::List(arr.into_iter().map(json_to_value).collect()),
        serde_json::Value::Object(map) => {
            Value::Object(map.into_iter().map(|(k, v)| (k, json_to_value(v))).collect())
        }
    }
}
