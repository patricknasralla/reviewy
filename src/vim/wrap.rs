//! Soft-wrap layout shared by the diff view and the editor.

use std::ops::Range;

use unicode_width::UnicodeWidthChar;

pub fn char_width(c: char) -> usize {
    c.width().unwrap_or(0)
}

/// Byte ranges of the screen segments. A char wider than `width` gets a segment of its own.
pub fn wrap_line(line: &str, width: u16) -> Vec<Range<usize>> {
    let max = usize::from(width.max(1));
    let mut segments = Vec::new();
    let (mut start, mut col) = (0, 0);
    for (i, c) in line.char_indices() {
        let w = char_width(c);
        if col > 0 && col + w > max {
            segments.push(start..i);
            (start, col) = (i, 0);
        }
        col += w;
    }
    segments.push(start..line.len());
    segments
}

pub fn line_height(line: &str, width: u16) -> usize {
    wrap_line(line, width).len()
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

/// Char `col` → (segment index, display column within the segment).
pub fn to_screen(line: &str, width: u16, col: usize) -> (usize, usize) {
    let byte = line.char_indices().nth(col).map_or(line.len(), |(i, _)| i);
    let segments = wrap_line(line, width);
    let last = segments.len() - 1;
    let seg = segments.iter().position(|r| byte < r.end).unwrap_or(last);
    let start = segments[seg].start;
    (seg, line[start..byte].chars().map(char_width).sum())
}

/// (segment, display column) → char index, clamped to the segment.
pub fn from_screen(line: &str, width: u16, seg: usize, dcol: usize) -> usize {
    let segments = wrap_line(line, width);
    let range = segments[seg.min(segments.len() - 1)].clone();
    line[..range.start].chars().count() + char_at_display(&line[range], dcol)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wraps_by_width() {
        assert_eq!(wrap_line(&"a".repeat(25), 10), [0..10, 10..20, 20..25]);
    }

    #[test]
    fn wide_char_moves_to_next_segment() {
        let line = "abcdefghi世x";
        assert_eq!(wrap_line(line, 10), [0..9, 9..13]);
        assert_eq!(to_screen(line, 10, 10), (1, 2));
        assert_eq!(from_screen(line, 10, 1, 1), 9);
    }
}
