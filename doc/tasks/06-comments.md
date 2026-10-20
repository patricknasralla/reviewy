# Task 06 — Comments model and output

## Goal

Implement `src/comments/`: the comment data model, JSON load/save, and markdown rendering for the agent. No UI in this task.

Read first: `doc/spec.md` §12 (all of it), §13 (`[comments]`). Read handoff notes in `doc/tasks/01-scaffold.md`.

## Context

The reviewer selects lines or text in the diff view and writes a comment. On exit, comments are written to `.reviewy/comments.json` (source of truth, read back on the next run) and `.reviewy/comments.md` (pasted into an AI agent session, never parsed back). The md must contain everything an LLM needs to act on each comment.

## Deliverables

### `comments/model.rs`

- Types from §12: `CommentSet`, `Base { ref_name: String /* serialized "ref" */, commit: String }`, `Comment`, `Side` (`Old`/`New`, serialized lowercase), `LineRange { start, end }` (1-based, inclusive), `ColRange` (1-based, inclusive, chars), `Context`, `NumberedLine`. Serialized names exactly as the JSON example in §12.
- `CommentSet` methods:
  - `new(base: Base)`.
  - `add(draft: CommentDraft) -> u32`: assigns `next_id` (ids only increase), returns the id.
  - `get(id)`, `get_mut(id)`, `remove(id)`.
  - `at(file, side, line) -> Option<&Comment>` (first comment covering that 1-based line).
  - `overlaps(file, side, lines) -> Option<u32>`: id of an existing comment on the same file and side whose range intersects (used by the UI to refuse new overlapping comments).
  - `count_for(file) -> usize` (explorer).
- `CommentDraft` holds everything except `id`. A helper builds it from a selection:
  `fn draft(file, side, lines, cols: Option<ColRange>, language: Option<&str>, side_lines: &[String], context_lines: usize, body: String) -> CommentDraft`. It fills `selected_text` (the selected chars for charwise selections, whole lines otherwise) and `context` (up to N lines before/after on the same side, 1-based numbers).
  - **Open question (from task 03):** `SideText::lines` are tab-expanded, so `selected_text` and context built from them contain spaces where the source has tabs (matters for Makefiles, Go). Decide with the developer whether to keep raw lines in `SideText` (plus a raw ↔ expanded column mapping in task 09) or accept it.
- `fn prefill_body(selected_text, language: Option<&str>) -> String`: fenced code block tagged with the language, using a backtick fence one longer than the longest backtick run in the text (minimum 3), followed by a newline so the cursor starts on an empty line below the fence.

### `comments/store.rs`

- `load(path) -> Result<Option<CommentSet>, CommentsError>`: `None` if the file doesn't exist. Reject `version != 1` with a clear error.
- `save(dir, set: &CommentSet, preamble: Option<&str>) -> Result<(), CommentsError>`: writes `comments.json` (pretty) and `comments.md`, each to a temp file in `dir` then `rename`. If the set has no comments, delete both files instead (missing files are fine).

### `comments/render_md.rs`

- `fn render(set: &CommentSet, preamble: Option<&str>) -> String` exactly following the `comments.md` layout in §12: title, preamble (default text from §12 unless overridden), base commit (short 8-char SHA + ref), one `##` section per file, comments sorted by file then start line.
- Heading forms: `### #3 — line 42, cols 9–20 (new)`, `### #5 — lines 10–12 (old: base commit version)`; cols only for charwise comments.
- Context block: fenced with the language tag, right-aligned line numbers, `>` marking selected lines, `  ` otherwise. Fence length chosen as in `prefill_body`.
- `Selected: \`…\`` line for charwise comments only (single-line selections; for multi-line charwise selections show it as a fenced block instead).
- Body verbatim after it.

## Tests

- JSON round-trip of a set with one new-side charwise comment and one old-side line comment is lossless, and matches the field names in §12's example.
- Overlap: same file/side intersecting → `Some(id)`; other side or adjacent non-intersecting range → `None`.
- Fence: text containing ```` ``` ```` gets a 4-backtick fence.
- `render`: one `insta` snapshot covering both comments above, across two files.
- `save` with an empty set deletes existing files.

## Acceptance

- fmt, clippy, tests clean.

## Handoff notes

### Public interfaces

- `comments::CommentsError::{Io { path, source }, Json { path, source }, Version(u32)}`.
- `comments::model`:
  - `VERSION = 1`; `CommentSet { version, base: Base, next_id, comments: Vec<Comment> }`; `Base { ref_name /* "ref" */, commit }`.
  - `Comment { id, anchor: Anchor, language: Option<String>, excerpt: Excerpt, body }`; `Comment::selected_text() -> String`.
  - `Anchor { file: PathBuf, side: Side, selection: Selection }`; `Side::{Old, New}` (`Copy`).
  - `Selection::{Line { line }, Lines { start, end }, Chars { start: Pos, end: Pos }}` (serde `kind` tag); `from_lines(a, b)`, `from_chars(a, b)` (order the ends), `line_range() -> RangeInclusive<u32>`, `Display` = md heading text. `Pos { line, col }` (`Ord`).
  - `Excerpt { start, lines: Vec<String> }`, `Excerpt::line(n) -> &str` ("" outside).
  - `CommentSet::{new(Base), add(CommentDraft) -> u32, get, get_mut, remove(id) -> Option<Comment>, at(&Path, Side, line), overlaps(&Anchor) -> Option<u32>, count_for(&Path)}`.
  - `CommentDraft { anchor, language, excerpt, body }`; `draft(anchor, language: Option<&str>, side_lines: &[String], context_lines: usize, body: String) -> CommentDraft`.
  - `fence(text) -> String`, `prefill_body(text, language: Option<&str>) -> String` (ends with `\n`: cursor on the empty line after the fence).
- `comments::store::{load(dir) -> Result<Option<CommentSet>, _>, save(dir, &CommentSet, preamble: &str) -> Result<(), _>}`: both take the `.reviewy/` dir.
- `comments::render_md::render(&CommentSet, preamble: &str) -> String`.

### Decisions and deviations (developer decisions; spec §5, §12 rewritten)

- **Selection enum** replaces `LineRange` + `ColRange`: one variant per md heading form, invalid states unrepresentable. `Block` would be a future variant.
- **`Anchor`** groups file/side/selection; it is task 08's comment target (`CommentTarget` dropped; task 08 updated). JSON nests it (no `flatten`).
- **`Excerpt` replaces `Context`/`NumberedLine`/`selected_text`**: contiguous lines (selected + context), numbered from `start`. Fixes charwise comments, whose full selected lines had nowhere to live. Selected text is derived, never stored.
- **Markdown:** ASCII only; headings `#3 - line 42`, `#4 - lines 52-53`, `#5 - line 60 col 3 - line 60 col 7`; old side adds `(old: base commit version)`, new side unmarked (the preamble says so); no `Selected:` line. Sorted by file, start line, id.
- **Default preamble moved to `defaults.toml`**; `CommentsConfig::preamble: String` (task 01 module change, task 01 notes updated).
- **`load` checks `version` first** (small header struct), so a future schema gives the version error.
- **Tabs deferred:** excerpt text and cols are in tab-expanded space in v1. The fix is caller-only (task 11 updated).
- `next_id` kept (not derived from `len()`/max id) so deleted ids are never reused.
- Writes go to `.<name>.tmp` then `rename`; no fsync.
- **Dead code:** `#[allow(dead_code, reason = "used by task 09; remove then")]` on `mod comments;` in `main.rs` (task 09 updated).
- Extra test beyond the list: `selected_text` for a backwards multi-line `Chars` selection.

### Left undone

- `store::load` is not unit-tested here; task 11's `tests/comments.rs` round trip covers it.
