//! Policy enforcement shared by CLI and MCP. The low-level stdlib functions
//! remain usable by Rust callers; language execution always enters here.
use ailang_core::{AilangError, AilangResult, DryRunEntry, SecurityPolicy, Value};
use std::path::Path;

fn arg(args: &[Value], n: usize) -> AilangResult<&str> {
    args.get(n)
        .and_then(Value::as_str)
        .ok_or_else(|| AilangError::RuntimeError {
            stage: "policy".into(),
            msg: format!("Expected path argument at position {n}"),
        })
}

fn input_path<'a>(args: &'a [Value], input: &'a Option<Value>) -> AilangResult<&'a str> {
    match input {
        Some(Value::FileEntry(f)) => Ok(&f.path),
        Some(Value::Str(s)) => Ok(s),
        _ => arg(args, 0),
    }
}

fn check_entries(value: &Value, policy: &SecurityPolicy) -> AilangResult<()> {
    match value {
        Value::FileEntry(f) => policy.check_read(&f.path),
        Value::List(items) => {
            for item in items {
                check_entries(item, policy)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn check_tree(path: &str, policy: &SecurityPolicy) -> AilangResult<()> {
    policy.check_read(path)?;
    if policy.allowed_dirs.is_empty() {
        return Ok(());
    }
    // Fail closed on links outside the allowed roots, including links within
    // recursively removed/moved directories. Do not follow directory links.
    for entry in walkdir::WalkDir::new(path) {
        let entry = entry.map_err(|e| AilangError::SecurityError { msg: e.to_string() })?;
        policy.check_read(&entry.path().to_string_lossy())?;
    }
    Ok(())
}

pub fn dispatch(
    name: &str,
    args: &[Value],
    input: Option<Value>,
    rt: &tokio::runtime::Handle,
    policy: &SecurityPolicy,
    entries: &mut Vec<DryRunEntry>,
) -> AilangResult<Vec<Value>> {
    if let Some(value) = &input {
        check_entries(value, policy)?;
    }
    let edit = matches!(name, "replace_block" | "replace_all" | "replace") && args.len() >= 3
        || matches!(
            name,
            "insert_after" | "insert_before" | "prepend" | "append" | "patch"
        );
    let mut target: Option<String> = None;
    if edit || name == "write" {
        let path = arg(args, 0)?;
        policy.check_write(path)?;
        target = Some(path.into());
    }
    match name {
        "read" | "exists" => policy.check_read(arg(args, 0)?)?,
        "stat" => policy.check_read(input_path(args, &input)?)?,
        "diff" => {
            policy.check_read(arg(args, 0)?)?;
            policy.check_read(arg(args, 1)?)?;
        }
        "head" | "tail" if args.len() > 1 => policy.check_read(arg(args, 1)?)?,
        "lines" if !args.is_empty() => policy.check_read(arg(args, 0)?)?,
        "search" => check_tree(if args.len() >= 2 { arg(args, 0)? } else { "." }, policy)?,
        "tree" => check_tree(args.first().and_then(Value::as_str).unwrap_or("."), policy)?,
        "walk" => check_tree(arg(args, 0)?, policy)?,
        "glob" => check_tree(
            &super::fs::glob_root(arg(args, 0)?)?.to_string_lossy(),
            policy,
        )?,
        "cd" => {
            // Relative policies must not change meaning when the process cwd changes.
            if !policy.allowed_dirs.is_empty() {
                return Err(AilangError::SecurityError {
                    msg: "cd denied with --allow-dir; use absolute paths".into(),
                });
            }
            if policy.dry_run {
                entries.push(DryRunEntry {
                    action: name.into(),
                    target: arg(args, 0)?.into(),
                    diff: None,
                });
                return Ok(vec![]);
            }
        }
        "cp" | "mv" => {
            let source = input_path(args, &input)?;
            check_tree(source, policy)?;
            if name == "mv" {
                policy.check_write(source)?;
            }
            let dest = arg(args, if input.is_some() { 0 } else { 1 })?;
            policy.check_write(dest)?;
            if Path::new(dest).is_dir() {
                if let Some(filename) = Path::new(source).file_name() {
                    policy.check_write(&Path::new(dest).join(filename).to_string_lossy())?;
                }
            }
            target = Some(dest.into());
        }
        "rm" => {
            let path = input_path(args, &input)?;
            policy.check_write(path)?;
            check_tree(path, policy)?;
            target = Some(path.into());
        }
        "exec" => {
            policy.check_exec()?;
            target = Some(arg(args, 0)?.into());
        }
        "net.get" | "net.post" | "net.download" => {
            policy.check_net()?;
            if name == "net.post" {
                if policy.read_only {
                    return Err(AilangError::SecurityError {
                        msg: "net.post denied in --read-only mode".into(),
                    });
                }
                target = Some(arg(args, 0)?.into());
            } else if name == "net.download" {
                policy.check_write(arg(args, 1)?)?;
                target = Some(arg(args, 1)?.into());
            }
        }
        _ => {}
    }
    if policy.dry_run {
        if let Some(target) = target {
            let diff = if edit || name == "write" {
                // Reuse the real edit implementation against a private copy. This
                // validates anchors and patch syntax without touching the target.
                let old = match super::fs::read_file_string(Path::new(&target)) {
                    Ok(s) => s,
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
                    Err(e) => return Err(e.into()),
                };
                let temp = tempfile::tempdir()?;
                let copy = temp.path().join("preview");
                if Path::new(&target).exists() {
                    std::fs::write(&copy, &old)?;
                }
                let mut staged = args.to_vec();
                staged[0] = Value::Str(copy.to_string_lossy().into_owned());
                super::dispatch(name, &staged, input, rt)?;
                let new = super::fs::read_file_string(&copy)?;
                Some(ailang_core::unified_diff(&target, &old, &new))
            } else {
                None
            };
            entries.push(DryRunEntry {
                action: name.into(),
                target,
                diff,
            });
            return Ok(vec![]);
        }
    }
    super::dispatch(name, args, input, rt)
}

pub fn write_sink(
    path: &str,
    content: &str,
    append: bool,
    policy: &SecurityPolicy,
    entries: &mut Vec<DryRunEntry>,
) -> AilangResult<()> {
    policy.check_write(path)?;
    let old = if append || policy.dry_run {
        match super::fs::read_file_string(Path::new(path)) {
            Ok(s) => s,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(e) => return Err(e.into()),
        }
    } else {
        String::new()
    };
    let new = if append {
        format!("{old}{content}\n")
    } else {
        content.into()
    };
    if policy.dry_run {
        entries.push(DryRunEntry {
            action: if append { "append" } else { "write" }.into(),
            target: path.into(),
            diff: Some(ailang_core::unified_diff(path, &old, &new)),
        });
    } else {
        super::fs::atomic_write(Path::new(path), new.as_bytes())?;
    }
    Ok(())
}
