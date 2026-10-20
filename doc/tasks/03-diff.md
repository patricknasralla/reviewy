# Task 03 - Diff engine and alignment

## Goal

Implement `src/diff/`: run difftastic for one file, parse its JSON, and build the whole-file, row-aligned model the diff view renders. Add the difft version check to startup.

Read first: `doc/spec.md` §7 and §8 (all of both; they record the observed difft 0.71 JSON shape), §6 (file content). Read handoff notes in `doc/tasks/01-scaffold.md` and `02-git.md`.

## Context

Reviewy shows old (left) and new (right) versions of a file side by side, as the whole file, with changed lines tinted and changed tokens tinted more strongly. difft (`difft --display json`) provides the alignment and the changed-token spans. Task 02's `git.rs` already gives you `ChangedFile` and `FileContent` (raw text, not normalized). Highlighting is task 04: leave `SideText::highlights` empty (one empty `Vec` per line).

## Deliverables

### `diff.rs`

```rust
pub trait DiffEngine {
    fn diff(&self, root: &Path, file: &ChangedFile) -> Result<RawDiff, DiffError>;
}
```

`RawDiff` is the parsed difft output (status, aligned lines, chunks) in Reviewy's own types. It is internal to `diff/`; nothing outside `diff/` sees difft JSON types.

### `diff/difft.rs`

- `DifftEngine { binary: PathBuf }` (from `config.difft.binary`).
- `check_version(binary) -> Result<Version, DiffError>`: run `<binary> --version`, parse `Difftastic X.Y.Z` (first line). Missing binary or version < 0.71.0 → error with an install hint (`cargo install difftastic`). Wire into `main.rs` at the `// task 03:` marker (startup step 4, §4). Remove the `#[allow(dead_code)]` on `DifftConfig` in `config.rs`.
- Invocation exactly as §7:
  - Tracked (`Modified`, `Added`, `Deleted`): `git diff HEAD -- <path>` with env `GIT_EXTERNAL_DIFF=<binary>`, `DFT_DISPLAY=json`, `DFT_UNSTABLE=yes`.
  - `Renamed { from }`: `git diff HEAD -- <from> <path>` (both paths, or git reports `created`).
  - `Untracked`: `<binary> --display json /dev/null <path>` with `DFT_UNSTABLE=yes`.
  - Run from the repo root. Empty stdout or non-zero exit → `DiffError`.
- Serde types for the JSON with unknown fields ignored. Fields: `status` (`changed|created|deleted|unchanged`), optional `aligned_lines: Vec<[Option<u32>; 2]>`, optional `chunks: Vec<Vec<Entry>>` where `Entry { lhs: Option<SideEntry>, rhs: Option<SideEntry> }`, `SideEntry { line_number: u32, changes: Vec<Change { start: usize, end: usize }> }`. Ignore `language`, `path`, `content`, `highlight`.

### `diff/model.rs`

Types from §8: `FileDiff`, `SideText`, `LineKind`, `Row`, plus `RowRange` (half-open `start..end` over rows). `SideText::highlights` uses the `StyledSpan` type task 04 will define; for now define `pub struct StyledSpan` as a placeholder in `highlight/highlighter.rs` with a `range: Range<usize>` field so task 04 extends it rather than moving it.

### `diff/align.rs`

`pub fn build(raw: &RawDiff, content: &FileContent, tab_width: usize) -> FileDiff`, following §8:

1. **Normalize each side**: split into lines on `\n`, strip a trailing `\r` per line, expand tabs to the next multiple of `tab_width` columns. A trailing newline does not create an extra empty line (`"a\nb\n"` → 2 lines; `"a\nb"` → 2 lines; `""` → 0 lines).
2. **Convert spans**: difft offsets are bytes into the raw line (tab = 1 byte). Map each offset to the byte offset in the tab-expanded line. Drop zero-width spans. Clamp out-of-range spans to the line length instead of panicking (log at `warn`).
3. **Rows**: `changed` → from `aligned_lines`; any index ≥ that side's line count becomes `None`; drop rows that are then `(None, None)`. `created`/`deleted` → one-sided rows. `unchanged` → rows paired 1:1 (sides have equal line counts; if they don't, pair by index and pad).
4. **Line kinds**: line in a chunk and its row has both sides → `Changed`; line in a chunk with no opposite side in its row → `Removed` (lhs) / `Added` (rhs); everything else `Unchanged`, including unpaired lines outside chunks (reformatting).
5. **Hunks**: maximal runs of rows where either side's line is not `Unchanged`.

## Tests

Fixtures are in `tests/fixtures/difft/<case>/` with `old.*`, `new.*`, `difft.json` (captured from difft 0.71.0). Load them with `include_str!` from `align.rs` tests.

- `mod`: assert row count (no phantom trailing row), a `(None, Some(1))` row for the added `use std::fmt;`, a `(Some(17), None)` removed row, the `Changed` kinds on lhs line 2 / rhs line 3 (lhs has `changes: []`), and 4 hunks (rows 1, 3, 5 and the run 17–18; unchanged lines split runs).
- `reformat`: rhs lines 1–6 have `lhs: None` rows and kind `Unchanged`; one hunk (the `let y` change).
- `uni`: a span over `αβγ` maps to the right byte range in the expanded line.
- `tab`: the changed `1` on line 1 maps from raw byte 9 to byte `9 - 1 + tab_width` with `tab_width = 4`.
- Inline JSON: `{"path":"a","language":"Text","status":"created"}` builds all-`Added` one-sided rows; same for `deleted`; a zero-width span is dropped.
- `check_version` parsing: `Difftastic 0.71.0` ok, `Difftastic 0.70.2` too old.

## Acceptance

- `DifftEngine::diff` + `align::build` produce a `FileDiff` for every viewable file in a real repo with mixed changes (check with a temporary debug log, run with `REVIEWY_LOG=debug`; remove it after).
- Startup fails with the install hint when `config.difft.binary` points to a missing binary.
- fmt, clippy, tests clean.

## Handoff notes

### Public interfaces

- `diff::DiffEngine::diff(&self, root: &Path, target: DiffTarget, file: &ChangedFile) -> Result<RawDiff, DiffError>`.
- `diff::DiffError::{NotInstalled { binary, source }, TooOld(Version), BadVersion(String), Command { cmd, stderr }, Json(serde_json::Error), Io(io::Error)}`.
- `diff/difft.rs`: `DifftEngine { pub binary: PathBuf }`; `check_version(binary: &Path) -> Result<Version, DiffError>`; `Version(u32, u32, u32)` (`Ord`, `Display`); `MIN_VERSION`. `RawDiff { status: Status, aligned_lines: Vec<[Option<u32>; 2]>, chunks: Vec<Vec<Entry>> }`, `Status::{Changed, Created, Deleted, Unchanged}`, `Entry { lhs, rhs: Option<SideEntry> }`, `SideEntry { line_number: u32, changes: Vec<Change> }`, `Change { start, end: usize }`.
- `diff/model.rs`: `FileDiff { lhs, rhs: Option<SideText>, rows: Vec<Row>, hunks: Vec<RowRange> }`, `SideText { lines, kinds, changes: Vec<Vec<Range<usize>>>, highlights: Vec<Vec<StyledSpan>> }`, `LineKind::{Unchanged, Changed, Added, Removed}`, `Row { lhs, rhs: Option<u32> }`, `type RowRange = Range<usize>`.
- `diff::align::build(raw: &RawDiff, content: &FileContent, tab_width: usize) -> FileDiff`.
- `highlight::highlighter::StyledSpan { pub range: Range<usize> }` placeholder.

### Decisions and deviations

- **Version check runs after config load** (spec §4 steps 4/5 swapped): the binary path comes from `config.difft.binary`. It runs after logging init and logs the version at `debug`.
- **`target: DiffTarget` added to `diff`** (task and spec omitted it); the git invocation uses `target.rev()`. Spec §7 updated.
- **Empty stdout with exit 0 → `Status::Unchanged`**, not an error: git doesn't call the external diff when the worktree matches `HEAD` (staged change reverted). Verified on git with a `1 MM` record. Spec §7 and task 02's handoff note corrected.
- **`RawDiff` is the serde type itself**; no second set of types. Still only used inside `diff/` and by callers passing it to `build`.
- **Version parsing:** first whitespace-separated word on the first line that reads as `X.Y.Z` after trimming leading non-digits (`v0.71.0`, `difftastic0.71.0`, `0.73.0-dev` all parse). No `regex` dependency.
- **Sides from content, rows/kinds from difft.** Mismatches (file edited between `read_content` and diffing) clamp and log one `warn`; lines can then be missing from rows. Accepted: the workflow is review, close, then edit. Task 11's "rows cover each line exactly once" holds for files not edited mid-run.
- **Tab stops by display width** (`unicode-width`), consistent with `vim::wrap`. `tab_width = 0` is treated as 1.
- **Spans snapped to char boundaries** after clamping (lossy UTF-8 decoding shifts difft's byte offsets), so slicing `lines[i]` with a span never panics.
- `CR` stripped per line (`str::lines()` plus a trailing lone `\r`).
- **`RowRange` is `Range<usize>`** (type alias), not a new struct.
- **Dead code:** one `#[allow(dead_code, reason = "used by task 07; remove then")]` on `mod diff;` in `main.rs` (`expect` fails under `cargo test` since tests use the items). `StyledSpan` has its own `allow` for task 04. Tasks 04 and 07 updated.
- Chunk `line_number` is 0-based (spec §7 now says so).
- Verified on a scratch repo: modified, deleted, staged rename, staged add, untracked, empty untracked, CRLF+tabs, staged-then-reverted all build a `FileDiff`; a missing binary fails startup with the install hint.

### Left undone / flagged

- **Tabs in comments:** `SideText::lines` are tab-expanded, so `selected_text`/context built from them (tasks 06/09) contain spaces where the source has tabs. Flagged as an open question in task 06.
- A `.gitattributes` `diff=<driver>` with `diff.<driver>.command` overrides `GIT_EXTERNAL_DIFF`; the output then fails JSON parsing → per-file error. Accepted.
