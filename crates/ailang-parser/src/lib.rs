use ailang_core::{
    ast::*,
    error::{AilangError, AilangResult},
};
use ailang_lexer::{tokenize, Token};

/// Stateful parser — consumes a flat token stream and produces a Pipeline AST.
pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn advance(&mut self) -> Option<Token> {
        if self.pos < self.tokens.len() {
            let tok = self.tokens[self.pos].clone();
            self.pos += 1;
            Some(tok)
        } else {
            None
        }
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
        match self.advance() {
            Some(Token::Ident(s)) => Ok(s),
            other => Err(AilangError::ParseError {
                msg: format!("Expected identifier, got {:?}", other),
            }),
        }
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

        // Optional sink: > "path" or >> "path"
        let sink = match self.peek() {
            Some(Token::Write) => {
                self.advance();
                let path = self.parse_string_arg()?;
                Some(Sink::Write(path))
            }
            Some(Token::Append) => {
                self.advance();
                let path = self.parse_string_arg()?;
                Some(Sink::Append(path))
            }
            _ => None,
        };

        Ok(Pipeline { stages, sink })
    }

    fn parse_stage(&mut self) -> AilangResult<Stage> {
        match self.peek().cloned() {
            // let binding: let x = pipeline
            Some(Token::Let) => {
                self.advance();
                let name = self.expect_ident()?;
                // consume '='
                match self.advance() {
                    Some(Token::Eq) => {}
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

            // filter(expr) — parsed as a special Call with one Expr arg
            Some(Token::Filter) => {
                self.advance();
                self.expect(&Token::LParen)?;
                let expr = self.parse_expr()?;
                self.expect(&Token::RParen)?;
                Ok(Stage::Filter(expr))
            }

            // sort(.field) — parsed as a Call
            Some(Token::Sort) => {
                self.advance();
                let args = if matches!(self.peek(), Some(Token::LParen)) {
                    self.advance();
                    let args = self.parse_arg_list()?;
                    self.expect(&Token::RParen)?;
                    args
                } else { vec![] };
                Ok(Stage::Call(Call { name: "sort".into(), args }))
            }

            // group(.field)
            Some(Token::Group) => {
                self.advance();
                let args = if matches!(self.peek(), Some(Token::LParen)) {
                    self.advance();
                    let args = self.parse_arg_list()?;
                    self.expect(&Token::RParen)?;
                    args
                } else { vec![] };
                Ok(Stage::Call(Call { name: "group".into(), args }))
            }

            // set(.field, value)
            Some(Token::Set) => {
                self.advance();
                self.expect(&Token::LParen)?;
                let args = self.parse_arg_list()?;
                self.expect(&Token::RParen)?;
                Ok(Stage::Call(Call { name: "set".into(), args }))
            }

            // assert("value")
            Some(Token::Assert) => {
                self.advance();
                self.expect(&Token::LParen)?;
                let args = self.parse_arg_list()?;
                self.expect(&Token::RParen)?;
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

            // Bare keywords that transform the stream
            Some(Token::Json)   => { self.advance(); Ok(Stage::Keyword("json".into())) }
            Some(Token::Yaml)   => { self.advance(); Ok(Stage::Keyword("yaml".into())) }
            Some(Token::Csv)    => { self.advance(); Ok(Stage::Keyword("csv".into())) }
            Some(Token::Lines)  => { self.advance(); Ok(Stage::Keyword("lines".into())) }
            Some(Token::Count)  => { self.advance(); Ok(Stage::Keyword("count".into())) }
            Some(Token::Trim)   => { self.advance(); Ok(Stage::Keyword("trim".into())) }
            Some(Token::Upper)  => { self.advance(); Ok(Stage::Keyword("upper".into())) }
            Some(Token::Lower)  => { self.advance(); Ok(Stage::Keyword("lower".into())) }
            Some(Token::Unique) => { self.advance(); Ok(Stage::Keyword("unique".into())) }
            Some(Token::Sha256) => { self.advance(); Ok(Stage::Keyword("sha256".into())) }
            Some(Token::Sha512) => { self.advance(); Ok(Stage::Keyword("sha512".into())) }
            Some(Token::Md5)    => { self.advance(); Ok(Stage::Keyword("md5".into())) }
            Some(Token::WriteKw) => { self.advance(); Ok(Stage::Keyword("write".into())) }

            // Named function call: name(...) or ns.name(...)
            Some(Token::Ident(_)) => {
                let name = self.build_call_name()?;
                let args = if matches!(self.peek(), Some(Token::LParen)) {
                    self.advance();
                    let args = self.parse_arg_list()?;
                    self.expect(&Token::RParen)?;
                    args
                } else {
                    vec![]
                };
                Ok(Stage::Call(Call { name, args }))
            }

            other => Err(AilangError::ParseError {
                msg: format!("Unexpected token in stage: {:?}", other),
            }),
        }
    }

    /// Build a potentially namespaced call name: `net.get`, `fs.read`, `base64.enc`
    fn build_call_name(&mut self) -> AilangResult<String> {
        let first = self.expect_ident()?;
        if matches!(self.peek(), Some(Token::Dot)) {
            // Check if next-next is an ident (namespace.method)
            if self.pos + 1 < self.tokens.len() {
                if let Token::Ident(_) = &self.tokens[self.pos + 1] {
                    self.advance(); // consume dot
                    let method = self.expect_ident()?;
                    return Ok(format!("{}.{}", first, method));
                }
            }
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

    fn parse_string_arg(&mut self) -> AilangResult<String> {
        match self.advance() {
            Some(Token::Str(s)) => Ok(s),
            other => Err(AilangError::ParseError {
                msg: format!("Expected string, got {:?}", other),
            }),
        }
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
        // Note: Token::Write is `>` in the lexer — dual use as sink AND comparator.
        // In expression context (inside parens/args), `>` means greater-than.
        let op = match self.peek() {
            Some(Token::Eq)    => BinOp::Eq,
            Some(Token::Ne)    => BinOp::Ne,
            Some(Token::Write) => BinOp::Gt,  // `>` = greater-than in expr context
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
            Some(Token::Null)       => { self.advance(); Ok(Expr::Null) }
            Some(Token::True)       => { self.advance(); Ok(Expr::Bool(true)) }
            Some(Token::False)      => { self.advance(); Ok(Expr::Bool(false)) }
            Some(Token::Int(i))     => { self.advance(); Ok(Expr::Int(i)) }
            Some(Token::Float(f))   => { self.advance(); Ok(Expr::Float(f)) }
            Some(Token::Str(s))     => { self.advance(); Ok(Expr::Str(s)) }
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
            // Field path: .field or .field.method(args)
            Some(Token::Dot) => {
                let mut parts = Vec::new();
                while matches!(self.peek(), Some(Token::Dot)) {
                    self.advance(); // consume dot
                    let field = self.expect_ident()?;
                    parts.push(field);
                }
                // Check if it's a method call: .path.contains("x")
                if matches!(self.peek(), Some(Token::Dot)) {
                    self.advance();
                    let method = self.expect_ident()?;
                    self.expect(&Token::LParen)?;
                    let args = self.parse_arg_list()?;
                    self.expect(&Token::RParen)?;
                    Ok(Expr::FieldMethod(parts, method, args))
                } else {
                    Ok(Expr::Field(parts))
                }
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
    let tokens: Vec<Token> = token_pairs.into_iter().map(|(t, _)| t).collect();
    let mut parser = Parser::new(tokens);
    parser.parse_pipeline()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_call() {
        let ast = parse(r#"read("config.yaml")"#).unwrap();
        assert_eq!(ast.stages.len(), 1);
    }

    #[test]
    fn test_parse_pipeline_with_sink() {
        let ast = parse(r#"glob("*.log") | grep("ERROR") | json > "out.json""#).unwrap();
        assert_eq!(ast.stages.len(), 3);
        assert!(matches!(ast.sink, Some(Sink::Write(_))));
    }

    #[test]
    fn test_parse_filter() {
        let ast = parse("filter(.age > 18)").unwrap();
        assert_eq!(ast.stages.len(), 1);
    }

    #[test]
    fn test_parse_namespaced_call() {
        let ast = parse(r#"net.get("https://example.com")"#).unwrap();
        if let Stage::Call(c) = &ast.stages[0] {
            assert_eq!(c.name, "net.get");
        } else {
            panic!("Expected Call stage");
        }
    }
}
