use ailang_core::{error::{AilangError, AilangResult}, Map, Value};

/// filter(.field > value) — filter a list by a predicate expression
pub fn filter(args: &[Value], input: Option<Value>) -> AilangResult<Vec<Value>> {
    let _ = (args, input);
    Err(AilangError::RuntimeError {
        stage: "filter".into(),
        msg: "filter must be handled by the evaluator (expression support required)".into(),
    })
}

/// sort(.field) — sort a list by a field
pub fn sort(args: &[Value], input: Option<Value>) -> AilangResult<Vec<Value>> {
    let field = args.get(0).and_then(|v| v.as_str().map(|s| s.to_string()));
    let mut items = match input {
        Some(Value::List(v)) => v,
        Some(other) => vec![other],
        None => return Ok(vec![]),
    };

    items.sort_by(|a, b| {
        let va = field.as_ref().map(|f| a.get_path(f)).unwrap_or_else(|| a.clone());
        let vb = field.as_ref().map(|f| b.get_path(f)).unwrap_or_else(|| b.clone());
        match (&va, &vb) {
            (Value::Int(a), Value::Int(b)) => a.cmp(b),
            (Value::Int(_) | Value::Float(_), Value::Int(_) | Value::Float(_)) =>
                va.as_f64().unwrap().total_cmp(&vb.as_f64().unwrap()),
            _ => va.to_string().cmp(&vb.to_string()),
        }
    });

    Ok(vec![Value::List(items)])
}

/// group(.field) — group a list into an Object keyed by field value
pub fn group(args: &[Value], input: Option<Value>) -> AilangResult<Vec<Value>> {
    let field = args.get(0)
        .and_then(|v| v.as_str().map(|s| s.to_string()))
        .ok_or_else(|| AilangError::RuntimeError {
            stage: "group".into(),
            msg: "group requires a field argument".into(),
        })?;

    let items = match input {
        Some(Value::List(v)) => v,
        Some(other) => vec![other],
        None => return Ok(vec![]),
    };

    let mut groups: Map = Map::new();
    for item in items {
        let key = item.get_path(&field).to_string();
        if let Some(Value::List(list)) = groups.get_mut(&key) {
            list.push(item);
        } else {
            groups.insert(key, Value::List(vec![item]));
        }
    }

    Ok(vec![Value::Object(groups)])
}

/// set(.field, value) — set a field on an object (immutable, returns new object)
pub fn set_field(args: &[Value], input: Option<Value>) -> AilangResult<Vec<Value>> {
    let field = args.get(0)
        .and_then(|v| v.as_str().map(|s| s.to_string()))
        .ok_or_else(|| AilangError::RuntimeError {
            stage: "set".into(),
            msg: "set requires a field name as first argument".into(),
        })?;
    let new_val = args.get(1).cloned().unwrap_or(Value::Null);

    match input {
        Some(Value::Object(mut map)) => {
            map.insert(field, new_val);
            Ok(vec![Value::Object(map)])
        }
        Some(other) => Err(AilangError::TypeError {
            expected: "Object".into(),
            got: format!("{:?}", other),
        }),
        None => Err(AilangError::RuntimeError {
            stage: "set".into(),
            msg: "set requires an object as input".into(),
        }),
    }
}

/// assert("expected") — verify input matches expected value
pub fn assert_val(args: &[Value], input: Option<Value>) -> AilangResult<Vec<Value>> {
    let expected = args.get(0)
        .map(|v| v.to_string())
        .ok_or_else(|| AilangError::RuntimeError {
            stage: "assert".into(),
            msg: "assert requires an expected value argument".into(),
        })?;
    let got = input.map(|v| v.to_string()).unwrap_or_default();

    if got.trim() == expected.trim() {
        Ok(vec![Value::Bool(true)])
    } else {
        Err(AilangError::AssertionError { expected, got })
    }
}

/// unique — remove duplicate values from a list
pub fn unique(input: Option<Value>) -> AilangResult<Vec<Value>> {
    let items = match input {
        Some(Value::List(v)) => v,
        Some(other) => return Ok(vec![other]),
        None => return Ok(vec![]),
    };

    let mut seen = std::collections::HashSet::new();
    let result: Vec<Value> = items
        .into_iter()
        .filter(|v| seen.insert(v.to_string()))
        .collect();

    Ok(vec![Value::List(result)])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sort_strings() {
        let input = Value::List(vec![
            Value::Str("banana".into()),
            Value::Str("apple".into()),
            Value::Str("cherry".into()),
        ]);
        let result = sort(&[], Some(input)).unwrap();
        if let Value::List(v) = &result[0] {
            assert_eq!(v[0], Value::Str("apple".into()));
        }
    }

    #[test]
    fn test_assert_pass() {
        let result = assert_val(
            &[Value::Str("hello".into())],
            Some(Value::Str("hello".into())),
        ).unwrap();
        assert_eq!(result[0], Value::Bool(true));
    }

    #[test]
    fn test_assert_fail() {
        let result = assert_val(
            &[Value::Str("expected".into())],
            Some(Value::Str("got".into())),
        );
        assert!(result.is_err());
    }
}
