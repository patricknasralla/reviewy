# CLAUDE.md

Reviewy is a terminal UI for reviewing a project's uncommitted changes, typically made by an AI agent, and leaving line-anchored review comments for the agent to act on. Think GitHub's PR review UI, but local, fast and vim-driven. Spec: `docs/spec.md`. Tasks: `docs/tasks/`.

## Commands

- Format: `cargo fmt`
- Typecheck: `cargo check`
- Lint: `cargo clippy --all-targets -- -D warnings`
- Test one: `cargo test <name>`
- Test all: `cargo test`

## Principles

These are judgement calls.

- Write the least code and documentation that does the job.
- Explicit over clever.

## Style

- Comment only the non-obvious; don't restate the code.
- One-line docstring only when it states what the signature can't: input format, return shape, or key/complex behaviour.
- Keep modules small, with one responsibility.
- Locally used helpers can stay in the module.
- Split complex modules into nested submodules.
- Rust edition 2024, toolchain 1.96.
- `src/foo.rs` + `src/foo/`, never `mod.rs`.
- One main struct per module. Nest submodules as `foo/bar.rs`, and export public ones with `pub mod bar;` in `foo.rs`.
- Errors: `thiserror` enums in library modules, `eyre::Result` in `main`/`app`.
- `unwrap()` only where the invariant is obvious.
- No async.

## Testing

- Cover non-obvious behaviour, gotchas, edge cases and side effects, not obvious code.

## Ask first

- Adding a dependency.

## Workflow

This project uses the code-skills plugin. The developer runs the skills; don't start them yourself.

- Plan: `/grill` → `/to-spec` → `/to-tasks` → `/finish-up`
- Build: `/implement <task>` → `reviewy` → `/review` → `/respond` → `/finish-up`
- Leave work uncommitted until `/finish-up`, because `reviewy` only shows changes against `HEAD`.

## Git

- Branch off `main` as `<type>/<description>`, e.g. `feat/add-<feature>`, `fix/<bug-description>`, `chore/<task-description>`.
- Don't start a task until its blockers are merged.
- Commit planning docs (`docs/spec.md`, `docs/tasks/`) directly to `main`. If `main` is protected, the developer moves that commit to a branch before pushing.
- Commit only through `/finish-up` or when the developer asks.
- Use Conventional Commits, e.g. `feat: add <feature>`.
- No AI attribution in commits or PRs.
- Don't push or open a PR unless the developer asks.
- PR review is between developers. Agents only help if a developer asks.
