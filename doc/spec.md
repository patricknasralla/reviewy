# Reviewy - Specification (v1)

> Working title. Status: spec for v1. Spikes resolved (§17).

## 1. Purpose

Reviewy is a terminal UI for quickly reviewing a project's uncommitted changes - typically changes made by an AI agent - and leaving line-anchored review comments, similar to GitHub's PR review UI.

The goal is the least possible friction between "the agent says it's done" and "I understand the code it wrote". Today the review loop usually runs through PRs, which is slow enough that review gets deferred until the end, producing large, hard-to-review PRs. Reviewy keeps the loop tight: review small changes locally and often, write comments, hand them straight back to the agent.

Design principles:

- **Small and fast.** Tiny footprint, no unnecessary disk I/O, no async runtime.
- **Use what the user has.** Shell out to `git` and `difft`; don't bundle or reimplement them.
- **Editor-like reading experience.** Full-file, syntax-highlighted, themed, vim-style navigation.
- **Simple data structures for the happy path.** Don't design the architecture around crashes.
- **Agent-readable output.** Comments must contain everything an LLM needs to act on them.

## 2. Scope

### v1

- Single command `reviewy`, run in a git repository.
- Diff: all changes since the last commit on the current branch (staged, unstaged and untracked) vs `HEAD`.
- Difftastic (`difft --display json`) as the diff engine.
- Explorer overlay listing changed files (status color, comment count, unviewable markers).
- Side-by-side, two-column, whole-file diff view with tree-sitter syntax highlighting and change backgrounds.
- Languages: Rust, Python, C#, TypeScript (incl. TSX). Other files render as plain text.
- Zed-format themes: bundled themes plus a user themes directory.
- Vim-style modal navigation (normal / visual / visual-line), counts, chords.
- Comment create / edit / delete, with a custom comment editor (vim Normal and Insert modes).
- Comments persisted on exit to `.reviewy/comments.json` (source of truth) and `.reviewy/comments.md` (generated for the agent).
- Configuration via `reviewy.toml` (global + project).

### v1.1 (planned, design must not block these)

- Modal (vim) editing in the comment editor, built on the shared vim engine: Normal/Insert modes and motions (task 13), operators, register and visual mode (task 14), undo/redo (task 15) done.
- Picker for lines covered by multiple overlapping comments (model already supports overlap).
- CLI arguments mirroring `git diff` (ranges, `--staged`, etc.). Argument scaffolding exists in v1 (§4); behavior **to be discussed**.
- Yank selection to the system clipboard. The editor has an internal unnamed register (`vim/register.rs`); named registers and the clipboard build on it.
- Configurable key bindings.
- Search (`/`, `n`, `N`).

### Later

- File-tree explorer (hierarchy instead of flat list).
- `BuiltinEngine` fallback diff (`imara-diff` + word-level pass) when difft is unavailable or fails.
- Inline comment previews in the diff view.
- Stale-comment detection (using the stored base commit).
- File watching / change notification.
- Sending comments directly to an agent session (MCP, harness integration).
- Syntax highlighting inside comment bodies; a "suggested change" interface.
- Crash-safe persistence of in-progress comments.
- Single-column diff view.
- Mouse interaction.
- Merge conflict resolution UI.
- Word-boundary soft wrap (wrap before a word instead of splitting it; needs care with markdown and symbols). Shared by the diff view and the editor (`vim/wrap.rs`).
- Fix the sticky column under wrap: `motion::apply` stores `Cursor::want` relative to the screen segment, but logical vertical motions (`Down`/`Up` = `gj`/`gk`, `HalfPageDown`/`HalfPageUp`) read it as a whole-line column, so they land on the wrong column after the cursor sat on a later segment. Diff view and editor.

## 3. Usage workflow

1. An agent modifies files in a project and reports that it's done.
2. The developer runs `reviewy` in the project directory.
3. Reviewy starts with the explorer overlay open over two blank columns.
4. The developer selects a file; the overlay closes and the file is shown side by side (old left, new right), whole file, syntax-highlighted, with changed regions on a tinted background.
5. The developer navigates like a read-only vim buffer, selects lines or text and presses Enter to comment. The selection (if any) is pasted into the comment box as a fenced code block. Commented lines get a gutter marker.
6. Moving to a commented line and pressing Enter reopens that comment for editing. `D` deletes it after confirmation.
7. `Ctrl-q` (with confirmation) quits. Comments are written to `.reviewy/comments.json` and `.reviewy/comments.md`. The developer pastes the md into the agent session or a harness reads either file.
8. On the next run, if `.reviewy/comments.json` exists, a dialog asks whether to discard (Enter) or keep (Esc) the previous comments. Either way the files are overwritten on exit.

## 4. Architecture overview

- **Single crate**, binary `reviewy`.
- **Synchronous** main loop. No async runtime.
- **External processes:** `git` (status, cat-file, diff) and `difft` (via `GIT_EXTERNAL_DIFF` or directly).
- **Lazy per-file work:** a file's diff, content and highlighting are computed when it is first opened and cached for the session.
- **One shared vim engine** (key parsing, motions, soft-wrap layout) used by both the diff view and the comment editor.
- **Plain in-memory state:** `App` owns everything, including the `CommentSet`. No shared ownership, no locks.

### CLI

```
reviewy [OPTIONS] [DIFF_ARGS]...
```

- v1 supports only `--help` and `--version`.
- The `clap` definition reserves trailing positional `DIFF_ARGS` for future `git diff`-style arguments (revisions, ranges, `--staged`, pathspecs). In v1, any value there exits with "not supported in v1".
- The pipeline takes a `DiffTarget` rather than hard-coding `HEAD`. v1 has a single variant, `WorkingTreeVsHead`; v1.1 adds variants without rethreading the code.

### Event loop

```
loop {
    if terminate.load() { app.save()?; break; }
    if event::poll(min(250 ms, time to next blink))? {
        match event::read()? {
            Key(key) => app.handle_key(key),   // editor on top: editor_key; else keys.handle → vim::Input → map → Action → apply
            Paste(text) => app.paste(&text),   // only when the editor is on top
            Resize(w, h) => app.resize(w, h),
        }
    }
    app.blink(now);                            // toggles the block cursor when due
    if app.dirty { terminal.draw(|f| ui::draw(f, &app))?; app.dirty = false; }
    if app.pending.is_some() { app.finish_open(); terminal.draw(…)?; }   // "Diffing…" is drawn first
    if app.should_quit { app.save()?; break; }
}
```

- Redraw only when state changed (or on resize).
- `poll` timeout (~250 ms) bounds signal-handling latency. While the block cursor is shown it is shortened to the next blink toggle, so blinks happen at `cursor_blink_ms`, not on 250 ms ticks.

### Startup sequence

1. Install panic hook (restore terminal, then color-eyre report), parse the CLI, install the signal flag.
2. Resolve repo root: `git rev-parse --show-toplevel`. Not a repo → error and exit.
3. Resolve base commit: `git rev-parse HEAD`. No commits → error and exit (v1).
4. Load config (built-in defaults, global, then project override).
   - If `REVIEWY_LOG` is set: create `.reviewy/` (as in step 7) and start logging. Done here so later steps can log.
   - Resolve the theme (§9); a failure falls back to the bundled default and becomes the startup status message.
5. Check `<difft.binary> --version` (the binary path comes from config). Missing or too old → error with install hint and exit.
6. Collect changed files (§6). None → print "No changes" and exit.
7. Ensure `.reviewy/` exists and contains a `.gitignore` with `*` (ignores the whole folder without touching the user's root `.gitignore`).
8. Build `App`, then `store::load(.reviewy/)`: if `comments.json` exists (or fails to load), push the "Load existing" modal (§10), so it is on top before any input.
9. Enter terminal (raw mode, alternate screen, bracketed paste), show explorer overlay (and the modal).

### Shutdown

- Triggered by confirmed `Ctrl-q` or a termination signal. Both call `App::save()` from the event loop (§12).
- If comments exist: write `comments.json` and `comments.md` (write-to-temp-then-rename).
- If no comments: delete both files.
- Restore terminal. A save error is returned from the loop, so it is printed to stderr after the terminal is restored and the exit code is non-zero.

## 5. Module layout

```
src/
  main.rs              hooks/signals, terminal setup/teardown, event loop
  cli.rs               clap args
  config.rs            reviewy.toml loading & merging
  defaults.rs          declares submodules
  defaults/
    defaults.toml      built-in defaults (embedded, first merge layer)
    default_theme.rs   bundled theme family JSON and default variant name
    tokyo-night.json   bundled theme family (embedded)
  git.rs               GitError, DiffTarget, repo root, base SHA; declares submodules
  git/
    status.rs          changed files: status/numstat parsing, classification, binary detection, ignore globs
    content.rs         file content (both sides), large-file check
  diff.rs              DiffEngine trait; declares submodules
  diff/
    model.rs           FileDiff, Row, LineKind, change spans
    difft.rs           DifftEngine: spawn, parse JSON, version check
    align.rs           build whole-file aligned rows from difft output
  highlight.rs         declares submodules; re-exports Highlighter, Theme
  highlight/
    languages.rs       language registry: extension → grammar + queries
    highlighter.rs     tree-sitter-highlight → per-line style spans
    theme.rs           Zed theme JSON parsing, alpha blending, capture fallback, bundled themes
  comments.rs          declares submodules
  comments/
    model.rs           CommentSet, Comment, Anchor, Selection, Excerpt, draft, prefill
    store.rs           JSON load/save
    render_md.rs       preamble + markdown rendering
  vim.rs               declares submodules
  vim/
    keys.rs            key/chord/count/operator parser → vim::Command
    motion.rs          Motion enum, TextView trait, motion application
    register.rs        Register, Registers (unnamed only), owned by App
    selection.rs       visual selection bounds and per-line ranges (diff view and editor)
    wrap.rs            soft-wrap layout (shared by diff view, editor, gj/gk)
  app.rs               App struct; declares submodules
  app/
    state.rs           Mode, modal stack, pending file open
    view.rs            FileView: per-file diff, text cursor, active/home side, scroll, row layout
    actions.rs         Action enum, vim::Command → Action mapping, App::apply
    open.rs            open-file pipeline (content, limits, difft, align, highlight)
    comment.rs         comment flows: create/edit/delete, editor input, comment target
    persist.rs         load existing (keep/discard) and App::save
  editor.rs            Editor: comment editor model, vim modes, key handling, wrap layout; declares submodules
  editor/
    buffer.rs          Buffer: lines with position-based insert/delete/slice, TextView
    operator.rs        operator spans (motion/line/selection) and d/c/y, p/P
    undo.rs            Undo: capped undo/redo stacks of buffer + cursor snapshots
  ui.rs                declares submodules; top-level draw(), drawn block cursor
  ui/
    explorer.rs        explorer overlay
    diff_view.rs       two-column rendering, gutter, cursor/selection
    editor.rs          comment editor box: frame, wrapped rows, cursor (rendering only)
    modals.rs          confirm, large file, load existing, comment editor frame
    status.rs          status line / transient messages
```

No `mod.rs` files: each directory module has a sibling `foo.rs`.

## 6. Git integration

All git access is via the `git` CLI (`std::process::Command`), run from the repo root.

### Changed files

`git status --porcelain=v2 -z --untracked-files=all` (with `-M` rename detection as git provides by default for staged renames), run with `GIT_OPTIONAL_LOCKS=0` so its index refresh never blocks a concurrent git command. Parse into:

```rust
enum FileStatus {
    Modified,
    Added,
    Deleted,
    Renamed { from: PathBuf },
    Untracked,
    Conflicted,   // unmerged entry
    Submodule,
    ModeOnly,     // file mode changed, content identical
}

struct ChangedFile {
    path: PathBuf,        // repo-relative, new path for renames
    status: FileStatus,
    binary: bool,
}
```

- Staged and unstaged changes are treated identically: the comparison is always working tree vs `HEAD`.
- Untracked files are listed as new (`Untracked` is shown with the same color as `Added`).
- Unstaged renames appear as a delete plus an untracked file. Accepted.
- Files matching `ignore` globs from config are dropped.
- Conflicted (unmerged) entries, submodules and mode-only changes are listed but not viewable in v1, like binary files. Markers: `C` conflicted, `S` submodule, `P` mode-only (permissions). Enter shows a status message. Conflict resolution is planned for a later version.

Porcelain v2 record types (with `-z`):

- `1 <XY> <sub> <mH> <mI> <mW> <hH> <hI> <path>` - ordinary change. Classified by modes, not XY, since only HEAD vs worktree matters: `mH` and `mW` both `000000` → dropped (no change vs HEAD, e.g. `AD`); `mH` zero → `Added` (includes intent-to-add `.A`); `mW` zero → `Deleted`; otherwise `Modified`.
- `2 <XY> <sub> <mH> <mI> <mW> <hH> <hI> <score> <path>\0<origPath>` - rename/copy; the original path is the next NUL-separated field.
- `u <XY> <sub> ...` - unmerged → `Conflicted`.
- `? <path>` - untracked.
- `<sub>` starting with `S` (e.g. `S...`) → `Submodule`; `N...` is a normal file.

### Numstat pass

Once at startup, after status: `git diff HEAD --numstat -z` (one call for all tracked files). Per path:

- `-\t-` → `binary = true`.
- `0\t0` with `mH != mW` (or `mH != mI`) and not a rename → `ModeOnly`.
- Untracked files are not included in numstat output (see binary detection).

Rename records in `-z` numstat output are `0\t0\t\0<old>\0<new>\0` (empty path field followed by both paths).

### Binary detection

- Tracked files: from the numstat pass (`-\t-`). No blobs are read at startup.
- Untracked files: `git diff --no-index --numstat -z -- /dev/null <path>` per file (exits 1 on difference; that is success here). Same `-\t-` rule, so `.gitattributes` applies to untracked files too. On failure (e.g. unreadable file) log a warning and treat as text.
- Binary files are listed with a `B` marker and are not openable (status message on Enter). This filter is required: git passes binary files to difft, which then diffs them as text.

### File content

- Old side: `git cat-file blob HEAD:<path>` (or `HEAD:<from>` for renames), captured to memory.
- New side: read the working-tree file. A symlink's content is its target path (as git stores it), not the target's content.
- Added/untracked files have no old side; deleted files have no new side.
- `\r\n` is normalized to `\n` for display; tabs expanded to `tab_width` spaces.
- `git cat-file blob HEAD:<missing>` exits 128 (`fatal: path '…' does not exist in 'HEAD'`); treat as a per-file error.

### Large files

Before diffing, check both sides against the limits:

| Limit | Default |
|---|---|
| `max_lines` | 5000 |
| `max_bytes` | 1 MiB |
| `max_line_length` | 1000 chars |

If any limit is exceeded, show a modal: "Large file (N lines, X KB, longest line L). View anyway?" (each number is the max over both sides) - Enter views, Esc cancels. `max_line_length` catches minified/obfuscated files that are few lines but huge.

## 7. Diff engine

### Trait

```rust
trait DiffEngine {
    fn diff(&self, root: &Path, target: DiffTarget, file: &ChangedFile) -> Result<RawDiff, DiffError>;
}
```

v1 has one implementation, `DifftEngine`. A `BuiltinEngine` fallback may follow later behind the same trait. Everything downstream consumes the internal model (§8), never difft JSON.

### Invocation

Environment for all difft runs: `DFT_UNSTABLE=yes`, `DFT_DISPLAY=json` (and `--display json` when calling directly), `DFT_GRAPH_LIMIT=<difft.graph_limit>` (§13).

- **Tracked changes** (modified, deleted, staged-added):
  ```
  GIT_EXTERNAL_DIFF=difft DFT_DISPLAY=json DFT_UNSTABLE=yes git diff HEAD -- <path>
  ```
  git materializes the old blob as a temp file and passes the 7-argument external diff convention to difft. `GIT_EXTERNAL_DIFF` takes precedence over any user `diff.external` config. No temp-file handling in Reviewy.
- **Renames:** the pathspec must contain **both** paths: `git diff HEAD -- <from> <path>`. With only the new path, git does not detect the rename and difft reports `created`. With both, git uses the 9-argument form and the JSON `path` is the new path.
- **Untracked files:**
  ```
  DFT_UNSTABLE=yes difft --display json /dev/null <path>
  ```
  Mirrors what git passes for an added file. difft reports `"status": "created"` (an empty file reports `"unchanged"`). Without `DFT_UNSTABLE=yes` difft exits 2.

If git prints nothing (exit 0), the worktree matches `HEAD` (e.g. a staged change reverted in the worktree): git never calls difft, and the result is treated as `unchanged`.

Results are cached per file for the session. difft takes ~1 s on a 4000-line file with many changes; since the loop is synchronous, draw a "Diffing…" status message before invoking it.

Cost scales with the size of the changed region, not the file: heavily rewritten files build a large graph, hit `DFT_GRAPH_LIMIT` and fall back to a line diff. With difft's default limit (3,000,000) a 300-line rewrite spends ~1.5 s before falling back; `difft.graph_limit = 500000` gives the same output in ~250 ms, while moderately changed files keep the structural diff.

### Difft JSON (observed shape, 0.71.0)

```json
{
  "path": "src/foo.rs",
  "language": "Rust",
  "status": "changed",
  "aligned_lines": [[0, 0], [null, 1], [1, 2], [2, 3], [3, null], [4, 4]],
  "chunks": [
    [
      { "rhs": { "line_number": 1, "changes": [ { "start": 0, "end": 3, "content": "use", "highlight": "keyword" } ] } },
      { "lhs": { "line_number": 2, "changes": [] },
        "rhs": { "line_number": 3, "changes": [ { "start": 31, "end": 37, "content": "Result", "highlight": "type" } ] } }
    ]
  ]
}
```

- `status`: `changed`, `created`, `deleted`, `unchanged`. Only `changed` has `aligned_lines` and `chunks`.
- `aligned_lines`: whole-file row alignment, `[lhs | null, rhs | null]`, 0-based line indices. It always ends with one phantom pair one past the last line (with or without a trailing newline).
- `chunks`: only lines with changes. `line_number` is 0-based, like `aligned_lines`. A side may appear with `changes: []` (that line is paired with a changed line but has no changed tokens itself).
- `changes[].start/end`: **byte** offsets into the **raw** line (before tab expansion; `\t` is one byte). Zero-width spans (`start == end`) occur.
- `language`: not reliable (e.g. `"Text (exceeded DFT_GRAPH_LIMIT)"`, `"Text (2 Python parse errors, …)"`). Ignored; language is detected by extension (§9).
- When difft falls back to line-based diffing (parse errors, graph limit) the shape is unchanged.
- `highlight` and `content` fields are not used.

The schema is unstable; parsing is isolated in `difft.rs` and should be tolerant (unknown fields ignored).

### Minimum difft version

Minimum version: **0.71.0**. Checked at startup by parsing the first line of `difft --version` (format: `Difftastic 0.71.0`; the first word reading as `X.Y.Z` is used, so prefix changes don't break it). If difft is missing or too old: hard error with install hint (v1 is a personal tool; the user ensures it's installed).

## 8. Diff model and alignment

```rust
struct FileDiff {
    lhs: Option<SideText>,       // None for added/untracked
    rhs: Option<SideText>,       // None for deleted
    rows: Vec<Row>,              // aligned whole-file rows
    hunks: Vec<RowRange>,        // for ] / [
}

struct SideText {
    lines: Vec<String>,
    kinds: Vec<LineKind>,        // per line
    changes: Vec<Vec<Range<usize>>>, // per line, changed token spans: byte ranges into the tab-expanded `lines[i]`
    highlights: Vec<Vec<StyledSpan>>, // per line, from highlighter
}

enum LineKind { Unchanged, Changed, Added, Removed }

struct Row { lhs: Option<u32>, rhs: Option<u32> } // 0-based line indices
```

Alignment (`align.rs`):

- `changed`: rows come directly from difft's `aligned_lines`. Any index ≥ that side's line count becomes `None`; rows that become `(None, None)` are dropped (removes the phantom trailing pair).
- Line kinds from chunks: a line listed in a chunk whose row has both sides → `Changed`; listed with no opposite side → `Removed` (lhs) / `Added` (rhs). Lines not in any chunk → `Unchanged`, **including unpaired ones** (e.g. difft treats reformatting - one line split into several - as unchanged; the extra lines get padding opposite but no tint).
- Change spans: difft's raw byte offsets are converted to byte offsets in the tab-expanded line when the file is loaded. Zero-width spans are dropped. Nothing downstream sees raw offsets.
- `created` / untracked: all rows `{ lhs: None, rhs: Some(i) }`, all lines `Added`. `deleted`: mirror.
- `unchanged` (pure rename, empty untracked file): rows paired 1:1, all `Unchanged`.
- Hunks are maximal runs of rows containing any non-`Unchanged` line.
- Sides come from the file content (§6), rows and kinds from difft. If they disagree (file edited between reading and diffing), out-of-range indices and spans are clamped and a warning is logged; lines may then be missing from the rows. Accepted: the workflow is review, close, then edit.
- Tab stops are measured in display columns (`unicode-width`). Spans are snapped to char boundaries (lossy UTF-8 decoding can shift difft's byte offsets).

## 9. Syntax highlighting and themes

### Highlighting

- Crates: `tree-sitter`, `tree-sitter-highlight`, grammar crates `tree-sitter-rust`, `tree-sitter-python`, `tree-sitter-c-sharp`, `tree-sitter-typescript` (TS + TSX), `tree-sitter-javascript` (queries only, required by TS). Versions in §15.
- Highlight queries: the grammar crates' bundled queries (MIT). Zed's own `.scm` queries are GPL and are **not** used.
- Language detection by extension: `.rs`; `.py`, `.pyi`; `.cs`; `.ts`, `.mts`, `.cts`; `.tsx`. Unknown → plain text.
- Each side is highlighted once, on file open, into per-line `StyledSpan`s, then cached. A `Highlighter` built once in `main` from the resolved theme (owned by `App`) configures each language on first use; `Highlighter::apply(&mut FileDiff, &ChangedFile)` highlights each side's normalized lines (CR-stripped, tab-expanded, so span offsets index `SideText::lines` directly). The old side of a rename is detected by its old path.
- API (0.27): `Highlighter::highlight(config, source, None, None, injection_callback)` - 5 arguments.

Per-language `HighlightConfiguration` (highlights / injections / locals):

| Language | Highlights | Injections | Locals |
|---|---|---|---|
| Rust | `tree_sitter_rust::HIGHLIGHTS_QUERY` | `INJECTIONS_QUERY` | - |
| Python | `HIGHLIGHTS_QUERY` | - | - |
| C# | `HIGHLIGHTS_QUERY` (crate ships no injections/locals) | - | - |
| TypeScript | JS `HIGHLIGHT_QUERY` + TS `HIGHLIGHTS_QUERY` | JS `INJECTIONS_QUERY` | JS `LOCALS_QUERY` + TS `LOCALS_QUERY` |
| TSX | JS `HIGHLIGHT_QUERY` + JS `JSX_HIGHLIGHT_QUERY` + TS `HIGHLIGHTS_QUERY` | JS `INJECTIONS_QUERY` | JS + TS locals |

- **Concatenation order matters**: when several patterns match a node, the later one wins. Order is JS, then JSX (TSX only), then TS. TS alone lacks strings, comments, functions and numbers. The JS constant is spelled `HIGHLIGHT_QUERY` (singular). TS grammars: `LANGUAGE_TYPESCRIPT`, `LANGUAGE_TSX`.
- **Injections**: the Rust query re-injects `rust` into macro token trees; the callback returns the Rust config so `vec![…]` / `println!(…)` bodies are highlighted. All other injection languages (JS `regex`, `jsdoc`, `css`, `html`, `graphql`, …) → `None`.
- Known upstream quirk: TS marks capitalized identifiers as `type` (e.g. `function App`). Accepted.

Capture → theme key mapping:

- Pass the theme's `syntax` key list (after applying the remap below) to `HighlightConfiguration::configure()`. tree-sitter-highlight matches each capture to the key with the most dotted parts that all appear in the capture name, so exact matches and suffix stripping (`property.definition` → `property`) need no extra code. Unmatched → default foreground.
- Captures that are not highlights (`_*`, `injection.*`, `local.*`, `glimmer`) need no handling: no theme key matches them.
- Remap table for captures the theme has no key for. Captures can't be renamed, so each alias is passed to `configure()` after the theme keys, styled like its target key:
  ```rust
  const REMAP: &[(&str, &str)] = &[
      ("escape", "string.escape"), // rust, python
  ];
  ```
- Against Tokyo Night Moon, all 31 highlight captures resolve. Of Zed's 94 syntax keys, 65 are unreachable with these queries (e.g. `keyword.*` subkeys, `function.call`, `boolean`); accepted.

### Themes

- Format: Zed theme family JSON (`themes[]` with `name`, `appearance`, `style`, `style.syntax`).
- Used keys (minimum): `editor.background`, `editor.foreground`, `editor.line_number`, `editor.active_line.background`, `syntax.*`, optional `players[0].selection` and `players[0].cursor`, and diff colors from `version_control.added|deleted|modified`, falling back to `created|deleted|modified`.
- `#RRGGBBAA` colors are alpha-blended onto `editor.background` at load time (terminals have no alpha).
- Changed **lines** get a low-intensity tint of the added/deleted color; changed **tokens** (difft spans) get a stronger tint. Strengths are the blend alphas `ui.tint.line` / `ui.tint.token` (§13), applied when the theme is resolved.
- Visual selections use `players[0].selection`; without it, `editor.foreground` blended onto `editor.active_line.background` at `ui.tint.selection`.
- The block cursor uses `players[0].cursor`; without it, `editor.foreground`. The text under it is drawn in `editor.background`.
- `font_style: italic` and `font_weight >= 700` map to italic/bold modifiers.
- Color depth: 24-bit truecolor only. No 256-color fallback is planned.

Theme resolution:

0. `ui.theme` unset → the bundled default (`default_theme::VARIANT` in `src/defaults/default_theme.rs`). The default theme is not a `defaults.toml` key, because its name and its compiled-in JSON must change together.
1. `theme` in config is either a variant name (e.g. `"Tokyo Night Moon"`), matched against variant names across all theme families, or a path to a theme JSON file plus `theme_variant`. A path starting with `~/` is expanded to the home directory; another relative path is relative to the config file that sets it.
2. Names are looked up in `~/.config/reviewy/themes/*.json`, then bundled themes. User themes shadow bundled ones.
3. No lookup in Zed's own directories - Reviewy's themes are decoupled so they can diverge.
4. Any failure (name not found, unreadable or invalid file, missing variant or required key) logs a warning, falls back to the bundled default and shows the error in the status line at startup.

Bundled: `src/defaults/tokyo-night.json`, compiled in via `include_str!` in `src/defaults/default_theme.rs`. It contains four variants (Tokyo Night, Tokyo Night Storm, Tokyo Night Moon, Tokyo Night Light); the default is **Tokyo Night Moon**.

- Source: [ssaunderss/zed-tokyo-night](https://github.com/ssaunderss/zed-tokyo-night), MIT. License kept at `3rd_party/tokyo-night.LICENSE`.
- Tokyo Night Moon has no `version_control.*` keys, so it uses the `created`/`deleted`/`modified` fallback for diff colors.

## 10. UI

### Screens and layers

- **Base:** the two-column diff view (blank before a file is opened).
- **Explorer overlay:** centered list over the base.
- **Modals:** stacked above everything; the top modal receives input.

### Explorer overlay

- Flat list of changed files in `git status` order (tree view later).
- Each entry: status color (theme added/modified/deleted; `conflict` for conflicted files), path (`old → new` for renames), unviewable marker (`B` binary, `C` conflicted, `S` submodule, `P` mode-only), comment count (e.g. `[2]`) when > 0.
- `j`/`k` (and counts, `gg`/`G`) move; Enter opens the selected file (replacing the current one) and closes the overlay; Esc closes the overlay (no-op if no file is open yet).
- No preview while moving.

### Diff view

- Two equal columns: old (left), new (right). Added files show an empty left column; deleted files an empty right column.
- Gutter per column: line number, change marker (`+` / `-` / `~`), comment marker (e.g. `●`) on the side the comment was made on.
- Active column is indicated by its line numbers (`editor.foreground`; the other column uses `editor.line_number`).
- Cursor line: `active_line` background on the active column; visual selections use the selection color (§9), drawn over all other backgrounds. The text cursor is a drawn block (§9 cursor color) over everything, shown only while the diff view has focus (no explorer, no modal). It blinks every `cursor_blink_ms` and turns solid again on any key or paste.
- On first open, the cursor is on the first non-blank char of the first hunk (as `]` from the top; a file starting with a pure deletion opens on the old column). Reopening a file restores its cursor and scroll.
- Vertical scroll is per screen line, so a wrapped row taller than the viewport scrolls by segment.
- **Wrapping** (default on, `wrap` in config): each aligned row's height is the max of both sides' wrapped heights; the shorter side is padded. With `wrap = false`, lines are clipped and both columns scroll horizontally together to keep the text cursor visible.
- Wrapping width uses `unicode-width`; layout is computed by `vim/wrap.rs`.

### Status line

Bottom line: mode, file path, cursor position, pending keys/count, transient messages (e.g. "Overlaps comment #3", "Binary file", "Conflicted file: not viewable in v1").

### Modals

Convention for all confirmations: **Enter confirms, Esc cancels.**

| Modal | Trigger | Enter | Esc |
|---|---|---|---|
| Load existing | Startup with `comments.json` present | Discard (in memory) | Keep comments |
| Large file | Opening a file over limits | View | Cancel |
| Delete comment | `D` on commented line | Delete | Cancel |
| Close comment | Esc in comment editor (Normal mode) | Close editor | Return to editing |
| Save comment | `Ctrl-q` in comment editor, when the body is not empty | Close editor, keeping the comment | Return to editing; `d` deletes the comment and closes the editor (the only modal with a third key) |
| Quit | `Ctrl-q` | Save and quit | Cancel |

For "Load existing", the question is "Found N comments from a previous session (base <short sha>). Discard them?": discarding is the usual answer, so it is the Enter action. A discarded set is only dropped from memory; the files are overwritten (or deleted) on exit. If loading fails, the modal shows the error and Enter/Esc both discard (the file is replaced on exit).

### Comment editor

- Modal box over the diff view, titled e.g. `Comment - src/foo.rs:42-45 (new)`.
- Own model (`src/editor.rs`, `Buffer` in `editor/buffer.rs`) drawn by `ui/editor.rs`; soft-wrapped via `vim/wrap.rs`. A line exactly filling the width gets an empty trailing screen row, so a cursor at its end stays in the box. Normal and visual modes show the drawn block cursor (blinking as in the diff view) and visual selections use the selection color (§9); Insert mode shows the real terminal cursor: a bar, set once at startup (blinking, or steady with `cursor_blink = false`) and colored with the theme cursor color via OSC 12. The terminal's own shape and color are restored on exit (including the panic hook) with `DefaultUserShape` and OSC 112.
- Lives on the modal stack. A new comment opens in Insert mode with the cursor at the end of the body (for a prefilled selection: the empty line below the fence); an existing comment opens in Normal mode at the end of the body.
- **Normal mode** goes through the key parser (`Context::EditorNormal`): the shared motions (`h j k l w b e 0 $ gg G gj gk`, arrows, `Ctrl-d`/`Ctrl-u`; `j`/`k` by screen row, `gj`/`gk` by line), `i a I A o O` (counts ignored), `x`/`X` (with counts; never join lines), operators and `v`/`V` (below). Cursor col `<= len - 1`, also in visual modes.
- **Insert mode** bypasses the parser: typing, Enter (newline), Backspace, Delete, arrows (Up/Down by screen row), Home/End, PageUp/PageDown. Tab and Ctrl-h do nothing. Cursor col `<= len`. Esc returns to Normal, moving the cursor one column left (not past col 0).
- Bracketed paste (`Event::Paste`) inserts at the cursor in any mode, leaving the mode unchanged; line breaks are normalized to `\n` and tabs expanded to spaces (`tab_width`, per pasted line): a raw tab has no display width.
- Edits update the in-memory comment immediately (no manual save).
- Esc in Normal mode with no pending count/chord opens the "close comment" confirmation (protects against habitual Esc taps); with pending input it only clears it.
- `Ctrl-q` (any mode) opens "save comment" (Enter save, `d` delete, Esc edit); an empty/whitespace body is closed (and so removed) without asking. Quit is ignored while the editor is on the modal stack, so quitting from the editor takes a second `Ctrl-q`.
- On close, a comment whose body is empty/whitespace is removed from the model (new or existing).
- **Operators** (Normal, `Context::EditorNormal`): `d`/`c`/`y` + motion, doubled for whole lines (`dd cc yy`, count = lines), `D`/`C`/`Y` (to line end; `Y` as in Neovim), `p`/`P` (with counts). Counts multiply (`2d3w` = `d6w`); `0` right after an operator is a motion (`d0`), a count typed after it continues (`d10w`); `g` chords work inside (`dgg`).
  - Motion kinds: exclusive `h l w b 0`; inclusive `e $`; linewise `j k gj gk gg G Ctrl-d Ctrl-u`, always on logical lines (`dj` deletes two lines even when wrapped).
  - A forward exclusive motion that can't move (`dl`/`x` on the last char, `dw` on the buffer's last word) ends at the line length, never the line break; a backward one that can't move is a no-op. `dw` landing on a later line at or before its first non-blank stops at the end of the previous line.
  - `cw` on a non-blank acts as `ce`, on a word's last char it changes just that char; on whitespace it acts as `dw` then insert.
  - Deleting every line leaves one empty line. `cc` / linewise `c` leaves one empty line (no auto-indent). Change always ends in Insert, even with nothing to change (`C` on an empty line).
  - Cursor after: delete -> range start (linewise: first non-blank of the line now there); yank -> range start (`yy` stays); change -> range start, Insert; charwise put -> last put char; linewise put -> first non-blank of the first put line.
- **Register:** one unnamed register owned by `App`, so it is shared across comments and survives closing the editor. Every `d c x X y` writes it; text has no trailing line break, `linewise` marks whole-line text.
- **Visual / visual-line** (`v`/`V` from Normal, `Context::EditorVisual`): motions extend the selection; `v`/`V` toggle as in the diff view; Esc -> Normal (never the close modal). `d`/`x` delete, `c` change, `y` yank the selection (charwise inclusive of the cursor char; whole lines in visual-line); `D X C Y` act on the lines the selection covers. `p` replaces the selection with the register without changing it; if the register and selection kinds differ, the register text is inserted as plain chars where the selection was. Operators return to Normal (Insert for change). Status line: `VISUAL` / `V-LINE`.
- **Undo / redo** (`u`, `Ctrl-r` in Normal, with counts): each Normal/visual change (`x X d c y p P`, visual ops, paste outside Insert) is one step; an insertion (entering Insert up to Esc, including the change that entered it: `cw cc C o O i a…`, and pastes during it) is one step. A new comment opens in Insert, so its first insertion starts when the editor opens. A step is recorded only when the text changed. A new step clears redo. Undo restores the saved text and cursor (vim moves the cursor to the change instead); the register is never touched. History is per editor (gone on close) and capped at `editor.undo_levels` (oldest dropped; `0` disables undo).

## 11. Input and modes

### Mode hierarchy (diff view)

```
Normal ──v / V──▶ Visual / VisualLine
  ▲                     │
  └────────Esc──────────┘
Normal ──Esc──▶ Explorer overlay
```

`Esc` always steps back one level. `Ctrl-q` (quit) works everywhere except while the comment editor is open (§10). `Ctrl-c` does nothing.

### Normal mode

A text cursor on the active column (`active_line` background on its line). It cannot leave the column except through column switches. Padding rows are skipped: motions work on the active side's lines.

| Key | Action |
|---|---|
| `h` / `l` | Left/right by char |
| `j` / `k` | Down/up by **screen row** |
| `gj` / `gk` | Down/up by line |
| arrows | Aliases for `h` `j` `k` `l` |
| `w` / `b` / `e` | Vim lowercase-word motions |
| `0` / `$` | Line start/end |
| `gg` / `G` | First/last line (`5G`: line 5), first non-blank char |
| `Ctrl-d` / `Ctrl-u` | Half page down/up, keeping the column |
| `Ctrl-h` / `Ctrl-l` (`Ctrl-Left` / `Ctrl-Right`) | Switch to the old/new column: nearest line there, first non-blank char |
| `]` / `[` | Next/previous hunk: first changed line of the hunk on the home side (last chosen with `Ctrl-h`/`Ctrl-l`); if the hunk changes only the other side (pure addition/deletion), switch to it. Lands on the first non-blank char |
| `gn` / `gp` | Next/previous file (explorer order) |
| `v` | Visual (charwise) |
| `V` | Visual line |
| Enter | Commented line → edit comment; otherwise → create a comment on the cursor line (empty body) |
| `D` | Delete comment on line (confirmation) |
| Esc | Open explorer overlay |

### Visual / VisualLine

All normal-mode motions extend the selection. Selections cannot cross columns: `Ctrl-h`/`Ctrl-l` and `]`/`[` first leave visual mode, then act as in Normal mode. `gn`/`gp` open the other file in Normal mode.

| Key | Action |
|---|---|
| `v` / `V` | As vim: the same key again → Normal; the other key switches between charwise and linewise, keeping the anchor |
| Enter | Create comment on the selection; prefill the editor with the selection as a fenced code block |
| Esc | Normal mode |

### Examples

- `Enter` → comment on current line, empty box.
- `V Enter` → comment on current line, line pasted (fenced) into box.
- `w w v w Enter` → charwise comment from the cursor's word to the first char of the word after (vim `v w` semantics), pasted (fenced) into box.

### Vim engine (`src/vim/`)

- `keys.rs`: stateful parser over key events. Accumulates an optional count and multi-key chords (`gg`, `gn`, `gp`) and emits `vim::Input { command: Command, count: Option<usize> }` (`Command`: motion, mode switches, operators such as "comment" / "delete comment"; `count` is `None` when none was typed, so `G` ≠ `1G`). No chord timeout; Esc clears pending input. Counts are supported (`5j`, `3w`). Operator-pending state (`d`/`c`/`y` in `EditorNormal`) sits next to the count and `g` chord state and emits `Command::Operator { op, target }`. One parser with a key table per `Context` (`Diff`, `EditorNormal`, `EditorVisual`, `Modal`; `Modal` has no counts or chords), built from shared slices (`MOTIONS`, `G_MOTIONS`) plus per-context ones; keys map to `Command`s through them so configurable bindings (v1.1+) are a table swap. Modals see `Command::Delete` for `d`; only "save comment" acts on it. Arrow keys alias `hjkl`; Ctrl-Left/Ctrl-Right alias Ctrl-h/Ctrl-l. Ctrl-h is its own key (crossterm reports `0x08` as Ctrl+`h`, `0x7F` as Backspace), never Backspace.
- `vim/` has no dependency on `app/`. The app maps `vim::Command` → `app::Action` in `app/actions.rs` (context-dependent, e.g. Enter in Normal mode → edit the line's comment or create one). The comment editor (`Editor::input`) consumes the same `Command`s in Normal mode.
- `motion.rs`: `Motion` enum (`Left`, `Right`, `Down`, `Up`, `ScreenDown`, `ScreenUp`, `WordForward`, `WordBackward`, `WordEnd`, `LineStart`, `LineEnd`, `FileStart`, `FileEnd`, `HalfPageDown`, `HalfPageUp`, …) applied over:
  ```rust
  trait TextView {
      fn line_count(&self) -> usize;
      fn line(&self, idx: usize) -> &str;
  }
  ```
  plus a wrap layout for screen-row motions. Word classes follow vim: keyword chars (alphanumeric + `_`), other non-blank punctuation runs, whitespace.
- `wrap.rs`: maps logical (line, col) ↔ screen rows for a given width; used by rendering and motions alike.
- The diff view implements `TextView` over a side's lines (read-only); the comment editor implements it over its buffer.

## 12. Comments

### Model

```rust
struct CommentSet {
    version: u32,            // schema version, 1
    base: Base,              // { ref: "HEAD", commit: "<sha>" }
    next_id: u32,            // ids only increase, never reused (survives deletions)
    comments: Vec<Comment>,
}

struct Comment {
    id: u32,
    anchor: Anchor,
    language: Option<String>, // fence tag
    excerpt: Excerpt,        // selected lines plus N lines of context either side
    body: String,
}

struct Anchor { file: PathBuf, side: Side, selection: Selection }  // file: repo-relative path on the commented side (old path of a rename on the old side)
enum Side { Old, New }       // Old = HEAD, New = working tree

enum Selection {             // 1-based, inclusive; cols in chars
    Line { line: u32 },                  // normal-mode comment on the cursor line (the default)
    Lines { start: u32, end: u32 },      // V mode, start < end
    Chars { start: Pos, end: Pos },      // v mode, start <= end; start.col may exceed end.col across lines
}
struct Pos { line: u32, col: u32 }

struct Excerpt { start: u32, lines: Vec<String> }  // contiguous lines from `start`
```

- `Selection::from_lines` / `from_chars` order the ends (backwards selections) and give `Line` for a single line.
- The selected text is not stored: `Comment::selected_text()` derives it from the excerpt and selection.
- `context_lines` (default 3) controls N.
- Overlap is allowed by the model (ranges are independent). **v1 UI refuses** to create a comment whose line range intersects an existing comment on the same file and side, with a status message. The v1.1 picker lifts this without model changes.
- Old-side comments are flagged by `side = Old`; the preamble explains what that means.
- No staleness checks in v1. The base commit is stored for later verification.
- **Tabs (v1):** excerpt text and cols come from tab-expanded lines, so tab-indented files get spaces and expanded columns. See `doc/tasks/v1-followups.md`.

### Lifecycle

- **Create:** Enter in Normal mode on an uncommented line (starts empty), or Enter in Visual/VisualLine mode. Visual modes (including a single-line `V`) prefill the body with the selection in a fenced block tagged with the file's language; the fence is longer than any backtick run in the selection. Cursor is placed on a new line below the fence.
- **Edit:** Normal-mode Enter on a commented line.
- **Delete:** `D` in Normal mode (confirmation), or closing the editor with an empty body.

### Persistence

- In-memory only during a session. No writes per keystroke.
- Written on confirmed quit and on termination signals (SIGTERM, SIGHUP, SIGINT via `signal-hook` -> `AtomicBool`, checked in the event loop).
- Panics: the panic hook restores the terminal and reports; comments are **not** saved (accepted for v1).
- Writes use write-temp-then-rename within `.reviewy/`, so a kill mid-write never leaves a truncated file.
- Empty comment set on exit -> both files deleted.
- Comments with an empty (whitespace-only) body are left out of the written files; the in-memory set is not changed, so a save is safe mid-session.
- While the "Load existing" modal is unanswered, nothing is saved (quit and signals exit without touching the files): the in-memory set is still empty and saving it would delete the previous files.
- Files: `.reviewy/comments.json` (source of truth, serde, lossless), `.reviewy/comments.md` (generated, never parsed back).
- `load` checks `version` before deserializing the rest; `!= 1` is an error.

### `comments.json` example

```json
{
  "version": 1,
  "base": { "ref": "HEAD", "commit": "3f2c9e1a..." },
  "next_id": 4,
  "comments": [
    {
      "id": 3,
      "anchor": {
        "file": "src/foo.rs",
        "side": "new",
        "selection": { "kind": "chars", "start": { "line": 42, "col": 9 }, "end": { "line": 42, "col": 20 } }
      },
      "language": "rust",
      "excerpt": { "start": 39, "lines": [ "...", "...", "...", "    let h = parse_header(buf);", "...", "...", "..." ] },
      "body": "```rust\nparse_header\n```\nThis should return a Result rather than panicking."
    }
  ]
}
```

Other selections: `{ "kind": "line", "line": 42 }`, `{ "kind": "lines", "start": 52, "end": 53 }`.

### `comments.md` layout

ASCII only. Comments are sorted by file, then start line, then id.

````markdown
# Review comments

<preamble>

Base commit: `3f2c9e1a` (HEAD)

## src/foo.rs

### #3 - line 42 col 9 - line 42 col 20

```rust
  39 | ...
  40 | ...
  41 | ...
> 42 |     let h = parse_header(buf);
  43 | ...
```

```rust
parse_header
```
This should return a Result rather than panicking.
````

- Headings: `### #3 - line 42`, `### #4 - lines 52-53`, `### #6 - line 61 col 9 - line 63 col 3`.
- New-side comments carry no side marker; old-side headings end with `(old: base commit version)`.
- Excerpt: fenced with the language tag, line numbers right-aligned to the widest in the excerpt, `>` on selected lines. No separate `Selected:` line: the visual-mode prefill quotes the selection in the body.
- Body verbatim after the excerpt.

### Default preamble

In `src/defaults/defaults.toml` (`comments.preamble`); replaceable in config.

> These are code review comments on uncommitted changes. Each comment refers to a file and a line range on one side:
>
> - new (unmarked): the current working-tree file. Line numbers refer to the file as it is now.
> - old (marked `(old: base commit version)`): the file as of the base commit. This code may since have been changed or removed - use it to understand what the reviewer is referring to; do not expect to find it in the working tree.
>
> Each comment shows the commented lines (marked `>`) with surrounding context. Columns are 1-based character positions. Code blocks inside a comment body are usually quoted from the selection; do not treat them as replacement code unless the comment says so. Address each comment.

## 13. Configuration

Files (TOML), merged in order, later overriding earlier:

1. Built-in defaults (`src/defaults/defaults.toml`, embedded; every non-optional key is set there).
2. `~/.config/reviewy/reviewy.toml` (XDG config dir via `etcetera`).
3. `<repo>/.reviewy/reviewy.toml`.

Tables deep-merge per key; arrays replace. Unknown keys and wrong types are errors naming the offending file.

```toml
[ui]
theme = "Tokyo Night Moon"           # variant name, or path to a theme JSON (relative to this file, or ~/...); unset → bundled default
# theme_variant = "Tokyo Night Moon" # required only when `theme` is a path
wrap = true
tab_width = 4
cursor_blink = true                  # block cursor blinks; terminal bar blinks (else steady)
cursor_blink_ms = 500                # block cursor toggle interval

[ui.tint]                            # 0-1: added/deleted color blended onto the background
line = 0.02                          # changed lines
token = 0.10                         # changed tokens
selection = 0.15                     # selection fallback: foreground onto the active line (themes without players[0].selection)

[limits]
max_lines = 5000
max_bytes = 1048576
max_line_length = 1000

[ignore]
paths = ["*.lock", "dist/**"] # globs, repo-relative (globset); default []

[comments]
context_lines = 3
# preamble = """
# Custom instructions for the agent...
# """

[editor]
undo_levels = 100            # undo steps kept per comment editor; 0 disables undo

[difft]
# binary = "difft"           # override difft path
graph_limit = 500000         # DFT_GRAPH_LIMIT: vertices before difft falls back to a line diff (difft default 3000000)
```

## 14. Errors, signals, logging

- **App errors:** `color-eyre` (`eyre::Result`) in `main`/`app`; `thiserror` enums in `git`, `diff`, `highlight`, `comments`.
- **Panic hook:** restore terminal (leave alternate screen, disable raw mode), then delegate to color-eyre's hook. Errors returned from the event loop also restore the terminal before being reported.
- **Signals:** `signal-hook::flag::register` for SIGTERM, SIGHUP, SIGINT onto an `Arc<AtomicBool>`; the loop saves and exits when set. In raw mode Ctrl-c is a key event (ignored), not SIGINT.
- **Recoverable per-file errors** (difft fails, unparsable JSON, unreadable file) show a status message; the app continues.
- **Logging:** `tracing` + `tracing-subscriber` writing to `.reviewy/reviewy.log` (never stdout/stderr while the TUI is up). Opt-in: the subscriber is installed only when `REVIEWY_LOG` is set (an `EnvFilter` directive, e.g. `debug`); otherwise `tracing` macros are no-ops and no log file is created.

## 15. Dependencies

| Purpose | Crate |
|---|---|
| TUI | `ratatui` (feature `unstable-rendered-line-info` for `Paragraph::line_count`), `crossterm` |
| CLI | `clap` (derive) |
| Serialization | `serde`, `serde_json`, `toml` |
| Errors | `color-eyre`, `thiserror` |
| Highlighting | `tree-sitter` 0.27.1, `tree-sitter-highlight` 0.27.1, `tree-sitter-rust` 0.24.2, `tree-sitter-python` 0.25.0, `tree-sitter-c-sharp` 0.23.5, `tree-sitter-typescript` 0.23.2, `tree-sitter-javascript` 0.25.0 |
| Text | `unicode-width`, `unicode-segmentation` |
| Paths | `etcetera` |
| Ignore globs | `globset` |
| Signals | `signal-hook` |
| Logging | `tracing`, `tracing-subscriber` |
| Tests (dev) | `insta`, `tempfile` (temporary `.reviewy/` dirs) |

Highlighting versions were verified together (S3): one `tree-sitter` in the tree, grammars linked via `tree-sitter-language` 0.1. Other versions are pinned at implementation time. All grammar crates are MIT; `tree-sitter-typescript` ships no LICENSE file, so it is credited manually in `THIRD_PARTY.md`.

## 16. Testing

- **Unit tests:**
  - `git.rs`: porcelain v2 parsing from fixture bytes (modified, added, deleted, renamed, untracked, conflicted, submodule, mode-only).
  - `diff/difft.rs` + `align.rs`: fixture difft JSON → rows/hunks. Fixtures in `tests/fixtures/difft/<case>/` (`old.*`, `new.*`, `difft.json`, captured with difft 0.71.0):
    - `mod`: modified Rust - added/removed/changed lines, `changes: []` entries, phantom trailing pair.
    - `reformat`: one line split into several - unpaired rows outside any chunk (→ `Unchanged`).
    - `uni`: multibyte characters - byte-offset spans.
    - `tab`: tab-indented line - raw-offset → tab-expanded conversion.
    - Trivial statuses (`created`, `deleted`, `unchanged`) and zero-width spans are inline JSON strings in the tests.
  - `vim/`: key parser (counts, chords, Esc), motions over fixture text, wrap layout ↔ screen-row motions.
  - `highlight/theme.rs`: Zed theme parsing, alpha blending, capture fallback.
  - `comments/`: JSON round-trip (lossless), overlap check, md rendering (insta snapshot), fence-length selection.
  - `comments/store.rs`: an empty set deletes both files.
  - `config.rs`: merge order, defaults, unknown keys, relative theme paths.
  - `app/`: hunk navigation, padding-skipping motions, column switches, comment targets per mode, create/close flow, save without empty-body comments, keeping a loaded set.
- **UI snapshot tests:** ratatui `TestBackend` + `insta` for explorer, diff view (wrap on/off, added file, charwise selection), large-file modal, comment editor.
- **No integration tests in v1** (developer decision). The crate is binary-only; the pipeline was verified by hand on scratch repos.

## 17. Spikes

All resolved (difft 0.71.0, tree-sitter 0.27.1). Findings are folded into §6, §7, §8, §9 and §15.

| ID | Topic | Outcome |
|---|---|---|
| S1 | difft JSON alignment sufficiency, span units | Sufficient: `aligned_lines` gives whole-file alignment; spans are raw byte offsets |
| S2 | git external-diff details | Renames need both paths; untracked via `/dev/null`; binary/mode-only via numstat |
| S3 | tree-sitter versions, capture remap | Versions pinned; TS needs JS queries; one remap entry |
