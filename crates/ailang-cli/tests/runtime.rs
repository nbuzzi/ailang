use serde_json::{json, Value};
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};

fn cli(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ailang"))
        .current_dir(dir)
        .args(args)
        .output()
        .unwrap()
}
fn success(out: Output) -> String {
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap()
}
fn denied(out: Output) {
    assert_eq!(
        out.status.code(),
        Some(11),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let error: Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(error["code"], 11);
}
fn quote(path: &Path) -> String {
    serde_json::to_string(&path.to_string_lossy().replace('\\', "/")).unwrap()
}

#[test]
fn readonly_blocks_every_mutation_route() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("source"), "old\n").unwrap();
    for expr in [
        "\"changed\" > output",
        "\"changed\" >> source",
        "write output changed",
        "replace source old new",
        "replace_all source old new",
        "replace_block source old new",
        "insert_after source old new",
        "insert_before source old new",
        "prepend source new",
        "append source new",
        "patch source bad",
        "cp source output",
        "mv source output",
        "rm source",
        "\"source\" | rm",
        "\"source\" | cp output",
        "exec missing_program",
        "net.post http://127.0.0.1:1 data",
        "net.download http://127.0.0.1:1 output",
    ] {
        denied(cli(dir.path(), &["--read-only", expr]));
        assert_eq!(
            fs::read_to_string(dir.path().join("source")).unwrap(),
            "old\n",
            "{expr}"
        );
        assert!(!dir.path().join("output").exists(), "{expr}");
    }
}

#[test]
fn dryrun_returns_diff_without_writing_targets() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("source"), "old\n").unwrap();
    for expr in [
        "replace source old new",
        "\"new\" > source",
        "\"new\" >> source",
        "append source new",
    ] {
        let result: Value =
            serde_json::from_str(&success(cli(dir.path(), &["--dry-run", expr]))).unwrap();
        assert!(result["dry_run"][0]["diff"]
            .as_str()
            .unwrap()
            .contains("+new"));
        assert_eq!(
            fs::read_to_string(dir.path().join("source")).unwrap(),
            "old\n"
        );
    }
    for expr in [
        "\"new\" > nested/output",
        "write output new",
        "cp source output",
        "mv source output",
        "rm source",
        "exec missing_program",
        "net.post http://127.0.0.1:1 data",
        "net.download http://127.0.0.1:1 output",
    ] {
        let result: Value =
            serde_json::from_str(&success(cli(dir.path(), &["--dry-run", expr]))).unwrap();
        assert_eq!(result["dry_run"].as_array().unwrap().len(), 1);
        assert!(dir.path().join("source").exists());
        assert!(!dir.path().join("output").exists());
        assert!(!dir.path().join("nested").exists());
    }
}

#[test]
fn dryrun_validates_edit_anchors() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("source"), "old\n").unwrap();
    let out = cli(dir.path(), &["--dry-run", "replace source missing new"]);
    assert_eq!(out.status.code(), Some(12));
    assert_eq!(
        fs::read_to_string(dir.path().join("source")).unwrap(),
        "old\n"
    );
}

#[test]
fn script_mode_applies_global_flags_and_checks_script_path() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("script.al"), "\"new\" > output").unwrap();
    denied(cli(dir.path(), &["run", "script.al", "--read-only"]));
    success(cli(dir.path(), &["run", "script.al", "--dry-run"]));
    assert!(!dir.path().join("output").exists());
    fs::create_dir(dir.path().join("safe")).unwrap();
    denied(cli(
        dir.path(),
        &["--allow-dir", "safe", "run", "script.al"],
    ));
}

#[test]
fn sandbox_checks_reads_writes_traversal_and_subprocesses() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join("safe")).unwrap();
    fs::write(dir.path().join("secret"), "secret").unwrap();
    fs::write(dir.path().join("safe/source"), "old").unwrap();
    success(cli(
        dir.path(),
        &["--allow-dir", "safe", "\"new\" > safe/nested/output"],
    ));
    assert_eq!(
        fs::read_to_string(dir.path().join("safe/nested/output")).unwrap(),
        "new"
    );
    success(cli(
        dir.path(),
        &["--allow-dir", "safe", "cp safe/source safe/copy"],
    ));
    for expr in [
        "< secret",
        "< safe/../secret",
        "head 1 secret",
        "tail 1 secret",
        "lines secret",
        "exists secret",
        "stat secret",
        "diff safe/source secret",
        "search . secret",
        "tree . 1",
        "walk .",
        "glob **/*",
        "cp secret safe/stolen",
        "cp safe/source stolen",
        "mv safe/source stolen",
        "\"secret\" | rm",
        "cd safe",
        "exec missing_program",
        "\"new\" > secret",
    ] {
        denied(cli(dir.path(), &["--allow-dir", "safe", expr]));
    }
    assert_eq!(
        fs::read_to_string(dir.path().join("secret")).unwrap(),
        "secret"
    );
}

#[test]
fn sandbox_resolves_directory_links_before_dotdot_and_new_files() {
    let dir = tempfile::tempdir().unwrap();
    let safe = dir.path().join("safe");
    let outside = dir.path().join("outside");
    fs::create_dir(&safe).unwrap();
    fs::create_dir(&outside).unwrap();
    let link = safe.join("link");
    #[cfg(windows)]
    {
        let status = Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(&link)
            .arg(&outside)
            .output()
            .unwrap();
        assert!(
            status.status.success(),
            "{}",
            String::from_utf8_lossy(&status.stderr)
        );
    }
    #[cfg(unix)]
    std::os::unix::fs::symlink(&outside, &link).unwrap();
    for expr in [
        "\"new\" > safe/link/output",
        "\"new\" > safe/link/nested/output",
        "\"new\" > safe/link/../escaped",
        "walk safe",
        "rm safe",
    ] {
        denied(cli(dir.path(), &["--allow-dir", "safe", expr]));
    }
    assert_eq!(fs::read_dir(outside).unwrap().count(), 0);
    // Remove only the link; the temporary directory owns both verified targets.
    #[cfg(windows)]
    fs::remove_dir(link).unwrap();
    #[cfg(unix)]
    fs::remove_file(link).unwrap();
}

#[test]
fn no_net_blocks_http_and_exec_before_side_effects() {
    let dir = tempfile::tempdir().unwrap();
    for expr in [
        "net.get http://127.0.0.1:1",
        "net.post http://127.0.0.1:1 data",
        "net.download http://127.0.0.1:1 output",
        "exec missing_program",
    ] {
        denied(cli(dir.path(), &["--no-net", expr]));
    }
}

#[test]
fn glob_grep_reads_all_files_and_keeps_line_metadata() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join("logs")).unwrap();
    fs::write(dir.path().join("logs/a.log"), "ok\nERROR alpha\n").unwrap();
    fs::write(dir.path().join("logs/b.log"), "ERROR beta\n").unwrap();
    fs::create_dir(dir.path().join("logs/directory.log")).unwrap();
    let expr =
        "glob logs/**/*.log | grep ERROR | map({file:.path, line:.num, text:.line}) > matches.json";
    success(cli(dir.path(), &[expr]));
    let matches: Value =
        serde_json::from_slice(&fs::read(dir.path().join("matches.json")).unwrap()).unwrap();
    assert_eq!(matches.as_array().unwrap().len(), 2);
    assert_eq!(matches[0]["line"], 2);
    assert_eq!(matches[1]["text"], "ERROR beta");
    assert!(matches[0]["file"].as_str().unwrap().ends_with("logs/a.log"));
    assert_eq!(
        success(cli(
            dir.path(),
            &["glob logs/a.log | grep ERROR | grep alpha | count"]
        ))
        .trim(),
        "1"
    );
}

#[test]
fn glob_supports_absolute_roots_in_sandbox() {
    let dir = tempfile::tempdir().unwrap();
    let logs = dir.path().join("logs");
    fs::create_dir(&logs).unwrap();
    fs::write(logs.join("a.log"), "ERROR alpha\n").unwrap();
    let expr = format!(
        "glob {} | grep ERROR | count",
        quote(&logs.join("**/*.log"))
    );
    assert_eq!(
        success(cli(
            dir.path(),
            &["--allow-dir", logs.to_str().unwrap(), &expr]
        ))
        .trim(),
        "1"
    );
}

#[test]
fn search_current_directory_does_not_skip_root() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("a.txt"), "needle").unwrap();
    assert!(success(cli(dir.path(), &["search needle"])).contains("a.txt:1:needle"));
}

#[test]
fn json_sink_preserves_objects_arrays_and_scalars() {
    let dir = tempfile::tempdir().unwrap();
    for value in [
        json!({"id":1}),
        json!([{"id":1}]),
        json!([]),
        json!({}),
        json!(true),
        json!(10),
        Value::Null,
    ] {
        fs::write(dir.path().join("input"), value.to_string()).unwrap();
        success(cli(dir.path(), &["< input | json > output"]));
        let actual: Value =
            serde_json::from_slice(&fs::read(dir.path().join("output")).unwrap()).unwrap();
        assert_eq!(actual, value);
    }
}

#[test]
fn field_arguments_and_numeric_sort_work_in_pipelines() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("input"),
        r#"[{"level":10},{"level":2},{"level":1}]"#,
    )
    .unwrap();
    let result: Value = serde_json::from_str(&success(cli(
        dir.path(),
        &["< input | json | sort(.level)"],
    )))
    .unwrap();
    assert_eq!(result, json!([{"level":1},{"level":2},{"level":10}]));
    let result: Value = serde_json::from_str(&success(cli(
        dir.path(),
        &["< input | json | group(.level)"],
    )))
    .unwrap();
    assert!(result["10"].is_array());
    fs::write(dir.path().join("input"), r#"{"level":1}"#).unwrap();
    success(cli(
        dir.path(),
        &["< input | json | set(.level, 2) > output"],
    ));
    assert_eq!(
        fs::read_to_string(dir.path().join("output")).unwrap(),
        r#"{"level":2}"#
    );
}

#[test]
fn exec_passes_arguments_without_shell_interpretation() {
    let dir = tempfile::tempdir().unwrap();
    let expr = format!(
        "exec({}, [\"'hello & goodbye' | upper\"])",
        quote(Path::new(env!("CARGO_BIN_EXE_ailang")))
    );
    assert_eq!(success(cli(dir.path(), &[&expr])).trim(), "HELLO & GOODBYE");
}

#[test]
fn bench_measures_source_without_executing_it() {
    let dir = tempfile::tempdir().unwrap();
    let result: Value =
        serde_json::from_str(&success(cli(dir.path(), &["bench", "\"new\" > output"]))).unwrap();
    assert_eq!(result["executed"], false);
    assert!(result["tokens"].is_null());
    assert!(!dir.path().join("output").exists());
    assert!(!cli(dir.path(), &["bench", "|"]).status.success());
}

#[test]
fn edits_fail_cleanly_and_patch_checks_context() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("source"), "old\n").unwrap();
    for expr in [
        "replace source 'longer\\nthan\\nsource' new",
        "patch source '@@ -1,1 +1,1 @@\\n-wrong\\n+new\\n'",
        "patch source 'garbage'",
        "patch source '@@ -1,1 +1,1 @@\\nébad\\n'",
        "patch source '@@ -1,2 +1,1 @@\\n-old\\n+new\\n'",
    ] {
        let out = cli(dir.path(), &[expr]);
        assert_eq!(
            out.status.code(),
            Some(12),
            "{expr}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(
            fs::read_to_string(dir.path().join("source")).unwrap(),
            "old\n"
        );
    }
    success(cli(
        dir.path(),
        &["patch source '@@ -1,1 +1,1 @@\\n-old\\n+new\\n'"],
    ));
    assert_eq!(
        fs::read_to_string(dir.path().join("source")).unwrap(),
        "new\n"
    );
}

fn mcp(dir: &Path, flags: &[&str], requests: &[Value]) -> Vec<Value> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_ailang"))
        .current_dir(dir)
        .args(flags)
        .args(["serve", "--mcp"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    for request in requests {
        writeln!(stdin, "{request}").unwrap();
    }
    drop(stdin);
    let output = success(child.wait_with_output().unwrap());
    output
        .lines()
        .map(|s| serde_json::from_str(s).unwrap())
        .collect()
}
fn call(id: i32, expr: &str) -> Value {
    json!({"jsonrpc":"2.0", "id":id, "method":"tools/call", "params":{"name":"ailang", "arguments":{"expr":expr}}})
}

#[test]
fn mcp_enforces_server_policy_and_marks_tool_errors() {
    let dir = tempfile::tempdir().unwrap();
    let responses = mcp(
        dir.path(),
        &["--read-only"],
        &[call(1, "\"new\" > output"), call(2, "'hello' | upper")],
    );
    assert_eq!(responses[0]["result"]["isError"], true);
    assert_eq!(responses[1]["result"]["isError"], false);
    assert!(!dir.path().join("output").exists());
    fs::create_dir(dir.path().join("safe")).unwrap();
    let responses = mcp(
        dir.path(),
        &["--allow-dir", "safe", "--no-net"],
        &[
            call(1, "< secret"),
            call(2, "'new' > output"),
            call(3, "exec missing_program"),
            call(4, "net.get http://127.0.0.1:1"),
            call(5, "'new' > safe/output"),
        ],
    );
    for response in &responses[..4] {
        assert_eq!(response["result"]["isError"], true);
    }
    assert_eq!(responses[4]["result"]["isError"], false);
    assert_eq!(
        fs::read_to_string(dir.path().join("safe/output")).unwrap(),
        "new"
    );
    let responses = mcp(
        dir.path(),
        &["--dry-run"],
        &[call(1, "\"new\" > output"), call(2, "pwd")],
    );
    let first: Value = serde_json::from_str(
        responses[0]["result"]["content"][0]["text"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    let second: Value = serde_json::from_str(
        responses[1]["result"]["content"][0]["text"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(first["dry_run"].as_array().unwrap().len(), 1);
    assert_eq!(second["dry_run"], json!([]));
    assert!(!dir.path().join("output").exists());
}

#[test]
fn mcp_does_not_execute_tool_notifications_or_unknown_tools() {
    let dir = tempfile::tempdir().unwrap();
    let notification = json!({"jsonrpc":"2.0", "method":"tools/call", "params":{"name":"ailang", "arguments":{"expr":"'new' > output"}}});
    let unknown = json!({"jsonrpc":"2.0", "id":1, "method":"tools/call", "params":{"name":"unknown", "arguments":{"expr":"'new' > output"}}});
    let responses = mcp(dir.path(), &[], &[notification, unknown]);
    assert_eq!(responses.len(), 1);
    assert_eq!(responses[0]["result"]["isError"], true);
    assert!(!dir.path().join("output").exists());
}

#[test]
fn patch_preserves_crlf_and_supports_insertions_and_multiple_hunks() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("source"), "a\r\nb\r\nc\r\nd\r\n").unwrap();
    success(cli(
        dir.path(),
        &["patch source '@@ -1,1 +1,1 @@\\n-a\\n+A\\n@@ -4,1 +4,1 @@\\n-d\\n+D\\n'"],
    ));
    assert_eq!(
        fs::read_to_string(dir.path().join("source")).unwrap(),
        "A\r\nb\r\nc\r\nD\r\n"
    );
    fs::write(dir.path().join("source"), "").unwrap();
    success(cli(
        dir.path(),
        &["patch source '@@ -0,0 +1,1 @@\\n+new\\n'"],
    ));
    assert_eq!(
        fs::read_to_string(dir.path().join("source")).unwrap(),
        "new\n"
    );
}
