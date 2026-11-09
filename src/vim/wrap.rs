//! Soft-wrap layout shared by the diff view and the editor.

use std::ops::Range;

use unicode_linebreak::linebreaks;
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

/// Byte ranges of the screen segments, broken at UAX #14 opportunities (spec §10 Soft wrap).
pub fn wrap_line(line: &str, wrap: Wrap) -> Vec<Range<usize>> {
    let mut segments = Vec::new();
    let (mut start, mut col, mut piece_start) = (0, 0, 0);
    for end in piece_ends(line) {
        let piece = &line[piece_start..end];
        let piece_w: usize = piece.chars().map(char_width).sum();
        if col + piece_w <= wrap.row_width(segments.len()) {
            col += piece_w;
        } else if col > 0 && piece_w <= wrap.row_width(segments.len() + 1) {
            segments.push(start..piece_start);
            (start, col) = (piece_start, piece_w);
        } else {
            // Wider than a whole row: fill the current row, then break by char.
            for (i, c) in piece.char_indices() {
                let char_w = char_width(c);
                if col > 0 && col + char_w > wrap.row_width(segments.len()) {
                    segments.push(start..piece_start + i);
                    (start, col) = (piece_start + i, 0);
                }
                col += char_w;
            }
        }
        piece_start = end;
    }
    segments.push(start..line.len());
    segments
}

/// Byte offsets where pieces end: every UAX #14 break opportunity, the last being the line
/// end. Mandatory breaks are treated like allowed ones, so they never force a row.
fn piece_ends(line: &str) -> impl Iterator<Item = usize> + '_ {
    linebreaks(line).map(|(i, _)| i)
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

    fn rows(line: &str, width: u16) -> Vec<&str> {
        let wrap = Wrap { width, indent: 0 };
        wrap_line(line, wrap)
            .into_iter()
            .map(|r| &line[r])
            .collect()
    }

    #[test]
    fn breaks_at_words_keeping_punctuation() {
        assert_eq!(rows("see this. foo.bar", 10), ["see this. ", "foo.bar"]);
        assert_eq!(rows("src/vim/wrap.rs", 10), ["src/vim/", "wrap.rs"]);
    }

    #[test]
    fn trailing_space_counts_toward_width() {
        // "abcd efgh" fits in 9, but its trailing space doesn't.
        assert_eq!(rows("abcd efgh ij", 9), ["abcd ", "efgh ij"]);
    }

    #[test]
    fn long_word_fills_the_row_then_hard_breaks() {
        assert_eq!(
            rows("ab cdefghijklm no", 5),
            ["ab cd", "efghi", "jklm ", "no"]
        );
        assert_eq!(rows(&"a".repeat(25), 10).len(), 3);
        // Normal breaking resumes: "m" joins the hard-broken tail row.
        assert_eq!(rows("abcdefghijkl m", 5), ["abcde", "fghij", "kl m"]);
    }

    #[test]
    fn mandatory_breaks_never_force_a_row() {
        for brk in ["\x0c", "\x0b", "\r", "\u{85}", "\u{2028}", "\u{2029}"] {
            let line = format!("ab{brk}cd");
            assert_eq!(rows(&line, 20), [line.as_str()], "{brk:?}");
        }
    }

    #[test]
    fn empty_line_is_one_row() {
        assert_eq!(rows("", 10), [""]);
    }

    #[test]
    fn hard_break_keeps_char_rules() {
        // Word joiners keep the run one piece, so the wide char is placed by the hard break.
        let line = "abcdefghi\u{2060}世\u{2060}x";
        assert_eq!(rows(line, 10), ["abcdefghi\u{2060}", "世\u{2060}x"]);
        assert_eq!(to_screen(line, W10, 11), (1, 2));
        assert_eq!(from_screen(line, W10, 1, 1), 10);
        assert_eq!(rows("a世b", 1), ["a", "世", "b"]);
    }
}
