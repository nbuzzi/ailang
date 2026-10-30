use ailang_core::{
    error::{AilangError, AilangResult},
    policy::unified_diff,
    FileEntry, Value,
};
use globset::{Glob, GlobSetBuilder};
use std::path::Path;
use walkdir::WalkDir;

/// read("path") → Value::Str with file contents (handles UTF-8, UTF-8 BOM, and UTF-16)
pub fn read(args: &[Value], _input: Option<Value>) -> AilangResult<Vec<Value>> {
    let path = get_str_arg(args, 0, "read")?;
    let content = read_file_string(Path::new(&path))?;
    Ok(vec![Value::Str(content)])
}

/// write("path", content) or piped content → write to file
pub fn write(args: &[Value], input: Option<Value>) -> AilangResult<Vec<Value>> {
    let path = get_str_arg(args, 0, "write")?;
    let content = match input {
        Some(v) => v.to_string(),
        None => get_str_arg(args, 1, "write")?,
    };
    if let Some(parent) = Path::new(&path).parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    std::fs::write(&path, content)?;
    Ok(vec![])
}

/// glob("pattern") → Stream<FileEntry>
pub fn glob(args: &[Value], _input: Option<Value>) -> AilangResult<Vec<Value>> {
    let pattern = get_str_arg(args, 0, "glob")?;

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
    let content_a = read_file_string(Path::new(&a))?;
    let content_b = read_file_string(Path::new(&b))?;
    let d = unified_diff(&format!("{} -> {}", a, b), &content_a, &content_b);
    Ok(vec![Value::Str(d)])
}

/// replace_block("file", "target_block", "replacement_block")
/// Surgical code replacement: searches for target_block and replaces with replacement_block.
/// Includes fallback fuzzy whitespace normalization so slight indentation mismatches by LLMs still succeed.
pub fn replace_block(args: &[Value], _input: Option<Value>) -> AilangResult<Vec<Value>> {
    let path = get_str_arg(args, 0, "replace_block")?;
    let target = get_str_arg(args, 1, "replace_block")?;
    let replacement = get_str_arg(args, 2, "replace_block")?;

    let content = read_file_string(Path::new(&path))?;

    // 1. Exact match
    if let Some(pos) = content.find(&target) {
        // Check uniqueness
        if content[pos + target.len()..].contains(&target) {
            return Err(AilangError::EditError {
                msg: format!("Target block occurs multiple times in '{}'. Provide more surrounding context.", path),
            });
        }
        let mut new_content = String::with_capacity(content.len() - target.len() + replacement.len());
        new_content.push_str(&content[..pos]);
        new_content.push_str(&replacement);
        new_content.push_str(&content[pos + target.len()..]);
        std::fs::write(&path, &new_content)?;
        return Ok(vec![Value::Bool(true)]);
    }

    // 2. Fuzzy match (line-by-line whitespace-trimmed comparison)
    let content_lines: Vec<&str> = content.lines().collect();
    let target_lines: Vec<&str> = target.lines().collect();

    if target_lines.is_empty() {
        return Err(AilangError::EditError { msg: "Target block is empty".into() });
    }

    let mut matches = Vec::new();
    for i in 0..=content_lines.len().saturating_sub(target_lines.len()) {
        let window = &content_lines[i..i + target_lines.len()];
        let matches_fuzzy = window.iter().zip(target_lines.iter()).all(|(w, t)| {
            w.trim() == t.trim()
        });
        if matches_fuzzy {
            matches.push(i);
        }
    }

    if matches.is_empty() {
        return Err(AilangError::EditError {
            msg: format!("Target block not found in '{}'", path),
        });
    }
    if matches.len() > 1 {
        return Err(AilangError::EditError {
            msg: format!("Fuzzy target block matched {} locations in '{}'. Be more specific.", matches.len(), path),
        });
    }

    let start_line = matches[0];
    let end_line = start_line + target_lines.len();

    let mut new_lines = Vec::new();
    new_lines.extend_from_slice(&content_lines[..start_line]);
    new_lines.push(&replacement);
    new_lines.extend_from_slice(&content_lines[end_line..]);

    let mut new_content = new_lines.join("\n");
    if content.ends_with('\n') {
        new_content.push('\n');
    }
    std::fs::write(&path, &new_content)?;
    Ok(vec![Value::Bool(true)])
}

/// patch("file", diff_content) — apply unified diff to a file
pub fn patch_file(args: &[Value], input: Option<Value>) -> AilangResult<Vec<Value>> {
    let path = get_str_arg(args, 0, "patch")?;
    let diff = match input {
        Some(v) => v.to_string(),
        None => get_str_arg(args, 1, "patch")?,
    };

    let original = read_file_string(Path::new(&path))?;
    let patched = apply_unified_patch(&original, &diff)?;
    std::fs::write(&path, &patched)?;
    Ok(vec![Value::Bool(true)])
}

fn apply_unified_patch(original: &str, diff: &str) -> AilangResult<String> {
    let orig_lines: Vec<&str> = original.lines().collect();
    let mut result_lines: Vec<String> = Vec::new();
    let mut orig_idx = 0;

    for line in diff.lines() {
        if line.starts_with("---") || line.starts_with("+++") {
            continue;
        }
        if line.starts_with("@@") {
            if let Some(minus_idx) = line.find('-') {
                let rest = &line[minus_idx + 1..];
                let num_str: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
                if let Ok(line_num) = num_str.parse::<usize>() {
                    let target = line_num.saturating_sub(1);
                    while orig_idx < target && orig_idx < orig_lines.len() {
                        result_lines.push(orig_lines[orig_idx].to_string());
                        orig_idx += 1;
                    }
                }
            }
            continue;
        }

        if let Some(added) = line.strip_prefix('+') {
            result_lines.push(added.to_string());
        } else if line.starts_with('-') {
            if orig_idx < orig_lines.len() {
                orig_idx += 1;
            }
        } else if let Some(context) = line.strip_prefix(' ') {
            if orig_idx < orig_lines.len() {
                result_lines.push(orig_lines[orig_idx].to_string());
                orig_idx += 1;
            } else {
                result_lines.push(context.to_string());
            }
        }
    }

    // Append remainder of original
    while orig_idx < orig_lines.len() {
        result_lines.push(orig_lines[orig_idx].to_string());
        orig_idx += 1;
    }

    let mut out = result_lines.join("\n");
    if original.ends_with('\n') {
        out.push('\n');
    }
    Ok(out)
}

// ── Helpers ────────────────────────────────────────────────────────────────

/// Read a file into a String, handling UTF-8, UTF-8 with BOM, and UTF-16 LE gracefully.
pub fn read_file_string(path: &Path) -> std::io::Result<String> {
    let bytes = std::fs::read(path)?;
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        return Ok(String::from_utf8_lossy(&bytes[3..]).into_owned());
    }
    if bytes.len() >= 2 && bytes[0] == 0xFF && bytes[1] == 0xFE {
        let u16_slice: Vec<u16> = bytes[2..]
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        if let Ok(s) = String::from_utf16(&u16_slice) {
            return Ok(s);
        }
    }
    match String::from_utf8(bytes) {
        Ok(s) => Ok(s),
        Err(e) => Ok(String::from_utf8_lossy(e.as_bytes()).into_owned()),
    }
}

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

    #[test]
    fn test_read_write_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        std::fs::write(&file_path, "hello world").unwrap();

        let result = read(&[Value::Str(file_path.to_string_lossy().into())], None).unwrap();
        assert_eq!(result[0], Value::Str("hello world".into()));
    }

    #[test]
    fn test_replace_block_exact() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("code.rs");
        std::fs::write(&file_path, "fn old_func() {\n    return 1;\n}\n").unwrap();

        let result = replace_block(&[
            Value::Str(file_path.to_string_lossy().into()),
            Value::Str("return 1;".into()),
            Value::Str("return 2;".into()),
        ], None).unwrap();
        assert_eq!(result[0], Value::Bool(true));

        let updated = std::fs::read_to_string(&file_path).unwrap();
        assert!(updated.contains("return 2;"));
    }

    #[test]
    fn test_replace_block_fuzzy() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("code.rs");
        std::fs::write(&file_path, "    fn test() {\n        let x = 1;\n    }\n").unwrap();

        let result = replace_block(&[
            Value::Str(file_path.to_string_lossy().into()),
            Value::Str("let x = 1;".into()),
            Value::Str("        let x = 99;".into()),
        ], None).unwrap();
        assert_eq!(result[0], Value::Bool(true));

        let updated = std::fs::read_to_string(&file_path).unwrap();
        assert!(updated.contains("let x = 99;"));
    }
}
