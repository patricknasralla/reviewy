# Reviewy

A terminal UI for reviewing a project's uncommitted changes, typically made by an AI agent, and leaving line-anchored review comments for the agent to act on. Think GitHub's PR review UI, but local, fast and vim-driven.

- Side-by-side, whole-file, syntax-highlighted diffs (structural diffing via [difftastic](https://difftastic.wilfred.me.uk/)).
- Comment on a line, a line range or a character range.
- Comments are written as JSON and as Markdown ready to paste into an agent session.

## Requirements

- `git`
- `difft` (difftastic) 0.71.0 or newer, on `PATH` (or set `[difft] binary`)
- A truecolor terminal
- Linux or macOS

## Install

Build from source (Rust 1.96 or newer):

```sh
cargo install --path .
```

## Usage

Run `reviewy` anywhere inside a git repository with uncommitted changes. It compares the working tree (staged and unstaged, plus untracked files) with `HEAD`.

1. Pick a file in the explorer.
2. Move around as in a read-only vim buffer, select lines or text, press Enter to comment.
3. `Ctrl-q`, then Enter, saves the comments and quits.
4. Paste `.reviewy/comments.md` into the agent session, or let a harness read `.reviewy/comments.json`.

## Keys

`Esc` steps back one level (visual, normal, explorer). `Ctrl-q` opens the quit dialog from anywhere. Counts work as in vim (`5j`, `3]`, `5G`).

### Explorer

| Key | Action |
|---|---|
| `j` / `k` | Down/up |
| `gg` / `G` | First/last file |
| Enter | Open file |
| Esc | Back to the open file |

### Diff view (Normal)

| Key | Action |
|---|---|
| `h` `j` `k` `l`, arrows | Move (`j`/`k` by screen row when wrapping) |
| `w` / `b` / `e` | Word motions |
| `0` / `$` | Line start/end |
| `gg` / `G` | First/last line |
| `Ctrl-d` / `Ctrl-u` | Half page down/up |
| `Ctrl-h` / `Ctrl-l` (`Ctrl-Left` / `Ctrl-Right`) | Switch to the old/new column |
| `]` / `[` | Next/previous hunk |
| `gn` / `gp` | Next/previous file |
| `v` / `V` | Visual / visual line |
| Enter | Edit the comment on this line, or create one |
| `D` | Delete the comment on this line |
| Esc | Explorer |

### Visual / visual line

Motions extend the selection. Enter creates a comment with the selection prefilled as a fenced code block. `v`/`V` toggle as in vim; Esc returns to Normal.

### Comment editor

Insert-only: type, Enter, Backspace, Delete, arrows, Home/End, PageUp/PageDown, paste. Changes are kept as you type. Esc asks to close the editor; a comment left empty is removed.

### Dialogs

Enter confirms, Esc cancels.

## Comments

Comments are written to `.reviewy/` in the repository root (the folder ignores itself with its own `.gitignore`):

- `comments.json`: the source of truth.
- `comments.md`: generated for agents, with instructions, the commented lines and their context.

They are saved on quit and when Reviewy receives SIGTERM, SIGHUP or SIGINT. If no comments are left, both files are deleted.

On the next start, if `comments.json` exists, Reviewy asks whether to discard the previous comments (Enter) or keep them (Esc).

## Configuration

Optional TOML files, merged in order:

1. `~/.config/reviewy/reviewy.toml` (global)
2. `<repo>/.reviewy/reviewy.toml` (project)

All keys, with their defaults unless noted:

```toml
[ui]
theme = "Tokyo Night Moon"           # unset by default (bundled Tokyo Night Moon); variant name, or path to a Zed theme JSON (relative to this file, or ~/...)
# theme_variant = "Tokyo Night Moon" # required only when `theme` is a path
wrap = true
tab_width = 4

[ui.tint]                            # 0-1: added/deleted color blended onto the background
line = 0.02                          # changed lines
token = 0.10                         # changed tokens
selection = 0.15                     # selection fallback for themes without players[0].selection

[limits]                             # larger files ask before opening
max_lines = 5000
max_bytes = 1048576
max_line_length = 1000

[ignore]
paths = ["*.lock", "dist/**"]        # repo-relative globs; default []

[comments]
context_lines = 3
# preamble = """
# Custom instructions for the agent...
# """

[difft]
# binary = "difft"
graph_limit = 500000                 # DFT_GRAPH_LIMIT
```

Themes are [Zed](https://zed.dev) theme files. Besides a path, `theme` can name a variant from any family in `~/.config/reviewy/themes/*.json`. Tokyo Night is bundled.

Set `REVIEWY_LOG=debug` to write a log to `.reviewy/reviewy.log`.

## Third-party code

See [THIRD_PARTY.md](THIRD_PARTY.md).
