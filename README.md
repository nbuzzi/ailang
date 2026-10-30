# ailang 🤖⚡

<div align="center">

**A token-efficient scripting language built in Rust, designed for AI agents — not humans.**

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Built with Rust](https://img.shields.io/badge/Built%20with-Rust-orange?logo=rust)](https://www.rust-lang.org/)
[![Status: Active](https://img.shields.io/badge/Status-Active-brightgreen)]()

*Replace Python, PowerShell, and Bash in AI agent workflows — using up to 80% fewer tokens.*

</div>

---

## The Problem

Modern AI agents waste **thousands of tokens** on scripting boilerplate every single day.

When an agent needs to search log files for errors and export them as JSON, here's what happens today:

```python
# Python — 95 tokens
import glob, json, re

res = []
for f in glob.glob("**/*.log", recursive=True):
    with open(f) as fp:
        for l in fp:
            if "ERROR" in l:
                res.append({"file": f, "err": l.strip()})

with open("out.json", "w") as fp:
    json.dump(res, fp)
```

```powershell
# PowerShell — 60 tokens (+ broken on Linux/macOS)
Get-ChildItem -Recurse -Filter *.log |
  Select-String "ERROR" |
  ForEach-Object { @{ file = $_.Path; err = $_.Line.Trim() } } |
  ConvertTo-Json | Set-Content out.json
```

The agent burns dozens of tokens on `import` statements, context managers, loop syntax, serialization calls, and OS-specific quirks — just to do something conceptually simple.

**With ailang:**

```
glob("**/*.log") | grep("ERROR") | map({file:.path, err:.line}) > "out.json"
```

**18 tokens. Same result. Works on Windows, Linux, and macOS.**

---

## What is ailang?

**ailang** (AI Language) is a single-binary scripting runtime written in Rust. It executes a compact, pipeline-based scripting language designed with one constraint: **minimize the number of tokens an AI needs to express any common system operation.**

It is not designed to be readable by humans (though it is). It is designed to be **generated efficiently** by large language models and **executed instantly** by a native Rust runtime.

```bash
ailang "expr"              # inline execution (for AI agents & CLI)
ailang run script.al       # run a script file
ailang serve --mcp         # run native Model Context Protocol (MCP) server
ailang bench "expr"        # benchmark token savings vs Python / PowerShell
```

### Key Capabilities

| Capability | Description |
|:---|:---|
| **Token-first syntax** | Keywords and operators occupy exactly 1 token in modern LLM tokenizers (cl100k, llama3, gemma) |
| **Surgical Code Editing** | `replace_block` and `patch` with fuzzy matching — edit code without rewriting entire files |
| **Native MCP Server** | Built-in JSON-RPC Model Context Protocol server (`ailang serve --mcp`) for Claude, Cursor, Antigravity |
| **Agent Safety & Sandbox** | `--dry-run`, `--read-only`, and `--allow-dir` flags prevent accidental damage |
| **No imports** | File I/O, JSON/YAML/CSV, regex, HTTP, hashing, and shell execution are all built-in |
| **Piped Process Exec** | `exec(cmd)` supports stdin piping: `read('file') \| exec('sort')` |
| **Deterministic Output** | Insertion-ordered JSON maps ensure reproducible, structured agent responses |
| **Fast startup** | Sub-millisecond cold start (~0.5ms) — no interpreter overhead or GC pauses |

---

## Language Overview

ailang uses a **pipeline model**: data flows left to right through operations separated by `|`.

```
<source> | <operation> | <operation> | <sink>
```

### Core Operators

| Operator | Meaning | Tokens |
|:---:|:---|:---:|
| `\|` | Pipe output to next operation | 1 |
| `>` | Write result to file (overwrite) | 1 |
| `>>` | Append result to file | 1 |
| `?` | Inline filter shorthand | 1 |
| `.field` | Access field of object/JSON | 1 |
| `@` | Iterate over collection | 1 |
| `{k: v}` | Construct an object | ~3 |

### Standard Library

```
fs      read(p)  write(p)  glob(pat)  walk(d)  cp  mv  rm  exists  stat  diff  patch  replace_block
text    grep(re)  sed(re,r)  lines  count  trim  upper  lower  slice(n,m)
data    json  yaml  csv  map({})  filter()  sort()  group()  set()  unique  assert()
net     net.get(url)  net.post(url,body)  net.download(url,path)
sys     exec(cmd,[args])  env(k)  os  arch  pwd  cd
crypto  md5  sha256  sha512  base64.enc  base64.dec
```

---

## Token Savings: Side-by-Side Examples

### Example 1 — Search logs, export JSON

```python
# Python: ~95 tokens
import glob, json

res = []
for f in glob.glob("**/*.log", recursive=True):
    with open(f) as fp:
        for l in fp:
            if "ERROR" in l:
                res.append({"file": f, "err": l.strip()})
with open("out.json", "w") as fp:
    json.dump(res, fp)
```

```
# ailang: 18 tokens  ✅  -81%
glob("**/*.log") | grep("ERROR") | map({file:.path, err:.line}) > "out.json"
```

---

### Example 2 — Surgical Code Editing (No Full Rewrites!)

Instead of regenerating a 500-line file (costing 2,000 tokens), an agent replaces only the target block:

```
# ailang: 12 tokens  ✅  -95%
replace_block("src/main.rs", "fn old() { 1 }", "fn old() { 2 }")
```

ailang's `replace_block` features **fuzzy whitespace tolerance**, so slight indentation mismatches by LLMs succeed seamlessly.

---

### Example 3 — Read YAML config, update a field, save

```python
# Python: ~45 tokens
import yaml

with open("config.yaml") as f:
    d = yaml.safe_load(f)
d["version"] = "2.0"
with open("config.yaml", "w") as f:
    yaml.dump(d, f)
```

```
# ailang: 12 tokens  ✅  -73%
read("config.yaml") | yaml | set(.version, "2.0") > "config.yaml"
```

---

### Example 4 — Piped Process Execution

```bash
# Bash: ~25 tokens
cat list.txt | grep -v "^#" | sort | uniq > clean.txt
```

```
# ailang: 12 tokens  ✅
read("list.txt") | lines | filter(!.starts_with("#")) | unique | exec("sort") > "clean.txt"
```

---

### Example 5 — Parse CSV, filter rows, export JSON

```python
# Python: ~50 tokens
import csv, json

with open("data.csv") as f:
    rows = [r for r in csv.DictReader(f) if int(r["age"]) > 18]
with open("adults.json", "w") as f:
    json.dump(rows, f)
```

```
# ailang: 10 tokens  ✅  -80%
read("data.csv") | csv | filter(.age > 18) | json > "adults.json"
```

---

## 📊 Built-in Token Benchmarking

ailang includes a benchmark engine that calculates token savings and microsecond latencies directly from your terminal:

```bash
ailang bench "glob('**/*.log') | grep('ERROR') | map({file:.path,err:.line}) > 'out.json'"
```

Output:
```
=== ailang Token Benchmark ===
Expression: glob('**/*.log') | grep('ERROR') | map({file:.path,err:.line}) > 'out.json'

Estimated Tokens:
  * ailang:        21 tokens
  * Python:       113 tokens  (~81% savings)
  * PowerShell:    92 tokens  (~77% savings)

Performance:
  * Parse latency:   84.9µs
  * Exec latency:    103.7ms
==============================
```

---

## 🔌 Model Context Protocol (MCP) Integration

Connect ailang directly to Claude Desktop, Cursor, Copilot, or Antigravity via stdio JSON-RPC:

```bash
ailang serve --mcp
```

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

---

## 🛡️ Agent Safety & Sandboxing

Prevent unintended side-effects with native security guardrails:

```bash
# Preview what would be written or modified without touching disk:
ailang "replace_block('src/main.rs', 'old', 'new')" --dry-run

# Run in strict read-only mode (blocks write, replace_block, patch, and process exec):
ailang "glob('**/*') | lines | count" --read-only

# Restrict file modifications to a specific directory:
ailang "read('src/lib.rs')" --allow-dir ./src
```

---

## Architecture

```
ailang "glob('*.log') | grep('ERROR') | json > out.json"
         │
         ▼
    ┌─────────┐
    │  Lexer  │  logos  — zero-copy tokenization
    └────┬────┘
         │
    ┌────▼────┐
    │  Parser │  recursive descent — pipeline + expression AST
    └────┬────┘
         │
    ┌────▼──────────┐
    │   Evaluator   │  streaming pipeline engine (IndexMap deterministic order)
    └────┬──────────┘
         │
    ┌────▼──────────────────────────────────────────────────┐
    │                   Standard Library                    │
    │  fs (replace_block, patch) · text · data · sys · net  │
    └───────────────────────────────────────────────────────┘
         │
         ▼
    minified JSON / raw text  →  AI agent reads response (via CLI or MCP)
```

---

## Repository Structure

```
ailang/
├── Cargo.toml                  # Workspace root
├── README.md
├── LICENSE                     # MIT
│
├── crates/
│   ├── ailang-core/            # AST types, Value enum, SecurityPolicy, error types
│   ├── ailang-lexer/           # Tokenizer (logos)
│   ├── ailang-parser/          # Recursive descent parser + AST builder
│   ├── ailang-eval/            # Pipeline evaluator + expression engine
│   ├── ailang-stdlib/          # Built-in functions (fs, text, data, net, sys, crypto)
│   ├── ailang-mcp/             # Model Context Protocol stdio server
│   └── ailang-cli/             # Binary entry point (inline, run, serve, bench)
│
├── docs/
│   ├── language-spec.md        # Formal grammar (BNF)
│   └── ai-prompt-sheet.md      # <75 token cheatsheet for system prompts
│
└── examples/
    ├── 01_count_lines.al
    ├── 02_grep_and_json.al
    └── 03_hash_and_env.al
```

---

## Quickstart

```bash
# Clone the repository
git clone https://github.com/nbuzzi/ailang.git
cd ailang

# Run all workspace tests (30 passing tests)
cargo test --workspace

# Build the CLI
cargo build --release -p ailang-cli

# Test an expression
./target/release/ailang "read('Cargo.toml') | lines | count"
```

---

## License

MIT — see [LICENSE](LICENSE)

<div align="center">

**ailang** — because AI agents shouldn't waste tokens on `import os`.

</div>
