# Reviewy — Specification (v1)

> Working title. Status: draft spec for v1. Items marked **[SPIKE]** must be validated before the dependent design is locked in.

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
        let actions = input.handle(event, &app.mode);
        for action in actions { app.apply(action); }
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
4. Check `difft --version`. Missing → error with install hint and exit.
5. Load config (global, then project override).
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
  main.rs              clap args, hooks/signals, terminal setup/teardown, event loop
  config.rs            reviewy.toml loading & merging, defaults
  git.rs               repo root, base SHA, status parsing, `git show`, binary detection
  diff/
    mod.rs             DiffEngine trait
    model.rs           FileDiff, Row, LineKind, change spans
    difft.rs           DifftEngine: spawn, parse JSON, version check
    align.rs           build whole-file aligned rows from difft chunks
  highlight/
    languages.rs       language registry: extension → grammar + queries
    highlighter.rs     tree-sitter-highlight → per-line style spans
    theme.rs           Zed theme JSON parsing, alpha blending, capture fallback, bundled themes
  comments/
    model.rs           Comment, Side, LineRange, ColRange, Context, CommentSet
    store.rs           JSON load/save
    render_md.rs       preamble + markdown rendering
  vim/
    keys.rs            key/chord/count parser → Action
    motion.rs          Motion enum, TextView trait, motion application
    wrap.rs            soft-wrap layout (shared by diff view, editor, gj/gk)
  app/
    state.rs           App, Screen, Mode, modal stack, per-file view state
    actions.rs         Action enum and App::apply
  ui/
    explorer.rs        explorer overlay
    diff_view.rs       two-column rendering, gutter, cursor/selection
    editor.rs          comment editor widget (buffer + rendering)
    modals.rs          confirm, large file, load existing, comment editor frame
    status.rs          status line / transient messages
```

## 6. Git integration

All git access is via the `git` CLI (`std::process::Command`), run from the repo root.

### Changed files

`git status --porcelain=v2 -z --untracked-files=all` (with `-M` rename detection as git provides by default for staged renames). Parse into:

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
- Conflicted (unmerged) entries, submodules and mode-only changes are listed but not viewable in v1, like binary files. Markers: `C` conflicted, `S` submodule, `M` mode-only. Enter shows a status message. Conflict resolution is planned for a later version.

### Binary detection

A file is binary if either side contains a NUL byte in its first 8000 bytes (git's heuristic). Binary files are listed with a `B` marker and are not openable (status message on Enter).

### File content

- Old side: `git show HEAD:<path>` (or `HEAD:<from>` for renames), captured to memory.
- New side: read the working-tree file.
- Added/untracked files have no old side; deleted files have no new side.
- `\r\n` is normalized to `\n` for display; tabs expanded to `tab_width` spaces.

### Large files

Before diffing, check both sides against the limits:

| Limit | Default |
|---|---|
| `max_lines` | 5000 |
| `max_bytes` | 1 MiB |
| `max_line_length` | 1000 chars |

If any limit is exceeded, show a modal: "Large file (N lines, X KB, longest line L). View anyway?" — Enter views, Esc cancels. `max_line_length` catches minified/obfuscated files that are few lines but huge.

## 7. Diff engine

### Trait

```rust
trait DiffEngine {
    fn diff(&self, file: &ChangedFile) -> Result<RawDiff, DiffError>;
}
```

v1 has one implementation, `DifftEngine`. A `BuiltinEngine` fallback may follow later behind the same trait. Everything downstream consumes the internal model (§8), never difft JSON.

### Invocation

Environment for all difft runs: `DFT_UNSTABLE=yes`, `DFT_DISPLAY=json` (and `--display json` when calling directly).

- **Tracked changes** (modified, deleted, renamed, staged-added):
  ```
  GIT_EXTERNAL_DIFF=difft DFT_DISPLAY=json DFT_UNSTABLE=yes git diff HEAD -- <path>
  ```
  git materializes the old blob as a temp file and passes the 7-argument (or 9 with renames) external diff convention to difft. `GIT_EXTERNAL_DIFF` takes precedence over any user `diff.external` config. No temp-file handling in Reviewy.
- **Untracked files:**
  ```
  difft --display json /dev/null <path>
  ```
  Mirrors what git passes for an added file. difft reports `"status": "created"`.

Results are cached per file for the session.

### Difft JSON (expected shape)

```json
{
  "path": "src/foo.rs",
  "language": "Rust",
  "status": "changed",
  "chunks": [
    [
      { "lhs": { "line_number": 10, "changes": [ { "start": 4, "end": 9, "content": "foo", "highlight": "normal" } ] },
        "rhs": { "line_number": 11, "changes": [ ... ] } },
      { "rhs": { "line_number": 12, "changes": [ ... ] } }
    ]
  ]
}
```

Line numbers are 0-based. The schema is unstable; parsing is isolated in `difft.rs` and should be tolerant (unknown fields ignored).

### Minimum difft version

Minimum version: **0.71.0**. Checked at startup by parsing `difft --version` output (format: `Difftastic 0.71.0`). If difft is missing or too old: hard error with install hint (v1 is a personal tool; the user ensures it's installed).

### [SPIKE] S1 — difft JSON sufficiency

Run difft JSON against real modified files and confirm:

1. Whether unchanged lines inside a chunk are listed and paired.
2. How lhs/rhs entries pair within a chunk (row alignment).
3. How gaps between chunks align, especially when difft treats reformatting as unchanged (gap lengths can differ between sides).
4. `changes[].start/end` units (bytes vs chars).

If alignment info is insufficient, `align.rs` fills gaps with its own line diff of the unchanged regions.

### [SPIKE] S2 — git external mode details

1. `path` in difft JSON under the 9-argument rename form matches our repo-relative path.
2. `difft /dev/null <path>` produces valid JSON with `status: created`.
3. Behavior for deleted files via `git diff HEAD -- <path>`.
4. Detecting mode-only changes (mode differs, content identical) and submodule entries from porcelain v2 output.

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
    changes: Vec<Vec<Range<usize>>>, // per line, changed token spans
    highlights: Vec<Vec<StyledSpan>>, // per line, from highlighter
}

enum LineKind { Unchanged, Changed, Added, Removed }

struct Row { lhs: Option<u32>, rhs: Option<u32> } // 0-based line indices
```

Alignment (`align.rs`):

- Walk difft chunks in order.
- Between chunks: unchanged lines are paired 1:1 in order; if the gap lengths differ, the shorter side is padded with empty rows.
- Within chunks: use difft's lhs/rhs pairing per entry; unpaired entries get an empty opposite side.
- Added files: all rows `{ lhs: None, rhs: Some(i) }`, all lines `Added`. Deleted: mirror.
- Hunks are maximal runs of rows containing any non-`Unchanged` line.

## 9. Syntax highlighting and themes

### Highlighting

- Crates: `tree-sitter`, `tree-sitter-highlight`, grammar crates `tree-sitter-rust`, `tree-sitter-python`, `tree-sitter-c-sharp`, `tree-sitter-typescript` (TS + TSX).
- Highlight queries: the grammar crates' bundled `HIGHLIGHTS_QUERY` (MIT). Zed's own `.scm` queries are GPL and are **not** used.
- Language detection by extension: `.rs`; `.py`, `.pyi`; `.cs`; `.ts`, `.mts`, `.cts`; `.tsx`. Unknown → plain text.
- Each side is highlighted once, on file open, into per-line `StyledSpan`s, then cached.
- Capture names map to theme `syntax` keys by exact match, then by stripping dotted suffixes (`function.method` → `function`), then default foreground. A small remap table handles naming differences between grammar queries and Zed keys.

### [SPIKE] S3 — tree-sitter

Confirm compatible versions of `tree-sitter`, `tree-sitter-highlight` and the four grammar crates (via `tree-sitter-language`), and list each grammar's capture names against Zed theme keys to build the remap table.

### Themes

- Format: Zed theme family JSON (`themes[]` with `name`, `appearance`, `style`, `style.syntax`).
- Used keys (minimum): `editor.background`, `editor.foreground`, `editor.line_number`, `editor.active_line.background`, `syntax.*`, and diff colors from `version_control.added|deleted|modified`, falling back to `created|deleted|modified`.
- `#RRGGBBAA` colors are alpha-blended onto `editor.background` at load time (terminals have no alpha).
- Changed **lines** get a low-intensity tint of the added/deleted color; changed **tokens** (difft spans) get a stronger tint.
- `font_style: italic` and `font_weight >= 700` map to italic/bold modifiers.
- Color depth: 24-bit truecolor only. No 256-color fallback is planned.

Theme resolution:

1. `theme` in config is either a variant name (e.g. `"Tokyo Night Moon"`), matched against variant names across all theme families, or a path to a theme JSON file plus `theme_variant`.
2. Names are looked up in `~/.config/reviewy/themes/*.json`, then bundled themes. User themes shadow bundled ones.
3. No lookup in Zed's own directories — Reviewy's themes are decoupled so they can diverge.

Bundled: `default_theme/tokyo-night.json`, compiled in via `include_str!`. It contains four variants (Tokyo Night, Tokyo Night Storm, Tokyo Night Moon, Tokyo Night Light); the default is **Tokyo Night Moon**.

- Source: [ssaunderss/zed-tokyo-night](https://github.com/ssaunderss/zed-tokyo-night), MIT. License kept at `default_theme/LICENSE`.
- Tokyo Night Moon has no `version_control.*` keys, so it uses the `created`/`deleted`/`modified` fallback for diff colors.

## 10. UI

### Screens and layers

- **Base:** the two-column diff view (blank before a file is opened).
- **Explorer overlay:** centered list over the base.
- **Modals:** stacked above everything; the top modal receives input.

### Explorer overlay

- Flat list of changed files in `git status` order (tree view later).
- Each entry: status color (theme added/modified/deleted; `conflict` for conflicted files), path (`old → new` for renames), unviewable marker (`B` binary, `C` conflicted, `S` submodule, `M` mode-only), comment count (e.g. `[2]`) when > 0.
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

- `keys.rs`: stateful parser over key events. Accumulates an optional count and multi-key chords (`gg`, `g]`, `gn`, …) and emits `Action`s. No chord timeout; Esc clears pending input. Counts are supported (`5j`, `3w`). Keys map to `Action` enums through a table so configurable bindings (v1.1+) are a table swap.
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

1. Built-in defaults.
2. `~/.config/reviewy/reviewy.toml` (XDG config dir via `etcetera`).
3. `<repo>/.reviewy/reviewy.toml`.

```toml
[ui]
theme = "Tokyo Night Moon"           # variant name, or path to a theme JSON
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
- **Panic hook:** restore terminal (leave alternate screen, disable raw mode), then delegate to color-eyre's hook.
- **Signals:** `signal-hook::flag::register` for SIGTERM, SIGHUP, SIGINT onto an `Arc<AtomicBool>`; the loop saves and exits when set. In raw mode Ctrl-c is a key event (ignored), not SIGINT.
- **Recoverable per-file errors** (difft fails, unparsable JSON, unreadable file) show a status message; the app continues.
- **Logging:** `tracing` + `tracing-subscriber` writing to `.reviewy/reviewy.log` (never stdout/stderr while the TUI is up). Level via `REVIEWY_LOG` env var; off/warn by default.

## 15. Dependencies

| Purpose | Crate |
|---|---|
| TUI | `ratatui`, `crossterm` |
| CLI | `clap` (derive) |
| Serialization | `serde`, `serde_json`, `toml` |
| Errors | `color-eyre`, `thiserror` |
| Highlighting | `tree-sitter`, `tree-sitter-highlight`, `tree-sitter-rust`, `tree-sitter-python`, `tree-sitter-c-sharp`, `tree-sitter-typescript` |
| Text | `unicode-width`, `unicode-segmentation` |
| Paths | `etcetera` |
| Ignore globs | `globset` |
| Signals | `signal-hook` |
| Logging | `tracing`, `tracing-subscriber` |
| Tests (dev) | `insta`, `tempfile` (test repos only) |

Versions are pinned at implementation time; `tree-sitter` and `tree-sitter-highlight` must be version-compatible with each other and the grammar crates (S3).

## 16. Testing

- **Unit tests:**
  - `git.rs`: porcelain v2 parsing from fixture bytes (modified, added, deleted, renamed, untracked, conflicted, submodule, mode-only).
  - `diff/difft.rs` + `align.rs`: fixture difft JSON → rows/hunks (captured during S1).
  - `vim/`: key parser (counts, chords, Esc), motions over fixture text, wrap layout ↔ screen-row motions.
  - `highlight/theme.rs`: Zed theme parsing, alpha blending, capture fallback.
  - `comments/`: JSON round-trip (lossless), overlap check, md rendering (insta snapshot), fence-length selection.
  - `config.rs`: merge order and defaults.
- **UI snapshot tests:** ratatui `TestBackend` + `insta` for explorer, diff view (wrap on/off, added/deleted files), modals, editor.
- **Integration tests:** build a temporary git repo (`tempfile` dev-dep), make changes, run the pipeline end to end. Require `git` and `difft`; gated behind `#[ignore]` or a feature if unavailable.

## 17. Spikes

| ID | Topic | Blocks |
|---|---|---|
| S1 | difft JSON alignment sufficiency, span units | `align.rs`, diff model |
| S2 | git external-diff path field with renames; `/dev/null` for untracked; deleted files; mode-only/submodule detection | `difft.rs` |
| S3 | tree-sitter crate version compatibility; capture-name remap table | `highlight/` |
