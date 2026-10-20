# Task 09 - Comment editor and comment lifecycle

## Goal

Let the reviewer create, edit and delete comments: the comment editor widget (insert-only), the editor modal frame, the delete and close-comment confirmations, gutter markers and explorer counts.

Read first: `doc/spec.md` §10 (comment editor, modals), §11 (Normal mode Enter/`D`), §12 (model, lifecycle). Read handoff notes in `doc/tasks/06-comments.md`, `07-app-row-mode.md`, `08-cursor-visual.md` (and skim the others).

## Context

Task 08 merged row and cursor modes into one **Normal** mode (text cursor always present; see its handoff notes). Enter in Normal/Visual/VisualLine maps to `Action::Comment`; `App::comment()` (`app/actions.rs`) builds a `comments::model::Anchor` with `anchor(mode, view, file)` and currently only shows it in the status line (`// task 09: open the comment editor`) - replace that. `comments::model` (task 06) provides `CommentSet`, `draft`, `prefill_body`, `overlaps`, `at`. `App` already has an empty `comments: CommentSet` field (base `HEAD` + startup SHA); the explorer already shows `[n]` via `count_for`, and `ui::diff_view` has a blank one-char comment-marker slot in each gutter (`GUTTER_EXTRA`). Comments live in memory only in this task; saving is task 10.

## Deliverables

### `ui/editor.rs`: comment editor widget

- Buffer `Vec<String>` plus cursor `(line, char col)`; soft-wrapped with `vim::wrap` at the box's inner width; scrolls to keep the cursor visible.
- Insert-only (v1): printable chars, Enter (newline), Backspace (joins lines at col 0), Delete, arrows (Up/Down by screen row), Home/End, PageUp/PageDown. Ctrl-h is **not** Backspace (developer decision, task 08): leave it unmapped in the editor. Bracketed paste (`Event::Paste`) inserts text verbatim (normalize `\r\n` → `\n`).
- Implement `vim::TextView` over the buffer (unused in v1; it's what v1.1 modal editing builds on).
- Every edit writes the buffer back to the comment's `body` immediately (no save step).

### Flows (`app/actions.rs`)

- **Create** (Enter in Normal mode on an uncommented line, or Enter in Visual/VisualLine): if `overlaps(...)` returns an id → status "Overlaps comment #N" and stay in the current mode. Otherwise build a draft with `draft(anchor, language, side_lines, context_lines, body)` (`side_lines` = the active `SideText::lines`, `context_lines` from config, language from `highlight::languages::detect(...).name()`). For visual modes (`Lines`/`Chars`) set `body = prefill_body(&selected_text, language)`, where `selected_text` comes from `Comment::selected_text()` (build the draft with an empty body, read it, or add then fill via `get_mut`); Normal mode (`Line`) starts empty. Add it to the set, open the editor with the cursor on the line after the fence (or at the start for an empty body). Return to Normal mode underneath.
- **Edit** (Enter in Normal mode on a line that has a comment on the active side, via `at(...)`, checked before building the anchor): open the editor on that comment, cursor at the end.
- **Delete** (`D` in Normal mode on a commented line; `Action::DeleteComment` currently sets a placeholder status): "Delete comment #N?" modal; Enter removes it. `D` on an uncommented line: status message.
- **Close editor** (Esc): "Close comment?" modal; Enter closes, Esc returns to editing. On close, if the body is empty or whitespace, remove the comment.
- `Ctrl-q` inside the editor still opens the quit modal.
- Remove the `#[allow(dead_code)]` on `Config` in `config.rs` once `comments.context_lines` is read.
- Remove the `#[allow(dead_code, reason = "used by task 09; remove then")]` on `mod comments;` in `main.rs` (task 06); `store` stays unused until task 10, so move an `#[allow]` onto `pub mod store;` if needed.

### Rendering

- Editor modal: centered box (~70% width, ~50% height) over the diff view, titled `Comment - <file>:<start>[-<end>] (<side>)`.
- Diff view gutter: comment marker `●` on lines covered by a comment, on the side the comment was made on.
- Explorer: `[n]` count per file.

## Tests

- Editor buffer unit tests: insert, newline in mid-line, backspace joining lines, paste with newlines, Up/Down across a wrapped line.
- Flow unit test: create from a VisualLine target → comment exists with prefilled fenced body; close with body cleared → comment removed.
- Snapshot: editor modal open over the diff view with a two-line body.

## Acceptance

- In a real repo: the §11 examples (`Enter`, `V Enter`, `w w v w Enter`) each produce a comment with the expected prefill; Enter on a commented line reopens it; `D` deletes after confirmation; overlapping creation is refused with the message; markers and counts update.
- fmt, clippy, tests clean.

## Handoff notes

### Public interfaces

- `ui::editor::Editor { pub id: u32, pub title: String, .. }`: `new(id, title, body: &str)` (cursor at the end), `key(KeyEvent) -> bool` (text changed), `paste(&str, tab_width)`, `text() -> String`, `resize(width, height)`, `draw(frame, area, theme, focused)`; `impl vim::motion::TextView` (unused in v1). Free fns `rect(area)` (box, 70% x 50%) and `inner(area)` (text area, used by the app to size the editor).
- `app::state::Modal` gained `Editor(Editor)`, `DeleteComment(u32)`, `CloseComment`.
- `app/comment.rs` (new): `App::{comment(), delete_comment(), editor_key(KeyEvent), close_editor()}` (`pub(super)`), `pub fn App::paste(&str)` (no-op unless the editor is on top); `pub fn side_path(&ChangedFile, Side) -> &Path`; private `anchor(mode, view, &ChangedFile)` (moved from `actions.rs` with its test).
- `App::editing() -> bool` (editor anywhere on the stack; status line shows `INSERT`), private `App::view_area() -> Rect`.
- `git::ChangedFile::old_path() -> &Path` (`from` for renames).
- `comments::model::CommentSet::count_for(old: &Path, new: &Path)` (task 06 module change: was `count_for(file)`); counts old-side comments on `old`, new-side on `new`.
- `diff::align::expand_tabs` is now `pub` (used by paste).

### Decisions and deviations (developer decisions; spec §5, §10, §12 updated)

- **Editor on the modal stack**, mode stays `Normal` underneath. `App::handle_key` routes keys straight to `editor_key` when the editor is the top modal (no `KeyParser`); Esc pushes `CloseComment`, Ctrl-q applies `Action::Quit`, key release events are ignored. Close/Delete/Quit modals go through the normal parser path.
- **Old side of a rename anchors to the old path** (`side_path`). Gutter markers and explorer counts look up by side path.
- **Paste**: `\r\n`/`\r` -> `\n`, tabs expanded per pasted line (tab stops relative to the pasted line, not the buffer line). Deviation from spec "verbatim". Tab key unmapped.
- **Full-width line**: gets an empty trailing screen row; a cursor at its end sits there. Up/Down keep a sticky display column; past a row's end they clamp to its last char (the line end on the last row). Home/End are logical line start/end.
- **Real terminal cursor** (`set_cursor_position`), only when the editor is the top modal.
- Prefill is decided by **mode**, not selection kind, so `V Enter` on one line (`Selection::Line`) still prefills.
- New comment flow: `draft` with empty body, `add`, then `get_mut` to set `prefill_body(selected_text)`. Overlap refusal keeps the current (visual) mode.
- `D` on an uncommented line: status "No comment on this line".
- Gutter marker `●` in `theme.modified`. Delete modal "Delete comment #N?" (`Enter: delete / Esc: cancel`); close modal "Close comment?" (`Enter: close / Esc: edit`).
- **Dead code:** allows removed from `mod comments;` and `Config`; `#[allow(dead_code, reason = "used by task 10; remove then")]` on `pub mod store;` only (`render_md` needed none). Task 10 updated.
- **No em-dashes** (developer decision): all replaced with hyphens in docs; title is `Comment - <file>:<lines> (<side>)`.

### Left undone / flagged

- Manual check (scratch repo, GNU screen): §11 examples, reopen, delete, overlap message, markers/counts, rename old side, Ctrl-q and Ctrl-h in the editor. Not verified manually: resize with the editor open (screen did not resize the detached window) and real bracketed paste (unit-tested only).
- Body sync rebuilds the whole body string per keystroke and the editor recomputes wrap for all lines per key; fine for comment-sized text.
