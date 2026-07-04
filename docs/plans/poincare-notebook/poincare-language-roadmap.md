# Poincare Language Roadmap

## Goal

Create a Poincare-native programming language for notebooks: a small interpreted language with first-class mathematical expressions, tables, plots, graphs, and analysis outputs.

The language should start practical and constrained, but it should be designed as the first version of a real optimized language rather than a throwaway graph DSL. It should support notebook workflows early, then grow toward symbolic math and optimized numerical evaluation over time.

## Progress

| Phase | Description | Status | Effort | Priority |
| --- | --- | --- | --- | --- |
| 1 | Language scope, syntax, and value model | Complete | Medium | High |
| 2 | Lexer, parser, AST, and diagnostics | Complete | Large | High |
| 3 | Name resolution, scopes, and runtime environment | Planned | Large | High |
| 4 | Tree-walking interpreter | Planned | Large | High |
| 5 | Poincare graph/table/math builtins | Planned | Large | High |
| 6 | Evaluator integration | Planned | Medium | High |
| 7 | Symbolic-capable `MathExpr` expansion | Planned | Large | Medium |
| 8 | Bytecode or IR optimization path | Planned | Large | Low |
| 9 | Optimized numeric kernels | Planned | Large | Medium |
| 10 | Optional JIT/AOT backend investigation | Planned | Very Large | Low |

## Scope Notes

- The first implementation should be an interpreter, not Rust source generation.
- The language should be Poincare-native. Python, Rust, Rhai, Jupyter, and external CAS tools may be optional interop or backend paths later, but they should not define the saved notebook language.
- The language should live in a separate `poincare-lang` crate.
- `poincare-evaluator-poincare` should adapt `poincare-lang` execution into the backend-neutral `poincare-evaluator` API.
- The language should be small but real: it needs statements, blocks, loops, printing, functions, and typed notebook values early.
- The initial runtime should favor clear semantics and good diagnostics over raw speed.
- V1 I/O should be notebook-scoped attachment I/O, not arbitrary filesystem access.

## Proposed Crate Split

### `poincare-lang`

Responsibilities:
- Lexer and parser for Poincare source.
- AST definitions.
- Source spans and parse diagnostics.
- Name resolution and scope model.
- Runtime value model.
- Tree-walking interpreter.
- Builtin registration API.
- Host/runtime API for safe notebook services such as attachment resolution.
- Poincare-native `MathExpr` representation or shared bridge to `poincare-evaluator::MathExpr`.
- Optional future IR/bytecode/compiler passes.

Non-responsibilities:
- Notebook document storage.
- egui UI.
- Markdown parsing.
- GPU rendering.
- Backend-neutral notebook evaluator traits.
- Direct filesystem access policy.

### `poincare-evaluator-poincare`

Responsibilities:
- Implement `poincare-evaluator::Evaluator` for the Poincare language.
- Own per-session interpreter state for notebook execution.
- Convert interpreter values into `EvalValue` outputs.
- Convert diagnostics into evaluator diagnostics.

Non-responsibilities:
- Defining syntax or AST.
- Owning notebook file format.
- App UI.

## Implementation Strategy

Do not translate notebook code directly to Rust.

Reasoning:
- Notebook cells need fast feedback.
- Rust compile latency is too high for normal cell execution.
- Rust compiler errors would not be good Poincare notebook diagnostics.
- Sandboxing native compiled code is harder.
- Dynamic notebook values such as graphs, tables, plots, and analysis outputs do not map naturally to generated Rust source.
- A Poincare AST/IR is needed anyway for diagnostics, formatting, dependencies, and future symbolic work.

Use this staged pipeline:

```text
source text
  ->
lexer / parser
  ->
AST (with spans)
  ->
name resolution / validation
  ->
untyped core IR          # semantic target; see Forward-Compatibility for a Typed Core
  ->
[future: elaborate / typecheck]
  ->
tree-walking interpreter over the core
  ->
Poincare values
  ->
EvalValue outputs
```

The core IR is a semantic target introduced early, not a later optimization. It is untyped in V1 and leaves room for an additive elaboration/typecheck pass. See `Forward-Compatibility for a Typed Core`.

Longer-term optimization can add:
- bytecode VM for control flow and faster repeated execution
- optimized expression kernels for sampling/plotting hot paths
- vectorized table operations
- optional JIT/AOT backends if profiling proves they are necessary

Rust source generation should remain a last resort, not the default optimization strategy.

## V1 Language Shape

V1 should be a small programming language with first-class Poincare values.

Required early features:
- comments
- numeric literals
- booleans
- strings
- symbols
- arithmetic expressions
- function calls
- lists
- assignments
- function definitions
- `do` blocks / sequencing
- `for` loops
- `if` expressions or statements
- `print`
- graph/table/math builtins
- attachment-scoped I/O builtins

Deliberately defer:
- classes/objects
- imports/modules
- async/concurrency
- macros
- user-defined operators
- pattern matching
- arbitrary filesystem access
- direct network access
- mutation of nested data structures
- complex static type system

Example:

```text
{
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
}
```

Equivalent value-oriented form:

```text
{
  g = graph()

  for a in [1, 2, 3] {
    p = surface(z = a * sin(x * y), x = -3..3, y = -3..3)
    g = add_plot(g, p)
  }

  g
}
```

The concise `plot surface ...` syntax can be sugar for emitting a graph or plot output from the current cell.

## Syntax Direction

The final surface syntax is intentionally not settled yet.

Leading options:
- Python-like syntax.
- Poincare-specific clean math syntax.

The examples in this document are illustrative, not final syntax decisions. In particular, the language should not accidentally drift into an R-like style if that does not match the desired product feel.

### Python-Like Candidate

```text
def f(x, y):
    return sin(x^2 + y^2) / (x^2 + y^2)

data = csv(attachment("samples.csv"))

for a in [1, 2, 3]:
    print("plotting a =", a)
    plot(surface(z = a * f(x, y), x = -6..6, y = -6..6))
```

Pros:
- familiar to many users
- readable for notebooks
- natural for `for`, `if`, and `print`

Cons:
- indentation-sensitive parsing
- risks feeling like a Python subset without Python compatibility
- graph construction may become function-call heavy unless Poincare adds clean plotting syntax

### Poincare-Specific Candidate

```text
f(x, y) = sin(x^2 + y^2) / (x^2 + y^2)

data = csv(attachment("samples.csv"))

for a in [1, 2, 3] {
  print("plotting a =", a)

  plot surface {
    z = a * f(x, y)
    x = -6..6
    y = -6..6
    resolution = [160, 160]
  }
}
```

Pros:
- distinctive Poincare identity
- avoids indentation-sensitive parsing
- good fit for structured graph blocks
- avoids noisy JavaScript-style object construction
- leaves room for symbolic/math-specific syntax later

Cons:
- custom language design burden
- less immediately familiar than Python-like syntax

### Current Preference

The likely direction is Python-like or Poincare-specific, with a current lean toward a Poincare-specific clean math syntax if it remains easy to learn.

Avoid committing to:
- R-like assignment/style as the primary syntax.
- JavaScript-like object-heavy plotting syntax.
- Mathematica-compatible syntax.

Reserve `:=` for future consideration only if delayed/symbolic assignment semantics become important. Prefer ordinary `=` / `let` / `fn` style in early syntax sketches unless a clear semantic distinction is needed.

### Expression Comparison

The following table sketches a representative sample of expressions in the current Poincare leaning against Python and Mathematica. The Poincare column reflects the direction in this document (Poincare-specific, brace-delimited, bare `=`, math-native function definitions, `..` ranges, ASCII-first type-directed syntax) and is illustrative, not a frozen grammar.

| Concept | Poincare | Python | Mathematica |
| --- | --- | --- | --- |
| Comment | `# note` | `# note` | `(* note *)` |
| Variable binding | `x = 3` | `x = 3` | `x = 3` |
| Expression function | `f(x, y) = sin(x*y)` | `f = lambda x, y: sin(x*y)` | `f[x_, y_] := Sin[x y]` |
| Block function | `fn g(a) { print(a); a * 2 }` | `def g(a):`<br>`    print(a)`<br>`    return a * 2` | `g[a_] := (Print[a]; a 2)` |
| Function call | `f(2, 3)` | `f(2, 3)` | `f[2, 3]` |
| List | `[1, 2, 3]` | `[1, 2, 3]` | `{1, 2, 3}` |
| Integer range | `1..10` | `range(1, 11)` | `Range[1, 10]` |
| Real interval (domain) | `-6..6` | `(-6, 6)` | `{-6, 6}` |
| Indexing | `xs[0]` | `xs[0]` | `xs[[1]]` |
| For loop | `for a in [1, 2, 3] { print(a) }` | `for a in [1, 2, 3]:`<br>`    print(a)` | `Do[Print[a], {a, {1, 2, 3}}]` |
| If expression | `if x > 0 { 1 } else { -1 }` | `1 if x > 0 else -1` | `If[x > 0, 1, -1]` |
| Power | `x^2` | `x ** 2` | `x^2` |
| Comparison | `x >= 0` | `x >= 0` | `x >= 0` |
| Print | `print("a =", a)` | `print("a =", a)` | `Print["a = ", a]` |
| Type signature | `f : R^2 -> R` | `def f(x: float, y: float) -> float:` | (no direct equivalent) |
| Composition | `g . f` (or `g ∘ f`) | `lambda x: g(f(x))` | `g @* f` |
| Data pipeline | `data \|> fit(poly(3))` | `fit(poly(3), data)` | `data // fit` |
| CSV attachment | `csv(attachment("d.csv"))` | `pandas.read_csv("d.csv")` | `Import["d.csv"]` |
| Plot surface | `plot f over x = -6..6, y = -6..6` | `ax.plot_surface(X, Y, Z)` | `Plot3D[f[x, y], {x, -6, 6}, {y, -6, 6}]` |
| Numeric derivative | `derivative(f, x)` | `numpy.gradient(...)` | `D[f[x], x]` |

Notes:
- The Python column uses idiomatic library-free equivalents where possible and names a common library (`pandas`, `numpy`, `matplotlib`) where the operation is not part of the base language, to show where Poincare folds plotting/data/analysis into the core language.
- Mathematica uses `[ ]` for calls and `{ }` for lists, which is the opposite of the Poincare convention; the table highlights how signature-driven plotting replaces Mathematica's explicit `Plot3D`/`Range` domain arguments.
- Anonymous-function syntax is deliberately omitted because it is unsettled: reusing `->` for both signatures (`R -> R`) and lambdas (`x -> x^2`) is an open question tracked under Type-Directed Syntax.

## Type-Directed Syntax

Poincare language should lean on a small amount of category-theory-inspired structure where it earns its keep, without adopting category-theory vocabulary in the surface syntax. The goal is a distinctive mathematical feel — "the notebook where a function's type is its picture" — that also removes special cases rather than adding them.

The organizing idea is that a function's signature, its domain and codomain, determines its geometric meaning. In a plotting notebook the arrow is not decoration; it is the plot type.

### Signature-Driven Plot Inference

A function annotated with a domain/codomain signature carries enough information to choose how it is sampled and rendered.

| Signature | Geometric interpretation |
| --- | --- |
| `R -> R` | curve `y = f(x)` |
| `R -> R^2` | parametric planar curve |
| `R -> R^3` | parametric space curve |
| `R^2 -> R` | height-field surface |
| `R^2 -> R^3` | parametric surface |
| `R^3 -> R` | scalar field (isosurface / volume) |
| `R^3 -> R^3` | vector field |
| `R^2 -> R^2` | planar vector field / transformation |

This collapses the `plot surface { ... }` / `plot curve { ... }` / `plot vector_field { ... }` family into a single form where the type disambiguates intent:

```text
f : R^2 -> R
f(x, y) = sin(x^2 + y^2) / (x^2 + y^2)

plot f over x = -6..6, y = -6..6
```

```text
c : R -> R^3
c(t) = (cos t, sin t, t)

plot c over t = 0..6.283
```

```text
v : R^3 -> R^3
v(x, y, z) = (-y, x, 0)

plot v over box(-3..3, -3..3, -3..3)
```

This maps directly onto Poincare's existing scalar, vector, parametric-curve, and parametric-surface wrappers in `poincare-lib`. The signature is a unifying surface syntax over capability that already exists, not a new engine.

### Composition

Function composition should be a first-class operator so that plottable objects form a small, coherent algebra.

```text
plot (warp . f) over x = -6..6, y = -6..6
data |> filter(x > 0) |> fit(poly(3)) |> plot
```

Two composition directions carry their established meanings and should not be blended into an invented middle:
- `.` (ASCII) / `∘` (display) is math-convention right-to-left: `g . f` means "apply `f`, then `g`".
- `|>` is a left-to-right pipe, natural for data transforms.

The composed type follows from the component types: composing `f : A -> B` with `g : B -> C` yields `g . f : A -> C`, and the result is plotted by its inferred signature like any other function.

### Design Constraints

- Structure, not jargon. Use typed arrows and composition, but keep functors, natural transformations, and monads out of the V1 surface syntax. A user who has never heard the word "morphism" must still read `f : R^2 -> R` as "takes two reals, gives one". Intro examples must not require category-theory vocabulary.
- ASCII-first canonical form. The saved notebook form uses ASCII (`->`, `.`, `R`, `in`) so notebooks stay diffable and grep-able. Unicode (`→`, `∘`, `ℝ`, `∈`) is an optional display and input convenience, ideally with editor substitution (`\circ` -> `∘`, `\to` -> `→`) similar to Julia/LaTeX. Unicode and ASCII forms must be interchangeable on load.
- Signatures are optional, not required. When present, a signature disambiguates the plot type and improves diagnostics. When absent, the plot type is inferred from arity and from how the value is plotted (for example an explicit `plot surface f`). Quick work stays quick; rigor is opt-in.
- Signatures describe shape, not a full type system. `R`, `R^2`, `R^3`, and simple products/tuples are enough for V1. This is not a static type checker and should not grow into one before the language semantics are stable.

### Open Decisions

- How far V1 carries the arrow notation into data transforms: whether pipelines and mapping a morphism over a table/list are V1 surface syntax or deferred until after plot inference and composition are proven.
- The exact spelling of the pipe and composition operators, and whether both `.`/`∘` and `|>` ship in V1 or only one.
- Whether domain annotations like `over x = -6..6, y = -6..6` are part of the signature, the plot call, or both.

### V1 Scope

In V1, type-directed syntax should include:
- optional domain/codomain signatures on function definitions
- signature-driven plot type inference for the existing `poincare-lib` plot families
- a single composition operator with a clear direction
- ASCII canonical form with optional unicode display

Deferred:
- category-theory vocabulary or abstractions in the surface language
- mapping/functorial operations over structured data as core syntax
- any static type checking beyond shape inference for plotting

## Attachment-Scoped I/O

Poincare language should support I/O through notebook attachments first.

The language should not initially expose arbitrary filesystem reads such as:

```text
read_file("/some/random/path.csv")
```

Instead, code should access files that are part of the notebook bundle:

```text
raw = text(attachment("notes.txt"))
table = csv(attachment("measurements.csv"))
matrix = csv_matrix(attachment("grid.csv"))
```

This keeps notebooks portable and gives the runtime a clear security boundary.

V1 attachment builtins:
- `attachment(name_or_id)`
- `bytes(attachment)`
- `text(attachment)`
- `csv(attachment)`
- `csv_matrix(attachment)`

Likely later table/array helpers:
- `column(table, name)`
- `columns(table, names)`
- `array2d(table)`
- `rows(table)`
- `filter(table, ...)`
- `sort(table, ...)`

Example:

```text
data = csv(attachment("samples.csv"))

plot scatter data {
  x = "x"
  y = "y"
  z = "z"
}
```

The language should request attachments from the host runtime rather than opening files itself.

Sketch:

```rust
pub trait RuntimeHost {
    fn resolve_attachment(&self, name_or_id: &str) -> Result<AttachmentHandle, RuntimeError>;
}
```

The concrete notebook runtime can resolve `AttachmentHandle` from the zipped notebook bundle, while future trusted backends may add explicit external-file refresh workflows.

## Runtime Values

The language value model should align with `poincare-evaluator::EvalValue`.

Initial values:
- `Unit`
- `Bool`
- `Number`
- `String`
- `List`
- `Function`
- `Attachment`
- `Bytes`
- `MathExpr`
- `Table`
- `Array`
- `Plot`
- `Graph`
- `Analysis`
- `ImageRef`

Potential later values:
- `Record`
- `Range`
- `Matrix`
- `ExactNumber`
- `SymbolicSet`
- `Region`
- `Module`

Graph, table, and analysis values should be normal values that can be assigned, returned, printed, emitted, and passed to functions.
Attachment and bytes values should be normal runtime values, but their resolution must remain mediated by the runtime host.

## `MathExpr` Direction

The existing plotting expression parser in `poincare-lib` is useful but too narrow for notebooks. `poincare-lang` needs a symbolic-capable expression representation that can initially evaluate numerically.

Initial shape:

```rust
pub enum MathExpr {
    Number(NumberExpr),
    Symbol(Symbol),
    String(String),
    Bool(bool),
    Call {
        head: Box<MathExpr>,
        args: Vec<MathExpr>,
    },
    Binary {
        op: BinaryOp,
        lhs: Box<MathExpr>,
        rhs: Box<MathExpr>,
    },
    Unary {
        op: UnaryOp,
        expr: Box<MathExpr>,
    },
    List(Vec<MathExpr>),
    Matrix(Vec<Vec<MathExpr>>),
    Relation {
        op: RelationOp,
        lhs: Box<MathExpr>,
        rhs: Box<MathExpr>,
    },
    Function {
        params: Vec<Symbol>,
        body: Box<MathExpr>,
    },
}
```

Number shape:

```rust
pub enum NumberExpr {
    Int(i64),
    Float(f64),
    Rational { numer: i64, denom: i64 },
    Constant(MathConstant),
}

pub enum MathConstant {
    Pi,
    E,
    Infinity,
}
```

Design notes:
- Use symbolic `Call` nodes so future functions and symbolic operations do not require enum expansion for every case.
- Keep explicit common nodes for operators, relations, lists, matrices, and functions because they need good formatting, diagnostics, and later optimization.
- Preserve source spans outside or alongside expression nodes for diagnostics.
- Make conversion from `MathExpr` to current `poincare-lib` numeric expressions explicit and fallible.
- Add symbolic operations gradually rather than pretending V1 is a complete CAS.

## Forward-Compatibility for a Typed Core

Poincare is intended to grow from a notebook language into a general programming language with a type-theory flavour. V1 should not build a type system, but it must avoid foreclosing one. The rule: decisions that are cheap now and expensive to reverse once a parser, interpreter, and saved notebooks exist should be made in the type-theory-compatible direction now.

### Core IR as a semantic target, not only an optimization

Type-theory languages elaborate: `surface syntax -> elaborate -> typed core -> evaluate`. V1 should already lower the surface AST into a small untyped core IR and interpret the core, rather than tree-walking the surface AST directly.

This reframes Phase 8: a core IR is introduced early as the semantic target, and bytecode/JIT remain later performance concerns layered on top. The V1 core can be tiny (variables, application, lambda, `let`, literals, primitives, and later `match`) and carry no types. Adding a checker later becomes an additive elaboration pass over an existing core, not a rewrite of the interpreter and resolver.

Pipeline:

```text
surface syntax
  -> parse (AST + spans)
  -> resolve (names, scopes)
  -> lower to untyped core IR
  -> [future: elaborate / typecheck]
  -> interpret core
```

### Terms and types should share one universe

A type-theory future treats types as terms and math expressions as terms. Two parallel expression hierarchies with lossy conversion is the expensive trap. `MathExpr` should be designed as an embedding/subset of the core term language, or at minimum surface-to-`MathExpr` conversion must be total and structure-preserving. The signature grammar (`R^2 -> R`) is a restricted expression today; it is intended to grow into full expressions, so it must not be built as a separate incompatible type-AST.

### Do-now checklist

- Uniform source spans on every AST and core node, not only expressions.
- Intern identifiers; represent names as structured paths, not bare strings, so alpha-equivalence, qualified names, and modules stay reachable.
- Keep numeric literals lossless in the AST (retain exact/textual form; commit to `f64` only at evaluation) so a future integer/rational/real tower can recover precision.
- Do not build a closed type grammar; keep type/signature positions as restricted expressions that can grow.
- Lower surface rebinding to fresh binders in the core. The surface may rebind names (`x = 1` then `x = 2`); the core should stay referentially clean via shadowing/`let`, so a future checker is not fighting mutation.
- Keep effects segregated. Host-mediated attachment I/O already does this; pure expression evaluation must stay pure so a future totality/effect discipline has a clean boundary.
- Reserve syntax and keywords now rather than repurposing them later.

### Syntax reserved for the typed future

- `->` is reserved exclusively for the function-type arrow (`R -> R`, later `(x : A) -> B x`). It must not be reused for lambdas.
- Anonymous functions use `=>` (`x => x^2`, `(x, y) => x + y`).
- `:` is type ascription (`x : T`), consistent with the current signature separator.
- Reserved-for-future keywords: `Type`, `match`, `forall`, `let`, `fun`. These tokenize as reserved even though V1 does not use them. `data` is deliberately not reserved (too common a variable name), and `in` is already an in-use keyword, not a reserved-for-future one.

### Explicit non-goals for V1

Not built now, only kept reachable:
- universes / sorts / `Type : Type`
- dependent function and pair types
- bidirectional or full inference typechecking
- totality and effect checking
- a proof/tactic layer

These become additive once the core IR, uniform spans, structured names, and unified term/type representation exist.

### The test

If adding types later means "insert an elaboration pass between resolve and interpret, annotate existing core nodes, and enable the lossless conversions," the V1 foundation was right. If it means rewriting the interpreter, resolver, and name model and re-parsing saved notebooks, a foreclosure was missed.

## Phase 1: Language Scope, Syntax, and Value Model

Goal: freeze enough of the V1 language shape to build parser and runtime work without churn.

Deliverables:
- V1 syntax sketch and examples.
- Explicit syntax comparison and final V1 syntax decision.
- Operator precedence table.
- Statement grammar:
  - assignment
  - function definition
  - `do`
  - `for`
  - `if`
  - `print`
  - expression statement
  - plot statement
- Runtime value list.
- Builtin categories:
  - math
  - graph
  - table
  - analysis
  - attachment
  - output
- Attachment-scoped I/O syntax and examples.
- Decisions on block syntax:
  - explicit `do` / `end`
  - no indentation-sensitive parsing initially
- Decisions on mutation:
  - allow rebinding names
  - defer nested data mutation

Notes:
- Prefer explicit block delimiters over indentation-sensitive syntax for easier parsing and error recovery.
- This phase should produce enough examples to guide parser tests.
- If Python-like syntax is chosen, explicitly accept the parser/error-recovery cost of indentation-sensitive blocks.

Frozen:
- The V1 language shape is specified in `docs/plans/poincare-notebook/poincare-language-v1-spec.md`.
- Syntax family: Poincare-specific, brace-delimited, not indentation-sensitive.
- Block delimiters are braces `{ }`, superseding the tentative `do` / `end` note in the deliverables above.
- Bindings: bare `x = 3` (rebindable); expression functions `f(x, y) = expr`; block functions `fn name(...) { ... }`. `:=` reserved for future delayed/symbolic semantics; `let` unused.
- Optional type signatures (`f : R^2 -> R`) drive type-directed plot inference.
- Operator precedence table, statement grammar, runtime value list, builtin categories, and attachment I/O are frozen in the spec.
- Mutation: rebinding allowed; nested data mutation deferred.
- Open sub-decisions (anonymous-function syntax, `plot` keyword vs builtin, composition/pipe operator shipping, `over` placement) are tracked in the spec's Open Decisions.

## Phase 2: Lexer, Parser, AST, and Diagnostics

Goal: parse Poincare source into a structured AST with useful source spans and errors.

Deliverables:
- Tokenizer.
- Parser.
- AST types.
- Source span model.
- Parse diagnostics with line/column information.
- Recovery for common syntax errors where practical.
- Parser test fixtures for:
  - expressions
  - assignments
  - function definitions
  - loops
  - blocks
  - print
  - plot statements
  - invalid syntax

Notes:
- Hand-written recursive descent or Pratt parsing is likely sufficient initially.
- Parser should be independent of notebook UI and evaluator runtime.

Implemented:
- New `poincare-lang` crate (depends only on `serde`; no notebook/evaluator/UI dependencies).
- Hand-written lexer producing a token stream with byte-offset spans. Newlines are significant statement separators but indentation is not; newlines are suppressed inside `()`/`[]` and after line-continuation tokens, matching the frozen brace-and-newline grammar.
- Precedence-climbing expression parser implementing the frozen precedence table, including right-associative `^` and composition, non-associative comparisons/ranges (chaining is a diagnostic), `|>` pipes, calls with positional and named arguments, indexing, lists, ranges, blocks-as-expressions, `if`/`else`, and `=>` lambdas.
- Statement grammar: signatures (`f : R^2 -> R`, types stored as restricted expressions), bindings, expression and block function definitions (with call-vs-definition lookahead), `for`, and `plot` (optional kind/target/`over`/config-block).
- Uniform `Span` on every AST node; `Symbol` newtype for names (interning-ready); numeric literals retained losslessly as raw text; reserved-for-future keywords tokenized and rejected. These satisfy the Forward-Compatibility constraints.
- Structured diagnostics with line/column rendering via a `SourceMap`, plus error recovery that synchronizes to the next separator so multiple errors are reported per parse.
- 31 parser/lexer test fixtures in `crates/poincare-lang/tests/parse.rs` covering expressions, assignments, function definitions, loops, blocks, print, plot statements, signatures, lambdas, line continuation, and invalid syntax. Lowering to a core IR is deferred to Phase 4 (interpreter) per the core-IR-as-semantic-target plan.

## Phase 3: Name Resolution, Scopes, and Runtime Environment

Goal: establish how names, cells, functions, and scopes behave.

Deliverables:
- Lexical scope model.
- Runtime environment model.
- Name resolution for:
  - variables
  - functions
  - builtins
  - function parameters
  - loop variables
- Cell/session environment behavior.
- Diagnostics for undefined names and invalid redefinitions.
- Decision on notebook-cell persistence:
  - evaluator session stores definitions from previous cells
  - run-all reconstructs session state from top to bottom

Notes:
- V1 can be dynamically typed, but it should still produce clear diagnostics where value kinds are obviously wrong.
- Execution-order staleness in the notebook relies on this environment being reconstructable from cell order.

## Phase 4: Tree-Walking Interpreter

Goal: execute the resolved AST directly and produce runtime values and outputs.

Deliverables:
- Expression evaluation.
- Statement execution.
- Function call support.
- `do` block execution.
- `for` loop execution.
- `if` execution.
- `print` output stream.
- Runtime diagnostics.
- Loop iteration limits or cancellation hooks.
- Tests for normal execution and runtime errors.

Notes:
- This is the first real execution engine.
- Keep it simple. The interpreter only needs to be fast enough for notebook orchestration, not dense numeric sampling.

## Phase 5: Poincare Graph/Table/Math Builtins

Goal: make the language useful for actual Poincare notebook work.

Deliverables:
- Builtins for attachments:
  - `attachment(...)`
  - `bytes(...)`
  - `text(...)`
  - `csv(...)`
  - `csv_matrix(...)`
- Table/data builtins:
  - column selection
  - table-to-array helpers
  - column selection
  - basic filtering/sorting later if feasible
- Graph builtins:
  - `graph()`
  - `add_plot(...)`
  - `surface(...)`
  - `curve(...)`
  - `scatter(...)`
  - `vector_field(...)`
  - `volume(...)`
  - `isosurface(...)`
- Analysis builtins:
  - `gradient(...)`
  - `derivative(...)`
  - `fit(...)`
  - other existing `poincare-lib` analysis actions where practical
- Output builtins:
  - `print(...)`
  - graph/plot emission
  - table emission

Notes:
- Builtins should return normal values, not mutate hidden UI state.
- Concise notebook syntax can emit values automatically, but the runtime model should remain value-oriented.
- General filesystem I/O should remain deferred until the notebook security/trust model is mature.

## Phase 6: Evaluator Integration

Goal: make `poincare-lang` usable from notebook cells through `poincare-evaluator`.

Deliverables:
- `poincare-evaluator-poincare` crate.
- `Evaluator` implementation for Poincare language cells.
- Conversion from language runtime values into `EvalValue`.
- Conversion from language diagnostics into evaluator diagnostics.
- Session state persistence or reset behavior.
- Runtime host bridge for attachment resolution.
- Tests showing notebook-style cell execution:
  - define function in cell 1
  - use function in cell 2
  - emit graph/table/print outputs
  - load a CSV attachment into a table value

Notes:
- `poincare-lang` should not depend on notebook document types.
- `poincare-evaluator-poincare` is the adapter layer.

## Phase 7: Symbolic-Capable `MathExpr` Expansion

Goal: grow the expression model toward symbolic notebook use without committing to a full CAS immediately.

Deliverables:
- Exact integer/rational support where practical.
- Symbolic substitution.
- Basic simplification:
  - constant folding
  - neutral elements
  - simple algebraic cleanup
- Basic symbolic differentiation rules for common functions.
- Expression formatting hooks.
- Fallible conversion to numeric plotting expressions.
- Diagnostics for unsupported symbolic operations.

Notes:
- This phase should not try to clone Mathematica.
- It should build enough symbolic capability to make notebook math feel coherent and prepare for optional CAS backends.

## Phase 8: Bytecode or IR Optimization Path

Goal: improve execution structure if the tree-walking interpreter becomes limiting.

Deliverables:
- Lower resolved AST to a compact IR or bytecode.
- Bytecode VM or IR interpreter.
- Function call frames.
- Control-flow instructions for loops and branches.
- Better cancellation and step limits.
- Performance comparison against tree-walking interpreter.

Notes:
- This is optional until profiling proves it is needed.
- Do not start here. The tree-walking interpreter is simpler and better for shaping semantics.
- Scope clarification: the untyped core IR itself is introduced in V1 as a semantic target (see `Forward-Compatibility for a Typed Core`), not here. This phase is only about a compact bytecode/VM for performance, layered on top of that core.

## Phase 9: Optimized Numeric Kernels

Goal: speed up hot mathematical evaluation paths used by plotting and sampling.

Deliverables:
- Identify hot expression-evaluation paths in plotting workflows.
- Compile `MathExpr` or resolved expressions into optimized evaluator closures or bytecode.
- Constant folding and common-subexpression opportunities where practical.
- Vectorized sampling investigation.
- Potential GPU/WGSL path investigation for dense grids.

Notes:
- Notebook orchestration and plotting kernels have different performance needs.
- Optimize numeric sampling separately from general language execution.

## Phase 10: Optional JIT/AOT Backend Investigation

Goal: decide whether heavier compilation is useful after the language and runtime semantics are stable.

Deliverables:
- Evaluate backend options:
  - Cranelift
  - Wasm
  - WGSL/GPU kernels
  - native dynamic libraries
  - Rust source generation as a last resort
- Security implications.
- Packaging implications.
- Performance benchmarks.

Notes:
- JIT/AOT is not a prerequisite for a useful notebook language.
- Avoid native code generation unless there is a clear performance reason and a clear sandboxing story.

## Recommended Order

1. Define V1 syntax and value semantics.
2. Build parser, AST, and diagnostics.
3. Add name resolution and runtime environment.
4. Implement tree-walking interpreter.
5. Add Poincare graph/table/math builtins.
6. Integrate through `poincare-evaluator-poincare`.
7. Expand symbolic-capable `MathExpr`.
8. Optimize only after profiling real notebooks.
