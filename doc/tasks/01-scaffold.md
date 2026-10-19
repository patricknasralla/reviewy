# Task 01 — Scaffold

## Goal

Create the crate and a running, empty TUI: it starts in a git repo, draws a blank screen, and quits cleanly on `Ctrl-q`. Implement configuration loading in full. Later tasks fill in the modules.

Read first: `doc/spec.md` §4 (architecture, CLI, event loop, startup, shutdown), §5 (module layout), §13 (configuration), §14 (errors, signals, logging), §15 (dependencies).

## Context

Reviewy is a terminal UI for reviewing a git repo's uncommitted changes side by side (difftastic as the diff engine) and leaving line-anchored comments for an AI agent. This task builds only the skeleton.

## Deliverables

### Crate

- `Cargo.toml`: single binary crate `reviewy`, edition 2024. Add all dependencies from §15 now, at their latest versions, except the highlighting crates, which must use the exact versions listed in §15 (they were verified together). Enable `clap` derive, `serde` derive, `crossterm` as ratatui's backend.
- Module files from §5 (no `mod.rs`; directory modules use a sibling `foo.rs`), created as empty stubs (`//! <one-line purpose>`) so later tasks only fill them in. Add `mod` declarations so the tree compiles.

### `main.rs`

- `clap` derive struct: `--help`, `--version`, and a trailing `DIFF_ARGS: Vec<String>` positional. Any value in `DIFF_ARGS` → exit with error "diff arguments are not supported in v1".
- Startup, in this order (later tasks add the remaining steps from §4; leave a `// task NN:` comment where each will go):
  1. Install `color-eyre`, then a panic hook that restores the terminal (leave alternate screen, disable raw mode, disable bracketed paste) before delegating to color-eyre's hook.
  2. Register SIGTERM, SIGHUP, SIGINT with `signal_hook::flag::register` onto one `Arc<AtomicBool>`.
  3. Resolve the repo root with `git rev-parse --show-toplevel`. Not a repo → error and exit. Put this in `git.rs` as `pub fn repo_root() -> Result<PathBuf, GitError>`; task 02 adds the rest of `git.rs`.
  4. Load config (below).
  5. Ensure `<root>/.reviewy/` exists and contains `.gitignore` with content `*\n` (create both if missing, don't overwrite an existing `.gitignore`).
  6. Init `tracing-subscriber` writing to `<root>/.reviewy/reviewy.log`, level from env var `REVIEWY_LOG` (an `EnvFilter` directive); default `warn`. Never log to stdout/stderr.
  7. Enter the terminal: raw mode, alternate screen, bracketed paste.
- Event loop as in §4: `crossterm::event::poll(250ms)`, redraw only when `app.dirty` or on resize, check the signal flag every iteration. For this task the `App` is a minimal stub in `app/state.rs` with `dirty` and `should_quit`; `Ctrl-q` sets `should_quit` directly (the quit confirmation modal comes in task 07). The draw function renders a blank screen with the theme-less default background.
- Shutdown: restore the terminal. Leave a `// task 10: save comments` comment where saving goes (both on quit and on signal).

### `config.rs`

- `Config` struct mirroring §13 exactly: `ui { theme, theme_variant, wrap, tab_width }`, `limits { max_lines, max_bytes, max_line_length }`, `ignore { paths }`, `comments { context_lines, preamble }`, `difft { binary }`. Defaults as in §13 (`theme = "Tokyo Night Moon"`, `wrap = true`, `tab_width = 4`, `max_lines = 5000`, `max_bytes = 1048576`, `max_line_length = 1000`, `paths = []`, `context_lines = 3`, `preamble = None`, `binary = "difft"`).
- Load order: defaults, then `<XDG config dir>/reviewy/reviewy.toml` (via `etcetera`, `choose_base_strategy`), then `<root>/.reviewy/reviewy.toml`. Missing files are fine.
- Merge at the `toml::Value` level (deep-merge tables, later file wins per key; arrays replace, not append), then deserialize the merged value into `Config` once. Unknown keys are an error (`#[serde(deny_unknown_fields)]`) with the file path in the message, so typos are caught.
- `pub fn load(repo_root: &Path) -> eyre::Result<Config>`, plus an inner function that takes the two TOML sources as `Option<&str>` so it can be tested without files.

## Tests

`config.rs` only:
- No files → defaults.
- Global sets `ui.tab_width = 8` and `ui.wrap = false`, project sets `ui.tab_width = 2` → `tab_width = 2`, `wrap = false`.
- Project `ignore.paths` replaces global `ignore.paths`.
- Unknown key → error.

## Acceptance

- `cargo run` inside a git repo shows a blank screen; `Ctrl-q` exits and the terminal is restored. `Ctrl-c` does nothing.
- `cargo run` outside a git repo prints an error and exits non-zero.
- `cargo run -- HEAD~1` exits with the "not supported in v1" error.
- `kill <pid>` (SIGTERM) exits cleanly and restores the terminal.
- `.reviewy/.gitignore` exists after a run; `.reviewy/reviewy.log` is created.
- fmt, clippy, tests clean.

## Handoff notes

_To be filled in by the implementing agent._
