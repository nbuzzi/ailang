use ailang_core::{error::{AilangError, AilangResult}, Value};

/// net.get("url") → Value::Str with response body
pub async fn get(args: &[Value], _input: Option<Value>) -> AilangResult<Vec<Value>> {
    let url = get_str_arg(args, 0, "net.get")?;
    let body = reqwest::get(&url)
        .await
        .map_err(|e| AilangError::HttpError(e.to_string()))?
        .text()
        .await
        .map_err(|e| AilangError::HttpError(e.to_string()))?;
    Ok(vec![Value::Str(body)])
}

/// net.post("url", body) → Value::Str with response body
pub async fn post(args: &[Value], input: Option<Value>) -> AilangResult<Vec<Value>> {
    let url = get_str_arg(args, 0, "net.post")?;
    let body = match input {
        Some(v) => v.to_string(),
        None => get_str_arg(args, 1, "net.post")?,
    };

    let client = reqwest::Client::new();
    let response = client
        .post(&url)
        .body(body)
        .header("Content-Type", "application/json")
        .send()
        .await
        .map_err(|e| AilangError::HttpError(e.to_string()))?
        .text()
        .await
        .map_err(|e| AilangError::HttpError(e.to_string()))?;

    Ok(vec![Value::Str(response)])
}

/// net.download("url", "dest") → downloads file to dest path
pub async fn download(args: &[Value], _input: Option<Value>) -> AilangResult<Vec<Value>> {
    let url  = get_str_arg(args, 0, "net.download")?;
    let dest = get_str_arg(args, 1, "net.download")?;

    let bytes = reqwest::get(&url)
        .await
        .map_err(|e| AilangError::HttpError(e.to_string()))?
        .bytes()
        .await
        .map_err(|e| AilangError::HttpError(e.to_string()))?;

    crate::fs::atomic_write(std::path::Path::new(&dest), &bytes)?;
    Ok(vec![Value::Str(dest)])
}

fn get_str_arg(args: &[Value], idx: usize, fn_name: &str) -> AilangResult<String> {
    args.get(idx)
        .and_then(|v| v.as_str().map(|s| s.to_string()))
        .ok_or_else(|| AilangError::RuntimeError {
            stage: fn_name.to_string(),
            msg: format!("Expected string argument at position {}", idx),
        })
}
