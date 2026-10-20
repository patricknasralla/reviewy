# Task 11 — Integration tests and polish

## Goal

Add the end-to-end tests from the spec, fix what they and a manual review session find, and tidy the v1 release.

Read first: `doc/spec.md` §16 (testing), then the whole spec once. Read handoff notes in **all** previous task files; collect every "left undone" item.

## Deliverables

- **Integration test** (`tests/pipeline.rs`): build a temp git repo with `tempfile`, commit files, then make one change of each kind — modified, staged add, delete, rename (staged), untracked, binary, mode-only. Run the non-UI pipeline end to end: `git::changed_files` → for each viewable file `read_content` → `DifftEngine::diff` → `align::build` → `highlight::apply`. Assert statuses, markers and that each viewable file yields a `FileDiff` whose rows cover both sides' lines exactly once. Requires `git` and `difft`; skip with a printed note if `difft` isn't on `PATH` (don't fail).
- **Persistence round trip** (`tests/comments.rs`): save a set into a temp `.reviewy/`, load it back, compare.
- Tabs in comments (task 06): excerpts and cols come from tab-expanded `SideText::lines`. Fix: keep raw (CR-stripped, unexpanded) lines in `SideText`, pass them to `comments::model::draft`, and map display col -> raw char col when building `Selection::Chars` (callers only: `anchor` in `app/actions.rs` and task 09's `draft` call; `comments/` unchanged). Do it or list it as a follow-up.
- Resolve "left undone" items from earlier handoff notes, or list them in a `doc/tasks/v1-followups.md` with a reason.
- Third-party notices: `THIRD_PARTY.md` crediting the grammar crates (MIT; `tree-sitter-typescript` ships no LICENSE file) and the bundled Tokyo Night theme (`3rd_party/tokyo-night.LICENSE`).
- `README.md`: what it is, requirements (`git`, `difft` ≥ 0.71.0, truecolor terminal), install, key reference (from §11), config example (§13), where comments are written.

## Manual review checklist

Run in a real repo with a realistic agent-sized change (10–20 files) and check: startup time, open time for the largest file, wrap on/off, every §11 key, all modals, comment create/edit/delete, quit/save/reload, SIGTERM. Record findings and fixes in handoff notes.

## Acceptance

- `cargo test` green (integration test runs where difft is installed).
- fmt, clippy clean. `cargo build --release` binary works from another directory inside a repo.

## Handoff notes

_To be filled in by the implementing agent._
