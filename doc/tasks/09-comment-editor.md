# Task 09 — Comment editor and comment lifecycle

## Goal

Let the reviewer create, edit and delete comments: the comment editor widget (insert-only), the editor modal frame, the delete and close-comment confirmations, gutter markers and explorer counts.

Read first: `doc/spec.md` §10 (comment editor, modals), §11 (row mode Enter/`D`), §12 (model, lifecycle). Read handoff notes in `doc/tasks/06-comments.md`, `07-app-row-mode.md`, `08-cursor-visual.md` (and skim the others).

## Context

Task 08 builds a `comments::model::Anchor` when Enter is pressed in Cursor/Visual/VisualLine mode. `comments::model` (task 06) provides `CommentSet`, `draft`, `prefill_body`, `overlaps`, `at`. `App` already has an empty `comments: CommentSet` field (base `HEAD` + startup SHA); the explorer already shows `[n]` via `count_for`, and `ui::diff_view` has a blank one-char comment-marker slot in each gutter (`GUTTER_EXTRA`). Comments live in memory only in this task; saving is task 10.

## Deliverables

### `ui/editor.rs`: comment editor widget

- Buffer `Vec<String>` plus cursor `(line, char col)`; soft-wrapped with `vim::wrap` at the box's inner width; scrolls to keep the cursor visible.
- Insert-only (v1): printable chars, Enter (newline), Backspace (joins lines at col 0), Delete, arrows (Up/Down by screen row), Home/End, PageUp/PageDown. Bracketed paste (`Event::Paste`) inserts text verbatim (normalize `\r\n` → `\n`).
- Implement `vim::TextView` over the buffer (unused in v1; it's what v1.1 modal editing builds on).
- Every edit writes the buffer back to the comment's `body` immediately (no save step).

### Flows (`app/actions.rs`)

- **Create** (Enter in Cursor/Visual/VisualLine): if `overlaps(...)` returns an id → status "Overlaps comment #N" and stay in the current mode. Otherwise build a draft with `draft(anchor, language, side_lines, context_lines, body)` (`side_lines` = the active `SideText::lines`, `context_lines` from config, language from `highlight::languages::detect(...).name()`). For visual modes (`Lines`/`Chars`) set `body = prefill_body(&selected_text, language)`, where `selected_text` comes from `Comment::selected_text()` (build the draft with an empty body, read it, or add then fill via `get_mut`); cursor mode (`Line`) starts empty. Add it to the set, open the editor with the cursor on the line after the fence (or at the start for an empty body). Return to Row mode underneath.
- **Edit** (Enter in Row mode on a line that has a comment on the active side, via `at(...)`): open the editor on that comment, cursor at the end.
- **Delete** (`D` in Row mode on a commented line): "Delete comment #N?" modal; Enter removes it. `D` on an uncommented line: status message.
- **Close editor** (Esc): "Close comment?" modal; Enter closes, Esc returns to editing. On close, if the body is empty or whitespace, remove the comment.
- `Ctrl-q` inside the editor still opens the quit modal.
- Remove the `#[allow(dead_code)]` on `Config` in `config.rs` once `comments.context_lines` is read.
- Remove the `#[allow(dead_code, reason = "used by task 09; remove then")]` on `mod comments;` in `main.rs` (task 06); `store` stays unused until task 10, so move an `#[allow]` onto `pub mod store;` if needed.

### Rendering

- Editor modal: centered box (~70% width, ~50% height) over the diff view, titled `Comment — <file>:<start>[-<end>] (<side>)`.
- Diff view gutter: comment marker `●` on lines covered by a comment, on the side the comment was made on.
- Explorer: `[n]` count per file.

## Tests

- Editor buffer unit tests: insert, newline in mid-line, backspace joining lines, paste with newlines, Up/Down across a wrapped line.
- Flow unit test: create from a VisualLine target → comment exists with prefilled fenced body; close with body cleared → comment removed.
- Snapshot: editor modal open over the diff view with a two-line body.

## Acceptance

- In a real repo: the §11 examples (`Enter Enter`, `Enter V Enter`, `Enter w w v w Enter`) each produce a comment with the expected prefill; Enter on a commented line reopens it; `D` deletes after confirmation; overlapping creation is refused with the message; markers and counts update.
- fmt, clippy, tests clean.

## Handoff notes

_To be filled in by the implementing agent._
