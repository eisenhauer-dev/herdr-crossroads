# Linked workspaces and filtering

Crossroads displays panes from the Herdr snapshot, excluding its own pane when
that pane ID is supplied. It orders workspace and tab blocks by their numbers
and preserves snapshot pane order within each tab.

## Worktree context

Best-effort `worktree list --workspace <id>` lookups annotate open linked
workspaces as `↳ label (branch)`, or `↳ label` when no nonempty branch is supplied.
Linked workspace blocks follow their source workspace when it is present in the
rows; tab and pane order within those blocks is unchanged. Failed or unusable
worktree lookups do not prevent the pane table from opening.

The arrow marks a linked workspace, not a separate selectable parent row. The
table shows workspace and tab context on the first displayed row of each block,
including when filtering hides earlier rows. Matching characters in displayed
fields are underlined; narrow columns can clip labels and branch text.

## Filtering and selection

- An empty query keeps the complete annotated row order.
- Matching is case-insensitive subsequence matching within a single workspace,
  tab, pane, or agent field. A query cannot span separate fields.
- Workspace matching uses the annotated display label, including its arrow and
  branch text. There is no separate raw branch-metadata search field. For example,
  `feature/demo` and `FTR/DM` both match `↳ Same (feature/demo)`.
- Filtering only removes rows; it does not rank or reorder matches. If only a
  linked workspace matches, its arrow and branch annotation remain visible, but
  no source-workspace placeholder is inserted.
- Selection remembers a pane ID. When that pane is hidden, rendering and focus
  confirmation use the first visible pane without replacing the remembered ID.
  Clearing the query restores that pane as the candidate if it still exists.
   Navigation instead updates the remembered ID to a visible pane.
- Pane, tab and workspace movement wraps in both directions through only visible
  rows/blocks, preserving their annotated topology order. Tab/workspace jumps
  land on the first visible pane of the adjacent block, wrapping past hidden
  source or linked workspace blocks without inserting placeholders. Navigation
  does not focus a pane; a single visible pane stays selected, and a single block
  remains the target with jumps selecting its first visible pane.
- No matches displays `No matching panes`, with no selected row or focus
  candidate. Confirming in Select does not close or focus a pane in that state.

See [keymaps](keymaps.md) for search and navigation controls, and
[architecture](architecture.md) for snapshot and reconnect behavior. These rules
are implemented in [topology](../src/topology.rs),
[selection](../src/selection.rs), [search](../src/search.rs), and the
[table renderer](../src/presentation/table.rs), with characterization coverage
in [topology tests](../tests/unit/topology.rs).
