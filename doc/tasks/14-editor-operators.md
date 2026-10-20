# Task 14 - Comment editor: operators, register, visual mode

## Goal

Add vim operators (`d`, `c`, `y` and their variants), a yank register shared by all editors, put (`p`/`P`), and visual / visual-line mode to the comment editor.

Read first: `doc/spec.md` §5, §10 (Comment editor), §11. Handoff notes in `doc/tasks/12-cursor.md` and `13-editor-modes.md`. Source: `src/vim/keys.rs`, `src/vim/motion.rs`, `src/editor.rs` and `src/editor/`, `src/ui/editor.rs`, `src/ui/diff_view.rs` (selection range code around line 106), `src/app/comment.rs`, `src/app/state.rs`.

## Context

Third of four tasks (12-15). Task 13 gave the editor Normal/Insert modes, the `EditorNormal` key context, `editor/buffer.rs` and `x`/`X`.

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
