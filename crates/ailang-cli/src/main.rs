use ailang_core::error::ErrorOutput;
use ailang_eval::{eval_pipeline, value_to_json, Context};
use ailang_parser::parse;
use clap::{Parser, Subcommand};
use std::process;

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

    /// Output format
    #[arg(long, default_value = "auto", value_name = "FORMAT",
          help = "Output format: auto | json | raw | pretty")]
    output: String,
}

#[derive(Subcommand)]
enum Commands {
    /// Execute an ailang script file
    Run {
        /// Path to the .al script file
        #[arg(value_name = "FILE")]
        file: String,
    },
    /// Show quick reference (AI prompt sheet)
    Ref,
}

fn main() {
    let cli = Cli::parse();

    // Build the tokio runtime for async operations (net.get, net.post, etc.)
    let rt = tokio::runtime::Runtime::new().expect("Failed to create async runtime");
    let handle = rt.handle().clone();

    // Determine the expression to run
    let expr = match (&cli.expression, &cli.command) {
        (Some(expr), None) => expr.clone(),
        (None, Some(Commands::Run { file })) => {
            match std::fs::read_to_string(file) {
                Ok(content) => content,
                Err(e) => {
                    emit_error(&format!("Cannot read file '{}': {}", file, e), 5);
                    process::exit(1);
                }
            }
        }
        (None, Some(Commands::Ref)) => {
            print_reference();
            return;
        }
        _ => {
            eprintln!("Usage: ailang \"expression\" or ailang run script.al");
            eprintln!("       ailang ref  — show quick reference");
            process::exit(1);
        }
    };

    // Parse
    let pipeline = match parse(&expr) {
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
            output_values(&values, &cli.output);
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
        // auto: raw for single string, minified JSON for everything else
        _ => {
            match values {
                [ailang_core::Value::Str(s)] => print!("{}", s),
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

fn print_reference() {
    println!(r#"
ailang quick reference — paste this in your AI system prompt (~75 tokens):

  ailang "<expression>"
  fs:     read(p) write(p) glob(pat) walk(d) cp(dst) mv(dst) rm exists(p) stat diff(a,b)
  text:   grep(re) sed(re,r) lines count trim upper lower slice(n,m)
  data:   json yaml csv map({{k:.f}}) filter(.f>v) sort(.f) group(.f) set(.f,v) unique assert(v)
  net:    net.get(url) net.post(url,body) net.download(url,dest)
  sys:    exec(cmd) env(k) pwd cd(p)
  crypto: sha256 sha512 md5 base64.enc base64.dec
  ops:    | pipe   > write   >> append   ? filter   @ iterate   .field

  examples:
    ailang "glob('**/*.log') | grep('ERROR') | map({{file:.path,err:.line}}) > 'out.json'"
    ailang "read('config.yaml') | yaml | set(.version,'2.0') > 'config.yaml'"
    ailang "net.get('https://api.example.com/data') | json | filter(.active == true)"
    ailang "glob('src/**/*.rs') | sed('TODO','DONE') | write"
"#);
}
