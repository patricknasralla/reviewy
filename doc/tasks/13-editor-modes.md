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
- Cursor: drawn block (task 12 helper, blinking) in Normal, terminal bar in Insert.
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
