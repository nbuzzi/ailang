use ailang_core::error::ErrorOutput;
use ailang_eval::{eval_program, value_to_json, Context};
use ailang_parser::parse_program;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value as JsonValue};
use std::io::{self, BufRead, Write};

#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    #[allow(dead_code)]
    jsonrpc: Option<String>,
    id: Option<JsonValue>,
    method: String,
    params: Option<JsonValue>,
}

#[derive(Debug, Serialize)]
struct JsonRpcResponse {
    jsonrpc: &'static str,
    id: JsonValue,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<JsonValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonValue>,
}

/// Runs the Model Context Protocol (MCP) server over stdin/stdout.
/// Can be plugged directly into Claude Desktop, Cursor, Antigravity, or any MCP client.
pub fn run_stdio_server() -> io::Result<()> {
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let reader = stdin.lock();

    let rt = tokio::runtime::Runtime::new().expect("Failed to create async runtime");
    let handle = rt.handle().clone();
    let mut ctx = Context::new();

    for line_res in reader.lines() {
        let line = line_res?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let req: JsonRpcRequest = match serde_json::from_str(trimmed) {
            Ok(r) => r,
            Err(e) => {
                let err_resp = json!({
                    "jsonrpc": "2.0",
                    "id": JsonValue::Null,
                    "error": { "code": -32700, "message": format!("Parse error: {}", e) }
                });
                writeln!(stdout, "{}", serde_json::to_string(&err_resp)?)?;
                stdout.flush()?;
                continue;
            }
        };

        // If it's a notification without an id (e.g. notifications/initialized)
        let is_notification = req.id.is_none();
        let id = req.id.unwrap_or(JsonValue::Null);

        match req.method.as_str() {
            "initialize" => {
                let resp = JsonRpcResponse {
                    jsonrpc: "2.0",
                    id,
                    result: Some(json!({
                        "protocolVersion": "2024-11-05",
                        "capabilities": {
                            "tools": {}
                        },
                        "serverInfo": {
                            "name": "ailang-mcp",
                            "version": env!("CARGO_PKG_VERSION")
                        }
                    })),
                    error: None,
                };
                writeln!(stdout, "{}", serde_json::to_string(&resp)?)?;
                stdout.flush()?;
            }

            "notifications/initialized" => {
                // MCP client initialized confirmation - no response needed
            }

            "ping" => {
                let resp = JsonRpcResponse {
                    jsonrpc: "2.0",
                    id,
                    result: Some(json!({})),
                    error: None,
                };
                writeln!(stdout, "{}", serde_json::to_string(&resp)?)?;
                stdout.flush()?;
            }

            "tools/list" => {
                let resp = JsonRpcResponse {
                    jsonrpc: "2.0",
                    id,
                    result: Some(json!({
                        "tools": [
                            {
                                "name": "ailang",
                                "description": "Execute an ailang pipeline script. Designed for AI agents: file I/O, regex, data transformation, HTTP, and system operations with up to 80% token savings.",
                                "inputSchema": {
                                    "type": "object",
                                    "properties": {
                                        "expr": {
                                            "type": "string",
                                            "description": "ailang pipeline expression. Examples: read('config.yaml') | yaml | set(.v, '2.0') > 'config.yaml' OR glob('**/*.log') | grep('ERROR') | json"
                                        }
                                    },
                                    "required": ["expr"]
                                }
                            }
                        ]
                    })),
                    error: None,
                };
                writeln!(stdout, "{}", serde_json::to_string(&resp)?)?;
                stdout.flush()?;
            }

            "tools/call" => {
                let expr_opt = req.params
                    .as_ref()
                    .and_then(|p| p.get("arguments"))
                    .and_then(|args| args.get("expr"))
                    .and_then(|e| e.as_str());

                let result_text = match expr_opt {
                    Some(expr) => match parse_program(expr) {
                        Ok(program) => match eval_program(&program, &mut ctx, &handle) {
                            Ok(values) => {
                                let json_vals: Vec<_> = values.iter().map(value_to_json).collect();
                                match json_vals.as_slice() {
                                    [] => "{}".to_string(),
                                    [single] => serde_json::to_string(single).unwrap_or_default(),
                                    multiple => serde_json::to_string(multiple).unwrap_or_default(),
                                }
                            }
                            Err(e) => {
                                let err = ErrorOutput::from(&e);
                                serde_json::to_string(&err).unwrap_or_else(|_| e.to_string())
                            }
                        },
                        Err(e) => {
                            let err = ErrorOutput::from(&e);
                            serde_json::to_string(&err).unwrap_or_else(|_| e.to_string())
                        }
                    },
                    None => {
                        serde_json::to_string(&json!({
                            "error": "Missing required argument 'expr'",
                            "code": 2
                        })).unwrap_or_default()
                    }
                };

                let resp = JsonRpcResponse {
                    jsonrpc: "2.0",
                    id,
                    result: Some(json!({
                        "content": [
                            {
                                "type": "text",
                                "text": result_text
                            }
                        ]
                    })),
                    error: None,
                };
                writeln!(stdout, "{}", serde_json::to_string(&resp)?)?;
                stdout.flush()?;
            }

            _ => {
                if !is_notification {
                    let resp = JsonRpcResponse {
                        jsonrpc: "2.0",
                        id,
                        result: None,
                        error: Some(json!({
                            "code": -32601,
                            "message": format!("Method not found: {}", req.method)
                        })),
                    };
                    writeln!(stdout, "{}", serde_json::to_string(&resp)?)?;
                    stdout.flush()?;
                }
            }
        }
    }

    Ok(())
}
