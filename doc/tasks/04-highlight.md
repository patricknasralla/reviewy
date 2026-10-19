# Task 04 — Syntax highlighting and themes

## Goal

Implement `src/highlight/`: load Zed-format themes, highlight each side of a file with tree-sitter into per-line styled spans, and give the UI the colors it needs (editor colors, diff tints).

Read first: `doc/spec.md` §9 (all of it; it records the verified crate versions, query combinations and capture mapping), §13 (`ui.theme`, `ui.theme_variant`), §15. Read handoff notes in `doc/tasks/01`–`03`.

## Context

Reviewy renders files side by side like an editor. Each side (`SideText` in `diff/model.rs`, task 03) holds tab-expanded lines and an empty `highlights: Vec<Vec<StyledSpan>>` that this task fills. `StyledSpan` exists as a placeholder in `highlight/highlighter.rs`; extend it there and remove its `#[allow(dead_code)]`. The bundled theme is `default_theme/tokyo-night.json` (four variants; default **Tokyo Night Moon**).

## Deliverables

### `highlight/theme.rs`

- Parse a Zed theme family: `{ themes: [ { name, appearance, style: { ..., syntax: { <key>: { color, font_style, font_weight } } } } ] }`. Unknown fields ignored. Colors are `#RRGGBB` or `#RRGGBBAA`; `null` or missing allowed.
- `Theme` (one resolved variant) exposes at least:
  - `background`, `foreground`, `line_number`, `active_line_background`: `ratatui::style::Color` (RGB).
  - `added`, `deleted`, `modified`, `conflict`: `version_control.added|deleted|modified|conflict`, falling back to `created|deleted|modified|conflict`.
  - Diff tints, precomputed: `line_added_bg`, `line_deleted_bg` (the added/deleted color blended onto `background` at alpha 0.15) and `token_added_bg`, `token_deleted_bg` (alpha 0.35). Make the two alphas `const`s.
  - `syntax_keys() -> Vec<String>` and `syntax_style(index) -> ratatui::style::Style` (fg plus italic for `font_style: "italic"`, bold for `font_weight >= 700`).
- `#RRGGBBAA` is blended onto `editor.background` at load time.
- Resolution (§9): `ui.theme` is either a variant name, searched across all families in `~/.config/reviewy/themes/*.json` (XDG via `etcetera`) then the bundled theme (`include_str!`), user themes shadowing bundled ones; or a path to a JSON file (contains `/` or ends in `.json`), in which case `ui.theme_variant` picks the variant. Not found → error listing available variant names.

### `highlight/languages.rs`

- `Language` enum (`Rust`, `Python`, `CSharp`, `TypeScript`, `Tsx`) and `fn detect(path: &Path) -> Option<Language>` by extension (§9). Also a `name()` giving the fence tag used in comments: `rust`, `python`, `csharp`, `typescript`, `tsx`.
- One `HighlightConfiguration` per language built exactly as the table in §9 (query concatenation order JS → JSX → TS matters). Build lazily, at most once per language per session.
- Before `configure()`: drop captures starting with `_`, `injection.`, `local.`, and `glimmer`; apply the remap table (`escape` → `string.escape`); then call `configure(&theme_keys)`, which handles longest-dotted-prefix matching.

### `highlight/highlighter.rs`

- `StyledSpan { range: Range<usize>, style: Style }`: byte range into the tab-expanded line.
- `fn highlight(lang: Language, lines: &[String], theme: &Theme) -> Vec<Vec<StyledSpan>>`: join lines with `\n`, run `Highlighter::highlight` (5 args in 0.27), split the resulting events into per-line spans. The injection callback returns the Rust config for `rust` and `None` for everything else.
- Highlight errors → log at `warn`, return empty spans (plain text). Never fail the file open.
- Wire: after `align::build`, fill `lhs.highlights` / `rhs.highlights` for files with a detected language. Expose this as one function the app (task 07) calls, e.g. `highlight::apply(&mut FileDiff, path, &Theme)`.

### `main.rs`

Resolve the theme at startup after config. Errors abort startup with a message.

## Tests

- Theme: parse the bundled theme; resolve `Tokyo Night Moon`; `added` falls back to `created` (Moon has no `version_control.*`); a `#RRGGBB80` color blends to the expected RGB over a known background; italic/bold mapping.
- Highlighter: one test per language highlighting a 2–3 line snippet; assert a keyword span exists with the theme's `keyword` style and that spans never cross line boundaries.

## Acceptance

- Highlighting a real 1000-line Rust file takes well under 100 ms in a release build (measure once, note the number in handoff notes).
- `ui.theme = "Tokyo Night Storm"` resolves; an unknown name errors with the list of names.
- fmt, clippy, tests clean.

## Handoff notes

_To be filled in by the implementing agent._
