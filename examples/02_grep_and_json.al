# Example 2: Grep text from a file and serialize matches to JSON
read("Cargo.toml") | grep("version") | json
