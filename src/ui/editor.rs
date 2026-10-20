//! Comment editor widget: insert-only, soft-wrapped text buffer.

use std::ops::Range;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Frame;
use ratatui::layout::{Position, Rect};
use ratatui::style::Style;
use ratatui::widgets::{Block, Clear, Padding};

use super::centered;
use crate::diff::align::expand_tabs;
use crate::highlight::Theme;
use crate::vim::motion::TextView;
use crate::vim::wrap;

pub struct Editor {
    /// Id of the comment being edited.
    pub id: u32,
    pub title: String,
    lines: Vec<String>,
    /// (line, char col); the col may equal the line length.
    cursor: (usize, usize),
    /// Sticky display column for vertical moves.
    want: Option<usize>,
    /// First visible screen row.
    scroll: usize,
    width: u16,
    height: u16,
}

/// The editor box: centered, 70% × 50% of `area`.
pub fn rect(area: Rect) -> Rect {
    centered(area, area.width * 7 / 10, area.height / 2)
}

/// Text area inside the box.
pub fn inner(area: Rect) -> Rect {
    block().inner(rect(area))
}

fn block() -> Block<'static> {
    Block::bordered().padding(Padding::horizontal(1))
}

impl Editor {
    /// Cursor at the end of `body`.
    pub fn new(id: u32, title: String, body: &str) -> Self {
        let lines: Vec<String> = body.split('\n').map(String::from).collect();
        let last = lines.len() - 1;
        let cursor = (last, lines[last].chars().count());
        Self {
            id,
            title,
            lines,
            cursor,
            want: None,
            scroll: 0,
            width: 1,
            height: 1,
        }
    }

    pub fn text(&self) -> String {
        self.lines.join("\n")
    }

    pub fn resize(&mut self, width: u16, height: u16) {
        (self.width, self.height) = (width.max(1), height.max(1));
        self.scroll_to_cursor();
    }

    /// Returns whether the text changed.
    pub fn key(&mut self, key: KeyEvent) -> bool {
        let plain = !key
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT);
        let (line, col) = self.cursor;
        let len = self.lines[line].chars().count();
        let page = self.height as isize;
        let mut changed = false;
        match key.code {
            KeyCode::Char(c) if plain => changed = self.insert(c.encode_utf8(&mut [0; 4])),
            KeyCode::Enter => changed = self.insert("\n"),
            KeyCode::Backspace => changed = self.backspace(),
            KeyCode::Delete => changed = self.delete(),
            KeyCode::Left if col > 0 => self.cursor.1 -= 1,
            KeyCode::Left if line > 0 => self.cursor = (line - 1, self.len(line - 1)),
            KeyCode::Right if col < len => self.cursor.1 += 1,
            KeyCode::Right if line + 1 < self.lines.len() => self.cursor = (line + 1, 0),
            KeyCode::Home => self.cursor.1 = 0,
            KeyCode::End => self.cursor.1 = len,
            KeyCode::Up => return self.vertical(-1),
            KeyCode::Down => return self.vertical(1),
            KeyCode::PageUp => return self.vertical(-page),
            KeyCode::PageDown => return self.vertical(page),
            _ => {}
        }
        self.want = None;
        self.scroll_to_cursor();
        changed
    }

    /// Inserts `text` at the cursor: line breaks normalized to `\n`, tabs expanded per pasted line.
    pub fn paste(&mut self, text: &str, tab_width: usize) {
        let text = text.replace("\r\n", "\n").replace('\r', "\n");
        let lines: Vec<String> = text
            .split('\n')
            .map(|l| expand_tabs(l, tab_width).0)
            .collect();
        self.insert(&lines.join("\n"));
        self.want = None;
        self.scroll_to_cursor();
    }

    pub fn draw(&self, frame: &mut Frame, area: Rect, theme: &Theme, focused: bool) {
        let rect = rect(area);
        let block = block()
            .title(format!(" {} ", self.title))
            .style(Style::new().bg(theme.background).fg(theme.foreground));
        let inner = block.inner(rect);
        frame.render_widget(Clear, rect);
        frame.render_widget(block, rect);
        let rows = (self.lines.iter())
            .flat_map(|l| self.segments(l).into_iter().map(move |r| &l[r]))
            .skip(self.scroll);
        let buf = frame.buffer_mut();
        for (y, text) in (inner.y..inner.bottom()).zip(rows) {
            buf.set_stringn(inner.x, y, text, usize::from(inner.width), Style::new());
        }
        let (seg, x) = self.locate();
        let row = self.tops()[self.cursor.0] + seg - self.scroll;
        if focused && row < usize::from(inner.height) {
            frame.set_cursor_position(Position::new(inner.x + x as u16, inner.y + row as u16));
        }
    }

    fn len(&self, line: usize) -> usize {
        self.lines[line].chars().count()
    }

    /// Inserts `text` (may contain `\n`) at the cursor; the cursor ends after it.
    fn insert(&mut self, text: &str) -> bool {
        let (line, col) = self.cursor;
        let at = byte(&self.lines[line], col);
        let tail = self.lines[line].split_off(at);
        let mut parts = text.split('\n');
        self.lines[line].push_str(parts.next().unwrap());
        let mut last = line;
        for part in parts {
            last += 1;
            self.lines.insert(last, part.to_owned());
        }
        self.cursor = (last, self.len(last));
        self.lines[last].push_str(&tail);
        true
    }

    fn backspace(&mut self) -> bool {
        let (line, col) = self.cursor;
        if col > 0 {
            let at = byte(&self.lines[line], col - 1);
            self.lines[line].remove(at);
            self.cursor.1 -= 1;
        } else if line > 0 {
            let rest = self.lines.remove(line);
            self.cursor = (line - 1, self.len(line - 1));
            self.lines[line - 1].push_str(&rest);
        } else {
            return false;
        }
        true
    }

    fn delete(&mut self) -> bool {
        let (line, col) = self.cursor;
        if col < self.len(line) {
            let at = byte(&self.lines[line], col);
            self.lines[line].remove(at);
        } else if line + 1 < self.lines.len() {
            let next = self.lines.remove(line + 1);
            self.lines[line].push_str(&next);
        } else {
            return false;
        }
        true
    }

    /// Moves `rows` screen rows, keeping the sticky display column. Never changes the text.
    fn vertical(&mut self, rows: isize) -> bool {
        let tops = self.tops();
        let (seg, x) = self.locate();
        let want = *self.want.get_or_insert(x);
        let total = tops[tops.len() - 1];
        let row = (tops[self.cursor.0] + seg)
            .saturating_add_signed(rows)
            .min(total - 1);
        let line = tops.partition_point(|&t| t <= row) - 1;
        self.cursor = (line, self.col_at(line, row - tops[line], want));
        self.scroll_to_cursor();
        false
    }

    /// Byte ranges of a line's screen rows. A line filling the width gets an empty trailing
    /// row, so a cursor at its end stays inside the box.
    fn segments(&self, line: &str) -> Vec<Range<usize>> {
        let mut segments = wrap::wrap_line(line, self.width);
        let last = segments[segments.len() - 1].clone();
        if !line.is_empty() && width(&line[last]) >= usize::from(self.width) {
            segments.push(line.len()..line.len());
        }
        segments
    }

    /// Screen row where each line starts, then the total row count.
    fn tops(&self) -> Vec<usize> {
        let mut tops = vec![0];
        for line in &self.lines {
            tops.push(tops[tops.len() - 1] + self.segments(line).len());
        }
        tops
    }

    /// Cursor → (row within its line, display column).
    fn locate(&self) -> (usize, usize) {
        let (line, col) = self.cursor;
        let text = &self.lines[line];
        let at = byte(text, col);
        let segments = self.segments(text);
        let seg = (segments.iter().position(|r| at < r.end)).unwrap_or(segments.len() - 1);
        (seg, width(&text[segments[seg].start..at]))
    }

    /// Char col at display column `want` in row `seg` of `line`; past the end, the row's last
    /// char (the line end on its last row).
    fn col_at(&self, line: usize, seg: usize, want: usize) -> usize {
        let text = &self.lines[line];
        let segments = self.segments(text);
        let range = segments[seg].clone();
        let before = text[..range.start].chars().count();
        let mut x = 0;
        for (i, c) in text[range.clone()].chars().enumerate() {
            x += wrap::char_width(c);
            if x > want {
                return before + i;
            }
        }
        let n = text[range].chars().count();
        if seg + 1 == segments.len() {
            before + n
        } else {
            before + n - 1
        }
    }

    fn scroll_to_cursor(&mut self) {
        let row = self.tops()[self.cursor.0] + self.locate().0;
        let height = usize::from(self.height);
        self.scroll = self.scroll.clamp((row + 1).saturating_sub(height), row);
    }
}

impl TextView for Editor {
    fn line_count(&self) -> usize {
        self.lines.len()
    }

    fn line(&self, idx: usize) -> &str {
        &self.lines[idx]
    }
}

fn byte(line: &str, col: usize) -> usize {
    line.char_indices().nth(col).map_or(line.len(), |(i, _)| i)
}

fn width(s: &str) -> usize {
    s.chars().map(wrap::char_width).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn editor(body: &str, width: u16) -> Editor {
        let mut e = Editor::new(1, String::new(), body);
        e.resize(width, 10);
        e
    }

    fn press(e: &mut Editor, code: KeyCode) -> bool {
        e.key(KeyEvent::from(code))
    }

    #[test]
    fn insert_and_newline_mid_line() {
        let mut e = editor("ac", 20);
        press(&mut e, KeyCode::Left);
        assert!(press(&mut e, KeyCode::Char('b')));
        assert!(press(&mut e, KeyCode::Enter));
        assert_eq!(e.text(), "ab\nc");
        assert_eq!(e.cursor, (1, 0));
    }

    #[test]
    fn backspace_at_line_start_joins() {
        let mut e = editor("ab\ncd", 20);
        press(&mut e, KeyCode::Home);
        assert!(press(&mut e, KeyCode::Backspace));
        assert_eq!(e.text(), "abcd");
        assert_eq!(e.cursor, (0, 2));
    }

    #[test]
    fn paste_normalizes_newlines_and_expands_tabs() {
        let mut e = editor("[]", 20);
        press(&mut e, KeyCode::Left);
        e.paste("a\r\n\tb", 4);
        assert_eq!(e.text(), "[a\n    b]");
        assert_eq!(e.cursor, (1, 5));
    }

    #[test]
    fn vertical_moves_by_screen_row_keeping_column() {
        // Width 4: "abcdefgh" fills two rows exactly, so it gets an empty third row.
        let mut e = editor("xy\nabcdefgh\nz", 4);
        e.cursor = (0, 1);
        press(&mut e, KeyCode::Down);
        assert_eq!(e.cursor, (1, 1));
        press(&mut e, KeyCode::Down);
        assert_eq!(e.cursor, (1, 5));
        press(&mut e, KeyCode::Down);
        assert_eq!(e.cursor, (1, 8));
        press(&mut e, KeyCode::Down);
        assert_eq!(e.cursor, (2, 1));
        press(&mut e, KeyCode::Up);
        press(&mut e, KeyCode::Up);
        assert_eq!(e.cursor, (1, 5));
    }
}
