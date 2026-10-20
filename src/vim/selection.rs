//! Visual selection ranges, shared by the diff view and the editor.

use std::ops::Range;

use super::motion::Pos;

/// The selection's first and last positions (inclusive), in text order.
pub fn bounds(anchor: Pos, cursor: Pos) -> (Pos, Pos) {
    let key = |p: Pos| (p.line, p.col);
    if key(anchor) <= key(cursor) {
        (anchor, cursor)
    } else {
        (cursor, anchor)
    }
}

/// Selected chars of `line`, half-open; may extend past the line end. `None` outside the
/// selection.
pub fn line_cols(anchor: Pos, cursor: Pos, linewise: bool, line: usize) -> Option<Range<usize>> {
    let (start, end) = bounds(anchor, cursor);
    if !(start.line..=end.line).contains(&line) {
        return None;
    }
    if linewise {
        return Some(0..usize::MAX);
    }
    let from = if line == start.line { start.col } else { 0 };
    let to = if line == end.line {
        end.col + 1
    } else {
        usize::MAX
    };
    Some(from..to)
}
