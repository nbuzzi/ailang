# ailang Language Specification (v0.1)

This document formally specifies the syntax and semantics of the **ailang** scripting language.

---

## 1. Grammar (EBNF)

```ebnf
Pipeline      ::= Stage ("|" Stage)* [Sink] ;

Sink          ::= (">" | ">>") (String | DottedIdent) ;

Stage         ::= LetBinding
                | VarRef
                | FilterShorthand
                | FilterCall
                | MapTransform
                | FieldAccess
                | Keyword
                | FunctionCall ;

LetBinding    ::= "let" Ident "=" Pipeline ;
VarRef        ::= "$" Ident ;
FilterShorthand ::= "?" Expr ;
FilterCall    ::= "filter" "(" Expr ")" ;
MapTransform  ::= "map" "(" "{" (Ident ":" Expr ("," Ident ":" Expr)*)? "}" ")" ;
FieldAccess   ::= "." Ident ;
Keyword       ::= "json" | "yaml" | "csv" | "lines" | "count" | "trim"
                | "upper" | "lower" | "unique" | "sha256" | "sha512"
                | "md5" | "write" ;

FunctionCall  ::= (Ident | (Ident "." Ident)) ("(" ArgList? ")")? ;
ArgList       ::= Expr ("," Expr)* ;

Expr          ::= OrExpr ;
OrExpr        ::= AndExpr ("||" AndExpr)* ;
AndExpr       ::= CmpExpr ("&&" CmpExpr)* ;
CmpExpr       ::= Primary (CmpOp Primary)? ;
CmpOp         ::= "==" | "!=" | ">" | ">=" | "<" | "<=" ;

Primary       ::= "null"
                | "true"
                | "false"
                | Integer
                | Float
                | String
                | DottedIdent
                | "$" Ident
                | "!" Primary
                | "(" Expr ")"
                | "." Ident ("." Ident)* ("." Ident "(" ArgList? ")")? ;

DottedIdent   ::= Ident ("." Ident)* ;
Ident         ::= [a-zA-Z_][a-zA-Z0-9_]* ;
String        ::= '"' [^"]* '"' ;
Integer       ::= "-"? [0-9]+ ;
Float         ::= "-"? [0-9]+ "." [0-9]+ ;
```

---

## 2. Evaluation Model

* **Streaming Semantics:** Stages pass data sequentially through the pipeline from left to right.
* **Typing:** Dynamic, strongly typed values (`Null`, `Bool`, `Int`, `Float`, `Str`, `List`, `Object`, `FileEntry`, `Line`).
* **Error Semantics:** Non-zero exit code with single-line minified JSON on `stderr` conforming to:
  ```json
  {"error": "message", "code": 1, "stage": "stage_name"}
  ```
