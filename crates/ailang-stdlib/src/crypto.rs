use ailang_core::{error::{AilangError, AilangResult}, Value};
use sha2::{Digest, Sha256, Sha512};
use md5::Md5;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

/// sha256 — compute SHA-256 hash of input string, returns hex string
pub fn sha256(input: Option<Value>) -> AilangResult<Vec<Value>> {
    let data = require_str_input(input, "sha256")?;
    let mut hasher = Sha256::new();
    hasher.update(data.as_bytes());
    let result = format!("{:x}", hasher.finalize());
    Ok(vec![Value::Str(result)])
}

/// sha512 — compute SHA-512 hash
pub fn sha512(input: Option<Value>) -> AilangResult<Vec<Value>> {
    let data = require_str_input(input, "sha512")?;
    let mut hasher = Sha512::new();
    hasher.update(data.as_bytes());
    let result = format!("{:x}", hasher.finalize());
    Ok(vec![Value::Str(result)])
}

/// md5 — compute MD5 hash
pub fn md5(input: Option<Value>) -> AilangResult<Vec<Value>> {
    let data = require_str_input(input, "md5")?;
    let mut hasher = Md5::new();
    hasher.update(data.as_bytes());
    let result = format!("{:x}", hasher.finalize());
    Ok(vec![Value::Str(result)])
}

/// base64.enc — encode input as base64
pub fn base64_enc(_args: &[Value], input: Option<Value>) -> AilangResult<Vec<Value>> {
    let data = require_str_input(input, "base64.enc")?;
    Ok(vec![Value::Str(BASE64.encode(data.as_bytes()))])
}

/// base64.dec — decode base64 input
pub fn base64_dec(_args: &[Value], input: Option<Value>) -> AilangResult<Vec<Value>> {
    let data = require_str_input(input, "base64.dec")?;
    let bytes = BASE64.decode(data.trim().as_bytes())
        .map_err(|e| AilangError::RuntimeError {
            stage: "base64.dec".into(),
            msg: e.to_string(),
        })?;
    let s = String::from_utf8(bytes).map_err(|e| AilangError::RuntimeError {
        stage: "base64.dec".into(),
        msg: e.to_string(),
    })?;
    Ok(vec![Value::Str(s)])
}

fn require_str_input(input: Option<Value>, stage: &str) -> AilangResult<String> {
    match input {
        Some(v) => Ok(v.to_string()),
        None => Err(AilangError::RuntimeError {
            stage: stage.to_string(),
            msg: "requires piped input".into(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sha256_known_value() {
        let result = sha256(Some(Value::Str("hello".into()))).unwrap();
        // Known SHA-256 of "hello"
        assert_eq!(
            result[0].to_string(),
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
    }

    #[test]
    fn test_base64_roundtrip() {
        let enc = base64_enc(&[], Some(Value::Str("hello world".into()))).unwrap();
        let dec = base64_dec(&[], Some(enc[0].clone())).unwrap();
        assert_eq!(dec[0].to_string(), "hello world");
    }
}
