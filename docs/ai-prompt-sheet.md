# ailang AI Prompt Sheet

Drop this compact cheatsheet into your LLM agent's system prompt to enable native, token-efficient scripting.

---

## ⚡ System Prompt Snippet (token count depends on the model)

```
ailang: token-minimized scripting. Invoke: ailang "expr" or ailang run script.al
fs/code: < f (stream input)  > f (write)  >> f (append)  """code""" > f
edit: replace(f,old,new) replace_all(f,old,new) insert_after(f,anchor,code) insert_before(f,anchor,code) prepend(f,code) append(f,code) patch(f,diff)
inspect: search(dir,pat) tree(dir,depth) head(n,f) tail(n,f) lines glob(pat)
text: grep(re) sed(re,r) count trim upper lower slice(n,m)
data: json yaml csv map({k:.f}) filter(.f>v) sort(.f) group(.f) set(.f,v) unique assert(v)
net: net.get(url) net.post(url,b) net.download(url,p)
sys: exec(cmd,[args]) env(k) pwd cd(p)
crypto: sha256 sha512 md5 base64.enc base64.dec
ops: | pipe  > write  >> append  < stream-in  ; statement  ? filter  .field
ex: replace src/main.rs "old_fn" "new_fn"; < Cargo.toml | lines | count
```

---

## 🛠️ Function Reference

### Code Generation & Surgical Editing (`fs` / `code`)
* `"""code""" > path`: Writes multiline string directly to file (creates parent directories automatically).
* `"""code""" >> path`: Appends multiline string directly to file.
* `replace(file, target, replacement)`: Surgically replaces target block in file with fuzzy whitespace tolerance. Also works piped: `< file | replace old new > file`.
* `replace_block(file, target, replacement)`: Multi-line block replacement with fuzzy whitespace matching.
* `replace_all(file, target, replacement)`: Global replacement across entire file.
* `insert_after(file, anchor, content)`: Inserts content immediately following the line containing `anchor`.
* `insert_before(file, anchor, content)`: Inserts content immediately preceding the line containing `anchor`.
* `prepend(file, content)`: Inserts content at line 1 of file (imports, headers, license).
* `append(file, content)`: Appends content to the end of file.
* `patch(file, diff)`: Applies standard unified diff to target file.

### Inspection & File System (`fs`)
* `< file`: Stream input operator (shorthand for `read(file)`).
* `read(path)`: Reads entire file as string (auto-detects UTF-8, UTF-8 BOM, UTF-16).
* `write(path)`: Writes stream/string to destination file (overwrite).
* `search(dir, pattern)`: AI Macro — recursively searches code files for regex pattern, auto-ignoring noise directories (`.git`, `target`, `node_modules`).
* `tree(dir, max_depth)`: Compact ASCII file tree visualization in ~3 tokens.
* `head(n, [file])`: Takes first `n` lines from stream or file.
* `tail(n, [file])`: Takes last `n` lines from stream or file.
* `glob(pattern)`: Returns files matching pattern (supports `**/*`).
* `walk(dir)`: Recursively traverses directory.
* `cp(src, dest)` or `src | cp(dest)`: Copies a file to destination.
* `mv(src, dest)` or `src | mv(dest)`: Moves/renames a file or directory.
* `rm(path)`: Removes file or directory recursively.
* `exists(path)`: Returns boolean `true`/`false`.
* `stat(path)`: Returns object with size, ext, name, is_dir.
* `diff(a, b)`: Produces unified line-by-line diff.

### Text (`text`)
* `grep(regex)`: Filters lines/items matching regex pattern.
* `sed(regex, replacement)`: Replaces regex matches in input.
* `lines`: Splits string into list of lines.
* `count`: Returns total count of items or lines.
* `trim`: Strips leading and trailing whitespace.
* `upper`: Converts string to uppercase.
* `lower`: Converts string to lowercase.
* `slice(start, end)`: Slices list or characters.

### Structured Data (`data`)
* `json`: Parses JSON string or serializes value to JSON.
* `yaml`: Parses YAML string or serializes value to YAML.
* `csv`: Parses CSV into list of key-value objects.
* `map({k: .field, ...})`: Projects objects into new structure.
* `filter(.field > value)`: Filters items by predicate.
* `sort(.field)`: Sorts collection by field or value.
* `group(.field)`: Groups collection into map keyed by field.
* `set(.field, value)`: Updates or sets field on object.
* `unique`: Deduplicates elements in list.
* `assert(expected)`: Asserts input equals expected value; fails with structured error if not.

### Networking (`net`)
* `net.get(url)`: Performs HTTP GET request and returns body.
* `net.post(url, body)`: Performs HTTP POST request.
* `net.download(url, path)`: Streams remote file directly to disk.

### System (`sys`)
* `exec(command, [args])`: Launches the executable directly with literal arguments and returns stdout; no implicit shell. Example: `exec("git", ["status"])`. Piped input is passed to process stdin (`read('file') | exec('sort')`).
* `env(var_name)`: Returns value of environment variable.
* `pwd()`: Returns current working directory.
* `cd(path)`: Changes working directory.

### Cryptography (`crypto`)
* `sha256`: Computes SHA-256 hash.
* `sha512`: Computes SHA-512 hash.
* `md5`: Computes MD5 hash.
* `base64.enc`: Encodes input string as Base64.
* `base64.dec`: Decodes Base64 string.

---

## 🔌 Model Context Protocol (MCP) Mode

Start `ailang` as a standard MCP server on stdio:
```bash
ailang serve --mcp
```
Configure in Claude Desktop (`claude_desktop_config.json`):
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

## Execution policy

Global flags apply to inline expressions, `run`, and `serve`: `--read-only`, `--dry-run`, `--allow-dir DIR`, `--no-net`. Tool failures set MCP `isError`. Dry-run returns JSON with action records and diffs; previews do not update a virtual filesystem. See the README for policy scope and limitations. Pipelines currently buffer their values in memory.
