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

### Public interfaces

`highlight.rs` re-exports `Highlighter` and `Theme`.

- `highlight::theme`: `Theme { name, background, foreground, line_number, active_line_background, added, deleted, modified, conflict, line_added_bg, line_deleted_bg, token_added_bg, token_deleted_bg: Color, syntax: BTreeMap<String, Style> }` (all `Color::Rgb`, alpha already blended). `Theme::resolve(theme: &str, variant: Option<&str>) -> Result<Theme, ThemeError>`, `Theme::bundled_default() -> Theme`, `is_path(&str) -> bool`, `LINE_ALPHA = 0.15`, `TOKEN_ALPHA = 0.35`. `ThemeError::{NotFound, NoVariant, MissingKey, BadColor, Json, Io, Home}`.
- `highlight::languages::Language::{Rust, Python, CSharp, TypeScript, Tsx}`: `detect(&Path) -> Option<Language>`, `name() -> &'static str` (fence tag).
- `highlight::highlighter`: `StyledSpan { range: Range<usize>, style: Style }`; `Highlighter::new(&Theme)`, `Highlighter::apply(&self, &mut FileDiff, &ChangedFile)`.
- `defaults::default_theme::{FAMILY, VARIANT}`: the bundled family JSON (`include_str!`) and the default variant name.
- `config::UiConfig::theme` is now `Option<String>`.
- `App` gained `theme: Theme`, `highlighter: Highlighter`, `status: Option<String>`; no longer `Default`, built in full in `main`.

### Decisions and deviations

- **`Highlighter` struct instead of `highlight::apply(&mut FileDiff, path, &Theme)`**: `configure()` indices point into the theme's key list, so the configs belong with the theme's styles. Built once in `main`, owned by `App`; each language's `HighlightConfiguration` is built on first use (`OnceCell`). Spec §9 updated.
- **No capture-drop step**: `configure()` can't exclude captures; `_*`, `injection.*`, `local.*`, `glimmer` stay unstyled because no theme key matches them. **Remap** is done by passing `escape` to `configure()` after the theme keys, with `string.escape`'s style. `configure()` matches keys whose dotted parts are a subset of the capture's parts (not a prefix). Spec §9 updated.
- **Default theme lives in `src/defaults/default_theme.rs`, not `defaults.toml`** (developer decision): its name and its compiled-in JSON must change together. `src/config/` renamed to `src/defaults/` (holds `defaults.toml`, `default_theme.rs`, `tokyo-night.json`); `ui.theme` is unset by default and `Option<String>`; unset → `Theme::bundled_default()`. License moved to `3rd_party/tokyo-night.LICENSE`. Spec §5, §9, §13 and tasks 01/11 updated.
- **`Theme::syntax` is a `BTreeMap`**: deterministic order for `configure()`, which takes the first of equally good matches.
- **Theme failure falls back** (developer decision; replaces "unknown name → error listing names"): any `resolve` error is logged at `warn`, `Theme::bundled_default()` is used, and `App::status` is set to `"<error>; using <name>"` for task 07 to show. A broken file in the user themes dir also triggers the fallback. Spec §9 updated, task 07 updated.
- **Theme path relative to the config file** that sets it: `config.rs` rewrites a relative path-like `ui.theme` against the file's directory before merging (task 01 module change). Spec §13 updated.
- **Required theme keys**: `editor.background`, `editor.foreground`, `editor.line_number`, `editor.active_line.background`, and added/deleted/modified/conflict (`version_control.*`, then `created/deleted/modified/conflict`). Missing → `MissingKey` (→ fallback). Syntax entries without a color keep only their modifiers.
- **Highlighting runs on `SideText::lines`** (CR-stripped, tab-expanded, joined with `\n`), so span offsets need no mapping. The innermost capture wins; unstyled text has no span.
- **Renames**: lhs language detected from `Renamed { from }`.
- **C#** uses highlights only, per the spec table, although the 0.23.5 crate ships injections and locals queries. Accepted by the developer for v1 (C# editors often rely on Roslyn analyzers rather than tree-sitter).
- **Dead code**: `#[allow(dead_code, reason = "used by task 07; remove then")]` on `mod highlight;` in `main.rs`; `#[expect(dead_code)]` on the three new `App` fields. Task 07 updated to remove them.
- **Timing** (release, 1000 lines of this crate's Rust): 23 ms on first use (includes building the Rust config), ~8 ms after.

### Left undone

- `ui.theme` paths don't expand `~`.
