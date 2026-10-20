# Task 10 - Persistence and shutdown

## Goal

Load previous comments at startup (keep/discard dialog) and save comments on quit and on termination signals.

Read first: `doc/spec.md` §3 (steps 7–8), §4 (startup, shutdown), §10 (modals: Load existing, Quit), §12 (persistence), §14 (signals, panic hook). Read handoff notes in `doc/tasks/06-comments.md`, `07-app-row-mode.md`, `09-comment-editor.md` (and skim the others).

## Context

`comments::store` (task 06) provides `load` and `save` (atomic write, deletes both files when the set is empty). `main.rs` has `// task 10:` markers at the quit and signal exit points; the confirmed-quit point is `// task 10: save` in `App::apply` (`Action::ModalConfirm` on `Modal::Quit`, `app/actions.rs`). `App` has no `base_commit` field (task 07): the current base SHA is `app.comments.base.commit` - read it before replacing `comments` with a loaded set. Add the "Load existing" variant to `app::state::Modal` and its text to `ui/modals.rs` (`confirm`). The app keeps comments in `App::comments`. Since task 09 the comment editor is `Modal::Editor` on the same stack (with `DeleteComment(id)`, `CloseComment`); `App::handle_key` sends keys to the editor when it is on top, so push "Load existing" before any editor can exist (startup), and `App::paste` only acts when the editor is on top. `#[allow(dead_code)]` sits on `pub mod store;` in `comments.rs` (remove it).

## Deliverables

- **Startup** (§4 step 8): if `.reviewy/comments.json` exists, load it (`store::load(.reviewy/)` returns `Ok(None)` when it doesn't). On success, show the "Load existing" modal on top of the explorer: "Found N comments from a previous session (base <short sha>). Discard them?" - Enter discards in memory only, Esc keeps (replace `App::comments` with the loaded set, keeping its `next_id`; update `base` to the current base commit). (Developer decision; was "Keep them?" with Enter = keep.) If loading fails (bad JSON, wrong version), show the error in the modal and treat Enter/Esc as discard; never delete the file at startup.
  - The modal must be on top of the stack before any other input is handled.
  - Until the reviewer answers it, save points must **not** call `store::save`: the in-memory set is still empty, and saving it would delete the previous `comments.json`. A signal or quit while the modal is open exits without touching the files.
- **Save points:** confirmed quit (`Ctrl-q` → Enter) and the signal flag (SIGTERM/SIGHUP/SIGINT). Both call one `App::save()` → `store::save(.reviewy/, &comments, &config.comments.preamble)`. The comment editor's current buffer is already in the model (task 09), so an open editor is saved as-is; empty-body comments are dropped before saving.
- Save errors: restore the terminal first, then print the error to stderr and exit non-zero, so the reviewer sees it and nothing is lost silently.
- Remove the `#[allow(dead_code)]` on `CommentsConfig` in `config.rs` once `preamble` is read.
- Panics: no save (accepted for v1). Make sure the panic hook from task 01 still restores the terminal with the editor open.

## Tests

- Unit: `App::save` with an empty-body comment in the set → that comment is absent from the written JSON.
- Unit: keeping a loaded set preserves `next_id` and comment ids.

## Acceptance

- Create comments, quit → `comments.json` and `comments.md` written; `comments.md` reads correctly for an agent.
- Restart → Esc (keep) → markers and counts are back; edit and quit → files updated. Restart → Enter (discard) → quit with no new comments → both files deleted.
- `kill <pid>` with comments open → files written, terminal restored.
- fmt, clippy, tests clean.

## Handoff notes

### Public interfaces

- `app::state::Modal::LoadExisting(Result<CommentSet, String>)`: the loaded set, or the load error's display text.
- `app/persist.rs` (new): `pub fn App::load_existing(Result<Option<CommentSet>, CommentsError>)` (pushes the modal unless `Ok(None)`), `pub(super) fn App::keep(CommentSet)` (sets `base` to the current one, swaps the set in), `pub fn App::save(&self) -> Result<(), CommentsError>`.
- `ui::modals::confirm` sizes the box with `Paragraph::line_count` (ratatui feature `unstable-rendered-line-info`, added to `Cargo.toml`).

### Decisions and deviations (developer decisions; spec §5, §10, §12, §15 updated)

- **"Discard them?"**: Enter discards, Esc keeps (the user usually wants to discard). The message pluralizes ("1 comment ... Discard it?"). Load error: the error text, hint `Enter/Esc: discard (file replaced on exit)`; the bad file is overwritten or deleted on exit (accepted).
- **Save in `run()`, not `App::apply`** (deviation from this task's Context): `ModalConfirm` on `Quit` only sets `should_quit`; both exit points in `run()` call `app.save().wrap_err("saving comments")?`, so a save error reaches stderr after `main` restores the terminal (exit 1).
- **`save(&self)` is Ctrl-s ready**: it filters empty-body comments (`body.trim()`) into a copy, leaving the in-memory set (and an open editor's comment) alone, and returns the error to the caller. A future Ctrl-s action can call it and show the error in the status line. The "Load existing" guard is inside `save`, covering every caller.
- **Guard checks the whole stack**: Ctrl-q can push Quit over "Load existing"; quitting or a signal then exits without touching the files.
- **`confirm` height fix** (task 07 module change): the old chars/width estimate was too short for word-wrapped text (long error paths), clipping the hint.
- No current-sha display in the modal when the base differs: stale anchors are a v1.1+ topic.
- Dead code allows removed from `pub mod store;` and `CommentsConfig`.

### Left undone / flagged

- Manual check (scratch repo, GNU screen): quit writes both files; keep/edit/quit updates them; discard/quit deletes them; Ctrl-q and SIGTERM with the modal open leave the files untouched; SIGTERM with the editor open saves its buffer; read-only `.reviewy/` gives the error on stderr and exit 1; broken JSON shows the error modal; a temporary panic with the editor open restored raw mode (exit 101). Leaving the alternate screen is not visible in screen; task 11 checks it in a real terminal.
- Save errors print the OS error twice (`CommentsError::Io` displays its source and also returns it from `source()`). Left as is (developer decision).
