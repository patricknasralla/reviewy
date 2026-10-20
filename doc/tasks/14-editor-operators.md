# Task 14 - Comment editor: operators, register, visual mode

## Goal

Add vim operators (`d`, `c`, `y` and their variants), a yank register shared by all editors, put (`p`/`P`), and visual / visual-line mode to the comment editor.

Read first: `doc/spec.md` §5, §10 (Comment editor), §11. Handoff notes in `doc/tasks/12-cursor.md` and `13-editor-modes.md`. Source: `src/vim/keys.rs`, `src/vim/motion.rs`, `src/editor.rs` and `src/editor/`, `src/ui/editor.rs`, `src/ui/diff_view.rs` (selection range code around line 106), `src/app/comment.rs`, `src/app/state.rs`.

## Context

Third of four tasks (12-15). Task 13 gave the editor Normal/Insert modes, the `EditorNormal` key context, `editor/buffer.rs` and `x`/`X`.

Things task 13 left that this task must change (see its handoff notes):
- `App::editor_key` (`app/comment.rs`) always parses with `Context::EditorNormal` and opens Close comment on an unpending Esc. Pick `EditorVisual` in visual modes, and only open Close comment from Normal (Esc in visual goes to Normal).
- `x`/`X` are `Command::DeleteChar` in the `EDITOR` table, handled in `Editor::input`; they must write the register. `Editor::input` will need the registers (e.g. `input(Input, &mut Registers)`).
- Code that matches `editor::Mode::Normal` must also cover the visual modes: the block cursor in `ui/editor.rs::draw`, `App::next_blink`, and the status line (`VISUAL` / `V-LINE`).
- `Modal` table maps `d` to `Command::Delete`; the editor's `d` operator is separate (in `EditorNormal`).
- `Buffer::delete(Range<Pos>)` returns the removed text (register content); `Buffer::insert` returns the end position.

## Deliverables

### Key parser

- Operator-pending state in the parser (next to the `g` chord state), `EditorNormal` only:
  - `d`/`c`/`y` then a motion -> `Command::Operator { op, target: Motion(m) }`.
  - The same key twice (`dd`, `cc`, `yy`) -> target `Line`.
  - Counts multiply: `2d3w` emits count 6. `count` stays `None` when neither was typed.
  - `0` after an operator is the motion unless a count was started after the operator (`d0` vs `d10w`). `g` chords work inside (`dgg`, `dgj`).
  - Esc or an unmapped key cancels. `pending()` shows e.g. `2d`.
- `D` = `d$`, `C` = `c$`, `Y` = `y$` (Neovim). `p`/`P` with counts.
- `v`/`V` in `EditorNormal` enter visual / visual-line.
- `EditorVisual` context: all motions; `v`/`V` as in the diff view (same key -> Normal, other key switches, keeping the anchor); Esc -> Normal. These emit immediately with target `Selection`: `d`/`x` delete, `c` change, `y` yank, `p` replace; `D`/`X` delete, `C` change, `Y` yank linewise over the lines the selection covers.

### Shared selection range (`vim/`)

- Move the code that turns a visual mode, anchor and cursor into a range out of `ui/diff_view.rs` into `vim/` (e.g. `vim/selection.rs`). The diff view and the editor both use it.

### Register (`vim/register.rs`)

- `Register { text: String, linewise: bool }` inside a `Registers` struct holding only the unnamed register (named registers and the system clipboard come later).
- Owned by `App` and passed to the editor, so a yank in one comment can be put in another and survives closing the editor.
- Every `d`, `c`, `x`, `X`, `y` writes it (`x`/`X` from task 13 included).

### Operators (`editor/operator.rs`)

- Motion type under an operator:
  - exclusive: `h`, `l`, `w`, `b`, `0`
  - inclusive: `e`, `$`
  - linewise: `j`, `k`, `gj`, `gk`, `gg`, `G`, `Ctrl-d`, `Ctrl-u`. Under an operator `j`/`k`/`gj`/`gk` all act on logical lines, so `dj` deletes two lines even when wrapped.
- Vim special cases:
  - `Y` follows Neovim: yank to line end (`y$`, charwise), not vim's `yy`. In visual mode `Y` is still linewise.
  - `dw`/`yw`: when `w` lands on a later line, the range ends at the end of the start line.
  - `cw` on a non-blank acts as `ce`; on whitespace it acts as `dw` then insert.
  - A motion that fails to move (`h` at col 0) is a no-op; `dl` on the last char deletes it.
- `dd`/`yy`/`cc` take a count of lines, clamped to the end of the buffer.
- `cc` and linewise `c` replace the lines with one empty line and enter Insert. `C`/`D` on an empty line: `C` enters Insert, `D` is a no-op.
- Deleting every line leaves one empty line.
- Cursor after:
  - delete: at the range start, clamped; after a linewise delete, first non-blank of the line now there (the previous line when the last line was deleted).
  - yank: at the range start (`yy` leaves it in place).
  - change: at the range start, in Insert.
- Put: charwise `p`/`P` after/before the cursor char, cursor on the last put char. Linewise `p`/`P` below/above the line, cursor on the first non-blank of the first put line. A count repeats the text.

### Visual mode in the editor

- Mode gains `Visual { anchor }` and `VisualLine { anchor }`; charwise selection is inclusive of the cursor char.
- Drawn with `theme.selection`; the cursor is the drawn block.
- After an operator: Normal (Insert for `c`/`C`).
- `p` replaces the selection with the register. The register keeps its content (as Neovim's visual `P`), so one yank can replace several selections.
- Status line shows `VISUAL` / `V-LINE`. Esc routing from task 13: Esc in visual -> Normal, never the close modal.

### Spec

- §5: `vim/register.rs`, `vim/selection.rs`, `editor/operator.rs`.
- §10 Comment editor / §11: operators, register, visual mode in the editor.
- §2: "yank to system clipboard" stays planned; note the internal register exists.

## Tests

- Parser: `2d3w` -> count 6; `dd` -> target `Line`; `d0` vs `d10w`; `dgg`; Esc mid-operator clears pending; in `EditorVisual`, `d` emits at once.
- Operators: `dw` on the last word of a line stops at line end; `cw` == `ce`; `de` inclusive; `dj` deletes two lines on a wrapped buffer; `dd` on the last line moves the cursor up; `dd` on the only line leaves one empty line.
- Put: linewise vs charwise placement, with a count.
- Visual: `V` + `D` over two lines; `p` replaces the selection and keeps the register.

## Acceptance

- In a real repo: `dw dd cw cc C D x X yy p P` and `v`/`V` with `d c y p` behave as in vim; a yank in one comment can be put in another.
- fmt, clippy, tests clean.

## Handoff notes

### Public interfaces

- `vim::keys`:
  - `pub enum Op { Delete, Change, Yank }`; `pub enum Target { Motion(Motion), Line, Selection, SelectionLines }`.
  - `Command::Operator { op, target }`, `Command::Put { before }`; `Context::EditorVisual`.
  - `KeyParser` gained `op: Option<(Op, Option<usize>)>` (operator + count before it); `OPS` table (`d c y`, `EditorNormal` only); `pending()` shows e.g. `2d3g`. `EDITOR` adds `D C Y p P v V`; new `VISUAL` table (`v V d x c y D X C Y p`, Esc, Ctrl-q) with `MOTIONS` + `G_MOTIONS`.
- `vim::selection` (new): `bounds(anchor, cursor) -> (Pos, Pos)` (ordered, end inclusive), `line_cols(anchor, cursor, linewise, line) -> Option<Range<usize>>`. The diff view's `marks` uses it.
- `vim::register` (new): `Register { text, linewise }`, `Registers { unnamed }` (both `Default`). `App::registers`.
- `vim::motion::same_word(a, b) -> bool` (new, earlier module): whether two adjacent chars are in one vim word.
- `editor`:
  - `Mode` gains `Visual { anchor }`, `VisualLine { anchor }`.
  - `Editor::input(Input, &mut Registers) -> bool`; `rows()` now yields `(line, byte range)`; new `line(idx)`, `selection_cols(line)`; private `ctx()`.
  - `editor::operator` (new, private): `Editor::operator(op, target, count, regs)`, `Editor::put(before, count, regs)`; private `Span`, `span`, `motion_span`.
  - `Buffer::slice(Range<Pos>)`, `Buffer::delete_lines(RangeInclusive<usize>)`.
- `ui::selection_cells(line, seg, sel, max) -> Option<Range<u16>>` (private to `ui`): cells of a wrapped row covered by a selection; shared by the diff view and the editor.

### Decisions (developer decisions; spec §2, §5, §10 and README updated)

- A forward exclusive motion (`l`, `w`) that can't move ends at the line length (never the line break); a backward one that can't move is a no-op.
- `dw` landing on a later line at or before its first non-blank ends at the end of the previous line (covers `d3w` across lines). `w` stuck on the buffer's last word (target not a word start, via `same_word`) deletes through the line end.
- `cw` on a word's last char changes just that char (vim); otherwise `cw` = `ce`. `cw` on whitespace acts as `dw` + insert.
- Visual `p` with mismatched register/selection kinds inserts the register text as plain chars where the selection was; the register is kept.
- `Y` = `y$` (Neovim). Change always enters Insert, even with nothing to change.
- `yy`/linewise yank moves the cursor to the first yanked line, keeping the column.
- `x`/`X` write the register only when something was deleted.
- `editor_key` picks `EditorVisual` in visual modes; Close comment only opens from Normal.

### Left undone / flagged

- Visual `d` on a selection that ends on an empty line does not delete that line's break (the charwise end is capped at the line length).
- No visual `o` (swap anchor), no auto-indent on `cc`/`o`.
