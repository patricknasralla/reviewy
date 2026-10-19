# Task 08 — Cursor and visual modes

## Goal

Add Cursor, Visual (charwise) and VisualLine modes to the diff view: a text cursor on the active side driven by the vim motions, selection rendering, and a `CommentTarget` describing what Enter would comment on. Task 09 consumes that target to open the comment editor.

Read first: `doc/spec.md` §11 (mode hierarchy, cursor mode, visual modes, examples), §10 (diff view, wrapping, status line). Read handoff notes in `doc/tasks/05-vim.md` and `07-app-row-mode.md` (and skim the others).

## Context

Row mode (task 07) moves a whole-line cursor on the active column. Enter on a non-commented line enters Cursor mode at column 0. In Cursor mode the cursor is a character on the active side and cannot leave that column. `v`/`V` start a selection; Esc steps back one level (§11 diagram). Motions come from `vim/motion.rs` over a `TextView` of the active side's lines.

## Deliverables

- `Mode::{Cursor, Visual, VisualLine}` with the anchor position stored for visual modes.
- Implement `TextView` for a side of `FileDiff` (read-only adapter; no copies).
- Cursor mode keys (§11): `h l w b e 0 $`, `j`/`k` as **screen-row** motions under wrap (`ScreenDown`/`ScreenUp`; plain `Down`/`Up` when wrap is off), `gg G Ctrl-d Ctrl-u g] g[ gn gp` as in row mode (`gn`/`gp` drop back to row mode in the new file), `v`, `V`, Enter, Esc → row mode.
  - Screen-row motions must respect the **diff view's** row layout: a row's height may exceed the active side's own wrapped height (padding from the other side). Moving down from the last segment of a line goes to the first segment of the next line on that side, skipping padding.
- Visual modes: every cursor-mode motion extends the selection; Esc → Cursor mode. Selections never cross columns.
- Horizontal scroll with wrap off: keep the cursor column visible on the active side (both columns scroll together).
- Rendering: cursor as a reversed cell on the active side; selection with a distinct background (use the theme's `active_line_background` lightened, or a `selection` key if the theme has one — document the choice).
- `CommentTarget { file, side: Side, lines: LineRange, cols: Option<ColRange>, selected_text: Option<String> }` (1-based, `comments::model` types), built on Enter:
  - Cursor mode: current line, no cols, no selected text (editor starts empty).
  - Visual: the char range (inclusive, 1-based chars), `cols` set only for single- or multi-line charwise selections, `selected_text` = the selected chars.
  - VisualLine: the line range, no cols, `selected_text` = the whole lines.
  - In this task Enter shows the target in the status line (e.g. "Comment target: new 12–14") and returns to Row mode. Task 09 replaces this with opening the editor.
- Status line shows `CURSOR` / `VISUAL` / `V-LINE` and `line:col`.

## Tests

- Unit: building `CommentTarget` for each of the three modes from a fixture side (including a charwise selection made backwards, anchor after cursor).
- Unit: screen-row `j` across a wrapped line followed by a padding row.
- Snapshot: diff view with a charwise selection spanning two lines.

## Acceptance

- The three examples in §11 produce the expected targets in a real repo.
- Esc steps back exactly one level from every mode.
- fmt, clippy, tests clean.

## Handoff notes

_To be filled in by the implementing agent._
