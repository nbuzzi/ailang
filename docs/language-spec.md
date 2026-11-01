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

* **Pipeline Semantics:** Stages pass buffered values from left to right; the evaluator does not yet implement incremental streaming.
* **Typing:** Dynamic, strongly typed values (`Null`, `Bool`, `Int`, `Float`, `Str`, `List`, `Object`, `FileEntry`, `Line`).
* **Error Semantics:** Non-zero exit code with single-line minified JSON on `stderr` conforming to:
  ```json
  {"error": "message", "code": 1, "stage": "stage_name"}
  ```

## 3. File sinks and execution policy

A sink writes a single string as raw text, a single non-string value as its JSON representation, and multiple values as a JSON array. A parsed object or array therefore keeps its structure when saved. Raw strings are not implicitly JSON-quoted. `>>` appends content and a newline.

CLI and MCP use the same execution policy. Flags also apply to script execution. Dry-run previews are independent against the current filesystem and return action records; they do not simulate dependent multi-statement transactions. See the README for full scope and isolation limitations.

`sort(.field)`, `group(.field)` and `set(.field, value)` treat the first field argument as a field name, not an expression evaluated against the whole input. Sort compares numeric values numerically. `exec(program, [args])` launches a program directly without shell expansion.
