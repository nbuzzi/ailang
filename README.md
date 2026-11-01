# ailang 🤖⚡

<div align="center">

**The ultra token-efficient scripting language and runtime built in Rust, engineered specifically for AI agents.**

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Built with Rust](https://img.shields.io/badge/Built%20with-Rust-orange?logo=rust)](https://www.rust-lang.org/)
[![CI](https://github.com/nbuzzi/ailang/actions/workflows/ci.yml/badge.svg)](https://github.com/nbuzzi/ailang/actions/workflows/ci.yml)
[![MCP: Native Ready](https://img.shields.io/badge/MCP-Native%20Stdio-blue?logo=protocol)]()
[![Platform: Linux%20%7C%20macOS%20%7C%20Windows](https://img.shields.io/badge/Platform-Linux%20%7C%20macOS%20%7C%20Windows-blueviolet)]()

*Replace repetitive one-off scripts with compact, reusable pipeline operations. Measure savings on your own agent workloads.*

[Key Features](#-key-features) •
[Token Benchmarks](#-comprehensive-token-benchmarks) •
[Architecture](#-architecture) •
[Language Guide](#-language-guide) •
[Code Editing & Generation](#-first-class-code-generation--surgical-editing) •
[MCP Server](#-model-context-protocol-mcp-integration) •
[Safety & Sandbox](#-agent-safety--security-sandboxing) •
[Quickstart](#-quickstart)

</div>

---

## 🛑 The Problem: The Agent Token Tax

Agent-generated scripts often repeat imports, file loops, serialization and shell wrappers. Ailang packages those operations into reusable built-ins.

When an agent needs to search log files for errors and save them as JSON:

```python
# Python example
import glob, json

results = []
for file_path in glob.glob("**/*.log", recursive=True):
    with open(file_path, "r", encoding="utf-8") as fp:
        for line_num, line in enumerate(fp, start=1):
            if "ERROR" in line:
                results.append({"file": file_path, "line": line_num, "text": line.strip()})

with open("out.json", "w", encoding="utf-8") as fp:
    json.dump(results, fp, indent=2)
```

```powershell
# PowerShell example
Get-ChildItem -Recurse -Filter *.log |
  Select-String "ERROR" |
  ForEach-Object { [PSCustomObject]@{ file = $_.Path; line = $_.LineNumber; text = $_.Line.Trim() } } |
  ConvertTo-Json | Set-Content out.json
```

```bash
# ailang pipeline (invoke with the CLI or MCP tool)
glob **/*.log | grep ERROR | map({file:.path, line:.num, text:.line}) > out.json
```

> This example illustrates less scripting boilerplate. It does not establish a token savings percentage; that depends on the model tokenizer and the complete agent interaction.

---

## 📊 Comprehensive Token Benchmarks

`ailang bench "expression"` reports UTF-8 source bytes, character count and parse latency as JSON. It does **not** execute the expression, count model tokens or infer baseline programs.

The earlier token savings table and startup/memory estimates were not backed by a reproducible measurement harness and have been removed. To measure savings, use equivalent programs with identical outputs, a named model tokenizer, and include tool descriptions, instructions, results and retries in the total. Compare against concise existing tools as well as Python and PowerShell.

The runtime is implemented in Rust and CI targets Windows, Linux and macOS. OS process behavior, filesystem rules and native library dependencies can differ. Pipelines currently materialize values in memory; they are not yet incremental streams suitable for unbounded logs.

---

## 🏗️ Architecture

```mermaid
flowchart TD
    subgraph AgentClient["AI Agent / LLM Client"]
        CLI["CLI Execution: ailang 'expr'"]
        MCP["Model Context Protocol: JSON-RPC stdio"]
        Script["Script Runner: ailang run script.al"]
    end

    subgraph ailangCore["ailang Native Rust Runtime"]
        subgraph PipelineEngine["Execution Pipeline"]
            Lexer["Zero-Copy Lexer (logos)<br/>• Command syntax<br/>• Multiline heredocs<br/>• Raw strings r'...'"]
            Parser["Recursive Descent Parser<br/>• Stream input operator<br/>• Sink redirection<br/>• Multi-pipeline support"]
            Evaluator["Pipeline Evaluator<br/>• IndexMap Determinism<br/>• Token-Compressed Output<br/>• Piped Stdin and Stdout"]
        end

        subgraph Stdlib["Standard Library (Zero Imports)"]
            FS["fs: read, write, glob, walk, stat"]
            Edit["code: replace, replace_all, insert_after/before, prepend, append, patch"]
            Inspect["inspect: search, tree, head, tail, lines"]
            Data["data: json, yaml, csv, map, filter, sort, group, set, unique, assert"]
            SysNet["sys and net: exec, env, pwd, net.get, net.post, download"]
            Crypto["crypto: sha256, sha512, md5, base64.enc/dec"]
        end

        subgraph Security["Agent Security Sandbox"]
            Policy{"Security Policy"}
            DryRun["--dry-run (LCS Diff Preview)"]
            ReadOnly["--read-only (Block Mutating Calls)"]
            AllowDir["--allow-dir (Filesystem Jailing)"]
        end
    end

    CLI --> Lexer
    MCP --> Lexer
    Script --> Lexer
    Lexer --> Parser --> Evaluator
    Evaluator --> Policy
    Policy --> Stdlib
    Policy -.-> DryRun
    Policy -.-> ReadOnly
    Policy -.-> AllowDir
```

### Agent Interaction Lifecycle

```mermaid
sequenceDiagram
    autonumber
    actor LLM as AI Agent (Claude / Cursor / Antigravity)
    participant MCP as ailang Native Engine
    participant FS as Local Filesystem & Processes

    LLM->>MCP: tools/call ailang expr='replace src/main.rs old_fn new_fn'
    activate MCP
    MCP->>FS: Read target file (auto-detect UTF-8 / BOM / UTF-16)
    FS-->>MCP: File content
    Note over MCP: Exact match attempt -> Fuzzy whitespace fallback
    MCP->>FS: Surgically write patched file
    MCP-->>LLM: JSON-RPC Result: true
    deactivate MCP
    Note over LLM: Agent continues workflow without context bloating!
```

---

## ⚡ Key Features

1. **Ultra Token Compression**:
   * Punctuation-free and parenthesis-free command-style calls (`glob **/*.log`, `grep ERROR`, `search src TODO`).
   * Stream input operator `< filename` replacing verbose read calls.
   * Auto-compressed output modes (`--output auto | json | raw | pretty`) stripping JSON whitespace.

2. **First-Class Code Editing & Generation**:
   * Surgical replacements with **fuzzy whitespace tolerance**: if an LLM outputs slightly different indentation or line breaks, `replace` matches and updates uniquely.
   * Direct code generation via multiline raw strings: `"""code""" > src/app.ts` auto-creates any missing parent directories.
   * Anchor-based insertions: `insert_after`, `insert_before`, `prepend`, `append`, and unified `patch`.

3. **Compound AI Verbs**:
   * `search src TODO`: Recursive code search that automatically ignores `.git`, `target`, `node_modules`, `dist`, and binary artifacts.
   * `tree src 2`: directory tree with configurable depth.
   * `head 10 file` & `tail 10 file`: Instant streaming file inspection.

4. **Zero-Overhead Piped Process Execution**:
   * `exec(cmd)` captures stdout and seamlessly pipes stdin from upstream stages: `< list.txt | lines | filter(!.empty) | exec("sort") > sorted.txt`.

5. **Native Model Context Protocol (MCP) Server**:
   * Run `ailang serve --mcp` to expose an instant stdio JSON-RPC tool for Claude Desktop, Cursor, Copilot, or Antigravity.

6. **Deterministic Insertion-Ordered Output**:
   * Powered by `IndexMap` to guarantee consistent JSON keys across runs, preventing cache busting in LLM context memories.

7. **Multi-Platform Encoding Resilience**:
   * Transparently detects and decodes standard UTF-8, UTF-8 BOM, and Windows UTF-16 LE/BE files.

---

## 📖 Language Guide

An ailang script consists of one or more pipelines separated by semicolons (`;`). Data flows from left to right through pipeline stages separated by `|`.

```
<source> | <transform> | <filter> | <sink>
```

### Core Syntax & Operators

| Syntax | Description | Example |
|:---|:---|:---|
| `< path` | Stream input operator (shorthand for `read`) | `< Cargo.toml \| lines \| count` |
| `\|` | Pipeline operator: passes data to next stage | `glob *.py \| grep def` |
| `> path` | Overwrite sink (auto-creates parent dirs) | `"""content""" > dist/bundle.js` |
| `>> path` | Append sink (auto-creates parent dirs) | `"export default App;" >> dist/bundle.js` |
| `"""..."""` | Multiline heredoc raw string literal | `"""fn main() {}""" > main.rs` |
| `'''...'''` | Multiline single-quoted raw string literal | `'''SELECT * FROM users'''` |
| `r"..."` | Single-line raw string (preserves `\` verbatim) | `r"C:\Windows\System32\*"` |
| `? expr` | Inline filter shorthand | `read data.json \| json \| ? .age > 18` |
| `.field` | Object field extractor | `read user.json \| json \| .name` |
| `{k: v}` | Map / Object constructor | `map({ file: .path, size: .size })` |
| `cmd a b` | Command-style invocation (no parens/commas) | `replace src/lib.rs old_val new_val` |
| `;` | Pipeline separator for multi-step scripts | `"""code""" > app.js; replace app.js a b;` |

---

## 🛠️ Standard Library Reference

### 1. File System & Code Manipulation (`fs` / `code`)

```ailang
# Direct multiline code generation
"""
import express from 'express';
const app = express();
export default app;
""" > src/server.ts;

# Surgical replacements (with fuzzy whitespace tolerance)
replace src/server.ts "const app = express();" "const app = express();\napp.use(express.json());";

# Global replacements
replace_all src/config.py "DEBUG = True" "DEBUG = False";

# Anchor insertions
insert_after src/main.rs "fn main() {" "    env_logger::init();";
insert_before src/main.rs "fn main()" "#[tokio::main]";

# Header / Footer additions
prepend src/lib.rs "// @license MIT";
append src/routes.py "routes.register(app)";

# Unified diff patch
patch src/utils.py """@@ -10,2 +10,2 @@\n-old_calc()\n+new_calc()""";
```

### 2. Codebase Inspection (`inspect`)

```ailang
# Instant codebase search (auto-skips node_modules, target, .git)
search src "FIXME"

# Directory tree up to depth 2
tree src 2

# Inspect file head or tail
head 15 src/lib.rs
tail 20 /var/log/syslog

# Stream input line counting
< src/main.rs | lines | count
```

### 3. Data Transformation (`data` & `text`)

```ailang
# Parse CSV, filter predicate, and serialize to compact JSON
< users.csv | csv | filter(.active == true && .age >= 21) | json > active_adults.json

# Read YAML, update property, save back
< config.yaml | yaml | set(.database.port, 5432) | yaml > config.yaml

# Extract unique matching lines
< logs.txt | lines | grep ERROR | trim | unique | count
```

### 4. Process Execution & System (`sys`)

```ailang
# Spawning processes with piped stdin and stdout capture
< entries.txt | lines | filter(!.starts_with("#")) | exec("sort") > sorted.txt

# Environment variables and working directory
env("DATABASE_URL")
pwd
```

### 5. Network & Cryptography (`net` & `crypto`)

```ailang
# HTTP Requests
net.get("https://api.github.com/zen")
net.post("https://httpbin.org/post", { key: "value" })
net.download("https://example.com/data.zip", "downloads/data.zip")

# Cryptographic Hashes & Encodings
< release.tar.gz | sha256
"secret message" | base64.enc
```

---

## 🎨 Complete Real-World Workflows

### Workflow 1: Generate, Instrument, and Export Code

```ailang
# Step 1: Create a service from scratch
"""
export class PaymentService {
    async process(amount: number) {
        return { status: 'success', id: 101 };
    }
}
""" > src/services/payment.ts;

# Step 2: Inject logging before the return statement
insert_before src/services/payment.ts "return" "        console.log(`Processing payment: ${amount}`);";

# Step 3: Add file header and export
prepend src/services/payment.ts "// Generated by AI Agent";
append src/services/payment.ts "export default new PaymentService();";

# Step 4: Verify the result
< src/services/payment.ts;
```

### Workflow 2: Find Outdated Dependencies and Alert

```ailang
< package.json | json | .dependencies | map({pkg: .key, ver: .val}) | filter(.ver.starts_with("^0.")) > outdated.json
```

---

## 🔌 Model Context Protocol (MCP) Integration

For installation, client setup and a copyable handoff prompt, see
[Agent setup and handoff](docs/agent-setup.md).

ailang natively implements the Model Context Protocol over standard I/O (`stdio`). Any MCP-compatible client can immediately delegate scripting, file manipulation, and code editing to ailang.

### 1. Launch Standalone Server
```bash
ailang serve --mcp
```

### 2. Configure Claude Desktop

Add to your `claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "ailang": {
      "command": "ailang",
      "args": ["serve", "--mcp"]
    }
  }
}
```

### 3. Configure Cursor / Antigravity / Open-Source Agents

In your client configuration settings:

```json
{
  "tools": [
    {
      "name": "ailang",
      "command": "ailang",
      "args": ["serve", "--mcp"]
    }
  ]
}
```

The MCP tool accepts a single string argument `expr` containing any valid single- or multi-statement ailang program and returns structured or text results.

---

## 🛡️ Agent Safety & Security Sandboxing

CLI expressions, script execution and MCP share the same execution policy:

```bash
# 1. Preview changes without changing target files (returns JSON with diffs)
ailang "replace src/main.rs old_fn new_fn" --dry-run

# 2. Strict read-only mode (forbids write, replace, patch, exec, rm, mv, cp)
ailang "glob src/**/*.rs | lines | count" --read-only

# 3. Path Jailing (restricts all file reading/writing to a designated directory)
ailang "replace safe_workspace/app.js a b" --allow-dir ./safe_workspace
```

---

Policy flags are global, so they also work with `run` and `serve`:

```bash
ailang run task.al --read-only
ailang serve --mcp --allow-dir ./safe_workspace --no-net
```

- `--read-only` denies local mutations, subprocesses and HTTP POST.
- `--no-net` denies HTTP operations and subprocesses, which could otherwise bypass it.
- `--allow-dir` checks existing paths and existing parents of new paths using canonical paths, including symlinks and Windows junctions. Recursive operations fail closed if their tree contains a link outside the allowed root. Subprocesses and `cd` are disabled in this mode. Use an explicit root, e.g. `glob safe_workspace/**/*.log`; a broad glob rooted outside the allowed directory is denied.
- `--dry-run` returns `{result, dry_run}`. Text edits are validated on private temporary copies and return unified diffs. Copy, move, deletion, process execution and HTTP POST/download are recorded without being performed. Reads and HTTP GET still run unless another flag blocks them. Previews are independent against the current filesystem, **not a virtual filesystem transaction**: a later statement cannot read a file that an earlier preview would create.
- Text file writes and edits use a temporary file in the target directory followed by replacement. Existing file permissions are preserved. This replaces the file identity and does not preserve hard-link relationships or all platform-specific metadata. Existing UTF-8 BOM/UTF-16 inputs are decoded and rewritten as UTF-8.

These are application-level restrictions, not OS isolation. They do not isolate environment variables or prevent races from another process changing paths between validation and use. Use OS isolation for untrusted workloads. Operations executed before a later error are not rolled back.

`exec(program, [args])` now launches the executable directly without an implicit shell. Migrate `exec("git status")` to `exec("git", ["status"])`. Shell syntax is literal unless you explicitly invoke a shell executable (subject to the same policy).

---

## 🚀 Quickstart

### Installation from Source

Requirements: [Rust 1.80+](https://www.rust-lang.org/)

```bash
# Clone the repository
git clone https://github.com/nbuzzi/ailang.git
cd ailang

# Run workspace unit and integration tests
cargo test --workspace

# Build the release binary
cargo build --release -p ailang-cli

# Copy to system PATH (optional)
# Linux/macOS:
cp target/release/ailang /usr/local/bin/
# Windows PowerShell:
copy target\release\ailang.exe C:\Windows\System32\
```

### Basic Commands

```bash
# 1. Inline execution
ailang "< Cargo.toml | lines | count"

# 2. Run a script file
ailang run examples/05_code_generation_and_editing.al

# 3. Measure source size and parse latency without execution
ailang bench "glob **/*.log | grep ERROR > out.json"

# 4. View the AI Prompt Sheet
ailang ref
```

---

## 📂 Repository Structure

```
ailang/
├── Cargo.toml                  # Cargo workspace definition
├── README.md                   # Project documentation
├── LICENSE                     # MIT License
│
├── crates/
│   ├── ailang-core/            # Core AST, Value enum, SecurityPolicy, Error models
│   ├── ailang-lexer/           # Zero-copy Logos tokenizer (raw strings, heredocs, commands)
│   ├── ailang-parser/          # Recursive descent parser & multi-pipeline program builder
│   ├── ailang-eval/            # Buffered pipeline evaluator (IndexMap deterministic order)
│   ├── ailang-stdlib/          # Built-in modules (fs, code, inspect, text, data, sys, net, crypto)
│   ├── ailang-mcp/             # Native JSON-RPC 2.0 Model Context Protocol stdio server
│   └── ailang-cli/             # Binary CLI entrypoint (inline, run, serve, bench, ref)
│
├── docs/
│   ├── language-spec.md        # Formal BNF grammar & syntax specification
│   └── ai-prompt-sheet.md      # Compact agent cheatsheet for AI system prompts
│
└── examples/
    ├── 01_count_lines.al       # File reading, line splitting, count
    ├── 02_grep_and_json.al     # Filtering and structured JSON serialization
    ├── 03_hash_and_env.al      # Cryptographic hashing & environment inspection
    ├── 04_ultra_compression.al # Command-style syntax & compound AI verbs
    └── 05_code_generation_and_editing.al # Surgical code generation & replacement
```

---

## 📄 License

This project is licensed under the [MIT License](LICENSE).

<div align="center">

**ailang** — *Because AI agents shouldn't waste tokens on `import os`.*

</div>
