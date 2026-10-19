# Task 07 — App, explorer and diff view (row mode)

## Goal

Build the real application: `App` state, the action pipeline, the explorer overlay, the two-column diff view in **row mode**, the status line, and the modal stack with the large-file and quit modals. At the end of this task Reviewy is usable for reading diffs (no comments yet).

Read first: `doc/spec.md` §3 (workflow), §4 (event loop), §10 (UI: screens, explorer, diff view, status line, modals), §11 (mode hierarchy, row mode). Read handoff notes in **all** previous task files (01–06); you build on their interfaces.

## Context

Modules available: `config`, `git` (changed files, content, limits), `diff` (`DifftEngine`, `align::build` → `FileDiff`), `highlight` (`Theme`, `apply`), `vim` (`KeyParser` → `Command`, motions, wrap), `comments` (model only; this task only holds an empty `CommentSet`). The event loop in `main.rs` exists from task 01 with a stub `App`; replace the stub.

## Deliverables

### `app/state.rs`

- `App` owns everything: config, theme, repo root, base commit, `Vec<ChangedFile>`, per-file cache `HashMap<PathBuf, FileView>` (a `FileDiff` plus per-file view state: active side, cursor row, scroll), current file index, `Mode` (`Explorer`, `Row`; tasks 08/09 add `Cursor`, `Visual`, `VisualLine`, `Editor`), modal stack `Vec<Modal>`, status message, `dirty`, `should_quit`, `KeyParser`.
- Leave a `comments: CommentSet` field in place (empty, base from the base commit) so task 09 wires it in without restructuring.

### `app/actions.rs`

- `Action` enum and `App::apply(Action)`. Map `vim::Command` → `Action` here, depending on the mode and the top modal (the top modal receives input first). Bracketed paste events are ignored in this task.
- **Open file** pipeline: `git::read_content` → `git::check_limits` → if over limits push the large-file modal (Enter continues, Esc cancels) → set the status "Diffing…" and **force a redraw before** calling `DifftEngine::diff` (synchronous call, ~1 s on big files) → `align::build` → `highlight::apply` → cache. Unviewable files (`!viewable()`) show a status message instead (§10 messages: "Binary file", "Conflicted file: not viewable in v1", "Submodule: not viewable in v1", "Mode change only: not viewable in v1"). Per-file errors show a status message; the app continues.

### Row mode (§11)

- `j`/`k` (counts), `gg`/`G`, `Ctrl-d`/`Ctrl-u` move between rows, **skipping rows where the active side has no line**.
- `h`/`l` switch the active column and jump to the nearest row with a line on that side.
- `g]`/`g[` next/previous hunk start; `gn`/`gp` next/previous **viewable** file in explorer order (no wraparound: at the first/last file, show a status message).
- Enter → in this task, a no-op status message ("Comments: task 08/09") — tasks 08/09 replace it. `D` likewise a no-op.
- Esc → explorer overlay. `Ctrl-q` → quit modal (Enter quits, Esc cancels). `// task 10: save` at the quit point.
- Scrolling: keep the cursor row visible with a small margin (3 rows).

### `ui/explorer.rs`

Centered overlay (~60% width, up to 80% height) over the diff view, per §10: status color (added/untracked → `added`, modified/renamed → `modified`, deleted → `deleted`, conflicted → `conflict`), path (`old → new` for renames), marker (`B` binary, `C` conflicted, `S` submodule, `P` mode-only), comment count `[n]` when > 0 (always 0 in this task). `j`/`k`/counts/`gg`/`G` move; Enter opens; Esc closes (no-op if no file is open). The app starts in this mode over two blank columns.

### `ui/diff_view.rs`

- Two equal columns, old left, new right. Per-column gutter: right-aligned line number, change marker (`+` added, `-` removed, `~` changed), and a one-char comment-marker slot (blank in this task).
- Backgrounds: changed/added/removed lines use `line_added_bg` (rhs) / `line_deleted_bg` (lhs); changed-token spans use `token_*_bg`; syntax colors from `highlights`. The active column's gutter is brighter (use `foreground` vs `line_number`); the cursor row gets `active_line_background` on the active column.
- Padding rows (side has no line) render as empty background.
- **Wrap on** (`ui.wrap`): each row's height is the max of both sides' `vim::wrap` heights at their column widths; the shorter side pads. Continuation segments get a blank gutter. **Wrap off**: lines are clipped; horizontal scroll stays at 0 in row mode (cursor mode, task 08, scrolls it).
- Rendering cost scales with visible rows only. Precompute row heights per width and cache until resize or wrap toggle.

### `ui/modals.rs`, `ui/status.rs`

- Generic centered confirm modal (title, message, "Enter: … / Esc: …" hint). Used now for large file ("Large file (N lines, X KB, longest line L). View anyway?") and quit ("Save and quit?").
- Status line: mode, file path, `line:col` of the active side (1-based), pending keys from `KeyParser::pending()`, transient message (cleared on the next key press).

## Tests

UI snapshots with ratatui `TestBackend` (80×24) and `insta`, built from small in-memory `FileDiff`s (no difft needed):
- Explorer with a modified, an untracked, a renamed and a binary file.
- Diff view of a small modified file, wrap on, including one line long enough to wrap.
- Same file, wrap off.
- An added file (empty left column).
- Large-file modal.

One unit test for row-mode `j` skipping padding rows on the active side.

## Acceptance

- Run in a real repo with mixed changes: explorer lists files with correct colors/markers; opening each viewable file shows the correct side-by-side diff with highlighting and tints; `g]`/`g[`/`gn`/`gp`/`h`/`l` behave as specified; resizing redraws correctly; `Ctrl-q` → Enter exits.
- fmt, clippy, tests clean.

## Handoff notes

_To be filled in by the implementing agent._
