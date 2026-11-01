use crate::error::{AilangError, AilangResult};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Security and execution policy for AI agent sandboxing and dry-run previews.
#[derive(Debug, Clone, Default)]
pub struct SecurityPolicy {
    pub dry_run: bool,
    pub read_only: bool,
    pub no_net: bool,
    pub allowed_dirs: Vec<PathBuf>,
}

/// Record of an action that would have been taken in `--dry-run` mode.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DryRunEntry {
    pub action: String,
    pub target: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diff: Option<String>,
}

impl SecurityPolicy {
    pub fn check_path(&self, path: &str) -> AilangResult<()> {
        if self.allowed_dirs.is_empty() {
            return Ok(());
        }

        let target = Path::new(path);
        let abs_target = if target.is_absolute() {
            target.to_path_buf()
        } else {
            std::env::current_dir()?.join(target)
        };

        // Normalize components without requiring the file to exist yet
        let normalized = resolve_path(&abs_target)?;

        let mut allowed = false;
        for dir in &self.allowed_dirs {
            let abs_dir = if dir.is_absolute() {
                dir.clone()
            } else {
                std::env::current_dir().unwrap_or_default().join(dir)
            };
            let norm_dir = resolve_path(&abs_dir)?;
            allowed |= normalized.starts_with(&norm_dir);
        }

        if !allowed {
            return Err(AilangError::SecurityError {
                msg: format!(
                    "Access denied to '{}': outside allowed directories {:?}",
                    path, self.allowed_dirs
                ),
            });
        }

        Ok(())
    }

    pub fn check_read(&self, path: &str) -> AilangResult<()> {
        self.check_path(path)
    }

    pub fn check_write(&self, path: &str) -> AilangResult<()> {
        if self.read_only {
            return Err(AilangError::SecurityError {
                msg: format!("Write denied to '{}': runtime is in --read-only mode", path),
            });
        }
        self.check_path(path)
    }

    /// Process execution can bypass every other restriction, so it is
    /// disabled in read-only mode and whenever a directory sandbox is active.
    pub fn check_exec(&self) -> AilangResult<()> {
        if self.read_only || self.no_net || !self.allowed_dirs.is_empty() {
            return Err(AilangError::SecurityError {
                msg: "exec denied: disabled in --read-only / --allow-dir / --no-net mode".into(),
            });
        }
        Ok(())
    }

    pub fn check_net(&self) -> AilangResult<()> {
        if self.no_net {
            return Err(AilangError::SecurityError {
                msg: "Network access denied: runtime is in --no-net mode".into(),
            });
        }
        Ok(())
    }
}

// Resolve links before handling `..`, including links in the existing parent
// of a new file. Pure lexical normalization permits junction/symlink escapes.
fn resolve_path(path: &Path) -> std::io::Result<PathBuf> {
    let mut out = PathBuf::new();
    for comp in path.components() {
        match comp {
            std::path::Component::ParentDir => {
                out.pop();
            }
            std::path::Component::CurDir => {}
            other => {
                out.push(other.as_os_str());
                match std::fs::symlink_metadata(&out) {
                    Ok(_) => out = std::fs::canonicalize(&out)?,
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                    Err(e) => return Err(e),
                }
            }
        }
    }
    Ok(out)
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Op {
    Eq(usize, usize),
    Del(usize),
    Ins(usize),
}

/// Max cells of the LCS table before falling back to a coarse diff.
const LCS_LIMIT: usize = 4_000_000;
/// Context lines around each hunk.
const CTX: usize = 2;

/// LCS-based line diff with common prefix/suffix trimming.
fn diff_ops(a: &[&str], b: &[&str]) -> Vec<Op> {
    let mut pre = 0;
    while pre < a.len() && pre < b.len() && a[pre] == b[pre] {
        pre += 1;
    }
    let mut suf = 0;
    while suf < a.len() - pre && suf < b.len() - pre && a[a.len() - 1 - suf] == b[b.len() - 1 - suf] {
        suf += 1;
    }

    let mut ops: Vec<Op> = (0..pre).map(|i| Op::Eq(i, i)).collect();
    let am = &a[pre..a.len() - suf];
    let bm = &b[pre..b.len() - suf];
    let (m, n) = (am.len(), bm.len());

    if m.saturating_mul(n) > LCS_LIMIT {
        ops.extend((0..m).map(|i| Op::Del(pre + i)));
        ops.extend((0..n).map(|j| Op::Ins(pre + j)));
    } else {
        // dp[i][j] = LCS length of am[i..], bm[j..]
        let w = n + 1;
        let mut dp = vec![0u32; (m + 1) * w];
        for i in (0..m).rev() {
            for j in (0..n).rev() {
                dp[i * w + j] = if am[i] == bm[j] {
                    dp[(i + 1) * w + j + 1] + 1
                } else {
                    dp[(i + 1) * w + j].max(dp[i * w + j + 1])
                };
            }
        }
        let (mut i, mut j) = (0, 0);
        while i < m || j < n {
            if i < m && j < n && am[i] == bm[j] {
                ops.push(Op::Eq(pre + i, pre + j));
                i += 1;
                j += 1;
            } else if j < n && (i == m || dp[i * w + j + 1] >= dp[(i + 1) * w + j]) {
                ops.push(Op::Ins(pre + j));
                j += 1;
            } else {
                ops.push(Op::Del(pre + i));
                i += 1;
            }
        }
    }

    let (ao, bo) = (a.len() - suf, b.len() - suf);
    ops.extend((0..suf).map(|k| Op::Eq(ao + k, bo + k)));
    ops
}

/// Generate a compact unified diff (with hunks and 2 lines of context).
/// Returns an empty string when there are no changes.
pub fn unified_diff(label: &str, old: &str, new: &str) -> String {
    let a: Vec<&str> = old.lines().collect();
    let b: Vec<&str> = new.lines().collect();
    let ops = diff_ops(&a, &b);
    let is_eq = |o: &Op| matches!(o, Op::Eq(..));
    if ops.iter().all(is_eq) {
        return String::new();
    }

    // Prefix counts of consumed old/new lines before each op
    let mut old_pos = Vec::with_capacity(ops.len() + 1);
    let mut new_pos = Vec::with_capacity(ops.len() + 1);
    let (mut o, mut nw) = (0usize, 0usize);
    for op in &ops {
        old_pos.push(o);
        new_pos.push(nw);
        match op {
            Op::Eq(..) => { o += 1; nw += 1; }
            Op::Del(_) => o += 1,
            Op::Ins(_) => nw += 1,
        }
    }

    let mut out = format!("--- a/{label}\n+++ b/{label}\n");
    let n = ops.len();
    let mut idx = 0;
    while idx < n {
        let Some(first_change) = (idx..n).find(|&k| !is_eq(&ops[k])) else { break };
        let start = first_change.saturating_sub(CTX).max(idx);

        // Extend the hunk while equal-runs between changes are short
        let mut last_change = first_change;
        let mut k = first_change;
        while k < n {
            if !is_eq(&ops[k]) {
                last_change = k;
                k += 1;
                continue;
            }
            let run = k;
            while k < n && is_eq(&ops[k]) {
                k += 1;
            }
            if k >= n || k - run > 2 * CTX {
                break;
            }
        }
        let end = (last_change + 1 + CTX).min(n);

        let hunk = &ops[start..end];
        let old_count = hunk.iter().filter(|o| !matches!(o, Op::Ins(_))).count();
        let new_count = hunk.iter().filter(|o| !matches!(o, Op::Del(_))).count();
        out.push_str(&format!(
            "@@ -{},{} +{},{} @@\n",
            old_pos[start] + usize::from(old_count > 0), old_count,
            new_pos[start] + usize::from(new_count > 0), new_count
        ));
        for op in hunk {
            match *op {
                Op::Eq(i, _) => { out.push(' '); out.push_str(a[i]); }
                Op::Del(i) => { out.push('-'); out.push_str(a[i]); }
                Op::Ins(j) => { out.push('+'); out.push_str(b[j]); }
            }
            out.push('\n');
        }
        idx = end;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diff_detects_insertion_without_shifting() {
        let d = unified_diff("f", "a\nb\nc\nd\n", "a\nb\nX\nc\nd\n");
        assert!(d.contains("+X"));
        assert!(!d.contains("-c"));
        assert!(d.contains("@@ -1,4 +1,5 @@"));
    }

    #[test]
    fn diff_empty_when_equal() {
        assert_eq!(unified_diff("f", "a\nb", "a\nb"), "");
    }

    #[test]
    fn diff_empty_ranges_use_zero_based_positions() {
        assert!(unified_diff("f", "", "new\n").contains("@@ -0,0 +1,1 @@"));
        assert!(unified_diff("f", "old\n", "").contains("@@ -1,1 +0,0 @@"));
    }

    #[test]
    fn read_only_blocks_writes() {
        let p = SecurityPolicy { read_only: true, ..Default::default() };
        assert!(p.check_write("x.txt").is_err());
        assert!(p.check_read("x.txt").is_ok());
    }

    #[test]
    fn sandbox_blocks_escape() {
        let p = SecurityPolicy { allowed_dirs: vec![PathBuf::from("sandbox")], ..Default::default() };
        assert!(p.check_path("sandbox/a.txt").is_ok());
        assert!(p.check_path("sandbox/../secret.txt").is_err());
        assert!(p.check_path("other/a.txt").is_err());
    }
}
