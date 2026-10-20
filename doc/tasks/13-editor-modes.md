# Task 13 - Comment editor: modes and motions

## Goal

Give the comment editor vim normal and insert modes: motions from the shared vim engine, the insert-mode entries `i a I A o O`, and `x`/`X`. Split the editor model out of `ui/` so operators (task 14) and undo (task 15) have a home.

Read first: `doc/spec.md` §4 (event loop), §5, §10 (Status line, Comment editor), §11 (all). Handoff notes in `doc/tasks/12-cursor.md`. Source: `src/vim/keys.rs`, `src/vim/motion.rs`, `src/vim/wrap.rs`, `src/ui/editor.rs`, `src/app/comment.rs`, `src/app/actions.rs`, `src/app.rs` (`handle_key`), `src/ui/status.rs`.

## Context

Second of four tasks (12-15). The editor is currently insert-only (`ui/editor.rs`) and bypasses the key parser. The parser has one key table and maps `j`/`k` to `Motion::Down`/`Up`; `app/actions.rs` remaps those to `ScreenDown`/`ScreenUp` outside the explorer. Task 12 added the drawn block cursor helper and the terminal bar.

## Deliverables

### Key parser (`vim/keys.rs`)

- One parser, per-context tables: `handle(event, Context)`. Contexts in this task: `Diff`, `EditorNormal` (task 14 adds `EditorVisual`). The `g` chord table is per context too.
- `j`/`k` (and Down/Up arrows) map to `ScreenDown`/`ScreenUp`; `gj`/`gk` map to `Down`/`Up` (logical lines). Applies to both contexts. Remove the remap in `app/actions.rs`; the explorer accepts the screen motions as its up/down.
- `EditorNormal` table: all motions (`h j k l w b e 0 $ gg G gj gk`, arrows, `Ctrl-d`/`Ctrl-u`), `i a I A o O`, `x`, `X`, Esc, `Ctrl-q`. Diff-only keys (`]`, `[`, `gn`, `gp`, `D`, Enter, column switches, `v`/`V`) are not mapped here.
- New `Command` variants as needed, e.g. `Insert(At)` and `DeleteChar { before: bool }`.

### Editor model (`src/editor.rs` + `src/editor/`)

- Move the model from `ui/editor.rs` to `src/editor.rs`; `ui/editor.rs` keeps drawing and the box layout (`rect`, `inner`).
- `editor/buffer.rs`: `Buffer { lines: Vec<String> }`, always at least one line. Edit primitives over positions (insert text, delete a range returning the removed text); implements `TextView`.
- Cursor becomes `vim::motion::Cursor`. Mode: `Normal | Insert`.
- Normal-mode motions go through `motion::apply` with `MotionCtx { height, wrap: Some(width) }`.
- Insert-mode keys keep their v1 behavior (typing, Enter, Backspace, Delete, arrows by screen row, Home/End, PageUp/PageDown). Reuse the motion engine where it gives the same result; keep existing code where it doesn't.
- Task 14 adds `editor/operator.rs` and task 15 `editor/undo.rs`; don't create them here.

### Behavior

- Cursor column: Normal `<= len - 1` (0 on an empty line), Insert `<= len`.
- Esc in Insert: back to Normal, cursor one column left (not below 0).
- `i` at cursor, `a` after it, `I` first non-blank, `A` line end, `o`/`O` open a line below/above. Counts are ignored for these.
- `x` deletes `count` chars from the cursor, `X` `count` chars before it. Neither joins lines; no-op on an empty line. (Task 14 makes them write the register.)
- Opening: a new comment opens in Insert with the cursor at the end of the body. For a prefilled selection the body ends with `\n`, so the cursor is on the empty line below the fence; keep it that way. An existing comment opens in Normal with the cursor at the end of the body.
- Esc routing (`App::editor_key`):
  - Insert: Esc -> Normal.
  - Normal with pending keys (`keys.pending()` non-empty, checked before handing the key to the parser): Esc only clears them.
  - Normal with nothing pending: Esc opens the Close comment modal, as today. Esc in that modal returns to the editor in Normal.
  - `Ctrl-q` (any mode) no longer opens the quit modal. It opens a new Save comment modal (`Modal::SaveComment`), "Save comment #N?", with hint `Enter: save / d: delete / Esc: edit`:
    - Enter: close the editor, keeping the comment (an empty body is still removed, as for any close).
    - `d`: delete the comment entirely (new or existing) and close the editor.
    - Esc: back to the editor in its previous mode.
  - Quitting the app from the editor is therefore two `Ctrl-q`s: the first closes the comment, the second opens the quit modal.
- Paste (`Event::Paste`): inserts at the cursor in any mode; the mode is unchanged (in Normal the cursor is then clamped).
- Cursor: drawn block in Normal, terminal bar in Insert. The block is `ui::cursor_block` (task 12, private to `ui`, callable from `ui/editor.rs`), drawn only when `app.cursor_on`. Only the bar comes from `frame.set_cursor_position`; in Normal don't set it (ratatui then hides the terminal cursor).
- Blink: task 12 runs the blink timer only while `App::diff_focused()` (no explorer, no modal). Extend the condition in `App::next_blink` so it also runs while the editor is the top modal in Normal mode (not Insert, where the terminal blinks the bar).
- Status line: shows the editor's mode (`NORMAL`, `INSERT`) instead of the hard-coded `INSERT`; pending count/chord as today.

### Spec

- §5: new `editor.rs` / `editor/` entries; `ui/editor.rs` is rendering only.
- §10 Comment editor and §11: replace the "insert-only" and "one key table" text; `j`/`k` vs `gj`/`gk`.
- §10 Modals table: add Save comment (`Ctrl-q` in the comment editor; Enter save, `d` delete, Esc cancel). It is the one modal with a third key. §11: `Ctrl-q` quits everywhere except the editor, where it opens Save comment.

## Tests

- Parser: the same key maps per context (`gn` in Diff, unmapped in EditorNormal); `j` -> `ScreenDown`, `gj` -> `Down`.
- Editor: Esc from Insert moves left, and not past col 0; `x`/`X` with counts stop at line bounds; `o`/`O`; `a` on an empty line.
- App flow: new comment opens in Insert, Esc -> Normal, Esc -> Close comment modal; Esc with a pending count does not open the modal; an existing comment opens in Normal; `Ctrl-q` then `d` removes an existing comment, `Ctrl-q` then Enter keeps it. Update `create_from_visual_line_then_close_empty` for the extra Esc.
- Update the `comment_editor` snapshot if needed.

## Acceptance

- In a real repo: create a comment, type, Esc, move with `hjkl w b e 0 $ gg G gj gk`, re-enter insert with each of `i a I A o O`, `x`/`X` work; Esc Esc Enter closes; `Ctrl-q` saves (Enter) or deletes (`d`) the comment, and a second `Ctrl-q` opens the quit modal. `j`/`k` in the diff view still move by screen row, `gj`/`gk` by line.
- fmt, clippy, tests clean.

## Handoff notes

### Public interfaces

- `vim::keys`:
  - `pub enum Context { Diff, EditorNormal, Modal }`; `KeyParser::handle(event, ctx) -> Option<Input>`.
  - `pub enum At { Before, After, LineStart, LineEnd, Below, Above }`.
  - New `Command` variants: `Insert(At)`, `DeleteChar { before: bool }`, `Delete` (`d` in a modal).
  - Tables are slices searched in order per context: `MOTIONS` + `DIFF` / `EDITOR` / `MODAL`; `g` chords `G_MOTIONS` (`gg gj gk`) + `G_DIFF` (`gn gp`). `type Table<K> = &'static [(K, Command)]`. `Modal` skips counts and chords.
- `editor` (new, `src/editor.rs`):
  - `pub enum Mode { Normal, Insert }`.
  - `Editor { pub id, pub title, .. }`: `new(id, title, body, Mode)` (cursor at the end; `resize` must follow, it lays out and clamps), `mode()`, `text()`, `resize(w, h)`, `input(Input) -> bool` (Normal-mode commands; text changed), `insert_key(KeyEvent) -> bool` (Insert keys incl. Esc), `paste(&str, tab_width)`, `rows()` (visible screen rows), `cursor_cell() -> Option<(row, col)>` (in the text area).
  - Private `settle()`: clamps col for the mode, resets `cursor.want` to the column within the screen row, scrolls to the cursor. Called after every non-motion edit and by `resize`. Motions keep `want` from `motion::apply`.
- `editor::buffer::Buffer` (new): `from_text`, `text`, `lines`, `len(line)` (chars), `insert(Pos, &str) -> Pos` (end), `delete(Range<Pos>) -> String` (end exclusive; joins lines when spanning), `impl TextView`. Free fn `byte(line, col)`.
- `ui::editor`: `rect`, `inner`, `draw(frame, area, &Editor, theme, focused, cursor_on)`: Insert sets the terminal cursor, Normal draws `ui::cursor_block`. `ui::modals::draw` gained `cursor_on`.
- `app`:
  - `App::editor() -> Option<&Editor>` (anywhere on the stack) replaces `editing()`.
  - `Modal::SaveComment(u32)`; `Action::ModalDelete`; `App::delete_editor_comment()` (`pub(super)`).
  - `open_editor(id, editor::Mode)`; `editor_key` routes Ctrl-q (any mode), raw Insert keys, Normal keys through `keys.handle(key, Context::EditorNormal)`.
  - `next_blink` also runs while the top modal is an editor in Normal mode.

### Decisions (developer decisions; spec §2, §5, §10, §11 and README updated)

- `j`/`k` (and Down/Up arrows) emit `ScreenDown`/`ScreenUp` from the key table; the remap in `actions.rs` is gone. `gj`/`gk` emit `Down`/`Up`. The explorer treats both pairs the same.
- Dialogs use their own `Modal` key table; `d` there is `Command::Delete`, acted on only by Save comment.
- Ctrl-q in the editor resets the parser, then: empty/whitespace body -> close (removes the comment) without a modal; otherwise Save comment. `Action::Quit` is ignored while the editor is anywhere on the modal stack (it previously allowed quitting from over Save comment).
- Esc in Normal opens Close comment only when `keys.pending()` was empty before the key.
- `editing()` removed (its only caller, the status line, uses `editor()`).
- Insert-mode Up/Down/PageUp/PageDown keep their own screen-row code (`vertical`/`col_at`): they handle the cursor at line end and the trailing empty row of a full-width line, which `motion::apply` doesn't. They share `cursor.want` with Normal mode.

### Left undone / flagged

- Logged in spec §2 Later (developer decision, not fixed): word-boundary wrapping, and the sticky column under wrap (`Down`/`Up`/half-page motions read the segment-relative `want` as a whole-line column; affects `gj`/`gk` and `Ctrl-d`/`Ctrl-u` in both views).
- Manually verified by the developer in ghostty.
