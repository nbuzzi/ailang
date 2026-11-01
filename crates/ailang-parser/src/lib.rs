use ailang_core::{
    ast::*,
    error::{AilangError, AilangResult},
};
use ailang_lexer::{tokenize, Token};

/// Stateful parser — consumes tokens with character spans for zero-loss command-style syntax.
pub struct Parser<'a> {
    input: &'a str,
    tokens: Vec<(Token, std::ops::Range<usize>)>,
    pos: usize,
}

impl<'a> Parser<'a> {
    pub fn new(input: &'a str, tokens: Vec<(Token, std::ops::Range<usize>)>) -> Self {
        Self { input, tokens, pos: 0 }
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos).map(|(t, _)| t)
    }

    fn peek_span(&self) -> Option<&std::ops::Range<usize>> {
        self.tokens.get(self.pos).map(|(_, s)| s)
    }

    fn advance(&mut self) -> Option<(Token, std::ops::Range<usize>)> {
        if self.pos < self.tokens.len() {
            let item = self.tokens[self.pos].clone();
            self.pos += 1;
            Some(item)
        } else {
            None
        }
    }

    fn advance_token(&mut self) -> Option<Token> {
        self.advance().map(|(t, _)| t)
    }

    fn expect(&mut self, expected: &Token) -> AilangResult<()> {
        match self.peek() {
            Some(t) if std::mem::discriminant(t) == std::mem::discriminant(expected) => {
                self.advance();
                Ok(())
            }
            other => Err(AilangError::ParseError {
                msg: format!("Expected {:?}, got {:?}", expected, other),
            }),
        }
    }

    fn expect_ident(&mut self) -> AilangResult<String> {
        match self.advance_token() {
            Some(Token::Ident(s)) => Ok(s),
            Some(tok) => {
                if let Some(s) = tok.as_ident_like() {
                    Ok(s)
                } else {
                    Err(AilangError::ParseError {
                        msg: format!("Expected identifier, got {:?}", tok),
                    })
                }
            }
            None => Err(AilangError::ParseError {
                msg: "Expected identifier, got EOF".into(),
            }),
        }
    }

    /// Parse multiple pipelines separated by semicolons into a Program AST
    pub fn parse_program(&mut self) -> AilangResult<Program> {
        let mut pipelines = Vec::new();
        while self.peek().is_some() {
            while matches!(self.peek(), Some(Token::Semi)) {
                self.advance();
            }
            if self.peek().is_none() {
                break;
            }
            pipelines.push(self.parse_pipeline()?);
            while matches!(self.peek(), Some(Token::Semi)) {
                self.advance();
            }
        }
        Ok(Program { pipelines })
    }

    /// Parse a full pipeline expression (top-level entry point)
    pub fn parse_pipeline(&mut self) -> AilangResult<Pipeline> {
        let mut stages: Vec<Stage> = Vec::new();

        // First stage
        stages.push(self.parse_stage()?);

        // More stages separated by |
        while matches!(self.peek(), Some(Token::Pipe)) {
            self.advance(); // consume |
            stages.push(self.parse_stage()?);
        }

        // Optional sink: > "path" or >> "path" (quoted or unquoted)
        let sink = match self.peek() {
            Some(Token::Write) => {
                self.advance();
                let path = self.parse_sink_path()?;
                Some(Sink::Write(path))
            }
            Some(Token::Append) => {
                self.advance();
                let path = self.parse_sink_path()?;
                Some(Sink::Append(path))
            }
            _ => None,
        };

        Ok(Pipeline { stages, sink })
    }

    fn parse_sink_path(&mut self) -> AilangResult<String> {
        match self.peek().cloned() {
            Some(Token::Str(s)) => {
                self.advance();
                Ok(s)
            }
            Some(_) => {
                // Collect contiguous tokens for unquoted path (e.g. out.json, dir/out.txt)
                self.parse_contiguous_raw()
            }
            None => Err(AilangError::ParseError {
                msg: "Expected destination path after redirection operator".into(),
            }),
        }
    }

    fn parse_stage(&mut self) -> AilangResult<Stage> {
        match self.peek().cloned() {
            // Stream input operator: `< filename` (shorthand for `read(filename)`)
            Some(Token::Lt) => {
                self.advance(); // consume '<'
                let path_expr = self.parse_command_arg()?;
                Ok(Stage::Call(Call {
                    name: "read".into(),
                    args: vec![path_expr],
                }))
            }

            // let binding: let x = pipeline
            Some(Token::Let) => {
                self.advance();
                let name = self.expect_ident()?;
                // consume '=' or '=='
                match self.advance_token() {
                    Some(Token::Assign) | Some(Token::Eq) => {}
                    other => return Err(AilangError::ParseError {
                        msg: format!("Expected '=' after let {}, got {:?}", name, other),
                    }),
                }
                let pipeline = self.parse_pipeline()?;
                Ok(Stage::Let(name, Box::new(pipeline)))
            }

            // Variable reference: $name
            Some(Token::Var(name)) => {
                self.advance();
                Ok(Stage::Var(name))
            }

            // Filter shorthand: ? expr
            Some(Token::Question) => {
                self.advance();
                let expr = self.parse_expr()?;
                Ok(Stage::Filter(expr))
            }

            // filter(expr) — parsed as a Stage::Filter with an Expr
            Some(Token::Filter) => {
                self.advance();
                let expr = if matches!(self.peek(), Some(Token::LParen)) {
                    self.advance();
                    let e = self.parse_expr()?;
                    self.expect(&Token::RParen)?;
                    e
                } else {
                    self.parse_expr()?
                };
                Ok(Stage::Filter(expr))
            }

            // sort(.field) or sort .field
            Some(Token::Sort) => {
                self.advance();
                let args = self.parse_call_args()?;
                Ok(Stage::Call(Call { name: "sort".into(), args }))
            }

            // group(.field) or group .field
            Some(Token::Group) => {
                self.advance();
                let args = self.parse_call_args()?;
                Ok(Stage::Call(Call { name: "group".into(), args }))
            }

            // set(.field, value)
            Some(Token::Set) => {
                self.advance();
                let args = self.parse_call_args()?;
                Ok(Stage::Call(Call { name: "set".into(), args }))
            }

            // assert("value") or assert value
            Some(Token::Assert) => {
                self.advance();
                let args = self.parse_call_args()?;
                Ok(Stage::Call(Call { name: "assert".into(), args }))
            }

            // Field access shorthand: .field
            Some(Token::Dot) => {
                self.advance();
                let field = self.expect_ident()?;
                Ok(Stage::FieldAccess(field))
            }

            // map({...})
            Some(Token::Map) => {
                self.advance();
                self.expect(&Token::LParen)?;
                let map_expr = self.parse_map_expr()?;
                self.expect(&Token::RParen)?;
                Ok(Stage::Map(map_expr))
            }

            // Bare keywords that transform the stream OR have command arguments:
            // e.g. `lines` (keyword) vs `lines app.log` (call with arg)
            Some(tok) if self.is_keyword_token(&tok) => {
                let kw = self.advance_token().unwrap();
                let kw_name = self.token_to_keyword_name(&kw);

                // Check if followed by arguments (before |, >, >>, ;, or EOF)
                if self.has_command_args_ahead() {
                    let args = self.parse_call_args()?;
                    Ok(Stage::Call(Call { name: kw_name, args }))
                } else {
                    Ok(Stage::Keyword(kw_name))
                }
            }

            // Named function call or command-style call:
            // e.g. `glob("**/*.log")` OR `glob **/*.log`
            // e.g. `grep("ERROR")` OR `grep ERROR`
            // e.g. `search src TODO`
            Some(Token::Ident(_)) => {
                let name = self.build_call_name()?;
                let args = if matches!(self.peek(), Some(Token::LParen)) {
                    self.advance();
                    let args = self.parse_arg_list()?;
                    self.expect(&Token::RParen)?;
                    args
                } else if self.has_command_args_ahead() {
                    self.parse_command_args_until_delimiter()?
                } else {
                    vec![]
                };
                Ok(Stage::Call(Call { name, args }))
            }

            // Literal value as stage: e.g. """fn main() {}""" > "main.rs" or "hello" | upper
            Some(Token::Str(s)) => {
                self.advance();
                Ok(Stage::Literal(ailang_core::Value::Str(s)))
            }
            Some(Token::Int(i)) => {
                self.advance();
                Ok(Stage::Literal(ailang_core::Value::Int(i)))
            }
            Some(Token::Float(f)) => {
                self.advance();
                Ok(Stage::Literal(ailang_core::Value::Float(f)))
            }
            Some(Token::True) => {
                self.advance();
                Ok(Stage::Literal(ailang_core::Value::Bool(true)))
            }
            Some(Token::False) => {
                self.advance();
                Ok(Stage::Literal(ailang_core::Value::Bool(false)))
            }

            other => Err(AilangError::ParseError {
                msg: format!("Unexpected token in stage: {:?}", other),
            }),
        }
    }

    fn is_keyword_token(&self, tok: &Token) -> bool {
        matches!(
            tok,
            Token::Json
                | Token::Yaml
                | Token::Csv
                | Token::Lines
                | Token::Count
                | Token::Trim
                | Token::Upper
                | Token::Lower
                | Token::Unique
                | Token::Sha256
                | Token::Sha512
                | Token::Md5
                | Token::WriteKw
        )
    }

    fn token_to_keyword_name(&self, tok: &Token) -> String {
        match tok {
            Token::Json => "json",
            Token::Yaml => "yaml",
            Token::Csv => "csv",
            Token::Lines => "lines",
            Token::Count => "count",
            Token::Trim => "trim",
            Token::Upper => "upper",
            Token::Lower => "lower",
            Token::Unique => "unique",
            Token::Sha256 => "sha256",
            Token::Sha512 => "sha512",
            Token::Md5 => "md5",
            Token::WriteKw => "write",
            _ => "unknown",
        }.to_string()
    }

    fn has_command_args_ahead(&self) -> bool {
        match self.peek() {
            Some(tok) => !matches!(
                tok,
                Token::Pipe
                    | Token::Write
                    | Token::Append
                    | Token::Semi
                    | Token::RParen
                    | Token::RBrace
                    | Token::RBracket
            ),
            None => false,
        }
    }

    /// Parse arguments either from `(a, b)` or space-separated `a b`
    fn parse_call_args(&mut self) -> AilangResult<Vec<Expr>> {
        if matches!(self.peek(), Some(Token::LParen)) {
            self.advance();
            let args = self.parse_arg_list()?;
            self.expect(&Token::RParen)?;
            Ok(args)
        } else if self.has_command_args_ahead() {
            self.parse_command_args_until_delimiter()
        } else {
            Ok(vec![])
        }
    }

    /// Parse space-separated command arguments until a delimiter (`|`, `>`, `>>`, `;`, EOF)
    fn parse_command_args_until_delimiter(&mut self) -> AilangResult<Vec<Expr>> {
        let mut args = Vec::new();
        while self.has_command_args_ahead() {
            args.push(self.parse_command_arg()?);
        }
        Ok(args)
    }

    /// Parse a single command-style argument (supports unquoted globs/paths/strings)
    fn parse_command_arg(&mut self) -> AilangResult<Expr> {
        match self.peek().cloned() {
            Some(Token::LBracket) => self.parse_primary(),
            Some(Token::Str(s)) => {
                self.advance();
                Ok(Expr::Str(s))
            }
            Some(Token::Int(i)) => {
                let (_, span) = self.advance().unwrap();
                // Check if this integer is part of a contiguous path like `10.txt`
                if let Some(next_span) = self.peek_span() {
                    if next_span.start == span.end && !self.is_delimiter(self.peek()) {
                        let mut end = next_span.end;
                        self.advance();
                        while let Some(ns) = self.peek_span() {
                            if ns.start == end && !self.is_delimiter(self.peek()) {
                                end = ns.end;
                                self.advance();
                            } else {
                                break;
                            }
                        }
                        return Ok(Expr::Str(self.input[span.start..end].to_string()));
                    }
                }
                Ok(Expr::Int(i))
            }
            Some(Token::Float(f)) => {
                self.advance();
                Ok(Expr::Float(f))
            }
            Some(Token::True) => {
                self.advance();
                Ok(Expr::Bool(true))
            }
            Some(Token::False) => {
                self.advance();
                Ok(Expr::Bool(false))
            }
            Some(Token::Null) => {
                self.advance();
                Ok(Expr::Null)
            }
            Some(Token::Var(v)) => {
                self.advance();
                Ok(Expr::Var(v))
            }
            Some(Token::Dot) => {
                // Could be field access `.field` or relative path `./file`
                let (_first_tok, first_span) = self.advance().unwrap();
                if let Some(next_span) = self.peek_span() {
                    if next_span.start == first_span.end {
                        // Contiguous! If it's a Slash, it's a path `./...`
                        if matches!(self.peek(), Some(Token::Slash)) {
                            let mut end = next_span.end;
                            self.advance();
                            while let Some(ns) = self.peek_span() {
                                if ns.start == end && !self.is_delimiter(self.peek()) {
                                    end = ns.end;
                                    self.advance();
                                } else {
                                    break;
                                }
                            }
                            return Ok(Expr::Str(self.input[first_span.start..end].to_string()));
                        }
                        // Field access: `.field`
                        if let Some(Token::Ident(field)) = self.advance_token() {
                            return Ok(Expr::Field(vec![field]));
                        }
                    }
                }
                Ok(Expr::Str(".".to_string()))
            }
            Some(_) => {
                // Any unquoted word, path, or glob pattern (e.g. `**/*.log`, `src`, `ERROR`)
                let raw = self.parse_contiguous_raw()?;
                Ok(Expr::Str(raw))
            }
            None => Err(AilangError::ParseError {
                msg: "Expected command argument, got EOF".into(),
            }),
        }
    }

    /// Read contiguous tokens (with zero whitespace in between) as a raw string slice
    fn parse_contiguous_raw(&mut self) -> AilangResult<String> {
        let (first_tok, first_span) = self.advance().ok_or_else(|| AilangError::ParseError {
            msg: "Expected token, got EOF".into(),
        })?;

        // If it's a quoted string, return inner directly
        if let Token::Str(s) = first_tok {
            return Ok(s);
        }

        let start = first_span.start;
        let mut end = first_span.end;

        // Keep consuming tokens as long as they are directly adjacent in the input (no whitespace)
        // and not a stage delimiter
        while let Some(next_span) = self.peek_span() {
            if next_span.start == end && !self.is_delimiter(self.peek()) {
                end = next_span.end;
                self.advance();
            } else {
                break;
            }
        }

        Ok(self.input[start..end].to_string())
    }

    fn is_delimiter(&self, tok_opt: Option<&Token>) -> bool {
        match tok_opt {
            Some(tok) => matches!(
                tok,
                Token::Pipe
                    | Token::Write
                    | Token::Append
                    | Token::Semi
                    | Token::RParen
                    | Token::RBrace
                    | Token::RBracket
            ),
            None => true,
        }
    }

    /// Build a potentially namespaced call name: `net.get`, `fs.read`, `base64.enc`
    fn build_call_name(&mut self) -> AilangResult<String> {
        let first = self.expect_ident()?;
        if matches!(self.peek(), Some(Token::Dot))
            && self.tokens[self.pos - 1].1.end == self.tokens[self.pos].1.start
            && self.pos + 1 < self.tokens.len()
            && matches!(&self.tokens[self.pos + 1].0, Token::Ident(_))
            && self.tokens[self.pos].1.end == self.tokens[self.pos + 1].1.start {
            self.advance(); // consume dot
            let method = self.expect_ident()?;
            return Ok(format!("{}.{}", first, method));
        }
        Ok(first)
    }

    fn parse_arg_list(&mut self) -> AilangResult<Vec<Expr>> {
        let mut args = Vec::new();
        if matches!(self.peek(), Some(Token::RParen)) {
            return Ok(args);
        }
        args.push(self.parse_expr()?);
        while matches!(self.peek(), Some(Token::Comma)) {
            self.advance();
            args.push(self.parse_expr()?);
        }
        Ok(args)
    }

    /// Parse a map expression: {key: expr, key: expr, ...}
    fn parse_map_expr(&mut self) -> AilangResult<MapExpr> {
        self.expect(&Token::LBrace)?;
        let mut fields = Vec::new();

        while !matches!(self.peek(), Some(Token::RBrace) | None) {
            let key = self.expect_ident()?;
            self.expect(&Token::Colon)?;
            let val = self.parse_expr()?;
            fields.push((key, val));
            if matches!(self.peek(), Some(Token::Comma)) {
                self.advance();
            }
        }

        self.expect(&Token::RBrace)?;
        Ok(MapExpr { fields })
    }

    /// Parse an expression (arguments, filter conditions, map values)
    fn parse_expr(&mut self) -> AilangResult<Expr> {
        self.parse_or_expr()
    }

    fn parse_or_expr(&mut self) -> AilangResult<Expr> {
        let mut lhs = self.parse_and_expr()?;
        while matches!(self.peek(), Some(Token::Or)) {
            self.advance();
            let rhs = self.parse_and_expr()?;
            lhs = Expr::BinOp(Box::new(lhs), BinOp::Or, Box::new(rhs));
        }
        Ok(lhs)
    }

    fn parse_and_expr(&mut self) -> AilangResult<Expr> {
        let mut lhs = self.parse_cmp_expr()?;
        while matches!(self.peek(), Some(Token::And)) {
            self.advance();
            let rhs = self.parse_cmp_expr()?;
            lhs = Expr::BinOp(Box::new(lhs), BinOp::And, Box::new(rhs));
        }
        Ok(lhs)
    }

    fn parse_cmp_expr(&mut self) -> AilangResult<Expr> {
        let lhs = self.parse_primary()?;
        let op = match self.peek() {
            Some(Token::Eq)    => BinOp::Eq,
            Some(Token::Ne)    => BinOp::Ne,
            Some(Token::Write) => BinOp::Gt,
            Some(Token::Ge)    => BinOp::Ge,
            Some(Token::Lt)    => BinOp::Lt,
            Some(Token::Le)    => BinOp::Le,
            _ => return Ok(lhs),
        };
        self.advance();
        let rhs = self.parse_primary()?;
        Ok(Expr::BinOp(Box::new(lhs), op, Box::new(rhs)))
    }

    fn parse_primary(&mut self) -> AilangResult<Expr> {
        match self.peek().cloned() {
            Some(Token::LBracket) => {
                self.advance();
                let mut items = Vec::new();
                if !matches!(self.peek(), Some(Token::RBracket)) {
                    loop {
                        items.push(self.parse_expr()?);
                        if !matches!(self.peek(), Some(Token::Comma)) { break; }
                        self.advance();
                    }
                }
                self.expect(&Token::RBracket)?;
                Ok(Expr::List(items))
            }
            Some(Token::Null)       => { self.advance(); Ok(Expr::Null) }
            Some(Token::True)       => { self.advance(); Ok(Expr::Bool(true)) }
            Some(Token::False)      => { self.advance(); Ok(Expr::Bool(false)) }
            Some(Token::Int(i))     => { self.advance(); Ok(Expr::Int(i)) }
            Some(Token::Float(f))   => { self.advance(); Ok(Expr::Float(f)) }
            Some(Token::Str(s))     => { self.advance(); Ok(Expr::Str(s)) }
            Some(Token::Var(v))     => { self.advance(); Ok(Expr::Var(v)) }
            Some(Token::Bang) => {
                self.advance();
                let inner = self.parse_primary()?;
                Ok(Expr::Not(Box::new(inner)))
            }
            Some(Token::LParen) => {
                self.advance();
                let expr = self.parse_expr()?;
                self.expect(&Token::RParen)?;
                Ok(expr)
            }
            Some(Token::Dot) => {
                let mut parts = Vec::new();
                while matches!(self.peek(), Some(Token::Dot)) {
                    self.advance();
                    let field = self.expect_ident()?;
                    parts.push(field);
                }
                if matches!(self.peek(), Some(Token::LParen)) {
                    let method = parts.pop().unwrap();
                    self.advance();
                    let args = self.parse_arg_list()?;
                    self.expect(&Token::RParen)?;
                    Ok(Expr::FieldMethod(parts, method, args))
                } else {
                    Ok(Expr::Field(parts))
                }
            }
            Some(Token::Ident(_)) => {
                let s = self.parse_contiguous_raw()?;
                Ok(Expr::Str(s))
            }
            other => Err(AilangError::ParseError {
                msg: format!("Unexpected token in expression: {:?}", other),
            }),
        }
    }
}

/// Parse an ailang expression string into a Pipeline AST.
pub fn parse(input: &str) -> AilangResult<Pipeline> {
    let token_pairs = tokenize(input).map_err(|msg| AilangError::LexError { pos: 0, msg })?;
    let mut parser = Parser::new(input, token_pairs);
    parser.parse_pipeline()
}

/// Parse an ailang program (one or more pipelines separated by semicolons) into a Program AST.
pub fn parse_program(input: &str) -> AilangResult<Program> {
    let token_pairs = tokenize(input).map_err(|msg| AilangError::LexError { pos: 0, msg })?;
    let mut parser = Parser::new(input, token_pairs);
    parser.parse_program()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_command_style_glob_grep() {
        let ast = parse("glob **/*.log | grep ERROR > out.json").unwrap();
        assert_eq!(ast.stages.len(), 2);
        if let Stage::Call(c) = &ast.stages[0] {
            assert_eq!(c.name, "glob");
            assert_eq!(c.args, vec![Expr::Str("**/*.log".into())]);
        } else {
            panic!("Expected Call stage for glob");
        }
        if let Stage::Call(c) = &ast.stages[1] {
            assert_eq!(c.name, "grep");
            assert_eq!(c.args, vec![Expr::Str("ERROR".into())]);
        } else {
            panic!("Expected Call stage for grep");
        }
        assert!(matches!(ast.sink, Some(Sink::Write(s)) if s == "out.json"));
    }

    #[test]
    fn test_parse_stream_input_operator() {
        let ast = parse("< Cargo.toml | lines | count").unwrap();
        assert_eq!(ast.stages.len(), 3);
        if let Stage::Call(c) = &ast.stages[0] {
            assert_eq!(c.name, "read");
            assert_eq!(c.args, vec![Expr::Str("Cargo.toml".into())]);
        } else {
            panic!("Expected Call stage for <");
        }
    }

    #[test]
    fn test_parse_compound_verb_search() {
        let ast = parse("search src TODO").unwrap();
        assert_eq!(ast.stages.len(), 1);
        if let Stage::Call(c) = &ast.stages[0] {
            assert_eq!(c.name, "search");
            assert_eq!(c.args.len(), 2);
        }
    }

    #[test]
    fn test_parse_literal_stage() {
        let ast = parse(r#""""fn hello() {}""" > "main.rs""#).unwrap();
        assert_eq!(ast.stages.len(), 1);
        assert!(matches!(&ast.stages[0], Stage::Literal(ailang_core::Value::Str(s)) if s == "fn hello() {}"));
        assert!(matches!(&ast.sink, Some(Sink::Write(s)) if s == "main.rs"));
    }

    #[test]
    fn argument_lists_and_spaced_directory_dot() {
        let ast = parse("exec('git', ['status', '--short'])").unwrap();
        assert!(matches!(&ast.stages[0], Stage::Call(c) if matches!(&c.args[1], Expr::List(v) if v.len() == 2)));
        let ast = parse("search . needle").unwrap();
        assert!(matches!(&ast.stages[0], Stage::Call(c) if c.name == "search" && c.args.len() == 2));
        let ast = parse("write('output', 'hello')").unwrap();
        assert!(matches!(&ast.stages[0], Stage::Call(c) if c.name == "write" && c.args.len() == 2));
        assert!(parse("exec('git', ['status')").is_err());
    }

    #[test]
    fn field_methods_are_parsed_as_methods() {
        let ast = parse("filter(.name.starts_with('npc_'))").unwrap();
        assert!(matches!(&ast.stages[0], Stage::Filter(Expr::FieldMethod(parts, method, _))
            if parts == &["name"] && method == "starts_with"));
    }
}
