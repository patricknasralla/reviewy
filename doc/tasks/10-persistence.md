# Task 10 — Persistence and shutdown

## Goal

Load previous comments at startup (keep/discard dialog) and save comments on quit and on termination signals.

Read first: `doc/spec.md` §3 (steps 7–8), §4 (startup, shutdown), §10 (modals: Load existing, Quit), §12 (persistence), §14 (signals, panic hook). Read handoff notes in `doc/tasks/06-comments.md`, `07-app-row-mode.md`, `09-comment-editor.md` (and skim the others).

## Context

`comments::store` (task 06) provides `load` and `save` (atomic write, deletes both files when the set is empty). `main.rs` has `// task 10:` markers at the quit and signal exit points. The app keeps comments in `App::comments`.

## Deliverables

- **Startup** (§4 step 8): if `.reviewy/comments.json` exists, load it (`store::load(.reviewy/)` returns `Ok(None)` when it doesn't). On success, show the "Load existing" modal on top of the explorer: "Found N comments from a previous session (base <short sha>). Keep them?" — Enter keeps (replace `App::comments` with the loaded set, keeping its `next_id`; update `base` to the current base commit), Esc discards in memory only. If loading fails (bad JSON, wrong version), show the error in the modal and treat Enter/Esc as discard; never delete the file at startup.
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
- Restart → keep → markers and counts are back; edit and quit → files updated. Restart → discard → quit with no new comments → both files deleted.
- `kill <pid>` with comments open → files written, terminal restored.
- fmt, clippy, tests clean.

## Handoff notes

_To be filled in by the implementing agent._
