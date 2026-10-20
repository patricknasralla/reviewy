# Task 08 — Cursor and visual modes

## Goal

Add Cursor, Visual (charwise) and VisualLine modes to the diff view: a text cursor on the active side driven by the vim motions, selection rendering, and an `Anchor` (task 06) describing what Enter would comment on. Task 09 consumes that target to open the comment editor.

Read first: `doc/spec.md` §11 (mode hierarchy, cursor mode, visual modes, examples), §10 (diff view, wrapping, status line). Read handoff notes in `doc/tasks/05-vim.md` and `07-app-row-mode.md` (and skim the others).

## Context

Row mode (task 07) moves a whole-line cursor on the active column. Enter on a non-commented line enters Cursor mode at column 0. In Cursor mode the cursor is a character on the active side and cannot leave that column. `v`/`V` start a selection; Esc steps back one level (§11 diagram). Motions come from `vim/motion.rs` over a `TextView` of the active side's lines: `motion::apply(motion, count, Cursor { pos, want }, view, &MotionCtx { height, wrap })`. Keep the returned `Cursor` (its `want` is the sticky display column) per file view and pass it back on the next motion. Set `MotionCtx::wrap` to the active column's text width when `ui.wrap` is on, `None` when off; screen motions then move by segment. `gg`/`G`/`Ctrl-d`/`Ctrl-u` behave "as row mode", so they may be better handled by diff rows than by `apply` (which moves by lines).

## Deliverables

- `Mode::{Cursor, Visual, VisualLine}` with the anchor position stored for visual modes.
- Implement `TextView` for a side of `FileDiff` (read-only adapter; no copies).
- Cursor mode keys (§11): `h l w b e 0 $`, `j`/`k` as **screen-row** motions under wrap (`ScreenDown`/`ScreenUp`; plain `Down`/`Up` when wrap is off), `gg G Ctrl-d Ctrl-u g] g[ gn gp` as in row mode (`gn`/`gp` drop back to row mode in the new file), `v`, `V`, Enter, Esc → row mode.
  - Screen-row motions must respect the **diff view's** row layout: a row's height may exceed the active side's own wrapped height (padding from the other side). Moving down from the last segment of a line goes to the first segment of the next line on that side, skipping padding.
- Visual modes: every cursor-mode motion extends the selection; Esc → Cursor mode. Selections never cross columns.
- Horizontal scroll with wrap off: keep the cursor column visible on the active side (both columns scroll together).
- Rendering: cursor as a reversed cell on the active side; selection with a distinct background (use the theme's `active_line_background` lightened, or a `selection` key if the theme has one — document the choice).
- The comment target is a `comments::model::Anchor { file, side: Side, selection: Selection }` (task 06; 1-based, inclusive, cols in chars), built on Enter:
  - Cursor mode: `Selection::from_lines(line, line)` -> `Line` (editor starts empty).
  - Visual: `Selection::from_chars(anchor, cursor)` -> `Chars`; it orders the ends, so a backwards selection needs no special handling.
  - VisualLine: `Selection::from_lines(anchor_line, cursor_line)` -> `Line` or `Lines`.
  - No selected text is built here: task 09 gets it from `Comment::selected_text()` after `draft`.
  - In this task Enter shows the target in the status line (e.g. "Comment target: new lines 12-14", using `Selection`'s `Display`) and returns to Row mode. Task 09 replaces this with opening the editor.
- Status line shows `CURSOR` / `VISUAL` / `V-LINE` and `line:col`.

## Tests

- Unit: building the `Anchor` for each of the three modes from a fixture side (including a charwise selection made backwards, anchor after cursor).
- Unit: screen-row `j` across a wrapped line followed by a padding row.
- Snapshot: diff view with a charwise selection spanning two lines.

## Acceptance

- The three examples in §11 produce the expected targets in a real repo.
- Esc steps back exactly one level from every mode.
- fmt, clippy, tests clean.

## Handoff notes

_To be filled in by the implementing agent._
