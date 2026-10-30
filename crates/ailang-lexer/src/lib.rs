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
    #[token(">>")]  Append,
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
    #[token("write")]    WriteKw,  // bare `write` keyword (no arg = write back)
    #[token("assert")]   Assert,
    #[token("sha256")]   Sha256,
    #[token("sha512")]   Sha512,
    #[token("md5")]      Md5,
    #[token("set")]      Set,

    // ── Namespaced calls (lexed as single ident) ───────────────────────────
    // net.get, fs.read, sys.exec, etc. — handled as Ident + Dot + Ident in parser

    // ── Literals ───────────────────────────────────────────────────────────
    #[regex(r#""([^"\\]|\\.)*""#, lex_string)]
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

fn lex_string(lex: &mut logos::Lexer<Token>) -> String {
    let raw = lex.slice();
    // Strip surrounding quotes and handle basic escape sequences
    let inner = &raw[1..raw.len() - 1];
    inner
        .replace("\\n", "\n")
        .replace("\\t", "\t")
        .replace("\\r", "\r")
        .replace("\\\"", "\"")
        .replace("\\\\", "\\")
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
    fn test_string_escapes() {
        let tokens = tokenize(r#""hello\nworld""#).unwrap();
        if let Token::Str(s) = &tokens[0].0 {
            assert_eq!(s, "hello\nworld");
        } else {
            panic!("Expected Str token");
        }
    }
}
