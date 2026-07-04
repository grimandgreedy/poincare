# Poincare Notebook Runtime Roadmap

## Goal

Define notebook execution semantics for `poincare-notebook-lib`, `poincare-evaluator`, and the Poincare language evaluator: shared session state across cells, ordered execution, outputs, variables, staleness, cancellation, and resource limits.

The runtime should support a Mathematica/Jupyter-like model where running a cell can define variables and functions that later cells can use. It should be honest about statefulness while still giving users clear stale-output and reproducibility feedback.

## Progress

| Phase | Description | Status | Effort | Priority |
| --- | --- | --- | --- | --- |
| 1 | Runtime state and evaluator session model | Complete | Large | High |
| 2 | Ordered cell execution and staleness | Complete | Medium | High |
| 3 | Output stream and typed output semantics | Complete | Medium | High |
| 4 | Variables/session inspection API | Complete | Medium | High |
| 5 | Error handling and partial execution | Complete | Medium | High |
| 6 | Cancellation and resource limits | Complete | Large | High |
| 7 | Attachment and data resolution | Planned | Medium | Medium |
| 8 | Kernel restart and reproducibility metadata | Planned | Medium | Medium |
| 9 | Future dependency tracking | Planned | Large | Low |

## Scope Notes

- V1 runtime is stateful and ordered.
- Running a cell can update variables/functions in the evaluator session.
- Later cells can use definitions created by earlier executed cells.
- If a notebook is reopened or a kernel is restarted, state must be reconstructed by running cells again.
- V1 staleness is execution-order based, not full dependency analysis.
- The runtime should expose variables for the UI side panel.
- Runtime internals should not depend on egui.
- V1 runtime I/O should be mediated through notebook attachments, not arbitrary filesystem access.

## Runtime Model

Core concepts:
- notebook document
- evaluator session
- executable cell
- run count / revision
- runtime environment
- variable/function bindings
- typed outputs
- diagnostics
- stale output metadata

The evaluator session owns current live state:
- variables
- functions
- cached runtime values
- run count
- evaluator diagnostics
- optional backend-specific state hidden behind evaluator APIs
- access to host services such as attachment resolution through explicit runtime/evaluator interfaces

The notebook document owns persisted source and outputs:
- cell source
- cell language id
- saved outputs
- output provenance
- stale markers
- graph previews and view state
- attachments

The runtime should keep these separate. Saved outputs are not the same thing as live evaluator state.

## Runtime Host Services

The evaluator should access notebook-owned services through an explicit host interface.

Initial host services:
- resolve attachment by id or display name
- read attachment bytes
- read attachment text
- parse attachment as table/CSV through Poincare table ingestion
- report attachment metadata and hashes where available

The evaluator should not directly own bundle storage or arbitrary filesystem access.

Sketch:

```rust
pub trait RuntimeHost {
    fn resolve_attachment(&self, name_or_id: &str) -> Result<AttachmentHandle, RuntimeError>;
    fn attachment_bytes(&self, handle: &AttachmentHandle) -> Result<Vec<u8>, RuntimeError>;
    fn attachment_text(&self, handle: &AttachmentHandle) -> Result<String, RuntimeError>;
}
```

The exact trait may live in `poincare-evaluator`, `poincare-notebook-lib`, or an adapter layer, but the dependency direction must remain UI-independent.

## Shared State Across Cells

Expected behavior:
- Running cell 1 can define `a = 3`.
- Running cell 2 can use `a`.
- Running cell 9 can use state produced by cells 1 through 8 only if those cells have been run in the current evaluator session.
- Running all cells reconstructs session state from top to bottom.
- Restarting the evaluator clears live state; saved outputs remain visible but may be marked stale or session-disconnected.

Example:

```text
# Cell 1
a = 3

# Cell 2
print(a + 2)
```

Cell 2 succeeds only if the current session has a binding for `a`.

## Execution Order and Staleness

V1 behavior:
- Cells execute in document order when using run-all.
- Running a single cell mutates the current evaluator session.
- Editing an executable cell marks that cell and all later executable cells stale.
- Running an edited cell clears stale status for that cell but does not automatically prove later cells fresh.
- Running all clears order-based staleness if all cells complete successfully.
- Outputs remain visible when stale.

Stale metadata should record reasons such as:
- source edited after output was produced
- earlier executable cell edited after output was produced
- evaluator restarted after output was produced
- attachment changed after output was produced
- graph output manually edited after computation

Deferred:
- exact variable dependency tracking
- recompute only affected cells
- static dependency graph
- hidden-state hazard detection beyond coarse warnings

## Cell Execution Commands

Required commands:
- run current cell
- run current cell and advance
- run selected cells
- run all
- run all above current cell
- run all below current cell
- restart evaluator
- restart and run all
- interrupt current evaluation
- clear current output
- clear all outputs

Command behavior should be explicit:
- `run current cell` uses the current live session state.
- `run all` starts from a clean or explicitly chosen session policy.
- `restart and run all` always clears live state first.
- `run all above` is the safe way to prepare state for the current cell.

Open decision:
- Whether normal `run all` should implicitly restart first. The most reproducible behavior is restart-and-run-all, but users may expect run-all to continue the current session. The UI can expose both.

## Output Semantics

Output categories:
- printed output stream
- returned/final value output
- emitted graph/table/analysis outputs
- diagnostics
- errors
- warnings
- generated attachments

Recommended V1 behavior:
- `print(...)` appends to a text output stream.
- Graph/table/analysis values emitted by explicit output forms become typed outputs.
- The final expression in a cell may be displayed as a value preview if it is not `Unit`.
- Runtime errors stop the current cell.
- Earlier outputs produced by the same cell run remain visible unless the runtime replaces the whole output list on failure.

Open decisions:
- Whether every final expression is displayed automatically.
- Whether graph-producing statements auto-emit outputs or only return graph values.
- Whether loop-generated graph outputs are grouped into one output or appended as multiple outputs.

Recommended initial rule:
- Replace all outputs for a cell at the start of a successful new run.
- If the cell fails, show outputs produced before failure plus the error diagnostic, clearly marked as failed/partial.

## Loop-Generated Outputs

Loops can produce many outputs. V1 needs limits.

Recommended behavior:
- `print` output from loops appends to the same text stream.
- Explicit graph/table emissions inside loops append multiple outputs in order.
- A later grouping helper can collect plots into one graph:

```text
g = graph()

for a in [1, 2, 3] {
  g = add_plot(g, surface(z = a * sin(x*y), x = -3..3, y = -3..3))
}

emit(g)
```

Output limits:
- maximum printed characters per cell
- maximum output count per cell
- maximum table preview rows
- maximum graph preview size

## Variables and Session Inspection API

The runtime should expose session variables to the notebook UI without exposing backend internals.

Variable metadata:
- name
- value kind
- short preview
- source cell id where known
- last updated run count
- stale/disconnected status where known
- size estimate
- inspectability flag

API shape:

```rust
pub struct SessionSnapshot {
    pub run_count: u64,
    pub variables: Vec<VariableSummary>,
    pub status: SessionStatus,
}

pub struct VariableSummary {
    pub name: String,
    pub kind: ValueKind,
    pub preview: String,
    pub source_cell: Option<NotebookCellId>,
    pub updated_at_run: Option<u64>,
    pub stale: bool,
    pub size_hint: Option<String>,
}
```

The side panel should use summaries, not full value serialization, for normal rendering.

## Error Handling

Error classes:
- parse error
- name resolution error
- type/value error
- runtime error
- cancelled
- resource limit exceeded
- attachment resolution error
- graph build/render error

Expected behavior:
- Parse/name errors prevent execution of that cell.
- Runtime errors stop the current cell.
- Run-all stops at the first failed cell by default.
- UI may later offer "continue after errors."
- Diagnostics should include source spans where available.
- Failed cells should not silently leave partial hidden state unless explicitly documented.

Open decision:
- Whether state mutations before a runtime error are committed or rolled back. The safer model is transactional cell execution: commit state only if the cell completes. This may be harder but avoids confusing partial state.

Recommended V1:
- Use transactional cell execution if practical.
- If not practical, clearly mark partial execution and expose that the session may include partial state.

## Cancellation and Resource Limits

This needs to exist before the language is heavily used, even before the full security model.

Required controls:
- interrupt current evaluation
- loop iteration limit
- wall-clock time limit per cell
- maximum printed output per cell
- maximum output count per cell
- maximum table preview size
- maximum graph count per cell
- cancellation token checked during loops and long builtins

Resource-limit diagnostics should be normal typed diagnostics.

Notes:
- Trusted notebooks can still hang accidentally.
- Infinite loops and huge outputs should not freeze the app.
- Long graph generation should be cancellable where practical.

## Attachments and Data Resolution

The runtime should resolve attachments through notebook APIs rather than arbitrary filesystem access.

V1 behavior:
- `attachment("name-or-id")` resolves a bundled attachment.
- `bytes(attachment(...))` reads raw bytes.
- `text(attachment(...))` reads UTF-8 text.
- `csv(attachment(...))` parses an attachment through table ingestion.
- `csv_matrix(attachment(...))` parses a numeric CSV into an array/matrix-like value where practical.
- Attachment metadata records original path when available, but runtime uses bundled data by default.
- External filesystem refresh is an explicit UI action, not implicit execution behavior.

This supports portability and later security.

Runtime value expectations:
- attachment references are values
- bytes/text are values
- parsed tables are values
- arrays/matrices from tabular data are values
- all can be assigned to variables and shown in the variables side panel with previews

Example:

```text
samples = csv(attachment("samples.csv"))
xs = column(samples, "x")

plot scatter samples {
  x = "x"
  y = "y"
  z = "z"
}
```

## Kernel Restart and Reproducibility

Expected behavior:
- Restart clears live evaluator state.
- Saved outputs remain in the document.
- Variables panel shows empty/restarted state.
- Outputs may be marked disconnected from current session.
- Restart-and-run-all reconstructs state from source cells in order.

Metadata to record:
- evaluator language id/version
- runtime version
- run count
- source hash per output
- attachment hashes used where known
- graph output provenance

## Phase 1: Runtime State and Evaluator Session Model

Goal: define the live runtime/session model separately from persisted notebook state.

Status: complete

Deliverables:
- Evaluator session type.
- Runtime environment model.
- Run count/revision model.
- Variable/function binding model.
- Session snapshot API.
- Runtime host service boundary.
- Clear separation between live state and saved outputs.

Implemented in `poincare-notebook-lib::runtime`, backed by the backend-neutral `poincare-evaluator` API. Runtime/session types live in `crates/poincare-notebook-lib/src/runtime.rs` rather than in `lib.rs`; `lib.rs` only wires and re-exports the module.

## Phase 2: Ordered Cell Execution and Staleness

Goal: implement v1 execution order semantics and coarse stale tracking.

Status: complete

Deliverables:
- Run current cell.
- Run all.
- Restart-and-run-all.
- Edit cell marks later cells stale.
- Stale metadata and visible stale reasons.
- Tests for order-based staleness.

Implemented in `poincare-notebook-lib::runner`. The runner owns document mutation for run-current, run-all, restart-and-run-all, output replacement, execution-state updates, and source-edit stale marking. Run-all stops at the first failed cell in V1.

## Phase 3: Output Stream and Typed Output Semantics

Goal: define how cells produce visible outputs.

Status: complete

Deliverables:
- Print stream output.
- Final value output.
- Graph/table/analysis emitted outputs.
- Output replacement policy.
- Partial/failure output policy.
- Output size limits.

Implemented in `poincare-notebook-lib::outputs`. Evaluator responses are converted into persisted notebook outputs with text, value, table, graph, image, analysis, attachment, and diagnostic cases. Per-cell output count, text size, and table preview limits are represented by `RuntimeOutputLimits`. Failed cells keep evaluator-provided partial outputs plus diagnostics.

## Phase 4: Variables/Session Inspection API

Goal: power the variables side panel.

Status: complete

Deliverables:
- `SessionSnapshot`.
- `VariableSummary`.
- Value preview formatting.
- Source-cell tracking where known.
- Delete variable support where runtime supports it.

Implemented across `poincare-notebook-lib::runtime` and `poincare-notebook-lib::inspection`. `RuntimeSessionSnapshot` and `RuntimeInspectionSnapshot` expose variables/functions without exposing backend runtime values. `poincare-evaluator` now has optional `delete_variable` support with an explicit unsupported default.

## Phase 5: Error Handling and Partial Execution

Goal: make failures understandable and avoid hidden broken state.

Status: complete

Deliverables:
- Error class taxonomy.
- Source-span diagnostics.
- Run-all stop behavior.
- Transactional cell execution decision.
- Partial execution markers if needed.

Implemented in `poincare-notebook-lib::errors` and wired into `poincare-notebook-lib::runner`. Runtime failures are classified into parse, name resolution, type/value, runtime, cancelled, resource-limit, attachment, graph/render, unsupported, and unknown categories. `RuntimeRunReport` now includes `stop_reason`, `RuntimeCellRun` includes `failure` and `partial_execution`, and run-all defaults to stopping at the first failed cell.

V1 transactionality decision: the notebook runtime does not claim transactional execution for opaque evaluator backends. Evaluators may provide transactional behavior internally later, but the shared runtime marks partial output/state risk explicitly through `RuntimePartialExecution` when a failed response produced outputs or reported a context delta. A runtime policy flag exists for future "continue after errors" behavior.

## Phase 6: Cancellation and Resource Limits

Goal: keep notebook execution responsive and bounded.

Status: complete

Deliverables:
- Cancellation token.
- Interrupt command.
- Loop limit.
- Time limit.
- Output size/count limits.
- Runtime diagnostics for limits.

Implemented in `poincare-notebook-lib::resources`, `poincare-notebook-lib::runtime`, and `poincare-notebook-lib::runner`. `NotebookRuntime` now owns a `RuntimeCancellationToken`, exposes `interrupt` / `clear_interrupt`, wraps evaluator hosts so backends can poll `RuntimeHost::should_cancel`, and passes `RuntimeResourceLimits` through `EvalContext`.

The shared runtime now carries loop-iteration, wall-clock, output-count, text-size, table-preview-row, and graph-output limits. Output, text, table, and graph-output limits are enforced by the notebook output mapper with normal diagnostics. Loop and wall-clock enforcement are intentionally evaluator/interpreter responsibilities; `poincare-lang` should read `EvalContext::resource_limits` and poll `RuntimeHost::should_cancel` in loops and long-running builtins.

## Phase 7: Attachment and Data Resolution

Goal: make runtime data access portable and bundle-oriented.

Deliverables:
- Attachment resolver trait/API.
- `attachment(...)` builtin support.
- `bytes(...)` and `text(...)` builtin support.
- CSV attachment parsing path.
- Numeric matrix/array parsing path where practical.
- Runtime values for attachment references and parsed data.
- Variable previews for attachment/table/array values.
- Attachment hash metadata where practical.

## Phase 8: Kernel Restart and Reproducibility Metadata

Goal: make session restart and rerun behavior predictable.

Deliverables:
- Restart evaluator command.
- Restart-and-run-all.
- Output/session disconnection markers.
- Evaluator/runtime version metadata.
- Source hash metadata.

## Phase 9: Future Dependency Tracking

Goal: replace coarse order-based staleness with precise dependency information when the language and evaluator are mature enough.

Deliverables:
- Static variable/function dependency extraction.
- Attachment dependency hashes.
- Graph/output dependency tracking.
- Recompute affected cells.
- Hidden-state hazard diagnostics.

## Recommended Order

1. Define session state and variable model.
2. Implement ordered execution and stale marking.
3. Define output semantics.
4. Add variables/session inspection API.
5. Harden error handling.
6. Add cancellation and limits.
7. Add attachment resolution.
8. Add restart/reproducibility metadata.
9. Defer precise dependency tracking until the language is stable.
