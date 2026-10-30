# Contributing to ailang

Thank you for your interest in contributing! This project is in early development and all contributions are welcome.

## Development Setup

```bash
# 1. Install Rust (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
# Windows: winget install Rustlang.Rustup

# 2. Clone the repo
git clone https://github.com/nbuzzi/ailang.git
cd ailang

# 3. Build all crates
cargo build --workspace

# 4. Run all tests
cargo test --workspace

# 5. Run the binary
cargo run -p ailang-cli -- "read(\"Cargo.toml\") | lines | count"
```

## Project Structure

| Crate | Role |
|:---|:---|
| `ailang-core` | AST types, Value enum, error types — no external deps |
| `ailang-lexer` | Tokenizer built with `logos` |
| `ailang-parser` | Recursive descent parser, builds AST from tokens |
| `ailang-eval` | Pipeline evaluator, expression evaluation engine |
| `ailang-stdlib` | All built-in functions (fs, text, data, net, sys, crypto) |
| `ailang-cli` | Binary entry point, output formatting |

## Adding a New Built-in Function

1. **Add to the appropriate module** in `crates/ailang-stdlib/src/<module>.rs`
2. **Register in the dispatcher** in `crates/ailang-stdlib/src/lib.rs`
3. **Add a token** to the lexer in `crates/ailang-lexer/src/lib.rs` (if it's a keyword)
4. **Write tests** — both unit tests in the module and integration tests in `tests/`
5. **Update docs** in `docs/stdlib-reference.md`

## Coding Guidelines

- **Error handling:** Always return `AilangResult<Vec<Value>>`. Never panic in stdlib functions.
- **Input handling:** Every stdlib function should gracefully handle `input: None` with a clear error message.
- **Token budget:** When naming functions, prefer short names already in the LLM vocabulary.
- **Tests:** Every new function needs at least one unit test.
- **Cross-platform:** Test with `\` vs `/` path handling. Use `replace('\\', "/")` when returning paths.

## Running Examples

```bash
# Count lines in a file
cargo run -p ailang-cli -- "read(\"README.md\") | lines | count"

# Quick reference
cargo run -p ailang-cli -- ref

# Run a script file
cargo run -p ailang-cli -- run examples/log-analysis.al
```

## Reporting Issues

Open a GitHub issue with:
- The expression that failed
- The error output (always JSON-structured)
- Your OS and `ailang --version`
