# Task 08 - Cursor and visual modes

## Goal

Add Cursor, Visual (charwise) and VisualLine modes to the diff view: a text cursor on the active side driven by the vim motions, selection rendering, and an `Anchor` (task 06) describing what Enter would comment on. Task 09 consumes that target to open the comment editor.

Read first: `doc/spec.md` §11 (mode hierarchy, cursor mode, visual modes, examples), §10 (diff view, wrapping, status line). Read handoff notes in `doc/tasks/05-vim.md` and `07-app-row-mode.md` (and skim the others).

## Context

Row mode (task 07) moves a whole-line cursor on the active column. Enter on a non-commented line enters Cursor mode at column 0. In Cursor mode the cursor is a character on the active side and cannot leave that column. `v`/`V` start a selection; Esc steps back one level (§11 diagram). Motions come from `vim/motion.rs` over a `TextView` of the active side's lines: `motion::apply(motion, count, Cursor { pos, want }, view, &MotionCtx { height, wrap })`. Keep the returned `Cursor` (its `want` is the sticky display column) per file view and pass it back on the next motion. Set `MotionCtx::wrap` to the active column's text width when `ui.wrap` is on, `None` when off; screen motions then move by segment. `gg`/`G`/`Ctrl-d`/`Ctrl-u` behave "as row mode", so they may be better handled by diff rows than by `apply` (which moves by lines).

From task 07 (see its handoff notes): per-file state is `app::view::FileView` (`row`, `side`, private `home`, `scroll`, cached row `tops`). `FileView::hunk(next, count)` may **switch side** (pure addition/deletion hunks); in Cursor/Visual modes the cursor must not leave its column, so add a no-switch variant that skips hunks with no changed line on the active side. Scroll is row-granular: a wrapped row taller than the viewport is clipped at the bottom; cursor mode needs segment-level scroll inside such a row. `allow(dead_code, reason = "used by task 08")` sits on `vim::motion::apply` and `Motion::ScreenUp`; remove both. `Action`/`App::map` live in `app/actions.rs`; Enter/`D` in row mode currently set the status "Comments: task 08/09".

## Deliverables

- `Mode::{Cursor, Visual, VisualLine}` with the anchor position stored for visual modes.
- Implement `TextView` for a side of `FileDiff` (read-only adapter; no copies).
- Cursor mode keys (§11): `h l w b e 0 $`, `j`/`k` as **screen-row** motions under wrap (`ScreenDown`/`ScreenUp`; plain `Down`/`Up` when wrap is off), `gg G Ctrl-d Ctrl-u g] g[ gn gp` as in row mode (`gn`/`gp` drop back to row mode in the new file), `v`, `V`, Enter, Esc → row mode.
  - Screen-row motions must respect the **diff view's** row layout: a row's height may exceed the active side's own wrapped height (padding from the other side). Moving down from the last segment of a line goes to the first segment of the next line on that side, skipping padding.
- Visual modes: every cursor-mode motion extends the selection; Esc → Cursor mode. Selections never cross columns.
- Horizontal scroll with wrap off: keep the cursor column visible on the active side (both columns scroll together).
- Rendering: cursor as a reversed cell on the active side; selection with a distinct background (use the theme's `active_line_background` lightened, or a `selection` key if the theme has one - document the choice).
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

### Public interfaces

- `app::state::Mode::{Explorer, Normal, Visual { anchor: vim::motion::Pos }, VisualLine { anchor }}` (`Copy`). Row and Cursor modes no longer exist (see below).
- `app::view::FileView` gained `pub cursor: vim::motion::Cursor` (always valid when the side has lines), `pub hscroll: usize` (display cols, shared by both columns), private `row_of` (line → row per side). `scroll` is now a **screen line**. Methods: `text(Side) -> Option<&SideText>`, `text_width() -> u16` (active column), `cursor_motion(Motion, Option<usize>, &MotionCtx)`, `switch(Side)` and `hunk(next, count) -> bool` (both land on the first non-blank char), `refresh_layout(width, height, wrap)` (keeps the text cursor's screen line, and its column when not wrapping, visible). Row-mode `motion` removed.
- `app::actions::Action::{ModalConfirm, ModalCancel, ExplorerMove, ExplorerOpen, ExplorerClose, Move(Motion, Option<usize>), Switch(Side), Hunk { next, count }, File { next }, ToExplorer, ToNormal, Visual { line: bool }, Comment, DeleteComment, Quit}`. Private `fn anchor(mode, &FileView, file: &Path) -> Option<Anchor>` builds the comment target (`None` in the explorer or when the side has no lines); `App::comment()` currently shows it as `Comment target: <old|new> <Selection>` and returns to Normal (`// task 09: open the comment editor`).
- `impl vim::motion::TextView for diff::model::SideText`.
- `vim::keys::Command` gained `ColumnLeft`, `ColumnRight`; `Key::CtrlCode(KeyCode)` matches Ctrl + non-char keys. `vim::motion::first_non_blank` and `vim::wrap::char_width` are `pub`.
- `highlight::theme::Theme::selection: Color`; `Tint` gained `selection: f32`.
- `config::DifftConfig::graph_limit: usize`; `diff::difft::DifftEngine { binary, graph_limit }`.

### Decisions and deviations (developer decisions; spec §7, §9, §10, §11, §12, §13 updated)

- **Row and Cursor modes merged into Normal** after functional testing: the diff view always has a text cursor. `h`/`l` move by char, `j`/`k` by screen row; **Ctrl-h/Ctrl-l** (aliases Ctrl-Left/Ctrl-Right) switch columns. Enter in Normal targets the cursor line (`Selection::Line`); `D` works in Normal only. Esc: visual → Normal → explorer.
- **Ctrl-h is never Backspace**: crossterm reports `0x08` as Ctrl+`h` and `0x7F` as Backspace; terminals sending `^H` for Backspace are not supported. Applies to the comment editor too.
- **`[`/`]` replace `g[`/`g]`** (counts work: `3]`).
- **`v`/`V` toggle as vim**: same key → Normal; other key switches kind, keeping the anchor. In visual, `Ctrl-h/l` and `[`/`]` leave visual first, then act as in Normal (selections never cross columns).
- **Jumps land on the first non-blank char**: file open (first hunk), hunk jumps, column switches. `want` is then the line display col (not segment-relative; differs only when indentation exceeds the column width; accepted). `Ctrl-d`/`Ctrl-u` keep the column; `gg`/`G` as vim.
- **All motions go through `motion::apply`**, including `gg`/`G`/`Ctrl-d`/`Ctrl-u`: on the active side's lines they match the old row-mode results. Padding needs no code: `apply` works in side-line space; screen motions use the active column's text width.
- **Scroll refactor** (task 07 module change): `FileView::scroll` is a screen line; `draw` starts mid-row (gutter hidden on clipped rows). Fixes task 07's "tall row clipped at the bottom". Scrolling up with the margin may show the bottom part of the row above.
- **Horizontal scroll** (wrap off): one `hscroll` for both columns, kept visible for the active column's text width; a wide char straddling the left edge is skipped.
- **Selection colour**: `players[0].selection` (blended onto the background), else `foreground` blended onto `active_line_background` at `[ui.tint] selection = 0.15`. Selection overrides line/token/cursor-line backgrounds; an empty selected line shows one selected cell; the cursor is a `REVERSED` cell.
- **difft speed**: difft's cost scales with the changed region; heavily rewritten files hit `DFT_GRAPH_LIMIT` (default 3M) after ~1.5 s and fall back to a line diff. New `[difft] graph_limit = 500000`, passed as `DFT_GRAPH_LIMIT`: same output in ~250 ms on this repo's 300-line rewrites; moderately changed files keep the structural diff (200k already changed output for some).
- **Examples**: `w w v w` selects vim-style (`v w` includes the first char of the next word); spec §11 wording updated.
- Status line: `NORMAL` / `VISUAL` / `V-LINE`, `line:col` of the text cursor (1-based char col; `0:0` when the side has no lines).
- Extra test beyond the list: `switch_lands_on_first_non_blank`; key parser test for Ctrl-h/Ctrl-Left. Row-mode test `j_skips_padding_on_active_side` removed with row motions.
- Manual check (scratch repo, GNU screen): the three §11 examples, toggles, Esc chain, Ctrl-h/l and Ctrl-arrows (target side and first non-blank), visual leaving on column/hunk jumps, `]`/`[`, wrap-off horizontal scroll.

### Left undone / flagged

- Selection and cursor colours are only checked by cell asserts (snapshots are text-only) and the developer's manual testing.
- Selection cols are in tab-expanded space (task 11 tabs follow-up; the caller to change is `anchor` in `app/actions.rs`).
