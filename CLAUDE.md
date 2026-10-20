# Overview

Reviewy is a terminal UI for reviewing a project's uncommitted changes, typically made by an AI agent, and leaving line-anchored review comments for the agent to act on. Think GitHub's PR review UI, but local, fast and vim-driven.

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
- There are two workflows:
  1. A discussion is initiated.
  2. A task file is given to read.

Steps for discussion:

1. **NO CODE IS TO BE WRITTEN THIS SESSION**
2. The developer will provide an outline of the work with a basic list of requirements.
3. Discuss the outline and architecture, ask questions and push back on assumptions. This will continue until we are happy with the structure and tasks. If needed, source files can be explored at this point.
4. At this point, the developer will ask for a document. This should be created in `/doc/tasks` and include all information so that a clean instance could carry out the task (including relevant source files to read).
5. Once the developer is happy with the document, commit.

Steps when given a task file:

1. Read the allocated task and subsequent files as outlined in the task.
2. Analyze and critique the approach and discuss with the developer.
3. When told to by developer, switch to plan mode and plan current task. When planning, write plan using the Principles and style outlined in this document.
4. Code can be written **when plan is approved**. Code should be written in new branch. If work spreads over multiple tasks, these should be separate commits in this branch.
5. Developer will review code written and comment on changes using Reviewy. Read comments md document in the `.reviewy` directory and respond as per document.
6. Once dev happy, perform handoff procedure as outlined below.
7. Dev will authorize commit. Each task should be 1 commit.

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
4. If decisions change the app's function, `README.md` should also be updated to match this.
