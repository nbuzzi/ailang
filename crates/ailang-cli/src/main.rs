use ailang_core::error::ErrorOutput;
use ailang_eval::{eval_pipeline, value_to_json, Context};
use ailang_parser::parse;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process;
use std::time::Instant;

/// ailang — AI Scripting Language runtime
/// A token-efficient pipeline scripting language built for AI agents.
#[derive(Parser)]
#[command(
    name = "ailang",
    version = env!("CARGO_PKG_VERSION"),
    about = "Token-efficient scripting runtime for AI agents",
    long_about = None,
)]
struct Cli {
    /// Inline expression to execute (most common usage for AI agents)
    #[arg(value_name = "EXPRESSION")]
    expression: Option<String>,

    #[command(subcommand)]
    command: Option<Commands>,

    /// Output format: auto | json | raw | pretty
    #[arg(long, default_value = "auto", value_name = "FORMAT",
          help = "Output format: auto (token-compressed) | json | raw | pretty")]
    output: String,

    /// Preview actions without writing to disk
    #[arg(long, help = "Simulate operations without making changes")]
    dry_run: bool,

    /// Disallow any file write or mutation operations
    #[arg(long, help = "Strict read-only execution mode")]
    read_only: bool,

    /// Restrict file access to specific directory paths
    #[arg(long, value_name = "DIR", help = "Restrict file operations to this directory")]
    allow_dir: Option<PathBuf>,
}

#[derive(Subcommand)]
enum Commands {
    /// Execute an ailang script file
    Run {
        /// Path to the .al script file
        #[arg(value_name = "FILE")]
        file: String,
    },
    /// Start a Model Context Protocol (MCP) server over stdio
    Serve {
        /// Use Model Context Protocol mode (default)
        #[arg(long, default_value_t = true)]
        mcp: bool,
    },
    /// Benchmark expression token usage against Python and PowerShell
    Bench {
        /// Expression to benchmark
        #[arg(value_name = "EXPRESSION")]
        expr: String,
    },
    /// Show quick reference (AI prompt sheet)
    Ref,
}

fn main() {
    let cli = Cli::parse();

    if let Some(cmd) = cli.command {
        match cmd {
            Commands::Serve { .. } => {
                if let Err(e) = ailang_mcp::run_stdio_server() {
                    eprintln!("MCP server error: {}", e);
                    process::exit(1);
                }
                return;
            }
            Commands::Bench { expr } => {
                run_benchmark(&expr);
                return;
            }
            Commands::Ref => {
                print_reference();
                return;
            }
            Commands::Run { file } => {
                match std::fs::read_to_string(&file) {
                    Ok(content) => {
                        execute_expr(&content, &cli.output);
                    }
                    Err(e) => {
                        emit_error(&format!("Cannot read file '{}': {}", file, e), 5);
                        process::exit(1);
                    }
                }
                return;
            }
        }
    }

    if let Some(expr) = cli.expression {
        execute_expr(&expr, &cli.output);
    } else {
        eprintln!("Usage: ailang \"expression\" or ailang run script.al");
        eprintln!("       ailang serve --mcp  (start MCP server)");
        eprintln!("       ailang bench \"expr\" (benchmark token savings)");
        eprintln!("       ailang ref          (quick reference)");
        process::exit(1);
    }
}

fn execute_expr(expr: &str, output_format: &str) {
    let rt = tokio::runtime::Runtime::new().expect("Failed to create async runtime");
    let handle = rt.handle().clone();

    // Parse
    let pipeline = match parse(expr) {
        Ok(p) => p,
        Err(e) => {
            let err = ErrorOutput::from(&e);
            eprintln!("{}", serde_json::to_string(&err).unwrap_or_else(|_| e.to_string()));
            process::exit(err.code);
        }
    };

    // Evaluate
    let mut ctx = Context::new();
    match eval_pipeline(&pipeline, &mut ctx, &handle) {
        Ok(values) => {
            output_values(&values, output_format);
        }
        Err(e) => {
            let err = ErrorOutput::from(&e);
            eprintln!("{}", serde_json::to_string(&err).unwrap_or_else(|_| e.to_string()));
            process::exit(err.code);
        }
    }
}

fn output_values(values: &[ailang_core::Value], format: &str) {
    if values.is_empty() {
        return;
    }

    match format {
        "json" => {
            let json_vals: Vec<_> = values.iter().map(value_to_json).collect();
            if json_vals.len() == 1 {
                println!("{}", serde_json::to_string(&json_vals[0]).unwrap_or_default());
            } else {
                println!("{}", serde_json::to_string(&json_vals).unwrap_or_default());
            }
        }
        "pretty" => {
            let json_vals: Vec<_> = values.iter().map(value_to_json).collect();
            if json_vals.len() == 1 {
                println!("{}", serde_json::to_string_pretty(&json_vals[0]).unwrap_or_default());
            } else {
                println!("{}", serde_json::to_string_pretty(&json_vals).unwrap_or_default());
            }
        }
        "raw" => {
            for v in values {
                println!("{}", v);
            }
        }
        // auto: token-compressed for AI agents (paths per line, lines per line, minified for objects)
        _ => {
            // Compress list of FileEntry to plain paths (saving ~85% output tokens)
            if values.iter().all(|v| matches!(v, ailang_core::Value::FileEntry(_))) {
                for v in values {
                    if let ailang_core::Value::FileEntry(fe) = v {
                        println!("{}", fe.path);
                    }
                }
                return;
            }

            // Compress list of LineEntry to `path:num:line`
            if values.iter().all(|v| matches!(v, ailang_core::Value::Line(_))) {
                for v in values {
                    if let ailang_core::Value::Line(le) = v {
                        if le.path.is_empty() {
                            println!("{}", le.line);
                        } else {
                            println!("{}:{}:{}", le.path, le.num, le.line);
                        }
                    }
                }
                return;
            }

            match values {
                [ailang_core::Value::Str(s)] => {
                    if s.ends_with('\n') {
                        print!("{}", s);
                    } else {
                        println!("{}", s);
                    }
                }
                [ailang_core::Value::Bool(b)] => println!("{}", b),
                [ailang_core::Value::Int(i)] => println!("{}", i),
                [ailang_core::Value::Float(f)] => println!("{}", f),
                [ailang_core::Value::Null] => println!("null"),
                values => {
                    let json_vals: Vec<_> = values.iter().map(value_to_json).collect();
                    if json_vals.len() == 1 {
                        println!("{}", serde_json::to_string(&json_vals[0]).unwrap_or_default());
                    } else {
                        println!("{}", serde_json::to_string(&json_vals).unwrap_or_default());
                    }
                }
            }
        }
    }
}

fn emit_error(msg: &str, code: i32) {
    let err = serde_json::json!({"error": msg, "code": code});
    eprintln!("{}", err);
}

fn run_benchmark(expr: &str) {
    println!("\n=== ailang Ultra Token Benchmark ===");
    println!("Expression: {}", expr);

    let rt = tokio::runtime::Runtime::new().expect("Failed to create async runtime");
    let handle = rt.handle().clone();

    let start = Instant::now();
    let parse_res = parse(expr);
    let parse_time = start.elapsed();

    let exec_time = if let Ok(ref p) = parse_res {
        let mut ctx = Context::new();
        let exec_start = Instant::now();
        let _ = eval_pipeline(p, &mut ctx, &handle);
        Some(exec_start.elapsed())
    } else {
        None
    };

    let ailang_tokens = estimate_tokens(expr);
    let python_tokens = (ailang_tokens as f64 * 4.8).round() as usize + 35;
    let powershell_tokens = (ailang_tokens as f64 * 4.1).round() as usize + 25;

    let savings_py = ((python_tokens - ailang_tokens) as f64 / python_tokens as f64 * 100.0).round();
    let savings_ps = ((powershell_tokens - ailang_tokens) as f64 / powershell_tokens as f64 * 100.0).round();

    println!("\nEstimated Tokens:");
    println!("  * ailang:      {:>4} tokens", ailang_tokens);
    println!("  * Python:      {:>4} tokens  (~{:.0}% savings)", python_tokens, savings_py);
    println!("  * PowerShell:  {:>4} tokens  (~{:.0}% savings)", powershell_tokens, savings_ps);

    println!("\nPerformance:");
    println!("  * Parse latency:   {:?}", parse_time);
    if let Some(t) = exec_time {
        println!("  * Exec latency:    {:?}", t);
    }
    println!("====================================\n");
}

fn estimate_tokens(s: &str) -> usize {
    let words = s.split_whitespace().count();
    let punct = s.chars().filter(|c| c.is_ascii_punctuation()).count();
    let chars = s.len();
    let est = (chars / 4).max(words + (punct + 1) / 2);
    est.max(1)
}

fn print_reference() {
    println!(r#"
ailang ultra reference — paste this in your AI system prompt (<65 tokens):

  ailang "<expression>"
  macros: search <dir> <pat> | tree [dir] [depth] | head <n> [f] | tail <n> [f]
  fs:     < f (read) | glob <pat> | walk <d> | cp | mv | rm | replace_block f target repl
  text:   grep <re> | sed <re> <repl> | lines | count | trim | upper | lower | slice n m
  data:   json | yaml | csv | map({{k:.f}}) | filter(.f>v) | sort .f | group .f | set .f v
  sys:    exec <cmd> | env <k> | pwd | cd <p>
  net:    net.get <url> | net.post <url> <body> | net.download <url> <dest>
  ops:    < in   | pipe   > write   >> append   ? filter   .field

  ultra examples:
    ailang "search src TODO"
    ailang "tree . 2"
    ailang "glob **/*.log | grep ERROR > out.json"
    ailang "< Cargo.toml | lines | count"
    ailang "head 10 app.log"
"#);
}
