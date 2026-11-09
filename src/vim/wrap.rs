//! Soft-wrap layout shared by the diff view and the editor.

use std::ops::Range;

use unicode_width::UnicodeWidthChar;

pub fn char_width(c: char) -> usize {
    c.width().unwrap_or(0)
}

/// Soft-wrap layout: `width` is the text column width, `indent` is `ui.wrap_indent`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Wrap {
    pub width: u16,
    pub indent: u16,
}

impl Wrap {
    /// Screen x where continuation text starts.
    pub fn offset(self) -> usize {
        0
    }

    /// Screen x where segment `seg`'s text starts.
    pub fn row_x(self, seg: usize) -> usize {
        if seg == 0 { 0 } else { self.offset() }
    }

    /// Text width of segment `seg`; at least 1.
    pub fn row_width(self, seg: usize) -> usize {
        usize::from(self.width)
            .saturating_sub(self.row_x(seg))
            .max(1)
    }
}

/// Byte ranges of the screen segments. A char wider than its row gets a segment of its own.
pub fn wrap_line(line: &str, wrap: Wrap) -> Vec<Range<usize>> {
    let mut segments = Vec::new();
    let (mut start, mut col) = (0, 0);
    for (i, c) in line.char_indices() {
        let w = char_width(c);
        if col > 0 && col + w > wrap.row_width(segments.len()) {
            segments.push(start..i);
            (start, col) = (i, 0);
        }
        col += w;
    }
    segments.push(start..line.len());
    segments
}

pub fn line_height(line: &str, wrap: Wrap) -> usize {
    wrap_line(line, wrap).len()
}

/// Display column where char `col` starts.
pub fn display_col(line: &str, col: usize) -> usize {
    line.chars().take(col).map(char_width).sum()
}

/// Char index covering display column `dcol`; the last char if `dcol` is past the end.
pub fn char_at_display(line: &str, dcol: usize) -> usize {
    let mut x = 0;
    let mut last = 0;
    for (i, c) in line.chars().enumerate() {
        x += char_width(c);
        last = i;
        if x > dcol {
            break;
        }
    }
    last
}

/// Char `col` → (segment index, screen x including the segment's `row_x`).
pub fn to_screen(line: &str, wrap: Wrap, col: usize) -> (usize, usize) {
    let byte = line.char_indices().nth(col).map_or(line.len(), |(i, _)| i);
    let segments = wrap_line(line, wrap);
    let last = segments.len() - 1;
    let seg = segments.iter().position(|r| byte < r.end).unwrap_or(last);
    let start = segments[seg].start;
    let x: usize = line[start..byte].chars().map(char_width).sum();
    (seg, wrap.row_x(seg) + x)
}

/// (segment, screen x) → char index, clamped to the segment; x before the segment's text
/// gives its first char.
pub fn from_screen(line: &str, wrap: Wrap, seg: usize, x: usize) -> usize {
    let segments = wrap_line(line, wrap);
    let seg = seg.min(segments.len() - 1);
    let range = segments[seg].clone();
    let dcol = x.saturating_sub(wrap.row_x(seg));
    line[..range.start].chars().count() + char_at_display(&line[range], dcol)
}

#[cfg(test)]
mod tests {
    use super::*;

    const W10: Wrap = Wrap {
        width: 10,
        indent: 0,
    };

    #[test]
    fn wraps_by_width() {
        assert_eq!(wrap_line(&"a".repeat(25), W10), [0..10, 10..20, 20..25]);
    }

    #[test]
    fn wide_char_moves_to_next_segment() {
        let line = "abcdefghi世x";
        assert_eq!(wrap_line(line, W10), [0..9, 9..13]);
        assert_eq!(to_screen(line, W10, 10), (1, 2));
        assert_eq!(from_screen(line, W10, 1, 1), 9);
    }
}
