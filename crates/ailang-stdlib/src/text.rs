use ailang_core::{error::{AilangError, AilangResult}, LineEntry, Value};
use regex::Regex;

/// grep("pattern") — filter lines matching a regex pattern
pub fn grep(args: &[Value], input: Option<Value>) -> AilangResult<Vec<Value>> {
    let pattern = get_str_arg(args, 0, "grep")?;
    let re = Regex::new(&pattern)
        .map_err(|e| AilangError::RegexError(e.to_string()))?;

    match input {
        Some(Value::Str(s)) => {
            // Input is a raw string — split into lines and filter
            let results: Vec<Value> = s
                .lines()
                .enumerate()
                .filter(|(_, line)| re.is_match(line))
                .map(|(i, line)| Value::Line(LineEntry {
                    path: String::new(),
                    line: line.to_string(),
                    num: i + 1,
                }))
                .collect();
            Ok(results)
        }
        Some(Value::List(items)) => {
            // Input is a list — filter items whose string representation matches
            let mut results = Vec::new();
            for item in items {
                if matches!(item, Value::FileEntry(_)) {
                    results.extend(grep(args, Some(item))?);
                } else if re.is_match(&item.to_string()) {
                    results.push(item);
                }
            }
            Ok(results)
        }
        Some(Value::FileEntry(fe)) => {
            if fe.is_dir { return Ok(vec![]); }
            // Input is a file entry — read the file and grep it
            let content = crate::fs::read_file_string(std::path::Path::new(&fe.path))
                .map_err(AilangError::IoError)?;
            let results: Vec<Value> = content
                .lines()
                .enumerate()
                .filter(|(_, line)| re.is_match(line))
                .map(|(i, line)| Value::Line(LineEntry {
                    path: fe.path.clone(),
                    line: line.to_string(),
                    num: i + 1,
                }))
                .collect();
            Ok(results)
        }
        Some(Value::Line(line)) => {
            Ok(if re.is_match(&line.line) { vec![Value::Line(line)] } else { vec![] })
        }
        None => Err(AilangError::RuntimeError {
            stage: "grep".into(),
            msg: "grep requires input (pipe a file, string, or list)".into(),
        }),
        other => Err(AilangError::TypeError {
            expected: "Str | FileEntry | List".into(),
            got: format!("{:?}", other),
        }),
    }
}

/// sed("pattern", "replacement") — replace regex matches in input
pub fn sed(args: &[Value], input: Option<Value>) -> AilangResult<Vec<Value>> {
    let pattern = get_str_arg(args, 0, "sed")?;
    let replacement = get_str_arg(args, 1, "sed")?;
    let re = Regex::new(&pattern)
        .map_err(|e| AilangError::RegexError(e.to_string()))?;

    match input {
        Some(Value::Str(s)) => {
            Ok(vec![Value::Str(re.replace_all(&s, replacement.as_str()).into_owned())])
        }
        Some(Value::FileEntry(fe)) => {
            let content = std::fs::read_to_string(&fe.path)?;
            let replaced = re.replace_all(&content, replacement.as_str()).into_owned();
            // Return the modified content as a string — the sink or next stage writes it
            Ok(vec![Value::Str(replaced)])
        }
        Some(Value::Line(le)) => {
            let replaced = re.replace_all(&le.line, replacement.as_str()).into_owned();
            Ok(vec![Value::Str(replaced)])
        }
        None => Err(AilangError::RuntimeError {
            stage: "sed".into(),
            msg: "sed requires piped input".into(),
        }),
        other => Err(AilangError::TypeError {
            expected: "Str | FileEntry | Line".into(),
            got: format!("{:?}", other),
        }),
    }
}

/// replace("target", "replacement") — literal non-regex replacement on stream
pub fn replace_stream(args: &[Value], input: Option<Value>) -> AilangResult<Vec<Value>> {
    let target = get_str_arg(args, 0, "replace")?;
    let replacement = get_str_arg(args, 1, "replace")?;

    match input {
        Some(Value::Str(s)) => {
            Ok(vec![Value::Str(s.replace(&target, &replacement))])
        }
        Some(Value::Line(le)) => {
            Ok(vec![Value::Line(LineEntry {
                path: le.path,
                line: le.line.replace(&target, &replacement),
                num: le.num,
            })])
        }
        Some(Value::List(items)) => {
            Ok(items.into_iter().map(|item| {
                if let Value::Str(s) = item {
                    Value::Str(s.replace(&target, &replacement))
                } else {
                    item
                }
            }).collect())
        }
        None => Err(AilangError::RuntimeError {
            stage: "replace".into(),
            msg: "replace requires piped input or file argument".into(),
        }),
        other => Ok(vec![other.unwrap()]),
    }
}

/// slice(n, m) — take elements n..m from a list or characters from a string
pub fn slice(args: &[Value], input: Option<Value>) -> AilangResult<Vec<Value>> {
    let start = args.get(0).and_then(|v| v.as_int()).unwrap_or(0) as usize;
    let end   = args.get(1).and_then(|v| v.as_int());

    match input {
        Some(Value::Str(s)) => {
            let chars: Vec<char> = s.chars().collect();
            let end = end.map(|e| e as usize).unwrap_or(chars.len());
            let sliced: String = chars[start.min(chars.len())..end.min(chars.len())].iter().collect();
            Ok(vec![Value::Str(sliced)])
        }
        Some(Value::List(items)) => {
            let end = end.map(|e| e as usize).unwrap_or(items.len());
            Ok(items[start.min(items.len())..end.min(items.len())].to_vec())
        }
        None => Err(AilangError::RuntimeError {
            stage: "slice".into(),
            msg: "slice requires piped input".into(),
        }),
        other => Err(AilangError::TypeError {
            expected: "Str | List".into(),
            got: format!("{:?}", other),
        }),
    }
}

// ── Helpers ────────────────────────────────────────────────────────────────
fn get_str_arg(args: &[Value], idx: usize, fn_name: &str) -> AilangResult<String> {
    args.get(idx)
        .and_then(|v| v.as_str().map(|s| s.to_string()))
        .ok_or_else(|| AilangError::RuntimeError {
            stage: fn_name.to_string(),
            msg: format!("Expected string argument at position {}", idx),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grep_string() {
        let input = Value::Str("line1\nERROR: something\nline3\nERROR: other".into());
        let result = grep(&[Value::Str("ERROR".into())], Some(input)).unwrap();
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_sed_string() {
        let input = Value::Str("TODO: fix this\nTODO: also this".into());
        let result = sed(
            &[Value::Str("TODO".into()), Value::Str("DONE".into())],
            Some(input),
        ).unwrap();
        assert!(result[0].to_string().contains("DONE"));
        assert!(!result[0].to_string().contains("TODO"));
    }

    #[test]
    fn test_replace_stream() {
        let input = Value::Str("fn old_name()".into());
        let result = replace_stream(
            &[Value::Str("old_name".into()), Value::Str("new_name".into())],
            Some(input),
        ).unwrap();
        assert_eq!(result[0], Value::Str("fn new_name()".into()));
    }
}
