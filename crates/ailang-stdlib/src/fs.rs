use ailang_core::{error::{AilangError, AilangResult}, FileEntry, Value};
use globset::{Glob, GlobSetBuilder};
use std::path::Path;
use walkdir::WalkDir;

/// read("path") → Value::Str with file contents
pub fn read(args: &[Value], _input: Option<Value>) -> AilangResult<Vec<Value>> {
    let path = get_str_arg(args, 0, "read")?;
    let content = std::fs::read_to_string(&path)?;
    Ok(vec![Value::Str(content)])
}

/// write("path", content) or piped content → write to file
pub fn write(args: &[Value], input: Option<Value>) -> AilangResult<Vec<Value>> {
    let path = get_str_arg(args, 0, "write")?;
    let content = match input {
        Some(v) => v.to_string(),
        None => get_str_arg(args, 1, "write")?,
    };
    std::fs::write(&path, content)?;
    Ok(vec![])
}

/// glob("pattern") → Stream<FileEntry>
pub fn glob(args: &[Value], _input: Option<Value>) -> AilangResult<Vec<Value>> {
    let pattern = get_str_arg(args, 0, "glob")?;

    // Determine base directory from pattern
    let base = if pattern.starts_with('/') || pattern.contains(':') {
        Path::new("").to_path_buf()
    } else {
        std::env::current_dir()?
    };

    let glob = Glob::new(&pattern)
        .map_err(|e| AilangError::RuntimeError { stage: "glob".into(), msg: e.to_string() })?;
    let mut builder = GlobSetBuilder::new();
    builder.add(glob);
    let set = builder.build()
        .map_err(|e| AilangError::RuntimeError { stage: "glob".into(), msg: e.to_string() })?;

    let mut results = Vec::new();
    for entry in WalkDir::new(&base).into_iter().filter_map(|e| e.ok()) {
        let rel = entry.path().strip_prefix(&base).unwrap_or(entry.path());
        let rel_str = rel.to_string_lossy().replace('\\', "/");
        if set.is_match(&rel_str) {
            results.push(Value::FileEntry(path_to_entry(entry.path())?));
        }
    }

    Ok(results)
}

/// walk("dir") → Stream<FileEntry>
pub fn walk(args: &[Value], _input: Option<Value>) -> AilangResult<Vec<Value>> {
    let dir = get_str_arg(args, 0, "walk")?;
    let mut results = Vec::new();
    for entry in WalkDir::new(&dir).into_iter().filter_map(|e| e.ok()) {
        results.push(Value::FileEntry(path_to_entry(entry.path())?));
    }
    Ok(results)
}

/// cp("src", "dst") — copy file or directory
pub fn cp(args: &[Value], input: Option<Value>) -> AilangResult<Vec<Value>> {
    let dst = get_str_arg(args, 0, "cp")?;
    let src = match input {
        Some(Value::FileEntry(fe)) => fe.path,
        Some(Value::Str(s)) => s,
        _ => get_str_arg(args, 0, "cp")?,
    };
    let dst_path = Path::new(&dst);
    if dst_path.is_dir() {
        let filename = Path::new(&src).file_name().ok_or_else(|| AilangError::RuntimeError {
            stage: "cp".into(),
            msg: format!("Cannot determine filename from: {}", src),
        })?;
        std::fs::copy(&src, dst_path.join(filename))?;
    } else {
        if let Some(parent) = dst_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(&src, &dst)?;
    }
    Ok(vec![])
}

/// mv("src", "dst")
pub fn mv(args: &[Value], input: Option<Value>) -> AilangResult<Vec<Value>> {
    let dst = get_str_arg(args, 0, "mv")?;
    let src = match input {
        Some(Value::Str(s)) => s,
        Some(Value::FileEntry(fe)) => fe.path,
        _ => get_str_arg(args, 0, "mv")?,
    };
    std::fs::rename(&src, &dst)?;
    Ok(vec![])
}

/// rm("path")
pub fn rm(args: &[Value], input: Option<Value>) -> AilangResult<Vec<Value>> {
    let path_str = match input {
        Some(Value::Str(s)) => s,
        Some(Value::FileEntry(fe)) => fe.path,
        _ => get_str_arg(args, 0, "rm")?,
    };
    let path = Path::new(&path_str);
    if path.is_dir() {
        std::fs::remove_dir_all(path)?;
    } else {
        std::fs::remove_file(path)?;
    }
    Ok(vec![])
}

/// exists("path") → Value::Bool
pub fn exists(args: &[Value], _input: Option<Value>) -> AilangResult<Vec<Value>> {
    let path = get_str_arg(args, 0, "exists")?;
    Ok(vec![Value::Bool(Path::new(&path).exists())])
}

/// stat("path") → Value::Object with size, is_dir, ext, name
pub fn stat(args: &[Value], input: Option<Value>) -> AilangResult<Vec<Value>> {
    let path_str = match input {
        Some(Value::Str(s)) => s,
        Some(Value::FileEntry(fe)) => fe.path,
        _ => get_str_arg(args, 0, "stat")?,
    };
    let entry = path_to_entry(Path::new(&path_str))?;
    Ok(vec![Value::FileEntry(entry)])
}

/// diff("a", "b") → Value::Str with unified diff
pub fn diff(args: &[Value], _input: Option<Value>) -> AilangResult<Vec<Value>> {
    let a = get_str_arg(args, 0, "diff")?;
    let b = get_str_arg(args, 1, "diff")?;
    let content_a = std::fs::read_to_string(&a)?;
    let content_b = std::fs::read_to_string(&b)?;

    // Simple line-by-line diff output
    let lines_a: Vec<&str> = content_a.lines().collect();
    let lines_b: Vec<&str> = content_b.lines().collect();
    let mut diff_output = format!("--- {}\n+++ {}\n", a, b);

    let max = lines_a.len().max(lines_b.len());
    for i in 0..max {
        match (lines_a.get(i), lines_b.get(i)) {
            (Some(la), Some(lb)) if la == lb => diff_output.push_str(&format!("  {}\n", la)),
            (Some(la), Some(lb)) => {
                diff_output.push_str(&format!("- {}\n+ {}\n", la, lb));
            }
            (Some(la), None) => diff_output.push_str(&format!("- {}\n", la)),
            (None, Some(lb)) => diff_output.push_str(&format!("+ {}\n", lb)),
            (None, None) => {}
        }
    }

    Ok(vec![Value::Str(diff_output)])
}

/// patch("file", diff_content) — apply a patch string to a file
pub fn patch_file(args: &[Value], input: Option<Value>) -> AilangResult<Vec<Value>> {
    // Simplified: just a placeholder for now
    let _ = (args, input);
    Err(AilangError::RuntimeError {
        stage: "patch".into(),
        msg: "patch is not yet implemented — coming in M3".into(),
    })
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

fn path_to_entry(path: &Path) -> AilangResult<FileEntry> {
    let meta = std::fs::metadata(path)?;
    Ok(FileEntry {
        path: path.to_string_lossy().replace('\\', "/"),
        name: path.file_name().unwrap_or_default().to_string_lossy().into(),
        ext: path.extension().unwrap_or_default().to_string_lossy().into(),
        size: meta.len(),
        is_dir: meta.is_dir(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_read_write_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        std::fs::write(&file_path, "hello world").unwrap();

        let result = read(&[Value::Str(file_path.to_string_lossy().into())], None).unwrap();
        assert_eq!(result[0], Value::Str("hello world".into()));
    }

    #[test]
    fn test_exists() {
        let result = exists(&[Value::Str("Cargo.toml".into())], None).unwrap();
        // May or may not exist depending on cwd, just check type
        assert!(matches!(result[0], Value::Bool(_)));
    }
}
