# Parser

The parser converts JSONata expression strings into an abstract syntax tree (AST). It uses a Pratt parsing algorithm, which handles operator precedence elegantly.

## Pipeline

```text
Expression string -> Tokenizer -> Token stream -> Pratt Parser -> Raw AST -> Post-processing -> Final AST
```

## Tokenizer

The tokenizer (`parser/tokenizer.rs`) converts the expression string into a stream of tokens. It handles:

- **Numbers** -- integer and floating-point literals
- **Strings** -- single and double-quoted, with escape sequences (`\n`, `\t`, `\uXXXX`)
- **Operators** -- `+`, `-`, `*`, `/`, `%`, `=`, `!=`, `<`, `>`, `<=`, `>=`, `&`, `~>`, `..`, `:=`
- **Keywords** -- `and`, `or`, `in`, `true`, `false`, `null`, `function`
- **Identifiers** -- field names and variable names (`$var`)
- **Regex literals** -- `/pattern/flags`
- **Brackets** -- `(`, `)`, `[`, `]`, `{`, `}`
- **Special** -- `*`, `**`, `?`, `:`, `;`, `,`, `.`
- **Comments** -- `/* ... */` (discarded)
- **Backtick-quoted names** -- `` `field name` ``

Errors at this stage have `S01xx` codes.

## Pratt Parser

The Pratt parser (`parser/pratt.rs`) transforms the token stream into an AST using a precedence-climbing algorithm. Each token type has:

- A **null denotation (nud)** -- how it behaves at the start of an expression
- A **left denotation (led)** -- how it behaves after a left-hand expression
- A **binding power** -- its precedence level

This approach handles:

- Binary operators with correct precedence
- Unary prefix operators
- Postfix operators (array indexing, function calls)
- Ternary conditional (`? :`)
- Variable binding (`:=`)
- Lambda definitions (`function(...){}`)
- Path expressions with dots, wildcards, and predicates

Errors at this stage have `S02xx` codes.

## AST

The AST (`parser/ast.rs`) represents the parsed expression as a tree of nodes. Key node types include:

- **Path** -- dotted field access (`a.b.c`)
- **Binary** -- operators (`+`, `-`, `=`, `and`, etc.)
- **Unary** -- prefix operators (`-x`)
- **Block** -- semicolon-separated expressions
- **Conditional** -- ternary `? :`
- **Lambda** -- function definitions
- **FunctionCall** -- function invocations
- **Array** -- array constructors
- **Object** -- object constructors
- **Filter** -- predicate expressions
- **Sort** -- order-by clauses
- **Transform** -- `|pattern|{update},deletes|`
- **Wildcard** -- `*`
- **Descendant** -- `**`
- **Parent** -- `%`

Each node carries a `Span` indicating its position in the source expression.

## Post-Processing

The post-processing step (`parser/process.rs`) transforms the raw AST into a form more suitable for evaluation:

- **Path flattening** -- nested path nodes are flattened into sequences of steps
- **Predicate attachment** -- filter predicates are attached to their parent steps
- **Group-by processing** -- grouping expressions are validated and attached
- **Sort clause processing** -- order-by clauses are validated
- **Lambda parameter validation** -- parameters must start with `$`
- **Literal step detection** -- literal values in path positions are flagged as errors

This phase catches semantic errors that the grammar-level parser cannot detect, such as predicates after grouping expressions (`S0209`) or multiple group-by clauses (`S0210`).

## Entry Point

The public parser API is a single function:

```rust
pub fn parse(expr: &str) -> Result<Ast>
```

This runs the full pipeline: tokenize, parse, and post-process. The resulting `Ast` is stored inside the `Seuil` struct and reused for every evaluation.

## Error Recovery

The parser does not attempt error recovery. The first syntax error terminates parsing and returns a descriptive error with the exact source position. This is deliberate -- partial ASTs are more dangerous than failing fast.
