# Task 15 - Comment editor: undo and redo

## Goal

Add `u` (undo) and `Ctrl-r` (redo) to the comment editor, with vim-like change grouping and a configurable history limit.

Read first: `doc/spec.md` §10 (Comment editor), §11, §13. Handoff notes in `doc/tasks/13-editor-modes.md` and `14-editor-operators.md`. Source: `src/editor.rs` and `src/editor/`, `src/vim/keys.rs`, `src/config.rs`, `src/defaults/defaults.toml`, `src/app/comment.rs`.

## Context

Last of four tasks (12-15). The editor has Normal/Insert/Visual modes, operators and put (tasks 13, 14).

From task 13: `Buffer` (`editor/buffer.rs`) keeps `lines` private; snapshots need it to be `Clone` (or a `lines` setter). Insert sessions start in `Editor::input` (`Command::Insert`, or `Command::Operator` with `Op::Change`, which ends in Insert via `Editor::operator`) and end at Esc in `Editor::insert_key`. Ctrl-r is not yet in any key table. From task 14: `Editor::input` takes `&mut Registers`; undo must not touch the register (vim doesn't restore it).

## Deliverables

### `editor/undo.rs`

- `Snapshot { lines: Vec<String>, cursor: Cursor }`: the state **before** a change.
- Undo stack capped at `editor.undo_levels`; when full, the oldest snapshot is dropped. `0` disables undo. Redo stack is uncapped (it can never exceed the undo cap).
- History belongs to the editor and is gone when it closes.

### Grouping

- Each Normal/Visual-mode change (`x`, `X`, `d…`, `p`, `P`, visual ops, paste) is one step.
- An Insert session is one step: everything from entering Insert to Esc, including the change that entered it (`cw`, `cc`, `C`, `o`, `O`) and pastes during the session.
- A step is only recorded when the text changed (e.g. `x` on an empty line or `i` then Esc records nothing).
- Recording a new step clears the redo stack.

### Commands

- `u` and `Ctrl-r` in `EditorNormal`, with counts (`3u`). Nothing to undo/redo: no-op.
- Undo restores the snapshot's text and cursor (clamped to Normal); the current state goes on the redo stack. Redo is the reverse. Vim instead puts the cursor at the start of the change; restoring the saved cursor is simpler and fine for comments.
- The comment body is synced after undo/redo like any edit.

### Config

- New `[editor]` table in `defaults.toml`: `undo_levels = 100`. `Config::editor: EditorConfig`.

### Spec

- §10 Comment editor / §11: undo/redo. §13: `[editor]` table.

## Tests

- `cw` + typing + Esc undoes in one step back to the original text.
- At the cap, the oldest step is dropped (small cap in the test).
- A new change after an undo clears redo.
- A no-op change records nothing.

## Acceptance

- In a real repo: `u` / `Ctrl-r` step through normal-mode changes and whole insert sessions as in vim.
- fmt, clippy, tests clean.

## Handoff notes

### Public interfaces

- `config::EditorConfig { undo_levels: usize }`, `Config::editor`; `[editor] undo_levels = 100` in `defaults.toml`.
- `editor::undo` (new, private): `Snapshot { buffer: Buffer, cursor: Cursor }`; `Undo::new(levels)`, `record(before)` (clears redo), `undo(current) -> Option<Snapshot>`, `redo(current) -> Option<Snapshot>`.
- `Editor::new(id, title, body, mode, undo_levels)`; private fields `undo`, `insertion: Option<Snapshot>`; private `snapshot()`, `commit(before)`.
- `Buffer` derives `Clone, PartialEq`.
- `vim::keys`: `Command::Undo`, `Command::Redo`; `u` and `Ctrl-r` in the `EDITOR` table.

### Decisions (developer decisions; spec §2, §5, §10, §13 and README updated)

- Naming: the Insert-mode group is an "insertion" (field `insertion`), not a "session".
- Snapshots are whole `Buffer` clones; "changed" is buffer equality against the snapshot, not the edit functions' return values (typing then deleting back records nothing).
- Only commands that can edit (`Insert`, `DeleteChar`, `Operator`, `Put`, paste) take a snapshot; motions and visual toggles don't. `commit` keeps the snapshot as the insertion start when the command left the editor in Insert, else records it if the text changed.
- Undo restores the snapshot's cursor (clamped for Normal), not vim's change position.
- Test helper `Run` (key-sequence driver) moved from `editor/operator.rs` tests to `editor.rs` tests and is shared with `editor/undo.rs` tests; `\x12` feeds Ctrl-r; `is()` returns the runner for chaining.

### Left undone / flagged

- No "Already at oldest change" message: `u`/`Ctrl-r` with nothing to restore do nothing.
