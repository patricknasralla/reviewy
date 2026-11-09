# 04: Continuation marker and indent in the comment editor

**What to build:** the comment editor shows the same `↪` marker and `ui.wrap_indent` indent on continuation rows as the diff view, with the cursor, selection and editing behaving correctly around it. The trailing empty row for an exactly full line stays markerless (spec §10 Comment editor).

**Blocked by:** 03: Continuation marker and indent in the diff view

**Status:** todo

- [ ] `Editor::new` takes `wrap_indent` (passed from `app/comment.rs`); the editor holds a `Wrap`, `resize` sets its width.
- [ ] Continuation rows draw `↪` in `editor.line_number` at the inner box's first cell, text from `offset`; narrow-box and `wrap_indent = 0` fall back to no marker.
- [ ] The trailing empty row appears only when a line's last row exactly fills its width (`W` or `W - offset`); it has no marker and the cursor sits at column 0.
- [ ] Normal/visual block cursor, Insert bar cursor, Insert Up/Down/PageUp/PageDown and scrolling land on the right cells with the offset.
- [ ] Visual selection highlighting on continuation rows starts at the text; yanks never contain the marker.
- [ ] Tests: trailing row only on exact fill (no marker, cursor at column 0), cursor mapping with the offset. `comment_editor` snapshot shows the marker.
