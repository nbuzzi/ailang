use logos::Logos;

/// All tokens recognized by the ailang lexer.
/// Designed to map 1:1 with meaningful syntax units — no noise.
#[derive(Logos, Debug, Clone, PartialEq)]
#[logos(skip r"[ \t\r\n]+")] // skip whitespace
#[logos(skip r"#[^\n]*")]    // skip comments (# to end of line)
pub enum Token {
    // ── Operators ──────────────────────────────────────────────────────────
    #[token("|")]  Pipe,
    #[token(">")]  Write,
    #[token(">>")] Append,
    #[token("?")]  Question,
    #[token("@")]  At,
    #[token(".")]  Dot,
    #[token(",")]  Comma,
    #[token(":")]  Colon,
    #[token(";")]  Semi,
    #[token("(")]  LParen,
    #[token(")")]  RParen,
    #[token("{")]  LBrace,
    #[token("}")]  RBrace,
    #[token("[")]  LBracket,
    #[token("]")]  RBracket,
    #[token("!")]  Bang,
    #[token("=")]  Assign,

    // ── Comparison / logical ───────────────────────────────────────────────
    #[token("==")] Eq,
    #[token("!=")] Ne,
    #[token(">=")] Ge,
    #[token("<=")] Le,
    #[token("<")]  Lt,
    #[token("&&")] And,
    #[token("||")] Or,

    // ── Arithmetic ─────────────────────────────────────────────────────────
    #[token("+")] Plus,
    #[token("-")] Minus,
    #[token("*")] Star,
    #[token("/")] Slash,

    // ── Keywords ───────────────────────────────────────────────────────────
    #[token("let")]      Let,
    #[token("true")]     True,
    #[token("false")]    False,
    #[token("null")]     Null,
    #[token("map")]      Map,
    #[token("filter")]   Filter,
    #[token("sort")]     Sort,
    #[token("group")]    Group,
    #[token("unique")]   Unique,
    #[token("count")]    Count,
    #[token("lines")]    Lines,
    #[token("json")]     Json,
    #[token("yaml")]     Yaml,
    #[token("csv")]      Csv,
    #[token("trim")]     Trim,
    #[token("upper")]    Upper,
    #[token("lower")]    Lower,
    #[token("write")]    WriteKw,
    #[token("assert")]   Assert,
    #[token("sha256")]   Sha256,
    #[token("sha512")]   Sha512,
    #[token("md5")]      Md5,
    #[token("set")]      Set,

    // ── Literals ───────────────────────────────────────────────────────────
    #[regex(r#""([^"\\]|\\.)*""#, lex_string)]
    #[regex(r"'([^'\\]|\\.)*'", lex_string)]
    Str(String),

    #[regex(r"-?[0-9]+\.[0-9]+", |lex| lex.slice().parse::<f64>().ok())]
    Float(f64),

    #[regex(r"-?[0-9]+", |lex| lex.slice().parse::<i64>().ok())]
    Int(i64),

    // ── Identifiers ────────────────────────────────────────────────────────
    #[regex(r"[a-zA-Z_][a-zA-Z0-9_]*", |lex| lex.slice().to_string())]
    Ident(String),

    // ── Variable reference: $name ──────────────────────────────────────────
    #[regex(r"\$[a-zA-Z_][a-zA-Z0-9_]*", |lex| lex.slice()[1..].to_string())]
    Var(String),
}

impl Token {
    /// Keywords that may also appear as plain identifiers in field paths,
    /// map keys and unquoted file names (e.g. `data.json`, `{count: .n}`).
    pub fn as_ident_like(&self) -> Option<String> {
        let s = match self {
            Token::Ident(s) => return Some(s.clone()),
            Token::Map => "map",
            Token::Filter => "filter",
            Token::Sort => "sort",
            Token::Group => "group",
            Token::Unique => "unique",
            Token::Count => "count",
            Token::Lines => "lines",
            Token::Json => "json",
            Token::Yaml => "yaml",
            Token::Csv => "csv",
            Token::Trim => "trim",
            Token::Upper => "upper",
            Token::Lower => "lower",
            Token::WriteKw => "write",
            Token::Assert => "assert",
            Token::Sha256 => "sha256",
            Token::Sha512 => "sha512",
            Token::Md5 => "md5",
            Token::Set => "set",
            Token::Let => "let",
            _ => return None,
        };
        Some(s.to_string())
    }
}

fn lex_string(lex: &mut logos::Lexer<Token>) -> String {
    let raw = lex.slice();
    // Strip surrounding quotes and handle basic escape sequences
    let inner = &raw[1..raw.len() - 1];
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some('"') => out.push('"'),
            Some('\'') => out.push('\''),
            Some('\\') => out.push('\\'),
            // Unknown escapes are preserved verbatim (important for regex: \d, \s, \.)
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}

/// Tokenize an ailang expression string into a vector of (Token, span) pairs.
pub fn tokenize(input: &str) -> Result<Vec<(Token, std::ops::Range<usize>)>, String> {
    let mut tokens = Vec::new();
    let mut lexer = Token::lexer(input);

    while let Some(result) = lexer.next() {
        match result {
            Ok(tok) => tokens.push((tok, lexer.span())),
            Err(_) => {
                return Err(format!(
                    "Unexpected character at position {}: {:?}",
                    lexer.span().start,
                    &input[lexer.span()]
                ));
            }
        }
    }

    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_pipeline() {
        let tokens = tokenize(r#"glob("*.log") | grep("ERROR") | json > "out.json""#).unwrap();
        let kinds: Vec<_> = tokens.iter().map(|(t, _)| t.clone()).collect();
        assert!(kinds.contains(&Token::Pipe));
        assert!(kinds.contains(&Token::Write));
        assert!(kinds.contains(&Token::Json));
    }

    #[test]
    fn test_field_access() {
        let tokens = tokenize(".path").unwrap();
        assert_eq!(tokens[0].0, Token::Dot);
        assert!(matches!(&tokens[1].0, Token::Ident(s) if s == "path"));
    }

    #[test]
    fn test_filter_expr() {
        let tokens = tokenize("filter(.age > 18)").unwrap();
        assert_eq!(tokens[0].0, Token::Filter);
        // `>` is lexed as Token::Write (dual use: sink and greater-than)
        assert!(tokens.iter().any(|(t, _)| *t == Token::Write));
    }

    #[test]
    fn test_single_quoted_string() {
        let tokens = tokenize("read('a b.txt')").unwrap();
        assert!(tokens.iter().any(|(t, _)| *t == Token::Str("a b.txt".into())));
    }

    #[test]
    fn test_assign_vs_eq() {
        let tokens = tokenize("let x = 1 == 1").unwrap();
        assert!(tokens.iter().any(|(t, _)| *t == Token::Assign));
        assert!(tokens.iter().any(|(t, _)| *t == Token::Eq));
    }

    #[test]
    fn test_regex_escapes_preserved() {
        let tokens = tokenize(r#"grep("\d+\.log")"#).unwrap();
        assert!(tokens.iter().any(|(t, _)| *t == Token::Str(r"\d+\.log".into())));
    }

    #[test]
    fn test_string_escapes() {
        let tokens = tokenize(r#""hello\nworld""#).unwrap();
        if let Token::Str(s) = &tokens[0].0 {
            assert_eq!(s, "hello\nworld");
        } else {
            panic!("Expected Str token");
        }
    }
}
