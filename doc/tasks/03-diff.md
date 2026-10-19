# Task 03 — Diff engine and alignment

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
- `check_version(binary) -> Result<Version, DiffError>`: run `<binary> --version`, parse `Difftastic X.Y.Z` (first line). Missing binary or version < 0.71.0 → error with an install hint (`cargo install difftastic`). Wire into `main.rs` as startup step 4 (§4).
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

- `DifftEngine::diff` + `align::build` produce a `FileDiff` for every viewable file in a real repo with mixed changes (check with a temporary debug log; remove it after).
- Startup fails with the install hint when `config.difft.binary` points to a missing binary.
- fmt, clippy, tests clean.

## Handoff notes

_To be filled in by the implementing agent._
