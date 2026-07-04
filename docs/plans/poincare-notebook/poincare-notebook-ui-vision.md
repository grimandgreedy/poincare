# Poincare Notebook UI Vision

## Vision

Poincare Notebook is a Mathematica-style computational notebook for mathematical visualization.

The core experience is a vertical document of editable cells. A user writes text, mathematical definitions, computations, and plotting commands; running a cell produces visible outputs directly below it. Outputs can include text, diagnostics, tables, analysis reports, and embedded Poincare graphs.

The notebook should feel like a working mathematical document, not a chat log, not a dashboard, and not a graphing app with comments attached.

## Product Identity

Poincare Notebook is separate from `poincare-app`.

- `poincare-app` remains the focused 3D graphing application.
- `poincare-notebook-app` is the computational document environment.
- Graphs in notebooks are embedded Poincare graph objects backed by `poincare-lib`.
- Users can open embedded graphs in `poincare-app` for full graph editing when needed.

The notebook app should make graphing feel native inside a document without absorbing every graph-editor workflow into the notebook surface.

## Core Interaction Model

The document is made of ordered cells.

Cells can be:
- text / Markdown cells
- executable Poincare-language cells
- output groups produced by execution

Executable cells have source input and outputs. Outputs appear directly below the input that produced them.

Running a cell:
- evaluates the cell in the current notebook session
- may update variables/functions in session state
- replaces or updates that cell's outputs
- may mark later cells stale when source changes affect execution order

Running later cells can use variables and functions defined by earlier executed cells.

## What "Mathematica-Style" Means

For Poincare, Mathematica-style means:
- a vertical notebook document
- visible cell grouping
- editable input cells
- outputs directly below inputs
- persistent evaluator/session state across cells
- repeated run/edit/run workflow
- keyboard-friendly execution
- mathematical notation and graph outputs as first-class document content
- side-panel visibility into current definitions and variables

It does not mean cloning Mathematica's full UI or symbolic feature set in the first version.

## Layout

Default app layout:

```text
┌──────────────────────────────────────────────────────────────┐
│ Menu / Toolbar                                                │
├───────────────────────────────────────┬──────────────────────┤
│ Notebook Document                     │ Variables / Session  │
│                                       │                      │
│  Text cell                            │  a      Number  3    │
│                                       │  f      Function     │
│  Input cell                           │  data   Table        │
│    f(x,y) = sin(x*y)                  │  g      Graph        │
│                                       │                      │
│  Output group                         │                      │
│    printed text                       │                      │
│    graph preview                      │                      │
│                                       │                      │
│  Next input cell                      │                      │
│                                       │                      │
└───────────────────────────────────────┴──────────────────────┘
```

The notebook document is the primary surface. The variables panel is supporting context, not the main workflow.

## Cell Design

Cells should be visually distinct but quiet.

Each executable cell should show:
- input/source area
- execution status
- stale/fresh state
- optional run count or revision
- output group below the source

Cell chrome should support:
- select cell
- run cell
- insert cell above/below
- change cell type
- collapse/expand input or output
- delete/move cell

The UI should avoid decorative card-heavy layouts. Cells are document blocks, not independent dashboard cards.

## Text Cells

Text cells are for explanation, headings, notes, and report prose.

Initial behavior:
- Markdown source
- rendered display mode
- edit on click or command
- support for links, code spans, lists, and headings

Later behavior:
- richer math rendering
- equation blocks
- better export formatting

## Executable Cells

Executable cells contain Poincare-language source.

Initial expectations:
- code-like editor
- syntax highlighting eventually
- diagnostics tied to source spans
- run command visible or keyboard accessible
- output appears below

Example:

```text
f(x, y) = sin(x^2 + y^2) / (x^2 + y^2)

plot surface {
  z = f(x, y)
  x = -6..6
  y = -6..6
  resolution = [160, 160]
}
```

## Outputs

Outputs are grouped under the cell that produced them.

Output types:
- printed text
- final value preview
- diagnostics/errors
- tables
- graph previews
- images
- analysis reports

Output behavior:
- preserve order produced during evaluation
- show stale state if the source/session changed
- allow collapse/expand
- allow clear output
- limit very large outputs with expansion affordances

Printed output should feel like an output stream. Graphs and tables should feel like inspectable document objects.

## Graph Outputs

Graph outputs are embedded Poincare graphs.

Default state:
- static preview image
- graph ownership/provenance indicator where useful
- basic controls on hover or selection

Interactive state:
- clicking/focusing activates the graph
- the preview is replaced with a live `viewport-lib` viewport
- only one graph output is interactive at a time
- deactivating saves the final graph/view state
- deactivating captures a new matching preview

Graph controls:
- activate/edit inline
- reset view
- open in `poincare-app`
- refresh preview
- freeze snapshot
- refresh linked source
- export image
- collapse output

The user should be able to reopen a notebook, activate a graph, and see the same camera/view state they last saved.

## Variables / Session Panel

The side panel shows the current evaluator session.

It answers:
- What variables are currently defined?
- What functions exist?
- What type of value is each name?
- Which cell produced it?
- Is it stale or disconnected from current session state?

Each row should show:
- variable name
- value kind
- short preview
- source cell link where known
- update/run metadata where useful

Actions:
- inspect
- jump to source
- insert reference into focused cell
- delete variable from session where supported
- copy preview

The panel reflects live session state. After restart, it may be empty until cells are run again.

## Execution Controls

Core commands:
- run current cell
- run current cell and advance
- run selected cells
- run all
- run all above
- run all below
- restart evaluator
- restart and run all
- interrupt execution
- clear current output
- clear all outputs

The UI should distinguish:
- running in current session
- restarting and reconstructing session from top to bottom
- stale outputs that may not match current source

## Stale State Presentation

Stale state should be visible but not alarming.

Examples:
- cell source changed since output was produced
- earlier cell changed
- evaluator restarted
- attachment changed
- computed graph manually edited

Stale outputs should remain visible because they may still be useful for reading the document. The UI should make it clear they are not guaranteed to match current source/session state.

## First-Run Document

The first-run document should show the actual notebook workflow, not a marketing page.

Suggested starter cells:
- short text cell explaining that cells run top-to-bottom and can define variables
- executable cell defining a function
- executable cell plotting that function
- text cell pointing to the variables panel

The first graph should appear as an embedded output preview.

## Non-Goals For First UI Milestone

Not required initially:
- full Mathematica-style symbolic formatting
- multi-pane notebook layouts
- collaborative editing
- full graph inspector parity inside notebook
- multiple simultaneous live graph viewports
- PDF export
- precise dependency graph UI
- advanced cell styling
- custom themes

## Design Principles

- The notebook document is the primary object.
- Outputs belong to cells.
- Graphs are real Poincare graph objects, not screenshots.
- Only one graph is live at a time.
- Session state should be visible and understandable.
- Source, output, and live runtime state should remain conceptually distinct.
- The UI should prefer repeated mathematical work over presentation-heavy decoration.
- Early versions should be simple, but they should not teach interactions that will break later.
