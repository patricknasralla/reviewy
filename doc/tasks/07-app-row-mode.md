# Task 07 - App, explorer and diff view (row mode)

## Goal

Build the real application: `App` state, the action pipeline, the explorer overlay, the two-column diff view in **row mode**, the status line, and the modal stack with the large-file and quit modals. At the end of this task Reviewy is usable for reading diffs (no comments yet).

Read first: `doc/spec.md` §3 (workflow), §4 (event loop), §10 (UI: screens, explorer, diff view, status line, modals), §11 (mode hierarchy, row mode). Read handoff notes in **all** previous task files (01–06); you build on their interfaces.

## Context

Modules available: `config`, `git` (changed files, content, limits), `diff` (`DifftEngine`, `align::build` → `FileDiff`), `highlight` (`Theme`, `Highlighter`), `vim` (`KeyParser` → `Input { command, count }`, motions, wrap; see `05-vim.md` handoff), `comments` (model only; this task only holds an empty `CommentSet`). The event loop in `main.rs` (`run()`) exists from task 01 with a stub `App` (in `src/app.rs`) and an inline `Ctrl-q`/`Resize` match; replace both: build `App` in `main` and route events through `KeyParser` → `App::apply`. Keep resize → `dirty`.

## Deliverables

### `app.rs` and `app/state.rs`

- `App` is defined in `app.rs` (task 01 decision); `app/state.rs` holds the supporting state types (`Mode`, `Screen`, `Modal`, `FileView`).
- `App` owns everything: config, theme, repo root, base commit, `Vec<ChangedFile>`, per-file cache `HashMap<PathBuf, FileView>` (a `FileDiff` plus per-file view state: active side, cursor row, scroll), current file index, `Mode` (`Explorer`, `Row`; tasks 08/09 add `Cursor`, `Visual`, `VisualLine`, `Editor`), modal stack `Vec<Modal>`, status message, `dirty`, `should_quit`, `KeyParser`.
- Remove the `#[allow(dead_code)]` on `UiConfig` in `config.rs` once `wrap` is read.
- Remove the `#[expect(dead_code/unused_imports, reason = "used by task 07")]` attributes task 02 left in `git.rs` (on `mod content` and its `pub use`), `git/status.rs` (`viewable`) and `app.rs` (`base_commit`, `files`, `theme`, `highlighter`, `status`), and the `#[allow(dead_code, reason = "used by task 07; remove then")]` tasks 03/04 left on `mod diff;`, `mod highlight;` and `mod vim;` (task 05) in `main.rs` (fix any warnings it then reveals).
- Leave a `comments: CommentSet` field in place (empty, base from the base commit) so task 09 wires it in without restructuring.

### `app/actions.rs`

- `Action` enum and `App::apply(Action)`. Map `vim::Input` → `Action` here (ignore `count` where it means nothing), depending on the mode and the top modal (the top modal receives input first). Bracketed paste events are ignored in this task.
- **Open file** pipeline: `git::read_content(root, DiffTarget::WorkingTreeVsHead, file)` → `git::check_limits` → if over limits push the large-file modal (Enter continues, Esc cancels) → set the status "Diffing…" and **force a redraw before** calling `DifftEngine::diff(root, DiffTarget::WorkingTreeVsHead, file)` (via the `DiffEngine` trait; synchronous call, ~1 s on big files) → `align::build` → `app.highlighter.apply(&mut diff, file)` → cache. Unviewable files (`!viewable()`) show a status message instead (§10 messages: "Binary file", "Conflicted file: not viewable in v1", "Submodule: not viewable in v1", "Mode change only: not viewable in v1"). Per-file errors show a status message; the app continues.

### Row mode (§11)

- `j`/`k` (counts), `gg`/`G`, `Ctrl-d`/`Ctrl-u` move between rows, **skipping rows where the active side has no line**.
- `h`/`l` switch the active column and jump to the nearest row with a line on that side.
- `g]`/`g[` next/previous hunk start; `gn`/`gp` next/previous **viewable** file in explorer order (no wraparound: at the first/last file, show a status message).
- Enter → in this task, a no-op status message ("Comments: task 08/09") - tasks 08/09 replace it. `D` likewise a no-op.
- Esc → explorer overlay. `Ctrl-q` → quit modal (Enter quits, Esc cancels). `// task 10: save` at the quit point.
- Scrolling: keep the cursor row visible with a small margin (3 rows).

### `ui/explorer.rs`

Centered overlay (~60% width, up to 80% height) over the diff view, per §10: status color (added/untracked → `added`, modified/renamed → `modified`, deleted → `deleted`, conflicted → `conflict`), path (`old → new` for renames), marker (`B` binary, `C` conflicted, `S` submodule, `P` mode-only), comment count `[n]` when > 0 (always 0 in this task). `j`/`k`/counts/`gg`/`G` move; Enter opens; Esc closes (no-op if no file is open). The app starts in this mode over two blank columns.

### `ui/diff_view.rs`

- Two equal columns, old left, new right. Per-column gutter: right-aligned line number, change marker (`+` added, `-` removed, `~` changed), and a one-char comment-marker slot (blank in this task).
- Backgrounds: changed/added/removed lines use `line_added_bg` (rhs) / `line_deleted_bg` (lhs); changed-token spans use `token_*_bg`; syntax colors from `highlights`. The active column's gutter is brighter (use `foreground` vs `line_number`); the cursor row gets `active_line_background` on the active column.
- Padding rows (side has no line) render as empty background.
- **Wrap on** (`ui.wrap`): each row's height is the max of both sides' `vim::wrap` heights at their column widths; the shorter side pads. Continuation segments get a blank gutter. **Wrap off**: lines are clipped; horizontal scroll stays at 0 in row mode (cursor mode, task 08, scrolls it).
- Rendering cost scales with visible rows only. Precompute row heights per width and cache until resize or wrap toggle.

### `ui/modals.rs`, `ui/status.rs`

- Generic centered confirm modal (title, message, "Enter: … / Esc: …" hint). Used now for large file ("Large file (N lines, X KB, longest line L). View anyway?") and quit ("Save and quit?").
- Status line: mode, file path, `line:col` of the active side (1-based), pending keys from `KeyParser::pending()`, transient message (cleared on the next key press). `App::status` is already set at startup when the theme fell back to the default (task 04); show it like any other message.

## Tests

UI snapshots with ratatui `TestBackend` (80×24) and `insta`, built from small in-memory `FileDiff`s (no difft needed):
- Explorer with a modified, an untracked, a renamed and a binary file.
- Diff view of a small modified file, wrap on, including one line long enough to wrap.
- Same file, wrap off.
- An added file (empty left column).
- Large-file modal.

One unit test for row-mode `j` skipping padding rows on the active side.

## Acceptance

- Run in a real repo with mixed changes: explorer lists files with correct colors/markers; opening each viewable file shows the correct side-by-side diff with highlighting and tints; `g]`/`g[`/`gn`/`gp`/`h`/`l` behave as specified; resizing redraws correctly; `Ctrl-q` → Enter exits.
- fmt, clippy, tests clean.

## Handoff notes

### Public interfaces

- `app::App` (`app.rs`): `config, theme, highlighter, root, files, views: HashMap<PathBuf, FileView>, current: Option<usize>, explorer: usize, mode: Mode, modals: Vec<Modal>, status: Option<String>, pending: Option<PendingOpen>, comments: CommentSet, size: (u16, u16), dirty, should_quit, keys: KeyParser` (+ private `engine: Box<dyn DiffEngine>`).
  - `App::new(config, root, base_commit: String, files, theme, status) -> App`; `handle_key(KeyEvent)` (clears `status`, `KeyParser` → `map` → `apply`); `resize(w, h)`; `current_file()`, `view() -> Option<&FileView>`.
- `app/state.rs`: `Mode::{Explorer, Row}`, `Modal::{LargeFile { pending, info: FileInfo }, Quit}`, `PendingOpen { index, content: FileContent }`.
- `app/view.rs`: `FileView { pub diff, pub side: Side, pub row, pub scroll, home, layout }` (`Side` = `comments::model::Side`). `new(FileDiff)`, `line(row, side)`, `cursor_line()`, `tops()` (screen line per row + end), `motion(Motion, Option<usize>, height)`, `switch(Side)`, `hunk(next, count) -> bool`, `refresh_layout(width, height, wrap)`.
- `app/actions.rs`: `Action::{ModalConfirm, ModalCancel, ExplorerMove(Motion, Option<usize>), ExplorerOpen, ExplorerClose, RowMove(Motion, Option<usize>), Switch(Side), Hunk { next, count }, File { next }, ToExplorer, Comment, DeleteComment, Quit}`; `App::map(Input) -> Option<Action>`, `App::apply(Action)`.
- `app/open.rs`: `App::open(index)`, `start_diff(PendingOpen)`, `finish_open()`, `insert_view(index, FileDiff)` (also the test entry point).
- `ui`: `draw(frame, &App)`; `diff_view::{columns(&FileDiff, width) -> [Column; 2], Column { gutter, text }, draw}`; `explorer::draw`; `modals::draw` (private `confirm(title, message, hint)`); `status::draw`.
- `highlight::theme::Tint { line, token: f32 }` (`Deserialize`); `Theme::resolve(theme, variant, tint)`, `Theme::bundled_default(tint)`.
- `config::UiConfig::tint: Tint` (`[ui.tint]`); `config::from_sources` is `pub(crate)`.

### Decisions and deviations

- **Two-phase open** (developer decision): `apply` reads content, checks limits, sets "Diffing…" and `App::pending`; the event loop draws, calls `finish_open` (difft → `align::build` → `Highlighter::apply` → cache), draws again. Errors become a status message; nothing is cached, so reopening retries.
- **Layout lives in `App`** (developer decision): terminal size in `App::size`, seeded from `terminal.size()`; each `FileView` caches row start lines keyed by (width, wrap) and recomputes on change. `ui::draw` stays `&App` and draws visible rows only.
- **No `Screen` type**: `Mode::Explorer` plus the modal stack cover it. **`FileView` in `app/view.rs`**, open pipeline in `app/open.rs` (spec §5 updated).
- **Initial cursor = first hunk** (developer decision; spec §10 updated).
- **Hunk navigation** (developer decision; spec §11 updated): each hunk's target is its first *changed* (kind ≠ `Unchanged`) row on the **home side** (set by `h`/`l` and the file's starting side), else on the other side - `g]`/`g[` then switch the active side. Targets strictly increase, so jumps never stall. `j`/`k` after an automatic switch stay on that side.
- **Diff tints configurable** (developer decision; task 04 module change, spec §9/§13 updated): `LINE_ALPHA`/`TOKEN_ALPHA` consts replaced by `[ui.tint] line = 0.02, token = 0.10` in `defaults.toml`. Bundled Tokyo Night Moon `editor.active_line.background` changed to `#212d48` by the developer.
- **`App::base_commit` removed** (unread): the SHA is `comments.base.commit` (task 10 updated).
- Explorer `[n]` already uses `CommentSet::count_for` (developer decision). Each entry: git-style status letter (`M A ? D R U`) and path in the status color, marker `B/C/S/P` (binary first), count.
- `h`/`l` to an absent side (added/deleted file) is a silent no-op; ties for "nearest row" go up. `gg`/`G` with a count go to that line on the active side (`5G`). Half page = `max(height/2, 1)` lines. Explorer Esc returns to Row only if a file is open.
- `Ctrl-q` pushes the quit modal from any state, including over another modal (not twice).
- Gutter: ` {number}{marker}{comment slot} ` (`GUTTER_EXTRA = 4` + digits of the side's line count). Cursor row: `active_line_background` replaces the line tint on the active column; token tints stay. Unviewable messages follow §10; the unviewable reason is in `app/open.rs` (`ChangedFile::viewable` still used by `gn`/`gp`).
- **Dead code:** `allow(dead_code, reason = "used by task 08; remove then")` on `vim::motion::apply` and `Motion::ScreenUp` (task 08 updated). `mod comments;` allow stays for task 09.

### Left undone / flagged

- **Tall rows:** scroll is row-granular; a wrapped row taller than the viewport is clipped at the bottom (task 08 updated).
- After the terminal grows, the view may leave empty lines below the last row until the next move.
- Manual check (scratch repo, GNU screen): explorer, open, all row keys, large-file modal, unviewable messages, quit modal verified. Resize and colors not verifiable there (snapshots are text-only); colors checked by the developer.

### Changed by task 08

- Row mode no longer exists: task 08 merged it with cursor mode into `Mode::Normal` (text cursor always present; Ctrl-h/Ctrl-l switch columns; `]`/`[` for hunks). `FileView::motion` and its row helpers were removed; `FileView::scroll` is now a screen line (fixes "tall rows" above). See task 08's handoff notes.
