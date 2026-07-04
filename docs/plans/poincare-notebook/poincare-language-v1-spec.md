# Poincare Language V1 Specification

This document is the Phase 1 deliverable of `poincare-language-roadmap.md`: a freeze of the V1 language shape (syntax, grammar, precedence, value model, builtins, and I/O) sufficient to build the lexer, parser, resolver, and interpreter without churn.

It is a specification of intent, not a formal standard. Where a decision is deliberately left open, it is listed under Open Decisions rather than implied by omission.

## Status

- Scope: freeze enough of V1 to build Phases 2-6 of the language roadmap.
- Syntax family: Poincare-specific, brace-delimited, not indentation-sensitive.
- This supersedes the tentative `do` / `end` block note in the roadmap Phase 1 deliverable list. V1 uses braces. See Frozen Decisions.

## Frozen Decisions

| Decision | Choice | Rationale |
| --- | --- | --- |
| Syntax family | Poincare-specific | Distinctive identity, no indentation-sensitive parsing, room for math syntax |
| Block delimiters | Braces `{ }` everywhere | One delimiter for control-flow bodies, block functions, and plot config; trivial parsing and error recovery |
| Indentation | Not significant | Simpler parser and error recovery |
| Variable binding | Bare `x = 3`, rebindable | Matches Julia/Matlab/Python muscle memory; no ceremony |
| Expression function | `f(x, y) = expr` | Math-native; LHS call pattern means "definition" |
| Block function | `fn name(params) { ... }` | Multi-statement functions with a keyword marker |
| Delayed/symbolic `:=` | Reserved, unused in V1 | Kept for future `SetDelayed`-style semantics |
| `let` keyword | Not used | Bare `=` is enough for V1 |
| Comments | `# line comment` | Familiar; matches Python |
| Ranges | `a..b`, inclusive both ends | Serves both integer iteration and real plotting domains |
| Strings | `"double quoted"` | Single quotes reserved |
| Composition | `g . f` (ASCII), `g ∘ f` (display) | Right-to-left math convention |
| Pipe | `x \|> f` | Left-to-right data transforms |
| Anonymous function | `x => x^2`, `(x, y) => x + y` | `=>` keeps `->` free for the type/function arrow |
| Core representation | Lower surface AST to an untyped core IR; interpret the core | Additive path to a later typechecker; see roadmap Forward-Compatibility |
| Type signatures | Optional, `f : R^2 -> R` | Drives plot inference; opt-in rigor |
| Canonical form | ASCII; unicode is display/input sugar | Diffable, grep-able notebook source |
| Mutation | Rebinding allowed; nested mutation deferred | Simple environment model for V1 |

## Lexical Structure

### Comments

```text
# line comment to end of line
```

Block comments are deferred.

### Identifiers

- Start with a letter or `_`, continue with letters, digits, or `_`.
- Case-sensitive.
- `R`, `R2`, `R3` are ordinary identifiers used in type signatures; they are not reserved keywords, but the signature grammar gives them meaning in signature position.

### Keywords

```text
fn  for  in  if  else  and  or  not  plot  over  true  false
```

`print` is not a keyword; it is a builtin function. Plot-kind words (`surface`, `curve`, `scatter`, `vector_field`, `volume`, `isosurface`) are contextual identifiers inside a `plot` statement, not global reserved words.

Reserved for the typed future (tokenized as reserved even though V1 does not use them, so they cannot be repurposed later): `Type`, `match`, `forall`, `let`, `fun`. `data` is deliberately not reserved (too common a variable name; the frozen examples use it), so a future ADT keyword must be spelled differently. See Forward-Compatibility in `poincare-language-roadmap.md`.

### Literals

- Integer: `0`, `42`, `1_000`
- Float: `1.5`, `3.14`, `1e3`, `2.5e-4`
- Boolean: `true`, `false`
- String: `"text"` with escapes `\" \\ \n \t`
- List: `[1, 2, 3]`
- Range: `a..b`

### Operators and punctuation

```text
+  -  *  /  %  ^
==  !=  <  <=  >  >=
and  or  not
.   ∘        (composition)
|>            (pipe)
->            (type / signature arrow only; reserved, never a lambda)
=>            (lambda / anonymous function)
=             (binding / definition)
..            (range)
:             (type ascription / signature separator, map/plot fields)
,  ;  ( )  [ ]  { }
```

## Operator Precedence

Highest to lowest. Associativity noted per level.

| Level | Operators | Associativity | Notes |
| --- | --- | --- | --- |
| 1 | `f(...)` call, `a[...]` index, `(...)`, `[...]`, `{...}` | n/a | primary / application |
| 2 | `^` | right | `2^3^2` = `2^(3^2)` |
| 3 | unary `-`, `not` | prefix | `-x^2` = `-(x^2)` because `^` binds tighter |
| 4 | `.` `∘` composition | right | `h . g . f` = `h . (g . f)` |
| 5 | `*` `/` `%` | left | |
| 6 | `+` `-` | left | |
| 7 | `..` range | non-assoc | `1..10` |
| 8 | `==` `!=` `<` `<=` `>` `>=` | non-assoc | chained comparisons are a parse error |
| 9 | `and` | left | |
| 10 | `or` | left | |
| 11 | `\|>` pipe | left | `d \|> f \|> g` = `(d \|> f) \|> g` |

Notes:
- The exponent right operand parses at the unary level, so `2 ^ -3` is valid.
- Applying a composed function requires parentheses: `(g . f)(x)`, since call binds tighter than composition.
- The signature arrow `->` is parsed only in signature position (after `name :`), not as a general expression operator.

## Statement Grammar

A program (or cell) is a sequence of statements. The value of a cell is the value of its final expression statement, if any (see Output Semantics in the runtime roadmap).

```text
program     := statement*

statement   := signature
             | binding
             | func_def
             | for_stmt
             | if_stmt
             | plot_stmt
             | expr_stmt

signature   := ident ":" type ("->" type)+          # optional, precedes a func_def
type        := ident ("^" int)?                     # R, R^2, R^3, and simple products later

binding     := ident "=" expr

func_def    := ident "(" params ")" "=" expr        # expression function
             | "fn" ident "(" params ")" block      # block function
params      := (ident ("," ident)*)?

for_stmt    := "for" ident "in" expr block

if_stmt     := "if" expr block ("else" (if_stmt | block))?

plot_stmt   := "plot" plot_kind? expr over_clause? plot_block?
plot_kind   := ident                                # surface, curve, scatter, ...
over_clause := "over" domain ("," domain)*
domain      := ident "=" expr                       # x = -6..6
plot_block  := "{" plot_field* "}"
plot_field  := ident "=" expr                       # z = f(x, y), resolution = [160, 160]

expr_stmt   := expr

block       := "{" statement* expr? "}"             # trailing expr is the block value
```

`print(...)` is an ordinary call and therefore an `expr_stmt`; it is not special grammar.

### Definition vs. binding disambiguation

- LHS is a bare identifier: `x = expr` is a variable binding.
- LHS is a call pattern: `f(x, y) = expr` is an expression-function definition.
- `fn name(...) { ... }` is always a block-function definition.

### Blocks are expressions

`{ ... }` is an expression whose value is its trailing expression (or `Unit` if none). This makes `if` an expression:

```text
sign = if x > 0 { 1 } else { -1 }
```

`for` is a statement and evaluates to `Unit`.

## Type-Directed Plotting

An optional signature gives a function a domain and codomain, which determines its geometric interpretation and lets `plot f over ...` infer the plot kind. See the Type-Directed Syntax section of `poincare-language-roadmap.md`.

```text
f : R^2 -> R
f(x, y) = sin(x^2 + y^2) / (x^2 + y^2)

plot f over x = -6..6, y = -6..6      # inferred as a surface
```

When no signature is present, the plot kind is taken from an explicit plot-kind word (`plot surface f ...`) or inferred from arity where unambiguous. Signature-driven inference and explicit plot-kind words must agree; a conflict is a diagnostic.

## Runtime Value Model

V1 runtime values (some may be stubs whose operations arrive in later phases):

| Value | V1 status | Notes |
| --- | --- | --- |
| `Unit` | full | value of statements and empty blocks |
| `Bool` | full | |
| `Number` | full | `f64` at evaluation in V1, but literals are retained losslessly in the AST; exact/rational tower deferred to language Phase 7 |
| `String` | full | UTF-8 |
| `List` | full | heterogeneous, ordered |
| `Function` | full | expression and block functions, closures over lexical scope |
| `Range` | full | inclusive `a..b` |
| `Attachment` | full | opaque handle resolved via runtime host |
| `Bytes` | full | raw attachment bytes |
| `Table` | partial | from `csv(...)`; column access builtins in Phase 5 |
| `Array` | partial | numeric matrix/vector from tabular data |
| `MathExpr` | partial | numeric evaluation in V1; symbolic growth in Phase 7 |
| `Plot` | partial | single plot spec |
| `Graph` | partial | collection of plots + presentation state |
| `Analysis` | partial | wraps existing `poincare-lib` analysis outputs |
| `ImageRef` | partial | reference to a bundled image/preview |

Graph, table, plot, and analysis values are ordinary values: they can be bound, returned, printed, emitted, and passed to functions. Attachment and bytes values are ordinary values whose resolution is mediated by the runtime host.

The runtime value model aligns with `poincare-evaluator::EvalValue`; conversion happens in `poincare-evaluator-poincare`.

## Builtin Categories

Builtins are ordinary functions that return values; they do not mutate hidden UI state. The full signatures are specified in language Phase 5; V1 freezes the categories and representative names.

| Category | Representative builtins |
| --- | --- |
| math | `sin`, `cos`, `tan`, `exp`, `log`, `sqrt`, `abs`, `min`, `max`, `floor`, `ceil` |
| graph | `graph`, `add_plot`, `surface`, `curve`, `scatter`, `vector_field`, `volume`, `isosurface` |
| table | `column`, `columns`, `rows`, `array2d` (filtering/sorting later) |
| analysis | `gradient`, `derivative`, `fit`, and existing `poincare-lib` analysis hooks |
| attachment | `attachment`, `bytes`, `text`, `csv`, `csv_matrix` |
| output | `print`, `emit` |

## Attachment-Scoped I/O

V1 I/O is notebook-scoped: code reads bundled attachments through the runtime host, never arbitrary filesystem paths.

```text
raw    = text(attachment("notes.txt"))
data   = csv(attachment("measurements.csv"))
matrix = csv_matrix(attachment("grid.csv"))
```

V1 attachment builtins: `attachment(name_or_id)`, `bytes(a)`, `text(a)`, `csv(a)`, `csv_matrix(a)`. External-filesystem refresh is an explicit UI action, not an execution-time capability.

## Mutation Rules

- Rebinding a name is allowed: `x = 1` then later `x = 2`.
- Nested/interior mutation of composite values (list elements, table cells) is deferred; builtins return new values instead.
- Function parameters and loop variables are bound in their own scope and do not leak outward.

## Scoping

- Lexical scoping.
- Blocks introduce a new scope; a binding inside a block does not escape it, except that top-level cell statements share the session environment.
- Functions close over the environment in which they are defined.
- Undefined-name use is a name-resolution diagnostic.

## Worked Examples

Concise plotting:

```text
f : R^2 -> R
f(x, y) = sin(x^2 + y^2) / (x^2 + y^2)

plot f over x = -6..6, y = -6..6
```

Sequenced work with a loop and printing:

```text
f(x, y) = sin(x^2 + y^2) / (x^2 + y^2)

for a in [1, 2, 3, 4] {
  print("plotting a =", a)
  plot surface {
    z = a * f(x, y)
    x = -6..6
    y = -6..6
    resolution = [160, 160]
  }
}
```

Value-oriented graph accumulation:

```text
g = graph()

for a in [1, 2, 3] {
  p = surface(z = a * sin(x * y), x = -3..3, y = -3..3)
  g = add_plot(g, p)
}

g
```

Data from an attachment:

```text
samples = csv(attachment("samples.csv"))
xs      = column(samples, "x")

plot scatter samples {
  x = "x"
  y = "y"
  z = "z"
}
```

Composition and conditional:

```text
warp(v) = 0.5 * v
h : R^2 -> R
h(x, y) = cos(x) * cos(y)

plot (warp . h) over x = -3..3, y = -3..3

label = if max(xs) > 0 { "positive" } else { "non-positive" }
print(label)
```

## Open Decisions

Deferred out of Phase 1 to avoid overreach; they do not block parser/interpreter work.

- Whether anonymous functions ship in V1 at all. Their syntax is settled regardless: `=>` (`x => x^2`), never `->`, which is reserved for the type/function arrow.
- Whether `plot` remains a statement keyword or becomes a pure builtin once type-directed inference is proven.
- Exact spelling and shipping of both composition operators (`.`/`∘`) and the pipe (`|>`) in V1 vs. one of each.
- Whether `over` domain annotations belong to the signature, the plot call, or both.
- Record/map literal syntax (deferred; `{ }` is a block in V1, so map literals need a distinct form later).
- `%` semantics for floats (modulo vs. remainder).

## Downstream Impact

Freezing this shape enables:
- Language Phase 2: lexer/parser/AST (with uniform spans) against this grammar and precedence table.
- Language Phase 3: resolver/scoping against the scoping rules here, producing interned, path-capable names.
- Language Phase 4: tree-walking interpreter over an untyped core IR, not the surface AST directly.
- Language Phase 5: builtins fleshed out per the frozen categories.

Forward-compatibility constraints that must hold from Phase 2 onward (see `Forward-Compatibility for a Typed Core` in `poincare-language-roadmap.md`):
- Lower the surface AST to an untyped core IR and interpret the core; do not tree-walk the surface AST directly.
- Uniform source spans on every AST and core node.
- Interned, path-capable names, not bare strings.
- Lossless numeric literals in the AST.
- Type/signature positions are restricted expressions that can grow, not a closed separate type-AST.
- Surface rebinding lowers to fresh binders so the core stays referentially clean.
- Pure expression evaluation stays pure; effects remain host-mediated.

The `:=`, `do`/`end`, and indentation-style `plot ... \n x: -6..6` snippets in the other notebook plan documents have been swept to match this spec.
