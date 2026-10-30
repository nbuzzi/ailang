use ailang_core::{error::{AilangError, AilangResult}, Value};
use std::process::Command;

/// exec("cmd", ["arg1", "arg2"]) → Value::Str with stdout
pub fn exec(args: &[Value], _input: Option<Value>) -> AilangResult<Vec<Value>> {
    let cmd = get_str_arg(args, 0, "exec")?;

    let mut command = if cfg!(target_os = "windows") {
        let mut c = Command::new("cmd");
        c.args(["/C", &cmd]);
        c
    } else {
        let mut c = Command::new("sh");
        c.args(["-c", &cmd]);
        c
    };

    // Additional args as extra parameters
    if let Some(Value::List(extra_args)) = args.get(1) {
        for a in extra_args {
            command.arg(a.to_string());
        }
    }

    let output = command.output()?;

    if output.status.success() {
        Ok(vec![Value::Str(String::from_utf8_lossy(&output.stdout).into_owned())])
    } else {
        Err(AilangError::RuntimeError {
            stage: "exec".into(),
            msg: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }
}

/// env("VAR") → Value::Str with environment variable value
pub fn env(args: &[Value], _input: Option<Value>) -> AilangResult<Vec<Value>> {
    let key = get_str_arg(args, 0, "env")?;
    match std::env::var(&key) {
        Ok(val) => Ok(vec![Value::Str(val)]),
        Err(_) => Ok(vec![Value::Null]),
    }
}

/// pwd() → Value::Str with current working directory
pub fn pwd(_args: &[Value], _input: Option<Value>) -> AilangResult<Vec<Value>> {
    let cwd = std::env::current_dir()?;
    Ok(vec![Value::Str(cwd.to_string_lossy().replace('\\', "/"))])
}

/// cd("path") → changes working directory
pub fn cd(args: &[Value], _input: Option<Value>) -> AilangResult<Vec<Value>> {
    let path = get_str_arg(args, 0, "cd")?;
    std::env::set_current_dir(&path)?;
    Ok(vec![])
}

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
    fn test_env_path() {
        let result = env(&[Value::Str("PATH".into())], None).unwrap();
        // PATH should exist on any OS
        assert!(matches!(&result[0], Value::Str(s) if !s.is_empty()));
    }

    #[test]
    fn test_env_missing() {
        let result = env(&[Value::Str("NONEXISTENT_VAR_XYZ_123".into())], None).unwrap();
        assert_eq!(result[0], Value::Null);
    }
}
