# Poincare Notebook Roadmap

## Goal

Build toward a Mathematica-style Poincare notebook: a persistent document made of text, mathematical input, computed output, tables, and embedded interactive Poincare graphs.

The long-term goal is ambitious, but the architecture should make useful intermediate products possible:

1. A report-style document with text, graph blocks, and analysis tables.
2. A computational notebook with executable cells and graph-producing outputs.
3. A symbolic notebook with richer mathematical language, formatted math I/O, assumptions, simplification, solving, and exact/numeric interop.

The key constraint is that phases 1 and 2 should not be throwaway implementations. They should establish document, block, graph, output, and execution boundaries that can grow into the symbolic notebook target.

## Progress

| Phase | Description | Status | Effort | Priority |
| --- | --- | --- | --- | --- |
| 1 | Shared notebook document model | Complete | Large | High |
| 2 | Report-mode notebook surface | Planned | Large | High |
| 3 | Embedded graph block lifecycle | Planned | Large | High |
| 4 | Evaluator API crate and typed value boundary | Complete | Large | High |
| 5 | Execution-ready cell and output model | Planned | Large | High |
| 6 | Computational kernel integration | Planned | Very Large | Medium |
| 7 | Mathematical language and symbolic layer | Planned | Very Large | Medium |
| 8 | Formatted math input/output | Planned | Very Large | Medium |
| 9 | Reactive dependencies and reproducibility | Planned | Large | Medium |
| 10 | Notebook security and trust model | Planned | Medium | High |
| 11 | Notebook export, sharing, and packaging | Planned | Large | Medium |

## Scope Notes

- A notebook is a new product surface, not a small extension of the graph viewport.
- The first durable architectural decision is the notebook document model. If that model only represents report blocks, it will be expensive to evolve into a computational notebook later.
- Graph blocks should store library-owned `GraphSpec` / project data rather than app-only viewport state where possible.
- Computed outputs should be first-class persisted objects with provenance, not loose text appended to a document.
- The symbolic notebook goal should guide the shape of the model, but symbolic computation should not block useful report and computational milestones.
- The notebook should reuse Poincare's existing graphing, analysis, table, export, and persistence layers instead of forking them.
- Notebook files should be bundle-native from the start. Images, CSV files, data attachments, previews, graph specs, and metadata are likely enough that a single loose JSON document would become a migration problem quickly.
- `poincare-app` and `poincare-notebook-app` should remain separate product surfaces. The graphing app stays a 3D plotting application; the notebook app embeds graphs through `poincare-lib` / `viewport-lib` rather than absorbing the graphing app.
- Poincare language I/O should be attachment-scoped in V1. Cells should read CSV/text/binary data through bundled attachments, not arbitrary filesystem paths.

## Proposed Crate Split

### `poincare-notebook-lib`

Responsibilities:
- Notebook document schema and versioned persistence.
- Notebook bundle manifest and attachment model.
- Stable block/cell ids and provenance.
- Typed notebook values and outputs:
  - text
  - diagnostics
  - tables
  - `GraphSpec`
  - images/previews
  - symbolic expressions
  - structured data
- Evaluation model:
  - evaluator trait
  - execution status
  - output lifecycle
  - stale-output tracking
  - dependency metadata
- Kernel protocol abstractions that are UI-independent.
- Graph/block integration types that can refer to `poincare-lib` without depending on egui.
- Optional feature-gated integrations for external engines.

Non-responsibilities:
- egui widgets.
- Docking, panels, menus, shortcuts, and editor presentation.
- GPU viewport ownership.
- File dialogs and platform app behavior.

### `poincare-notebook-app`

Responsibilities:
- egui/eframe notebook UI.
- Block editor and cell chrome.
- Markdown/rich-text rendering.
- Code-cell editing and syntax highlighting.
- Embedded interactive graph viewport blocks.
- Static graph previews.
- Switching graph blocks between static preview mode and one active interactive viewport.
- Notebook-specific commands, menus, keyboard shortcuts, and export UI.
- Integration with `poincare-app` or shared app components where useful.

Non-responsibilities:
- Defining the notebook file model.
- Defining typed output semantics.
- Owning mathematical evaluation semantics.
- Owning graph compilation logic that already belongs in `poincare-lib`.

### Workspace Direction

Likely workspace additions:

```toml
members = [
    "crates/poincare-lib",
    "crates/poincare-lang",
    "crates/poincare-evaluator",
    "crates/poincare-evaluator-poincare",
    "crates/poincare-evaluator-rhai",
    "crates/poincare-app",
    "crates/poincare-notebook-lib",
    "crates/poincare-notebook-app",
    "crates/poincare-dvd",
    "crates/poincare-mobile",
]
```

Dependency direction should be:

```text
poincare-notebook-app
  -> poincare-notebook-lib
  -> poincare-evaluator
  -> poincare-lib

poincare-notebook-app
  -> optional evaluator backend crates

poincare-evaluator-poincare
  -> poincare-evaluator
  -> poincare-lang
  -> poincare-lib

poincare-lang
  -> poincare-lib

poincare-evaluator
  -> poincare-lib

poincare-evaluator-rhai
  -> poincare-evaluator
  -> poincare-lib

poincare-notebook-lib
  -> poincare-evaluator
  -> poincare-lib

poincare-notebook-app
  -> viewport-lib
```

`poincare-notebook-lib` should not depend on `eframe`, `egui`, `wgpu`, or platform file-dialog crates.

## Notebook Bundle Format

Notebook files should be zipped bundles from the first implementation.

Recommended bundle shape:

```text
notebook.pnb/
  manifest.json
  document.json
  attachments/
    <attachment-id>/
      data.csv
      metadata.json
  graphs/
    <graph-block-id>/
      graph.json
      view.json
      preview.png
  previews/
    <block-id>.png
  outputs/
    <cell-id>/
      output.json
      assets/
```

The exact extension can be decided later, but the structure should separate:
- document structure
- graph specs
- graph view state
- static previews
- attachments
- evaluated outputs
- bundle/package metadata

Bundle-native design means:
- CSVs and imported data can be attached and made portable.
- Headless graph previews can be stored beside the graph data.
- Large generated outputs can be cached without bloating `document.json`.
- Later export/sharing work has a natural place to store assets.

## Attachments

Attachments should be first-class notebook objects.

Use cases:
- CSV files imported by cells.
- Image files referenced by Markdown/report blocks.
- Graph preview PNGs.
- Data files produced by analysis or export.
- External project/spec references copied into the notebook for portability.

Initial behavior:
- A cell or block can reference an attachment id.
- The evaluator can resolve an attachment into bytes, text, table rows, image data, or other typed values.
- Saving the notebook writes attachments into the zipped bundle.
- Attachment metadata records original path when available, media type, size/hash, and created/updated timestamps.

Deferred behavior:
- Refresh linked attachments from original filesystem paths.
- Deduplicate identical attachments by hash.
- Track cell dependencies on attachment hashes for precise staleness.

## Evaluator Crate Split

### `poincare-evaluator`

Responsibilities:
- Stable evaluator trait and request/response API.
- Poincare-owned typed values:
  - number
  - boolean
  - string
  - mathematical expression
  - table
  - plot spec
  - graph spec
  - analysis output
  - image/preview reference
  - diagnostic
- Source spans and structured diagnostics.
- Evaluation context and context-delta types.
- Host service interfaces for notebook-scoped resources such as attachments.
- Language ids and evaluator metadata.
- Optional evaluator registry/factory traits.
- Backend-neutral representation of executable-cell output.

Non-responsibilities:
- Rhai/Python/Rust/Symbolica-specific runtime objects.
- UI state.
- Notebook block ordering, layout, and persistence details.
- GPU rendering.

The central rule is that notebooks store source text, language id, and typed Poincare outputs. They should not persist backend runtime values.

Sketch:

```rust
pub trait Evaluator {
    fn language_id(&self) -> &'static str;

    fn evaluate_cell(&mut self, request: EvalRequest) -> EvalResponse;
}

pub struct EvalRequest {
    pub document_id: String,
    pub cell_id: String,
    pub source: String,
    pub context: EvalContext,
}

pub struct EvalResponse {
    pub status: EvalStatus,
    pub outputs: Vec<EvalOutput>,
    pub diagnostics: Vec<EvalDiagnostic>,
    pub context_delta: Option<EvalContextDelta>,
}

pub enum EvalValue {
    None,
    Number(f64),
    Bool(bool),
    String(String),
    Expr(MathExpr),
    Table(TableValue),
    Plot(PlotSpec),
    Graph(GraphSpec),
    Analysis(AnalysisOutput),
    Image(ImageRef),
}
```

Attachment and host-service details can be passed through `EvalContext` or an explicit runtime host adapter. The evaluator API should make it possible for `poincare-lang` to evaluate:

```text
data = csv(attachment("samples.csv"))
```

without giving the language direct filesystem access.

### `poincare-evaluator-rhai`

Responsibilities:
- Early embedded scripting backend.
- Register Poincare graph/table/math helper functions into Rhai.
- Convert Rhai results into `poincare-evaluator::EvalValue`.
- Prove the evaluator API can support executable cells.

Non-responsibilities:
- Defining the notebook language permanently.
- Owning persisted notebook semantics.
- Leaking Rhai types into notebook files.

### `poincare-evaluator-poincare`

Responsibilities:
- Backend for the Poincare-native notebook language.
- Execute `poincare-lang` AST/IR through the interpreter.
- Convert interpreter results into `poincare-evaluator::EvalValue`.
- Own notebook-language runtime state that is independent of app UI.

Non-responsibilities:
- Parsing Markdown or notebook documents.
- UI rendering.
- Compiling user code to Rust.
- Replacing `poincare-evaluator` as the backend-neutral API.

### Future Evaluator Backends

Possible later crates:
- `poincare-evaluator-python`: optional external Python/Jupyter-style integration.
- `poincare-evaluator-rust`: optional Rust/eval reference, likely inspired by Evcxr.
- `poincare-evaluator-symbolic`: internal symbolic implementation or CAS bridge.
- `poincare-evaluator-wasm`: sandboxed plugin/runtime evaluator.

These should be swappable behind the `poincare-evaluator` API.

## Existing Rust Building Blocks

These are candidate crates and systems that can make the plan more concrete. They should still be evaluated with small prototypes before adoption.

| Area | Candidate | Use | Fit |
| --- | --- | --- | --- |
| Serialization | `serde`, `serde_json`, possibly `ron` | Versioned notebook/project persistence | Strong fit; already used in Poincare |
| Bundled files | `zip` or similar archive crate | Notebook bundle with graph specs, assets, data, previews | Likely useful; needs format design |
| Markdown parsing | `pulldown-cmark` | Lightweight Markdown parsing | Strong fit for report-mode source text |
| Markdown to HTML / AST | `comrak` | CommonMark/GFM parsing, AST manipulation, HTML/CommonMark export | Strong fit if report export needs Markdown transforms |
| egui Markdown rendering | `egui_commonmark` | Render Markdown inside egui | Good app-side fit; supports custom math render hook |
| Text buffer | `ropey` | Large editable text/cell source buffers | Good fit if basic egui text editing becomes limiting |
| Incremental parsing | `tree-sitter` | Syntax trees, highlighting, structural editing | Good later fit for code/math cells |
| Syntax highlighting | `syntect` or tree-sitter queries | Code cell highlighting | App-side fit; `egui_commonmark` can also use `syntect` for Markdown code blocks |
| Code editor widget | `egui_code_editor` or custom egui widget | First code-cell editor | Possible bootstrap; likely custom work later |
| Scripting evaluator | `rhai` | Simple embedded evaluator for early executable cells | Strong early fit; safe Rust embedding and custom Rust functions |
| Alternative scripting | `rune` | Embedded dynamic language | Worth comparing to Rhai, less obviously simple |
| JavaScript evaluator | `boa_engine` | JS-like notebook cells | Possible, but less aligned with mathematical notation |
| Sandboxed runtime | `wasmtime` | Execute untrusted or plugin-like notebook code | Strong future option, more work than Rhai |
| Jupyter protocol | `jupyter-protocol` | Kernel/client message types and MIME-style outputs | Useful if adopting Jupyter-like kernel boundaries |
| Rust notebook reference | `evcxr` | Rust REPL/Jupyter kernel architecture reference | Reference only; not likely the core Poincare kernel |
| Symbolic algebra | `symbolica` | CAS-style expressions, derivatives, pattern matching, polynomial algebra, code generation | Technically strong, but licensing makes it a strategic decision |
| Math rendering/export | `typst`, `typst-svg` | Pretty math/docs and export pipeline | Strong candidate for formatted output/export, heavier than simple Markdown |
| LaTeX math rendering | `katex` | Render LaTeX to HTML | Good for HTML export; egui display still needs HTML/SVG/raster path |
| SVG rasterization | `resvg` | Render SVG previews for egui/image export | Useful if math/HTML export produces SVG |
| HTML/PDF export | Typst pipeline, HTML export, or external print pipeline | Report/notebook export | Needs prototyping; no one crate solves the whole path |

## What Exists vs What Poincare Must Build

### Mostly Available

- Serialization primitives.
- Markdown parsing and egui rendering.
- Basic text/code editing building blocks.
- Syntax parsing/highlighting infrastructure.
- Embedded scripting engines.
- Jupyter protocol message types.
- Math/document rendering engines.
- SVG rasterization.

### Needs Poincare-Specific Design

- Notebook document schema.
- Block/cell identity and editing model.
- Typed notebook output model.
- Graph block lifecycle and provenance.
- Integration between notebook outputs and `GraphSpec`.
- Evaluation abstraction that can grow from simple scripting to symbolic computation.
- Dependency/staleness model.
- Secure execution policy.
- Multi-viewport resource management for embedded graph blocks.
- Notebook bundle format.

### Strategic Unknowns

- Whether the first executable language should be:
  - a small Poincare expression language,
  - Rhai with Poincare-specific graph/math APIs,
  - a custom symbolic language,
  - a Jupyter-compatible kernel protocol,
  - or a hybrid.
- Whether to adopt Symbolica, interoperate with it optionally, or build a smaller symbolic layer first.
- Whether formatted math should be Typst-first, LaTeX/KaTeX-first, or custom-rendered from Poincare's symbolic AST.
- Whether notebook execution should be in-process for simplicity or process-isolated from the start.
- How much of the first Poincare-native language should be interpreted directly versus lowered into a backend such as Rhai.

## Graph Block Ownership and Interaction

Graph blocks should support both embedded snapshots and links/references.

Supported ownership modes:
- `snapshot`: self-contained graph spec, view state, and preview stored inside the notebook bundle.
- `linked`: graph block references an external Poincare project or graph spec and can be refreshed.
- `computed`: graph block is produced by evaluating a cell and carries source-cell provenance.

All graph blocks should preserve enough state to round-trip between static preview and interactive mode:
- graph spec / plot definitions
- visible plots
- selected plot where relevant
- camera position and target
- zoom / distance
- azimuth, elevation, roll, or equivalent camera orientation
- projection mode
- viewport/display settings that affect the rendered frame
- graph presentation settings such as scalarbars, labels, titles, legends, and background

Interaction model:
- Most graph blocks render as static preview images.
- Only one graph block is interactive at a time.
- Activating a graph replaces the preview with a live `viewport-lib` viewport using the saved graph/view state.
- Deactivating the graph captures a new headless preview from the final frame and stores the updated graph/view state.
- Reopening a notebook should restore the graph to the exact saved view when the user activates it again.
- Each graph block should have a reset-view command.
- Each graph block should have an open-in-`poincare-app` command for full graph editing.

Headless rendering:
- Poincare already uses `viewport-lib` / `poincare-lib` paths for PNG export, so notebook previews should reuse that infrastructure.
- `poincare-notebook-app` should manage preview invalidation and cache policy.
- `poincare-notebook-lib` should store preview metadata and graph/view state.

## Execution and Staleness Model

Initial execution should be order-based, not dependency-graph-based.

V1 behavior:
- Cells execute in notebook order.
- Running a cell can update evaluator state for later cells.
- Editing a cell marks that cell and all later executable cells as stale.
- Running all executes cells from top to bottom.
- Running an individual stale cell is allowed but should show that earlier stale cells may affect correctness.
- Outputs persist, but stale outputs are visibly marked.

This avoids pretending the notebook has precise dependency tracking before the language and evaluator are mature.

Deferred behavior:
- Static variable/function dependency extraction.
- Attachment-hash dependency tracking.
- Graph-output dependency tracking.
- Recompute only affected cells.
- Detect hidden state and out-of-order execution hazards.

Explicit stale/dependency metadata means the notebook records why an output may no longer be trustworthy, without initially trying to solve the whole dependency problem. Examples:
- source cell changed after output was produced
- earlier executable cell changed after this output was produced
- referenced attachment hash changed
- evaluator/kernel restarted since output was produced
- graph block was manually edited after being computed

## Language Direction

The notebook should use Markdown-style documents with executable cells, similar in spirit to Jupyter. The executable cell source should not be Python, Rust, or Rhai as the canonical product language.

Recommended direction:
- Define a Poincare-native notebook language and typed IR in a separate `poincare-lang` crate.
- Use `poincare-evaluator` as the stable semantic API.
- Use `poincare-evaluator-poincare` as the notebook evaluator backend for the native language.
- Optionally use Rhai as an early implementation detail behind `poincare-evaluator-rhai`.
- Keep Python, Rust, and Jupyter kernels as optional interoperability paths later.
- Avoid required dependency on a proprietary or restrictive CAS.
- Implement the native language interpreter-first. Do not translate notebook code directly to Rust.

Early Poincare-native syntax should be a small programming language with first-class math/plot/table values, not only a graph command DSL.

V1 should include:
- assignments
- function definitions
- `do` blocks / sequencing
- `for` loops
- `if` expressions or statements
- `print`
- lists
- function calls
- attachment-scoped I/O
- graph/table/math builtins

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

Simple graphing syntax should still be concise:

```text
f(x, y) = sin(x^2 + y^2) / (x^2 + y^2)

plot surface {
  z = f(x, y)
  x = -6..6
  y = -6..6
  resolution = [160, 160]
}
```

or:

```text
data = csv(attachment("samples.csv"))
points = scatter(data, x = "x", y = "y", z = "z")
plot points
```

Preferred attachment-oriented data access:

```text
data = csv(attachment("samples.csv"))
matrix = csv_matrix(attachment("grid.csv"))
```

The important pipeline is:

```text
Markdown cell source
Poincare executable cell source
        ->
lexer / parser
        ->
Poincare AST
        ->
resolver / type-ish checks
        ->
Poincare IR
        ->
interpreter
        ->
EvalValue outputs
        ->
NotebookOutput blocks
```

At first, operations such as `derivative`, `integral`, `gradient`, and `fit` can dispatch to existing sampled/numeric Poincare analysis. Later, the same user-facing functions can dispatch to symbolic implementations when available.

Direct translation to Rust should not be the first implementation strategy because notebook cells need fast feedback, Poincare-native diagnostics, sandboxing, and dynamic graph/table/plot values. Longer-term optimization should happen through Poincare-owned IR, bytecode, expression kernels, or specialized numeric backends rather than Rust source generation.

## Current Mathematical Infrastructure

Poincare already has enough infrastructure for a numeric and graphing-focused notebook.

Available now:
- Numeric expression parser/evaluator for plotting formulas.
- Scalar, vector, parametric curve, and parametric surface expression wrappers.
- `GraphSpec` / `PlotSpec` graph model and graph compilation.
- Table import, preview, mapping, and validation.
- Finite-difference gradient, divergence, and curl.
- Curve derivatives and integrals from sampled geometry.
- Curve fitting, smoothing, residual plots, and diagnostics.
- Point statistics and data-quality analysis.
- Surface area, curvature, normals, mesh-quality-style analysis, and curve-on-surface measurement.
- Surface-surface intersections.
- Analysis outputs that already include derived plots, reports, diagnostics, and tables.

Not available yet:
- Symbolic simplification.
- Exact arithmetic expression system.
- Symbolic solve/reduce.
- Symbolic integration.
- Assumptions.
- Symbolic equation/inequality regions.
- Dependency-aware notebook kernel state.
- A notebook language/runtime.

The first evaluator should therefore be honest about what it can do: graph creation, numeric expression evaluation, table operations, and existing Poincare analysis. Symbolic behavior should be added behind the evaluator boundary once the expression representation and backend strategy are settled.

## Undo / Redo Direction

Notebook undo/redo should follow the existing Poincare app pattern where practical.

Initial direction:
- Source edits, block insertion/deletion/reordering, captions, graph block settings, and attachment operations are normal undoable document edits.
- Cell execution is not treated as ordinary text editing.
- Output replacement, stale marking, preview refresh, and evaluator state updates should be managed as execution state, with explicit clear/recompute commands.
- Interactive graph edits made inside a graph block become undoable document changes when the graph block commits updated graph/view state on deactivation.

This keeps undo/redo focused on user-authored document state while execution remains reproducible through cell source and run commands.

## Phase 1: Shared Notebook Document Model

Goal: define a persistent document model that can represent report blocks today and executable symbolic notebook cells later.

Status: complete

Deliverables:
- A library-owned or shared notebook document schema with stable concepts:
  - notebook document
  - ordered blocks / cells
  - text blocks
  - graph blocks
  - table blocks
  - input cells
  - output cells
  - diagnostic cells
  - metadata
- Stable cell identity so outputs, graph blocks, and dependency references can survive editing.
- Versioned persistence for notebook files.
- Separation between:
  - source content
  - evaluated output
  - app UI state
  - graph viewport/camera state
  - kernel/session state
- Initial provenance model for outputs:
  - source cell id
  - evaluation timestamp or revision
  - input hash
  - graph/data dependencies
  - diagnostics
- Migration strategy for evolving notebook file formats.
- Initial `poincare-notebook-lib` crate with no egui dependency.
- Initial `NotebookDocument`, `NotebookBlock`, `NotebookCellId`, `NotebookOutput`, and `NotebookDiagnostic` types.
- Initial `NotebookBundleManifest`, `NotebookAttachment`, `AttachmentId`, and graph-block view-state types.
- Initial attachment reference types usable by evaluator/runtime host services.
- Feature-gated serialization tests and sample fixture documents.

Notes:
- This phase should not require a working executable notebook UI.
- The important outcome is that a report block, a computed table, and a graph produced by code can all fit the same document model later.
- Avoid baking Markdown-only assumptions into the core model. Markdown can be the first text representation, but the document model should allow richer math text and structured cells later.
- Use `serde` immediately.
- The document model should be independent of a specific archive implementation, but the file format should be bundle-native from the start.

Implemented:
- Added `poincare-notebook-lib` as a workspace crate with no egui/wgpu/platform dependencies.
- Added serializable notebook document, block/cell id, cell, output, diagnostic, graph-block, graph-view-state, attachment, bundle-manifest, and asset-reference types.
- Added graph block ownership modes for snapshot, linked, and computed graphs.
- Added graph view/presentation state sufficient for preview/live viewport round-tripping.
- Added attachment reference and bundle path types for later runtime host services.
- Added JSON round-trip tests for documents, graph blocks, and bundle manifests.

## Phase 2: Report-Mode Notebook Surface

Goal: ship a useful notebook-lite experience using the durable document model without requiring a computational kernel.

Deliverables:
- Initial `poincare-notebook-app` crate.
- A notebook editor surface with ordered blocks.
- Markdown or rich text blocks.
- Static table blocks for analysis outputs.
- Graph blocks inserted from:
  - current Poincare graph
  - selected plot
  - analysis output
  - saved project or graph spec
- Basic block operations:
  - insert
  - delete
  - duplicate
  - reorder
  - collapse / expand
  - title / caption
- Notebook save/load.
- Export to a shareable report format, initially HTML or a bundled project/report directory.
- Attachment insertion and attachment-backed Markdown references for basic images/data files.

Notes:
- This is the first user-facing milestone.
- It should feel like a notebook, but it is not yet a computational notebook.
- The model should already distinguish source blocks from output blocks so computed cells can be added without a rewrite.
- Use `egui_commonmark` or a small `pulldown-cmark`-backed renderer for the first Markdown view.
- Use `comrak` if report export needs CommonMark/GFM AST transforms or Markdown-to-HTML output.

## Phase 3: Embedded Graph Block Lifecycle

Goal: make embedded Poincare graphs reliable notebook objects rather than screenshots pasted into a document.

Deliverables:
- Graph blocks backed by `GraphSpec`, project data, or a narrow serializable graph snapshot.
- Per-block camera/view state.
- Static preview image stored in the notebook bundle.
- Per-block graph style and presentation metadata:
  - scalarbars
  - labels
  - title/caption
  - legend settings
  - export size
- Interactive graph blocks in the notebook surface.
- Static preview mode for inactive graph blocks.
- One-active-viewport policy for interactive graph editing.
- Refresh and detach workflows:
  - refresh from source project/spec
  - freeze as snapshot
  - open in full Poincare editor
  - replace embedded graph from current editor state
- Commit-on-deactivate behavior that saves the final graph/view state and captures a matching headless preview.
- Graph-block provenance linking graphs back to source cells, imported data, or analysis outputs when available.

Notes:
- This phase is where the normal Poincare app and notebook surface need a clean integration boundary.
- Embedded graphs should not depend on hidden global app state.
- Heavy rendering costs should be planned for early. A notebook with many interactive 3D viewports can become expensive quickly.
- `poincare-notebook-lib` should own graph-block data and provenance.
- `poincare-notebook-app` should own viewport widgets, preview cache, and GPU/resource scheduling.
- Reuse the existing headless rendering/export infrastructure from `poincare-lib` / `viewport-lib` for previews where possible.

## Phase 4: Evaluator API Crate and Typed Value Boundary

Goal: introduce `poincare-evaluator` as the stable boundary between notebook cells and whatever execution backend is used now or later.

Status: complete

Deliverables:
- New `poincare-evaluator` crate.
- Stable evaluator trait.
- `EvalRequest`, `EvalResponse`, `EvalStatus`, `EvalDiagnostic`, and `SourceSpan` types.
- `EvalValue` and `EvalOutput` types that can represent Poincare graph, table, expression, analysis, diagnostic, text, and image outputs.
- Minimal `MathExpr` representation sufficient for parsed expressions and future symbolic growth.
- `EvalContext` and `EvalContextDelta` for cell-to-cell state without exposing backend internals.
- Evaluator metadata:
  - language id
  - display name
  - supported features
  - safety/isolation level
- Optional evaluator factory/registry API.
- Tests showing a fake evaluator can return graph/table/text/diagnostic outputs without `poincare-notebook-lib` or `poincare-notebook-app` depending on a specific backend.
- Initial integration point for `poincare-evaluator-poincare`, but without requiring the native language to be complete.
- Attachment/host-service hooks sufficient for evaluator backends to resolve bundled data through notebook APIs.

Notes:
- This phase should happen before executable notebook UI work becomes deep.
- The evaluator API should depend on `poincare-lib` but not egui/eframe/wgpu.
- Rhai, Python, Rust, Symbolica, or custom symbolic engines must remain implementation details behind this API.
- This is the main architectural hedge that lets early notebook work move forward without locking the product into the wrong language/runtime.
- Implemented in `crates/poincare-evaluator` with backend-neutral evaluator/factory traits, serializable request/response/context/session types, typed values, diagnostics, source spans, attachment host hooks, and fake-evaluator tests covering text, table, attachment, and diagnostic output paths.

## Phase 5: Execution-Ready Cell and Output Model

Goal: add the notebook semantics needed for executable cells before committing to a full mathematical kernel.

Deliverables:
- Input cell model with:
  - source text
  - language / evaluator id
  - execution status
  - execution count
  - output references
  - diagnostics
- Output model with typed outputs:
  - plain text
  - formatted math placeholder
  - table
  - graph spec
  - image
  - diagnostic
  - structured data
- Execution state transitions:
  - idle
  - queued
  - running
  - complete
  - failed
  - stale
- Cell evaluation commands:
  - run cell
  - run selected
  - run all above
  - run all
  - clear output
  - restart evaluator
- Output persistence policy:
  - save outputs
  - clear outputs on save
  - mark outputs stale after source edits
- Initial execution-order staleness model:
  - editing a cell marks that cell and later executable cells stale
  - outputs remain visible but stale
  - run-all recomputes in document order
  - precise dependency tracking is deferred
- Initial evaluator abstraction that can be implemented by a simple expression evaluator first and a symbolic kernel later.
- Optional proof-of-concept evaluator behind a feature flag.

Notes:
- This phase can use a minimal evaluator or mock evaluator, but the public cell/output/evaluator contract should be real.
- The typed output model is what lets later symbolic results, tables, and Poincare graphs share a consistent notebook surface.
- Rhai is the most plausible first embedded evaluator because it is a small Rust-native scripting language designed for embedding and Rust function registration.
- Do not expose Rhai types directly in the notebook document model. Treat it as one evaluator implementation behind a Poincare-owned trait.
- The notebook model should reference `poincare-evaluator` outputs, not backend outputs.

## Phase 6: Computational Kernel Integration

Goal: introduce a real executable runtime that can evaluate cells and produce typed notebook outputs, including Poincare graphs.

Deliverables:
- Kernel process or in-process evaluator architecture.
- Kernel lifecycle:
  - start
  - interrupt
  - restart
  - shutdown
  - recover after crash
- Runtime environment state:
  - variables
  - functions
  - imported data
  - graph objects
  - cell definitions
- Typed bridge from evaluated values to notebook outputs:
  - scalar values
  - arrays
  - tables
  - symbolic expressions
  - graph specs
  - diagnostics
- Host bridge for attachments and other notebook-scoped resources.
- Ability for code cells to create and modify `GraphSpec`s.
- Basic package/module loading story for reusable notebook code.
- Security policy for executing notebooks from untrusted sources.

Notes:
- This is the phase where the notebook becomes a computational notebook rather than a report.
- Kernel architecture should be explicit early because it affects persistence, cancellation, dependency tracking, and security.
- A minimal first kernel can be much smaller than Mathematica, but it should speak the same typed-output protocol expected by the notebook model.
- `jupyter-protocol` is useful as a reference or compatibility layer because it already models Jupyter messages and rich media outputs.
- Evcxr is useful as an architecture reference for Rust notebook execution, but Poincare should not assume a Rust REPL is the notebook's primary kernel.
- Process isolation should be evaluated here even if early execution is in-process.
- Full dependency tracking is not required in this phase; execution order remains the baseline until Phase 9.

## Phase 7: Mathematical Language and Symbolic Layer

Goal: evolve from executable cells into a mathematical system capable of symbolic workflows.

Deliverables:
- A mathematical expression language or integration with an existing symbolic engine.
- Symbolic expression representation with:
  - variables
  - functions
  - exact numbers
  - approximate numbers
  - arrays / matrices
  - equations and inequalities
  - assumptions
- Core symbolic operations:
  - simplify
  - expand / factor
  - differentiate
  - integrate where feasible
  - solve / reduce where feasible
  - substitute
  - series / limits where feasible
- Numeric interop:
  - evaluate symbolic expression over domains
  - compile symbolic expressions into Poincare plot expressions
  - track exact vs approximate evaluation
- Region and domain semantics that can feed plotting and adaptive sampling.
- Interpreter-first Poincare language runtime based on `poincare-lang`.
- Longer-term optimization path:
  - tree-walking interpreter
  - bytecode VM if needed
  - optimized numeric expression kernels for plotting/sampling
  - optional JIT/AOT only if performance requires it

Notes:
- This is the start of the Mathematica-style target and likely the largest technical risk.
- Choosing whether to build, embed, or interoperate with a symbolic engine is a product-defining decision.
- Poincare does not need full Mathematica parity to benefit from symbolic workflows, but the representation must be strong enough not to trap the project in string manipulation.
- Symbolica is the strongest Rust-native CAS candidate found in this research. It supports expression manipulation, differentiation, pattern matching, polynomial arithmetic, exact/numeric computation, solving modules, and code generation.
- Symbolica is source-available but not a normal permissive open-source dependency. Redistribution and organizational use can require licensing, so it should be treated as optional or strategic until the licensing/product implications are settled.
- If Symbolica is not acceptable as a required dependency, Poincare likely needs a smaller internal symbolic AST plus selected operations first, with optional bridges to external CAS backends later.
- The symbolic layer should implement or consume the `MathExpr` / `EvalValue` boundary from `poincare-evaluator`, not replace it.
- See `docs/plans/poincare-language-roadmap.md` for the separate language/compiler/interpreter plan.

## Phase 8: Formatted Math Input/Output

Goal: make mathematical notebooks readable and writable as mathematical documents, not only as code logs.

Deliverables:
- Pretty-printed symbolic output.
- Rich math text in notebook cells.
- Input conveniences for common mathematical notation.
- Copy/paste behavior for:
  - plain text
  - source syntax
  - formatted math
- Optional rendered equation blocks.
- Error spans and diagnostics tied to source expressions.
- Formatting rules for matrices, piecewise functions, equations, assumptions, and graph definitions.

Notes:
- This should follow the expression model. Pretty output without a strong internal representation will become fragile.
- A code-like first input mode is acceptable for earlier phases, but long-term symbolic notebooks need readable math output.
- Typst is a strong Rust-native candidate for high-quality math/document rendering and export because its compiler can parse, evaluate, lay out, and export to PDF, PNG, SVG, and HTML.
- KaTeX is a simpler candidate for LaTeX-to-HTML math export. For egui display, it still needs an HTML/SVG/raster rendering path.
- `resvg` can help turn SVG math or document fragments into raster previews for egui.

## Phase 9: Reactive Dependencies and Reproducibility

Goal: make notebooks reliable when cells depend on earlier definitions, imported data, computed outputs, or embedded graphs.

Deliverables:
- Dependency tracking between cells where feasible.
- Stale-output marking when dependencies change.
- Recompute graph for affected cells.
- Explicit execution order display.
- Deterministic run-all behavior.
- Runtime environment snapshot or reproducibility metadata.
- Data/file dependency tracking:
  - imported CSV files
  - included project files
  - external assets
  - package versions
- Notebook diagnostics for hidden state, missing files, stale outputs, and failed evaluations.

Notes:
- A notebook with hidden state but no dependency feedback is difficult to trust.
- This phase should build on the provenance work started in Phase 1 rather than introducing a parallel dependency model.

## Phase 10: Notebook Security and Trust Model

Goal: revisit execution security before release-quality executable notebooks are shared outside trusted local workflows.

Deliverables:
- Trust model for notebooks opened from disk or downloaded from elsewhere.
- Clear disabled-by-default behavior for untrusted executable notebooks.
- Trust prompts before running executable cells from untrusted bundles.
- Attachment access policy.
- File/network access policy for evaluator backends.
- Backend capability declarations:
  - pure computation
  - filesystem read
  - filesystem write
  - network
  - process execution
  - native code
- UI indicators for trusted/untrusted notebook state.
- Safe mode for opening notebooks without running code.
- Security review of registered evaluator functions and attachment resolution.

Notes:
- This phase is intentionally later because the exact evaluator surface needs to exist before it can be secured rigorously.
- It must happen before executable notebooks are positioned as a shareable commercial product feature.
- Even embedded evaluators such as Rhai can become unsafe if registered host functions expose filesystem, network, or process capabilities.

## Phase 11: Notebook Export, Sharing, and Packaging

Goal: let notebooks move between editing, presentation, publication, and archival workflows.

Deliverables:
- Export to HTML report with static graph previews and embedded data where practical.
- Export to PDF through a stable HTML/print pipeline or dedicated renderer.
- Productionized bundle format for notebooks with embedded graph specs, data files, assets, attachments, and previews.
- Options for including or excluding computed outputs.
- Reproducibility manifest:
  - app version
  - graph/spec version
  - kernel/runtime version
  - dependencies
  - source data references
- Presentation mode for notebooks.

Notes:
- Export should not wait until the symbolic notebook is complete. Report-mode export should appear early and then grow with the notebook.
- Sharing untrusted executable notebooks requires clear trust and execution controls.

## Architecture Principles

- Treat `GraphSpec` and analysis outputs as typed notebook values.
- Treat `poincare-evaluator` as the stable execution boundary.
- Keep app UI state separate from notebook source and evaluated output.
- Make graph blocks reusable in both report-mode and executable notebooks.
- Store graph view state strongly enough that static preview and interactive viewport round-trip to the same final frame.
- Treat attachments as first-class bundled notebook resources.
- Design the evaluator boundary before choosing the final symbolic engine.
- Prefer typed outputs over string-rendered outputs.
- Preserve provenance for graphs, tables, diagnostics, and imported data.
- Keep useful intermediate products shippable, but do not let them define dead-end data models.

## Recommended Order

1. Define the notebook document model before building the UI.
2. Ship report-mode notebooks using the same model.
3. Harden embedded graph blocks as real graph objects, not screenshots.
4. Add `poincare-evaluator` before committing to a concrete language backend.
5. Add executable cell/output semantics over the evaluator API.
6. Introduce a real computational kernel.
7. Add symbolic expression representation and operations.
8. Add formatted math input/output once symbolic values are structured.
9. Add dependency tracking and reproducibility tooling.
10. Revisit security and trust before release-quality executable notebook sharing.
11. Expand export and sharing continuously, starting with report-mode HTML.
