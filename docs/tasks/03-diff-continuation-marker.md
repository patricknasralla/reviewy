# 03: Continuation marker and indent in the diff view

**What to build:** in the diff view, every continuation row of a wrapped line starts with a `↪` marker followed by `ui.wrap_indent` spaces before the text (spec §10 Soft wrap, Continuation rows), so wrapped lines are easy to tell apart from new lines. Configurable via `ui.wrap_indent` (default 1, `0` disables).

**Blocked by:** 02: Word wrap at UAX #14 break points

**Status:** todo

- [ ] `ui.wrap_indent` (`u16`) in `UiConfig` and `defaults.toml` (`wrap_indent = 1`); config tests updated; README config example updated.
- [ ] `Wrap::offset()` returns `indent + 1`, or 0 when `indent == 0` or `indent + 1 > width / 2`; continuation rows are `W - offset` wide.
- [ ] Diff view builds `Wrap { width: col.text, indent: config.ui.wrap_indent }` per column, so the two columns can differ.
- [ ] Continuation rows draw `↪` at the text column's first cell in `editor.line_number`, text from `offset`; the line/cursor-line tint covers the marker cells; the gutter stays blank.
- [ ] Selection highlighting on continuation rows starts at the text, not the marker; the cursor and `j`/`k` screen-row motions land on the right cells.
- [ ] The marker never appears in yanked text, the visual prefill or comment excerpts.
- [ ] Unit tests: offset rules (including too narrow and `wrap_indent = 0`), `to_screen`/`from_screen` with the offset. `modified_wrap_on` snapshot shows the marker and a selection on a continuation row.
