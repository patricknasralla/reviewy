# 02: Word wrap at UAX #14 break points

**What to build:** long lines in the diff view and the comment editor wrap at word breaks instead of mid-word, following spec §10 Soft wrap (Breaking). Punctuation stays with its word (`this.`), a word whose trailing space would overflow moves to the next row, and a word wider than the row fills the current row before hard-breaking.

**Blocked by:** 01: Thread a `Wrap` layout parameter through the wrapping code

**Status:** done

- [x] `unicode-linebreak` added to `Cargo.toml` (approved by the developer); break opportunities come from `linebreaks()`, ignoring the end-of-line break.
- [x] `wrap_line` lays out pieces (break to break, trailing spaces included and counted) greedily: append if it fits, else new row if it fits an empty continuation row, else fill the current row and hard-break by char, then resume normal breaking.
- [x] Mandatory-break chars (form feed, vertical tab, lone `\r`, NEL, U+2028/U+2029) are ordinary break opportunities and never force a row.
- [x] Existing char rules still hold inside a hard break (wide char moves down; a char wider than the row gets its own row); an empty line is one row.
- [x] Unit tests in `vim/wrap.rs` cover the behaviours above (spec §16).
- [x] Wrap snapshots (`modified_wrap_on`, `comment_editor`, others affected) updated and reviewed: lines break at words.
- [x] Editor Insert mode: typing a space at the end of a full row moves the last word down; the trailing empty row still appears when a line exactly fills its row.
- [x] "Word Boundary Wrapping" removed from `docs/v1-followups.md`.
