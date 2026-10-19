# Implementation tasks

Reviewy v1 is built one task at a time, in order. Each task is carried out by a fresh agent with no prior context. Everything the agent needs is in this folder, the spec, and the code already written.

| # | Task | Spec sections | Depends on |
|---|---|---|---|
| 01 | [Scaffold](01-scaffold.md) | §4, §5, §13, §14, §15 | — |
| 02 | [Git integration](02-git.md) | §6 | 01 |
| 03 | [Diff engine and alignment](03-diff.md) | §7, §8 | 02 |
| 04 | [Syntax highlighting and themes](04-highlight.md) | §9 | 03 |
| 05 | [Vim engine](05-vim.md) | §11 (vim engine), §10 (wrapping) | 01 |
| 06 | [Comments model and output](06-comments.md) | §12 | 01 |
| 07 | [App, explorer, diff view (row mode)](07-app-row-mode.md) | §4, §10, §11 (row mode) | 02–05 |
| 08 | [Cursor and visual modes](08-cursor-visual.md) | §11 | 07 |
| 09 | [Comment editor and comment lifecycle](09-comment-editor.md) | §10 (editor, modals), §12 | 06, 08 |
| 10 | [Persistence and shutdown](10-persistence.md) | §4 (startup/shutdown), §12 (persistence) | 09 |
| 11 | [Integration tests and polish](11-integration.md) | §16 | 10 |

## Project Basics:

### Principles

- **Stay in scope.** Read this file plus only the files you are pointed to, including the source modules your task builds on. Don't explore the wider project.
- **Be minimalist.** Write the least code and documentation that does the job.
- **Do not write tests for obvious code.** Tests should cover non-obvious behavior, gotchas, edge cases and side effects.

### Style

- Rust edition 2024, toolchain 1.96. 
- Idiomatic Rust.
  - `src/foo.rs` + `src/foo/`, never `mod.rs`.
- Keep modules small and focused on a single struct.
  - for complex modules, split into smaller submodules. Nest these under the main module as `mainmodule/submodule.rs`. Public submodules should be exported via `pub mod submodule;` in the main module.
  - It is okay to have locally used enums, structs, or functions in a single module.
- Explicit over clever, but aim for brevity.
- Cut any line that doesn't earn its place.
- Comment only the non-obvious; don't restate the code.
- One-line docstring only when it states what the signature can't: input format, return shape, or key/complex behavior. 
- `cargo fmt`, `cargo clippy --all-targets` clean (no warnings), `cargo test` green before you finish.
- Errors: `thiserror` enums in library modules, `eyre::Result` in `main`/`app`. `unwrap()` where invariant is obvious.

### Workflow

- All work starts with discussion and follows the following stepwise approach. **ONLY WRITE CODE AT THE CORRECT STEP AS OUTLINED HERE**

Steps:

1. Read allocated task and subsequent files as outlined in the task.
2. Analyze and critique the approach and discuss with the developer.
3. When told to by developer, switch to plan mode and plan current task. When planning, write plan using the Principles and style outlined in this document.
5. Code can be written **when plan is approved**.
6. Developer will review code written and comment on changes.
7. Once dev happy, perform handoff procedure as outlined below. 
8. Dev will authorize commit to main. Each task should be 1 commit. Note no need for branches for this project as it is the initial work.

## Rules for every task

- **Note Project Style**.
- **Spec:** `doc/spec.md` is the source of truth. Read the sections listed in your task before writing code. If the spec and a task file disagree, the task file wins (it may record a later decision); note the conflict in your handoff notes.
- **Follow Workflow for task**
- **Scope:** do only your task. Don't build ahead for later tasks beyond the interfaces your task file asks for. Don't refactor earlier modules unless your task needs it; if you do, say so in your handoff notes.
- **Tests:** write only the tests your task lists. No speculative or exhaustive tests. A test exists to pin down behavior that is easy to get wrong.
- **No async, no extra dependencies** beyond §15 without a note explaining why.
- **Commits:** one per task, straight to `main`, only when the developer authorizes (Workflow step 8). Conventional commit messages (`feat(git): …`).

## Handoff

Each task file ends with a **Handoff notes** section. When you finish:

1. Fill in your task's Handoff notes: public interfaces you created (type/function signatures), decisions you made that the spec did not settle, deviations from the spec, and anything left undone.
2. If a decision changes what a later task must do, edit that later task file directly so it stays correct for an agent with no context.
3. If a decision changes the spec's design, update `doc/spec.md` too.
