# Task 05 — Vim engine

## Goal

Implement `src/vim/`: a key parser that turns key events into commands (with counts and chords), motions over read-only text, and a soft-wrap layout. It is shared by the diff view (tasks 07, 08) and the comment editor (v1.1), so it must not depend on `app/`, `ui/` or `diff/`.

Read first: `doc/spec.md` §11 (all of it, especially "Vim engine"), §10 (diff view wrapping). Read handoff notes in `doc/tasks/01-scaffold.md`.

## Context

The diff view is navigated like a read-only vim buffer. Row mode moves by whole lines; cursor and visual modes move a text cursor. The parser does **not** know about modes' meanings: it emits key-level commands (`Motion(Left, 1)`), and the app decides what they mean in each mode (in row mode `h`/`l` switch columns). The only mode information the parser needs is which key table to use (see below).

## Deliverables

### `vim/keys.rs`

```rust
pub enum Command {
    Motion { motion: Motion, count: usize },  // count defaults to 1
    Confirm,            // Enter
    Escape,             // Esc (also clears pending input)
    Visual, VisualLine, // v, V
    DeleteComment,      // D
    NextHunk, PrevHunk, // g] g[  (count applies)
    NextFile, PrevFile, // gn gp
    Quit,               // Ctrl-q
}
pub struct KeyParser { /* pending count + chord prefix */ }
impl KeyParser {
    pub fn handle(&mut self, key: crossterm::event::KeyEvent) -> Option<Command>;
    pub fn pending(&self) -> String; // e.g. "5g", for the status line
}
```

- Counts: digits accumulate (`12j`); a leading `0` is the `LineStart` motion, not a count.
- Chords: `g` prefix → `gg`, `g]`, `g[`, `gn`, `gp`. No timeout. An unknown second key cancels the chord silently.
- Esc clears pending state and emits `Escape`. `Ctrl-q` always emits `Quit`, even mid-chord. `Ctrl-c` emits nothing.
- Keys map to commands through a static table (`&[(KeySpec, Command)]` or similar) so configurable bindings later are a table swap.
- Key → command table (motions per §11): `h j k l w b e 0 $`, `gg`, `G`, `Ctrl-d`, `Ctrl-u`, `g] g[ gn gp`, `v V D`, Enter, Esc, `Ctrl-q`. Ignore key release events.

### `vim/motion.rs`

- `Motion` enum: `Left, Right, Down, Up, ScreenDown, ScreenUp, WordForward, WordBackward, WordEnd, LineStart, LineEnd, FileStart, FileEnd, HalfPageDown, HalfPageUp`. The parser emits `Down`/`Up` for `j`/`k`; the app picks `ScreenDown`/`ScreenUp` in cursor mode.
- `TextView` trait as in §11 (`line_count`, `line`).
- `Pos { line: usize, col: usize }`, `col` = **char index** in the line.
- `fn apply(motion, count, pos, view: &dyn TextView, ctx: &MotionCtx) -> Pos`, where `MotionCtx` carries the viewport height (for half page) and an optional wrap width (for screen motions). Vim semantics:
  - `h`/`l` stop at line edges (no wrapping to the next line). On an empty line col is 0.
  - `j`/`k` keep a desired column (track `want_col` in the caller or in `Pos`; document which).
  - Word classes: keyword (alphanumeric + `_`), punctuation (other non-blank), whitespace. `w`, `b`, `e` cross lines; empty lines are word stops for `w`/`b`, as in vim.
  - `$` goes to the last char; `0` to col 0.
  - `FileStart`/`FileEnd` with a count go to that line (`5G`), like vim.
- Motions clamp to valid positions and never panic on empty files.

### `vim/wrap.rs`

- `fn wrap_line(line: &str, width: u16) -> Vec<Range<usize>>`: split a line into screen segments by display width (`unicode-width`), breaking at char boundaries (no word wrap). An empty line is one empty segment. Wide chars never split across segments.
- `fn line_height(line, width) -> usize` and mapping helpers: (line, char col) ↔ (segment index, display column). Screen motions use these.
- Single-column only. The diff view (task 07) combines two sides into row heights (each row's height is the max of both sides) on top of this.

## Tests

- Parser: `5j` → `Motion{Down,5}`; `gg`; `3g]`; `0` vs `10j`; Esc mid-chord clears it; `Ctrl-q` mid-chord quits; `g` then unknown key emits nothing.
- Motions over a small fixture text: `w`/`b`/`e` across punctuation, whitespace and an empty line; `$`/`0`; `j` onto a shorter line and back keeps the column; `5G`.
- Wrap: a line of width 25 at width 10 → 3 segments; a wide CJK char at the boundary moves to the next segment; `ScreenDown` inside a wrapped line moves to the next segment of the same line.

## Acceptance

- `vim/` compiles with no imports from `app/`, `ui/` or `diff/`.
- fmt, clippy, tests clean.

## Handoff notes

_To be filled in by the implementing agent._
