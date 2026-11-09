# 03: Continuation marker in the diff view gutter

**What to build:** in the diff view, every continuation row of a wrapped line shows `↪` in the gutter in place of the line number (spec §10 Diff view), so wrapped lines are easy to tell apart from new lines. The text column has no continuation indent.

**Blocked by:** 02: Word wrap at UAX #14 break points

**Status:** done

- [x] Continuation rows draw `↪` right-aligned in the line number's place, in `editor.line_number`; the rest of the gutter stays blank; text starts at the text column's first cell.
- [x] The marker never appears in yanked text, the visual prefill or comment excerpts.
- [x] `Wrap` keeps only `width`: no `indent`, offset or per-row x; no `ui.wrap_indent` config.
- [x] `modified_wrap_on` snapshot shows the marker and a selection on a continuation row.
