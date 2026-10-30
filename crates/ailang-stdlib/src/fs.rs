use ailang_core::{
    error::{AilangError, AilangResult},
    policy::unified_diff,
    FileEntry, LineEntry, Value,
};
use globset::{Glob, GlobSetBuilder};
use regex::Regex;
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

/// search(dir, pattern) or search(pattern) — AI Macro
/// Recursively searches code files in `dir` matching regex `pattern`.
/// Automatically skips noise directories (.git, target, node_modules, dist, etc.)
pub fn search(args: &[Value], _input: Option<Value>) -> AilangResult<Vec<Value>> {
    let (dir, pattern) = match args {
        [Value::Str(d), Value::Str(p)] => (d.as_str(), p.as_str()),
        [Value::Str(p)] => (".", p.as_str()),
        _ => return Err(AilangError::RuntimeError {
            stage: "search".into(),
            msg: "Usage: search [dir] <pattern>".into(),
        }),
    };

    let re = Regex::new(pattern).map_err(|e| AilangError::RegexError(e.to_string()))?;
    let mut results = Vec::new();

    let walker = WalkDir::new(dir).into_iter().filter_entry(|e| {
        let name = e.file_name().to_string_lossy();
        !name.starts_with('.') && name != "target" && name != "node_modules" && name != "dist" && name != "build"
    });

    for entry in walker.filter_map(|e| e.ok()) {
        if entry.file_type().is_file() {
            let path_str = entry.path().to_string_lossy().replace('\\', "/");
            if let Ok(content) = read_file_string(entry.path()) {
                for (i, line) in content.lines().enumerate() {
                    if re.is_match(line) {
                        results.push(Value::Line(LineEntry {
                            path: path_str.clone(),
                            line: line.to_string(),
                            num: i + 1,
                        }));
                    }
                }
            }
        }
    }

    Ok(results)
}

/// tree([dir], [max_depth]) — AI Macro
/// Produces a visual tree representation of a directory tree for fast agent architecture surveys.
pub fn tree(args: &[Value], _input: Option<Value>) -> AilangResult<Vec<Value>> {
    let dir = args.get(0).and_then(|v| v.as_str()).unwrap_or(".");
    let max_depth = args.get(1).and_then(|v| v.as_int()).unwrap_or(3) as usize;

    let mut out = format!("{}\n", dir);
    build_tree(Path::new(dir), "", 0, max_depth, &mut out)?;
    Ok(vec![Value::Str(out)])
}

fn build_tree(dir: &Path, prefix: &str, depth: usize, max_depth: usize, out: &mut String) -> AilangResult<()> {
    if depth >= max_depth {
        return Ok(());
    }

    let mut entries: Vec<_> = match std::fs::read_dir(dir) {
        Ok(read_dir) => read_dir.filter_map(|e| e.ok()).collect(),
        Err(_) => return Ok(()),
    };

    entries.sort_by_key(|e| e.file_name());

    let filtered: Vec<_> = entries.into_iter().filter(|e| {
        let name = e.file_name().to_string_lossy().to_string();
        !name.starts_with('.') && name != "target" && name != "node_modules"
    }).collect();

    let count = filtered.len();
    for (i, entry) in filtered.into_iter().enumerate() {
        let is_last = i + 1 == count;
        let connector = if is_last { "└── " } else { "├── " };
        let name = entry.file_name().to_string_lossy().to_string();

        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
        if is_dir {
            out.push_str(&format!("{}{}{}/\n", prefix, connector, name));
            let new_prefix = format!("{}{}", prefix, if is_last { "    " } else { "│   " });
            build_tree(&entry.path(), &new_prefix, depth + 1, max_depth, out)?;
        } else {
            out.push_str(&format!("{}{}{}\n", prefix, connector, name));
        }
    }

    Ok(())
}

/// head(n, [file]) — first n items or lines
pub fn head(args: &[Value], input: Option<Value>) -> AilangResult<Vec<Value>> {
    let n = args.get(0).and_then(|v| v.as_int()).unwrap_or(10) as usize;

    // If file arg passed: head 10 filename
    if let Some(path_val) = args.get(1) {
        if let Some(path) = path_val.as_str() {
            let content = read_file_string(Path::new(path))?;
            let lines: Vec<Value> = content.lines().take(n).map(|l| Value::Str(l.to_string())).collect();
            return Ok(lines);
        }
    }

    match input {
        Some(Value::List(items)) => Ok(items.into_iter().take(n).collect()),
        Some(Value::Str(s)) => {
            let lines: Vec<Value> = s.lines().take(n).map(|l| Value::Str(l.to_string())).collect();
            Ok(lines)
        }
        None => Err(AilangError::RuntimeError {
            stage: "head".into(),
            msg: "head requires either a file argument or piped input".into(),
        }),
        other => Ok(vec![other.unwrap()]),
    }
}

/// tail(n, [file]) — last n items or lines
pub fn tail(args: &[Value], input: Option<Value>) -> AilangResult<Vec<Value>> {
    let n = args.get(0).and_then(|v| v.as_int()).unwrap_or(10) as usize;

    if let Some(path_val) = args.get(1) {
        if let Some(path) = path_val.as_str() {
            let content = read_file_string(Path::new(path))?;
            let all_lines: Vec<&str> = content.lines().collect();
            let start = all_lines.len().saturating_sub(n);
            let lines: Vec<Value> = all_lines[start..].iter().map(|l| Value::Str(l.to_string())).collect();
            return Ok(lines);
        }
    }

    match input {
        Some(Value::List(items)) => {
            let start = items.len().saturating_sub(n);
            Ok(items[start..].to_vec())
        }
        Some(Value::Str(s)) => {
            let all_lines: Vec<&str> = s.lines().collect();
            let start = all_lines.len().saturating_sub(n);
            let lines: Vec<Value> = all_lines[start..].iter().map(|l| Value::Str(l.to_string())).collect();
            Ok(lines)
        }
        None => Err(AilangError::RuntimeError {
            stage: "tail".into(),
            msg: "tail requires either a file argument or piped input".into(),
        }),
        other => Ok(vec![other.unwrap()]),
    }
}

/// lines([file]) — reads file or splits piped input into lines
pub fn lines_cmd(args: &[Value], input: Option<Value>) -> AilangResult<Vec<Value>> {
    if let Some(path_val) = args.get(0) {
        if let Some(path) = path_val.as_str() {
            let content = read_file_string(Path::new(path))?;
            return Ok(content.lines().map(|l| Value::Str(l.to_string())).collect());
        }
    }
    match input {
        Some(Value::Str(s)) => Ok(s.lines().map(|l| Value::Str(l.to_string())).collect()),
        Some(other) => Ok(other.to_string().lines().map(|l| Value::Str(l.to_string())).collect()),
        None => Ok(vec![]),
    }
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
    fn test_tree_generates_output() {
        let result = tree(&[Value::Str(".".into()), Value::Int(2)], None).unwrap();
        assert!(result[0].to_string().contains("Cargo.toml"));
    }

    #[test]
    fn test_search_finds_matches() {
        let result = search(&[Value::Str("src".into()), Value::Str("ailang".into())], None).unwrap();
        assert!(!result.is_empty());
    }

    #[test]
    fn test_head_and_tail() {
        let input = Some(Value::Str("1\n2\n3\n4\n5".into()));
        let h = head(&[Value::Int(2)], input.clone()).unwrap();
        assert_eq!(h.len(), 2);
        assert_eq!(h[0], Value::Str("1".into()));

        let t = tail(&[Value::Int(2)], input).unwrap();
        assert_eq!(t.len(), 2);
        assert_eq!(t[0], Value::Str("4".into()));
    }
}
