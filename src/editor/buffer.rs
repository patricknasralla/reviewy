//! Editable text: lines of chars with position-based edits.

use std::ops::Range;

use crate::vim::motion::{Pos, TextView};

/// Always at least one line.
pub struct Buffer {
    lines: Vec<String>,
}

impl Buffer {
    pub fn from_text(text: &str) -> Self {
        Self {
            lines: text.split('\n').map(String::from).collect(),
        }
    }

    pub fn text(&self) -> String {
        self.lines.join("\n")
    }

    pub fn lines(&self) -> &[String] {
        &self.lines
    }

    /// Line length in chars.
    pub fn len(&self, line: usize) -> usize {
        self.lines[line].chars().count()
    }

    /// Inserts `text` (may contain `\n`) at `pos`; returns the position after it.
    pub fn insert(&mut self, pos: Pos, text: &str) -> Pos {
        let at = byte(&self.lines[pos.line], pos.col);
        let tail = self.lines[pos.line].split_off(at);
        let mut parts = text.split('\n');
        self.lines[pos.line].push_str(parts.next().unwrap());
        let mut last = pos.line;
        for part in parts {
            last += 1;
            self.lines.insert(last, part.to_owned());
        }
        let end = Pos {
            line: last,
            col: self.len(last),
        };
        self.lines[last].push_str(&tail);
        end
    }

    /// Removes `range` (end exclusive; joins lines when it spans several) and returns the
    /// removed text.
    pub fn delete(&mut self, range: Range<Pos>) -> String {
        let Range { start, end } = range;
        let from = byte(&self.lines[start.line], start.col);
        if start.line == end.line {
            let to = byte(&self.lines[start.line], end.col);
            return self.lines[start.line].drain(from..to).collect();
        }
        let to = byte(&self.lines[end.line], end.col);
        let tail = self.lines[end.line].split_off(to);
        let mut removed = self.lines[start.line].split_off(from);
        for line in self.lines.drain(start.line + 1..=end.line) {
            removed.push('\n');
            removed.push_str(&line);
        }
        self.lines[start.line].push_str(&tail);
        removed
    }
}

impl TextView for Buffer {
    fn line_count(&self) -> usize {
        self.lines.len()
    }

    fn line(&self, idx: usize) -> &str {
        &self.lines[idx]
    }
}

/// Byte offset of char `col`; the line length past the end.
pub fn byte(line: &str, col: usize) -> usize {
    line.char_indices().nth(col).map_or(line.len(), |(i, _)| i)
}
