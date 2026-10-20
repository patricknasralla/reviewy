# Task 15 - Comment editor: undo and redo

## Goal

Add `u` (undo) and `Ctrl-r` (redo) to the comment editor, with vim-like change grouping and a configurable history limit.

Read first: `doc/spec.md` §10 (Comment editor), §11, §13. Handoff notes in `doc/tasks/13-editor-modes.md` and `14-editor-operators.md`. Source: `src/editor.rs` and `src/editor/`, `src/vim/keys.rs`, `src/config.rs`, `src/defaults/defaults.toml`, `src/app/comment.rs`.

## Context

Last of four tasks (12-15). The editor has Normal/Insert/Visual modes, operators and put (tasks 13, 14).

From task 13: `Buffer` (`editor/buffer.rs`) keeps `lines` private; snapshots need it to be `Clone` (or a `lines` setter). Insert sessions start in `Editor::input` (`Command::Insert`, and from task 14 `c`) and end at Esc in `Editor::insert_key`. Ctrl-r is not yet in any key table.

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
