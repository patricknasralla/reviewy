# Reviewy — Specification (v1)

> Working title. Status: spec for v1. Spikes resolved (§17).

## 1. Purpose

Reviewy is a terminal UI for quickly reviewing a project's uncommitted changes — typically changes made by an AI agent — and leaving line-anchored review comments, similar to GitHub's PR review UI.

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
- Explorer overlay listing changed files (status color, comment count, binary marker).
- Side-by-side, two-column, whole-file diff view with tree-sitter syntax highlighting and change backgrounds.
- Languages: Rust, Python, C#, TypeScript (incl. TSX). Other files render as plain text.
- Zed-format themes: bundled themes plus a user themes directory.
- Vim-style modal navigation (row / cursor / visual / visual-line), counts, chords.
- Comment create / edit / delete, with a custom comment editor widget (insert-only in v1).
- Comments persisted on exit to `.reviewy/comments.json` (source of truth) and `.reviewy/comments.md` (generated for the agent).
- Configuration via `reviewy.toml` (global + project).

### v1.1 (planned, design must not block these)

- Modal (vim) editing in the comment editor, built on the shared vim engine.
- Picker for lines covered by multiple overlapping comments (model already supports overlap).
- CLI arguments mirroring `git diff` (ranges, `--staged`, etc.). Argument scaffolding exists in v1 (§4); behavior **to be discussed**.
- Yank selection to the system clipboard.
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

## 3. Usage workflow

1. An agent modifies files in a project and reports that it's done.
2. The developer runs `reviewy` in the project directory.
3. Reviewy starts with the explorer overlay open over two blank columns.
4. The developer selects a file; the overlay closes and the file is shown side by side (old left, new right), whole file, syntax-highlighted, with changed regions on a tinted background.
5. The developer navigates like a read-only vim buffer, selects lines or text and presses Enter to comment. The selection (if any) is pasted into the comment box as a fenced code block. Commented lines get a gutter marker.
6. Moving to a commented line and pressing Enter reopens that comment for editing. `D` deletes it after confirmation.
7. `Ctrl-q` (with confirmation) quits. Comments are written to `.reviewy/comments.json` and `.reviewy/comments.md`. The developer pastes the md into the agent session or a harness reads either file.
8. On the next run, if `.reviewy/comments.json` exists, a dialog asks whether to keep or discard the previous comments. Either way the files are overwritten on exit.

## 4. Architecture overview

- **Single crate**, binary `reviewy`.
- **Synchronous** main loop. No async runtime.
- **External processes:** `git` (status, show, diff) and `difft` (via `GIT_EXTERNAL_DIFF` or directly).
- **Lazy per-file work:** a file's diff, content and highlighting are computed when it is first opened and cached for the session.
- **One shared vim engine** (key parsing, motions, soft-wrap layout) used by both the diff view and the comment editor.
- **Plain in-memory state:** `App` owns everything, including `Vec<Comment>`. No shared ownership, no locks.

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
    if terminate_flag.load() { save(); break; }
    if crossterm::event::poll(timeout)? {
        let event = crossterm::event::read()?;
        if let Some(input) = keys.handle(key_event) {    // vim::Input { command, count }
            app.apply(app.map_input(input));             // → Action
        }
    }
    if app.dirty { terminal.draw(|f| ui::draw(f, &app))?; app.dirty = false; }
    if app.should_quit { save(); break; }
}
```

- Redraw only when state changed (or on resize).
- `poll` timeout (~250 ms) bounds signal-handling latency.

### Startup sequence

1. Install panic hook (restore terminal, then color-eyre report) and signal flag.
2. Resolve repo root: `git rev-parse --show-toplevel`. Not a repo → error and exit.
3. Resolve base commit: `git rev-parse HEAD`. No commits → error and exit (v1).
4. Load config (built-in defaults, global, then project override).
   - If `REVIEWY_LOG` is set: create `.reviewy/` (as in step 7) and start logging. Done here so later steps can log.
5. Check `<difft.binary> --version` (the binary path comes from config). Missing or too old → error with install hint and exit.
6. Collect changed files (§6). None → print "No changes" and exit.
7. Ensure `.reviewy/` exists and contains a `.gitignore` with `*` (ignores the whole folder without touching the user's root `.gitignore`).
8. If `.reviewy/comments.json` exists → "keep / discard" dialog.
9. Enter terminal (raw mode, alternate screen, bracketed paste), show explorer overlay.

### Shutdown

- Triggered by confirmed `Ctrl-q` or a termination signal.
- If comments exist: write `comments.json` and `comments.md` (write-to-temp-then-rename).
- If no comments: delete both files.
- Restore terminal.

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
    model.rs           Comment, Side, LineRange, ColRange, Context, CommentSet
    store.rs           JSON load/save
    render_md.rs       preamble + markdown rendering
  vim.rs               declares submodules
  vim/
    keys.rs            key/chord/count parser → vim::Command
    motion.rs          Motion enum, TextView trait, motion application
    wrap.rs            soft-wrap layout (shared by diff view, editor, gj/gk)
  app.rs               App struct; declares submodules
  app/
    state.rs           Screen, Mode, modal stack, per-file view state
    actions.rs         Action enum, vim::Command → Action mapping, App::apply
  ui.rs                declares submodules; top-level draw()
  ui/
    explorer.rs        explorer overlay
    diff_view.rs       two-column rendering, gutter, cursor/selection
    editor.rs          comment editor widget (buffer + rendering)
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

- `1 <XY> <sub> <mH> <mI> <mW> <hH> <hI> <path>` — ordinary change. Classified by modes, not XY, since only HEAD vs worktree matters: `mH` and `mW` both `000000` → dropped (no change vs HEAD, e.g. `AD`); `mH` zero → `Added` (includes intent-to-add `.A`); `mW` zero → `Deleted`; otherwise `Modified`.
- `2 <XY> <sub> <mH> <mI> <mW> <hH> <hI> <score> <path>\0<origPath>` — rename/copy; the original path is the next NUL-separated field.
- `u <XY> <sub> ...` — unmerged → `Conflicted`.
- `? <path>` — untracked.
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

If any limit is exceeded, show a modal: "Large file (N lines, X KB, longest line L). View anyway?" (each number is the max over both sides) — Enter views, Esc cancels. `max_line_length` catches minified/obfuscated files that are few lines but huge.

## 7. Diff engine

### Trait

```rust
trait DiffEngine {
    fn diff(&self, root: &Path, target: DiffTarget, file: &ChangedFile) -> Result<RawDiff, DiffError>;
}
```

v1 has one implementation, `DifftEngine`. A `BuiltinEngine` fallback may follow later behind the same trait. Everything downstream consumes the internal model (§8), never difft JSON.

### Invocation

Environment for all difft runs: `DFT_UNSTABLE=yes`, `DFT_DISPLAY=json` (and `--display json` when calling directly).

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
    hunks: Vec<RowRange>,        // for g] / g[
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
- Line kinds from chunks: a line listed in a chunk whose row has both sides → `Changed`; listed with no opposite side → `Removed` (lhs) / `Added` (rhs). Lines not in any chunk → `Unchanged`, **including unpaired ones** (e.g. difft treats reformatting — one line split into several — as unchanged; the extra lines get padding opposite but no tint).
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
- API (0.27): `Highlighter::highlight(config, source, None, None, injection_callback)` — 5 arguments.

Per-language `HighlightConfiguration` (highlights / injections / locals):

| Language | Highlights | Injections | Locals |
|---|---|---|---|
| Rust | `tree_sitter_rust::HIGHLIGHTS_QUERY` | `INJECTIONS_QUERY` | — |
| Python | `HIGHLIGHTS_QUERY` | — | — |
| C# | `HIGHLIGHTS_QUERY` (crate ships no injections/locals) | — | — |
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
- Used keys (minimum): `editor.background`, `editor.foreground`, `editor.line_number`, `editor.active_line.background`, `syntax.*`, and diff colors from `version_control.added|deleted|modified`, falling back to `created|deleted|modified`.
- `#RRGGBBAA` colors are alpha-blended onto `editor.background` at load time (terminals have no alpha).
- Changed **lines** get a low-intensity tint of the added/deleted color; changed **tokens** (difft spans) get a stronger tint.
- `font_style: italic` and `font_weight >= 700` map to italic/bold modifiers.
- Color depth: 24-bit truecolor only. No 256-color fallback is planned.

Theme resolution:

0. `ui.theme` unset → the bundled default (`default_theme::VARIANT` in `src/defaults/default_theme.rs`). The default theme is not a `defaults.toml` key, because its name and its compiled-in JSON must change together.
1. `theme` in config is either a variant name (e.g. `"Tokyo Night Moon"`), matched against variant names across all theme families, or a path to a theme JSON file plus `theme_variant`. A relative path is relative to the config file that sets it.
2. Names are looked up in `~/.config/reviewy/themes/*.json`, then bundled themes. User themes shadow bundled ones.
3. No lookup in Zed's own directories — Reviewy's themes are decoupled so they can diverge.
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
- Active column is indicated (e.g. brighter gutter / border).
- Current line in row mode: `active_line` background on the active column.
- **Wrapping** (default on, `wrap` in config): each aligned row's height is the max of both sides' wrapped heights; the shorter side is padded. With `wrap = false`, lines are clipped and the view scrolls horizontally to follow the cursor.
- Wrapping width uses `unicode-width`; layout is computed by `vim/wrap.rs`.

### Status line

Bottom line: mode, file path, cursor position, pending keys/count, transient messages (e.g. "Overlaps comment #3", "Binary file", "Conflicted file: not viewable in v1").

### Modals

Convention for all confirmations: **Enter confirms, Esc cancels.**

| Modal | Trigger | Enter | Esc |
|---|---|---|---|
| Load existing | Startup with `comments.json` present | Keep comments | Discard (in memory) |
| Large file | Opening a file over limits | View | Cancel |
| Delete comment | `D` on commented line | Delete | Cancel |
| Close comment | Esc in comment editor | Close editor | Return to editing |
| Quit | `Ctrl-q` | Save and quit | Cancel |

For "Load existing", a discarded set is only dropped from memory; the files are overwritten (or deleted) on exit.

### Comment editor

- Modal box over the diff view, titled e.g. `Comment — src/foo.rs:42-45 (new)`.
- Own widget (`ui/editor.rs`): `Vec<String>` buffer, soft-wrapped via `vim/wrap.rs`.
- v1 is insert-only: typing, Enter (newline), Backspace, Delete, arrows, Home/End, scrolling. Bracketed paste (`Event::Paste`) inserts text verbatim.
- Edits update the in-memory comment immediately (no manual save).
- Esc opens the "close comment" confirmation (protects against habitual Esc taps).
- On close, a comment whose body is empty/whitespace is removed from the model (new or existing).
- v1.1 adds normal/visual modes using the shared vim engine.

## 11. Input and modes

### Mode hierarchy (diff view)

```
Row ──Enter──▶ Cursor ──v / V──▶ Visual / VisualLine
 ▲               │  ▲                 │
 └──────Esc──────┘  └──────Esc────────┘
Row ──Esc──▶ Explorer overlay
```

`Esc` always steps back one level. `Ctrl-q` (quit) works everywhere. `Ctrl-c` does nothing.

### Row mode

The cursor is a whole line on the active column. Rows where the active side has no line (padding) are skipped.

| Key | Action |
|---|---|
| `h` / `l` | Switch active column (left/right); jump to nearest row with a line on that side |
| `j` / `k` | Next/previous logical line |
| arrows | Aliases for `h` `j` `k` `l` (row and cursor modes) |
| `gg` / `G` | First/last line |
| `Ctrl-d` / `Ctrl-u` | Half page down/up |
| `g]` / `g[` | Next/previous hunk |
| `gn` / `gp` | Next/previous file (explorer order) |
| Enter | Commented line → edit comment; otherwise → Cursor mode at column 0 of the line |
| `D` | Delete comment on line (confirmation) |
| Esc | Open explorer overlay |

### Cursor mode

A text cursor on the active side. Cannot leave the column.

| Key | Action |
|---|---|
| `h` / `l` | Left/right by char |
| `j` / `k` | Down/up by **screen row** (vim `gj`/`gk` semantics under wrap) |
| `w` / `b` / `e` | Vim lowercase-word motions |
| `0` / `$` | Line start/end |
| `gg` / `G`, `Ctrl-d` / `Ctrl-u`, `g]` / `g[`, `gn` / `gp` | As row mode |
| `v` | Visual (charwise) |
| `V` | Visual line |
| Enter | Create comment on the cursor line (no prefill) |
| Esc | Row mode |

### Visual / VisualLine

All cursor-mode motions extend the selection. Selections cannot cross columns.

| Key | Action |
|---|---|
| Enter | Create comment on the selection; prefill the editor with the selection as a fenced code block |
| Esc | Cursor mode |

### Examples

- `Enter Enter` → comment on current line, empty box.
- `Enter V Enter` → comment on current line, line pasted (fenced) into box.
- `Enter w w v w Enter` → comment on the third word, that word pasted (fenced) into box.

### Vim engine (`src/vim/`)

- `keys.rs`: stateful parser over key events. Accumulates an optional count and multi-key chords (`gg`, `g]`, `gn`, …) and emits `vim::Input { command: Command, count: Option<usize> }` (`Command`: motion, mode switches, operators such as "comment" / "delete comment"; `count` is `None` when none was typed, so `G` ≠ `1G`). No chord timeout; Esc clears pending input. Counts are supported (`5j`, `3w`). One key table for all modes; keys map to `Command`s through it so configurable bindings (v1.1+) are a table swap. Arrow keys alias `hjkl`.
- `vim/` has no dependency on `app/`. The app maps `vim::Command` → `app::Action` in `app/actions.rs` (context-dependent, e.g. Enter in row mode → edit comment or enter cursor mode). The comment editor (v1.1 modal editing) consumes the same `Command`s with its own mapping.
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
    next_id: u32,            // ids only increase, never reused
    comments: Vec<Comment>,
}

struct Comment {
    id: u32,
    file: PathBuf,           // repo-relative path on the commented side
    side: Side,              // Old (HEAD) | New (working tree)
    lines: LineRange,        // 1-based, inclusive
    cols: Option<ColRange>,  // 1-based, inclusive, in chars; charwise selections only
    language: Option<String>,
    selected_text: String,   // the selected text (whole line(s) for line comments)
    context: Context,        // N lines before/after on the same side
    body: String,
}

struct Context { before: Vec<NumberedLine>, after: Vec<NumberedLine> }
struct NumberedLine { line: u32, text: String }
```

- `context_lines` (default 3) controls N.
- Overlap is allowed by the model (ranges are independent). **v1 UI refuses** to create a comment whose line range intersects an existing comment on the same file and side, with a status message. The v1.1 picker lifts this without model changes.
- Old-side comments are flagged by `side = Old`; the preamble explains what that means.
- No staleness checks in v1. The base commit is stored for later verification.

### Lifecycle

- **Create:** Enter in Cursor/Visual/VisualLine mode. Visual modes prefill the body with the selection in a fenced block tagged with the file's language; the fence is longer than any backtick run in the selection. Cursor is placed on a new line below the fence. Cursor-mode Enter starts empty.
- **Edit:** Row-mode Enter on a commented line.
- **Delete:** `D` in row mode (confirmation), or closing the editor with an empty body.

### Persistence

- In-memory only during a session. No writes per keystroke.
- Written on confirmed quit and on termination signals (SIGTERM, SIGHUP, SIGINT via `signal-hook` → `AtomicBool`, checked in the event loop).
- Panics: the panic hook restores the terminal and reports; comments are **not** saved (accepted for v1).
- Writes use write-temp-then-rename within `.reviewy/`.
- Empty comment set on exit → both files deleted.
- Files: `.reviewy/comments.json` (source of truth, serde, lossless), `.reviewy/comments.md` (generated, never parsed back).

### `comments.json` example

```json
{
  "version": 1,
  "base": { "ref": "HEAD", "commit": "3f2c9e1a..." },
  "next_id": 4,
  "comments": [
    {
      "id": 3,
      "file": "src/foo.rs",
      "side": "new",
      "lines": { "start": 42, "end": 42 },
      "cols": { "start": 9, "end": 20 },
      "language": "rust",
      "selected_text": "parse_header",
      "context": {
        "before": [ { "line": 39, "text": "..." }, { "line": 40, "text": "..." }, { "line": 41, "text": "..." } ],
        "after":  [ { "line": 43, "text": "..." }, { "line": 44, "text": "..." }, { "line": 45, "text": "..." } ]
      },
      "body": "```rust\nparse_header\n```\nThis should return a Result rather than panicking."
    }
  ]
}
```

### `comments.md` layout

Comments are sorted by file, then by start line.

````markdown
# Review comments

<preamble>

Base commit: `3f2c9e1a` (HEAD)

## src/foo.rs

### #3 — line 42, cols 9–20 (new)

```rust
  39 | ...
  40 | ...
  41 | ...
> 42 |     let h = parse_header(buf);
  43 | ...
```

Selected: `parse_header`

```rust
parse_header
```
This should return a Result rather than panicking.
````

- Selected lines in the context block are marked with `>`.
- Old-side headings read e.g. `### #5 — lines 10–12 (old: base commit version)`.

### Default preamble

Replaceable via `comments.preamble` in config.

> These are code review comments on uncommitted changes. Each comment refers to a file, a side, and a line range:
>
> - **new**: the current working-tree file. Line numbers refer to the file as it is now.
> - **old**: the file as of the base commit. This code may since have been changed or removed — use it to understand what the reviewer is referring to; do not expect to find it in the working tree.
>
> Each comment shows the commented lines (marked `>`) with surrounding context. Code blocks inside a comment body are usually quoted from the selection; do not treat them as replacement code unless the comment says so. Address each comment.

## 13. Configuration

Files (TOML), merged in order, later overriding earlier:

1. Built-in defaults (`src/defaults/defaults.toml`, embedded; every non-optional key is set there).
2. `~/.config/reviewy/reviewy.toml` (XDG config dir via `etcetera`).
3. `<repo>/.reviewy/reviewy.toml`.

Tables deep-merge per key; arrays replace. Unknown keys and wrong types are errors naming the offending file.

```toml
[ui]
theme = "Tokyo Night Moon"           # variant name, or path to a theme JSON (relative to this file); unset → bundled default
# theme_variant = "Tokyo Night Moon" # required only when `theme` is a path
wrap = true
tab_width = 4

[limits]
max_lines = 5000
max_bytes = 1048576
max_line_length = 1000

[ignore]
paths = ["*.lock", "dist/**"] # globs, repo-relative (globset)

[comments]
context_lines = 3
# preamble = """
# Custom instructions for the agent...
# """

[difft]
# binary = "difft"           # override difft path
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
| TUI | `ratatui`, `crossterm` |
| CLI | `clap` (derive) |
| Serialization | `serde`, `serde_json`, `toml` |
| Errors | `color-eyre`, `thiserror` |
| Highlighting | `tree-sitter` 0.27.1, `tree-sitter-highlight` 0.27.1, `tree-sitter-rust` 0.24.2, `tree-sitter-python` 0.25.0, `tree-sitter-c-sharp` 0.23.5, `tree-sitter-typescript` 0.23.2, `tree-sitter-javascript` 0.25.0 |
| Text | `unicode-width`, `unicode-segmentation` |
| Paths | `etcetera` |
| Ignore globs | `globset` |
| Signals | `signal-hook` |
| Logging | `tracing`, `tracing-subscriber` |
| Tests (dev) | `insta`, `tempfile` (test repos only) |

Highlighting versions were verified together (S3): one `tree-sitter` in the tree, grammars linked via `tree-sitter-language` 0.1. Other versions are pinned at implementation time. All grammar crates are MIT; `tree-sitter-typescript` ships no LICENSE file, so credit it in third-party notices manually.

## 16. Testing

- **Unit tests:**
  - `git.rs`: porcelain v2 parsing from fixture bytes (modified, added, deleted, renamed, untracked, conflicted, submodule, mode-only).
  - `diff/difft.rs` + `align.rs`: fixture difft JSON → rows/hunks. Fixtures in `tests/fixtures/difft/<case>/` (`old.*`, `new.*`, `difft.json`, captured with difft 0.71.0):
    - `mod`: modified Rust — added/removed/changed lines, `changes: []` entries, phantom trailing pair.
    - `reformat`: one line split into several — unpaired rows outside any chunk (→ `Unchanged`).
    - `uni`: multibyte characters — byte-offset spans.
    - `tab`: tab-indented line — raw-offset → tab-expanded conversion.
    - Trivial statuses (`created`, `deleted`, `unchanged`) and zero-width spans are inline JSON strings in the tests.
  - `vim/`: key parser (counts, chords, Esc), motions over fixture text, wrap layout ↔ screen-row motions.
  - `highlight/theme.rs`: Zed theme parsing, alpha blending, capture fallback.
  - `comments/`: JSON round-trip (lossless), overlap check, md rendering (insta snapshot), fence-length selection.
  - `config.rs`: merge order and defaults.
- **UI snapshot tests:** ratatui `TestBackend` + `insta` for explorer, diff view (wrap on/off, added/deleted files), modals, editor.
- **Integration tests:** build a temporary git repo (`tempfile` dev-dep), make changes, run the pipeline end to end. Require `git` and `difft`; gated behind `#[ignore]` or a feature if unavailable.

## 17. Spikes

All resolved (difft 0.71.0, tree-sitter 0.27.1). Findings are folded into §6, §7, §8, §9 and §15.

| ID | Topic | Outcome |
|---|---|---|
| S1 | difft JSON alignment sufficiency, span units | Sufficient: `aligned_lines` gives whole-file alignment; spans are raw byte offsets |
| S2 | git external-diff details | Renames need both paths; untracked via `/dev/null`; binary/mode-only via numstat |
| S3 | tree-sitter versions, capture remap | Versions pinned; TS needs JS queries; one remap entry |
