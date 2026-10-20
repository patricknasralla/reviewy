//! Comment editor model: buffer, cursor, vim modes and soft-wrap layout.

pub mod buffer;
mod operator;

use std::ops::Range;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::diff::align::expand_tabs;
use crate::vim::keys::{At, Command, Input};
use crate::vim::motion::{self, Cursor, MotionCtx, Pos, TextView};
use crate::vim::register::{Register, Registers};
use crate::vim::{selection, wrap};
use buffer::Buffer;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Insert,
    /// Visual modes store where the selection started.
    Visual {
        anchor: Pos,
    },
    VisualLine {
        anchor: Pos,
    },
}

pub struct Editor {
    /// Id of the comment being edited.
    pub id: u32,
    pub title: String,
    buffer: Buffer,
    /// `want` is the display column within the cursor's screen row, kept by vertical moves.
    cursor: Cursor,
    mode: Mode,
    /// First visible screen row.
    scroll: usize,
    width: u16,
    height: u16,
}

impl Editor {
    /// Cursor at the end of `body`. Call `resize` before use: it lays out the text.
    pub fn new(id: u32, title: String, body: &str, mode: Mode) -> Self {
        let buffer = Buffer::from_text(body);
        let line = buffer.line_count() - 1;
        let col = buffer.len(line);
        Self {
            id,
            title,
            buffer,
            cursor: Cursor {
                pos: Pos { line, col },
                want: 0,
            },
            mode,
            scroll: 0,
            width: 1,
            height: 1,
        }
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }

    pub fn text(&self) -> String {
        self.buffer.text()
    }

    pub fn resize(&mut self, width: u16, height: u16) {
        (self.width, self.height) = (width.max(1), height.max(1));
        self.settle();
    }

    /// Applies a normal- or visual-mode command; returns whether the text changed.
    pub fn input(&mut self, input: Input, regs: &mut Registers) -> bool {
        let Pos { line, col } = self.cursor.pos;
        let len = self.buffer.len(line);
        let changed = match input.command {
            Command::Motion(m) => {
                self.cursor = motion::apply(m, input.count, self.cursor, &self.buffer, &self.ctx());
                self.scroll_to_cursor();
                return false;
            }
            Command::Visual | Command::VisualLine => {
                let line_mode = input.command == Command::VisualLine;
                let anchor = match self.mode {
                    Mode::Visual { anchor } | Mode::VisualLine { anchor } => anchor,
                    _ => self.cursor.pos,
                };
                self.mode = match (self.mode, line_mode) {
                    (Mode::Visual { .. }, false) | (Mode::VisualLine { .. }, true) => Mode::Normal,
                    (_, true) => Mode::VisualLine { anchor },
                    (_, false) => Mode::Visual { anchor },
                };
                false
            }
            Command::Escape => {
                self.mode = Mode::Normal;
                false
            }
            Command::Operator { op, target } => self.operator(op, target, input.count, regs),
            Command::Put { before } => self.put(before, input.count, regs),
            Command::Insert(at) => {
                self.mode = Mode::Insert;
                match at {
                    At::Before => {}
                    At::After => self.cursor.pos.col = (col + 1).min(len),
                    At::LineStart => {
                        self.cursor.pos.col = motion::first_non_blank(self.buffer.line(line))
                    }
                    At::LineEnd => self.cursor.pos.col = len,
                    At::Below => self.cursor.pos = self.buffer.insert(Pos { line, col: len }, "\n"),
                    At::Above => {
                        self.buffer.insert(Pos { line, col: 0 }, "\n");
                        self.cursor.pos.col = 0;
                    }
                }
                matches!(at, At::Below | At::Above)
            }
            Command::DeleteChar { before } => {
                let n = input.count.unwrap_or(1);
                let cols = if before {
                    col.saturating_sub(n)..col
                } else {
                    col..(col + n).min(len)
                };
                self.cursor.pos.col = cols.start;
                let at = |col| Pos { line, col };
                let text = self.buffer.slice(at(cols.start)..at(cols.end));
                let changed = self.delete_cols(line, cols);
                if changed {
                    regs.unnamed = Register {
                        text,
                        linewise: false,
                    };
                }
                changed
            }
            _ => false,
        };
        self.settle();
        changed
    }

    /// Applies an insert-mode key; returns whether the text changed.
    pub fn insert_key(&mut self, key: KeyEvent) -> bool {
        let plain = !key
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT);
        let Pos { line, col } = self.cursor.pos;
        let len = self.buffer.len(line);
        let page = self.height as isize;
        let mut changed = false;
        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.cursor.pos.col = col.saturating_sub(1);
            }
            KeyCode::Char(c) if plain => changed = self.insert(c.encode_utf8(&mut [0; 4])),
            KeyCode::Enter => changed = self.insert("\n"),
            KeyCode::Backspace => changed = self.backspace(),
            KeyCode::Delete => changed = self.delete(),
            KeyCode::Left if col > 0 => self.cursor.pos.col -= 1,
            KeyCode::Left if line > 0 => self.cursor.pos = self.line_end(line - 1),
            KeyCode::Right if col < len => self.cursor.pos.col += 1,
            KeyCode::Right if line + 1 < self.buffer.line_count() => {
                self.cursor.pos = Pos {
                    line: line + 1,
                    col: 0,
                }
            }
            KeyCode::Home => self.cursor.pos.col = 0,
            KeyCode::End => self.cursor.pos.col = len,
            KeyCode::Up => return self.vertical(-1),
            KeyCode::Down => return self.vertical(1),
            KeyCode::PageUp => return self.vertical(-page),
            KeyCode::PageDown => return self.vertical(page),
            _ => {}
        }
        self.settle();
        changed
    }

    /// Inserts `text` at the cursor in any mode: line breaks normalized to `\n`, tabs expanded
    /// per pasted line.
    pub fn paste(&mut self, text: &str, tab_width: usize) {
        let text = text.replace("\r\n", "\n").replace('\r', "\n");
        let lines: Vec<String> = text
            .split('\n')
            .map(|l| expand_tabs(l, tab_width).0)
            .collect();
        self.insert(&lines.join("\n"));
        self.settle();
    }

    /// Visible screen rows as (line, byte range in the line), from the scroll position down.
    pub fn rows(&self) -> impl Iterator<Item = (usize, Range<usize>)> + '_ {
        (0..self.buffer.line_count())
            .flat_map(|i| (self.segments(self.buffer.line(i)).into_iter()).map(move |r| (i, r)))
            .skip(self.scroll)
    }

    pub fn line(&self, idx: usize) -> &str {
        self.buffer.line(idx)
    }

    /// Selected chars of `line` in a visual mode; half-open, may extend past the line end.
    pub fn selection_cols(&self, line: usize) -> Option<Range<usize>> {
        let cur = self.cursor.pos;
        match self.mode {
            Mode::Visual { anchor } => selection::line_cols(anchor, cur, false, line),
            Mode::VisualLine { anchor } => selection::line_cols(anchor, cur, true, line),
            _ => None,
        }
    }

    /// Cursor (row, column) within the text area, if it is in view.
    pub fn cursor_cell(&self) -> Option<(u16, u16)> {
        let (seg, x) = self.locate();
        let row = (self.tops()[self.cursor.pos.line] + seg).checked_sub(self.scroll)?;
        (row < usize::from(self.height)).then_some((row as u16, x as u16))
    }

    fn ctx(&self) -> MotionCtx {
        MotionCtx {
            height: usize::from(self.height),
            wrap: Some(self.width),
        }
    }

    fn line_end(&self, line: usize) -> Pos {
        Pos {
            line,
            col: self.buffer.len(line),
        }
    }

    /// Clamps the cursor for the mode, resets the sticky column and scrolls the cursor into view.
    fn settle(&mut self) {
        let Pos { line, col } = self.cursor.pos;
        let len = self.buffer.len(line);
        let max = match self.mode {
            Mode::Insert => len,
            _ => len.saturating_sub(1),
        };
        self.cursor.pos.col = col.min(max);
        self.cursor.want = self.locate().1;
        self.scroll_to_cursor();
    }

    /// Inserts `text` (may contain `\n`) at the cursor; the cursor ends after it.
    fn insert(&mut self, text: &str) -> bool {
        self.cursor.pos = self.buffer.insert(self.cursor.pos, text);
        true
    }

    fn delete_cols(&mut self, line: usize, cols: Range<usize>) -> bool {
        if cols.is_empty() {
            return false;
        }
        let at = |col| Pos { line, col };
        self.buffer.delete(at(cols.start)..at(cols.end));
        true
    }

    fn backspace(&mut self) -> bool {
        let Pos { line, col } = self.cursor.pos;
        if col > 0 {
            self.cursor.pos.col -= 1;
            self.delete_cols(line, col - 1..col)
        } else if line > 0 {
            self.cursor.pos = self.line_end(line - 1);
            self.buffer.delete(self.cursor.pos..Pos { line, col: 0 });
            true
        } else {
            false
        }
    }

    fn delete(&mut self) -> bool {
        let Pos { line, col } = self.cursor.pos;
        if col < self.buffer.len(line) {
            self.delete_cols(line, col..col + 1)
        } else if line + 1 < self.buffer.line_count() {
            let next = Pos {
                line: line + 1,
                col: 0,
            };
            self.buffer.delete(self.cursor.pos..next);
            true
        } else {
            false
        }
    }

    /// Moves `rows` screen rows, keeping the sticky display column. Never changes the text.
    fn vertical(&mut self, rows: isize) -> bool {
        let tops = self.tops();
        let (seg, _) = self.locate();
        let total = tops[tops.len() - 1];
        let row = (tops[self.cursor.pos.line] + seg)
            .saturating_add_signed(rows)
            .min(total - 1);
        let line = tops.partition_point(|&t| t <= row) - 1;
        let col = self.col_at(line, row - tops[line], self.cursor.want);
        self.cursor.pos = Pos { line, col };
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
        for line in self.buffer.lines() {
            tops.push(tops[tops.len() - 1] + self.segments(line).len());
        }
        tops
    }

    /// Cursor → (row within its line, display column).
    fn locate(&self) -> (usize, usize) {
        let Pos { line, col } = self.cursor.pos;
        let text = self.buffer.line(line);
        let at = buffer::byte(text, col);
        let segments = self.segments(text);
        let seg = (segments.iter().position(|r| at < r.end)).unwrap_or(segments.len() - 1);
        (seg, width(&text[segments[seg].start..at]))
    }

    /// Char col at display column `want` in row `seg` of `line`; past the end, the row's last
    /// char (the line end on its last row).
    fn col_at(&self, line: usize, seg: usize, want: usize) -> usize {
        let text = self.buffer.line(line);
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
        let row = self.tops()[self.cursor.pos.line] + self.locate().0;
        let height = usize::from(self.height);
        self.scroll = self.scroll.clamp((row + 1).saturating_sub(height), row);
    }
}

fn width(s: &str) -> usize {
    s.chars().map(wrap::char_width).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn editor(body: &str, width: u16, mode: Mode) -> Editor {
        let mut e = Editor::new(1, String::new(), body, mode);
        e.resize(width, 10);
        e
    }

    fn press(e: &mut Editor, code: KeyCode) -> bool {
        e.insert_key(KeyEvent::from(code))
    }

    fn cmd(e: &mut Editor, command: Command, count: Option<usize>) -> bool {
        e.input(Input { command, count }, &mut Registers::default())
    }

    fn at(e: &Editor) -> (usize, usize) {
        (e.cursor.pos.line, e.cursor.pos.col)
    }

    #[test]
    fn insert_and_newline_mid_line() {
        let mut e = editor("ac", 20, Mode::Insert);
        press(&mut e, KeyCode::Left);
        assert!(press(&mut e, KeyCode::Char('b')));
        assert!(press(&mut e, KeyCode::Enter));
        assert_eq!(e.text(), "ab\nc");
        assert_eq!(at(&e), (1, 0));
    }

    #[test]
    fn backspace_at_line_start_joins() {
        let mut e = editor("ab\ncd", 20, Mode::Insert);
        press(&mut e, KeyCode::Home);
        assert!(press(&mut e, KeyCode::Backspace));
        assert_eq!(e.text(), "abcd");
        assert_eq!(at(&e), (0, 2));
    }

    #[test]
    fn paste_normalizes_newlines_and_expands_tabs() {
        let mut e = editor("[]", 20, Mode::Insert);
        press(&mut e, KeyCode::Left);
        e.paste("a\r\n\tb", 4);
        assert_eq!(e.text(), "[a\n    b]");
        assert_eq!(at(&e), (1, 5));
    }

    #[test]
    fn vertical_moves_by_screen_row_keeping_column() {
        // Width 4: "abcdefgh" fills two rows exactly, so it gets an empty third row.
        let mut e = editor("xy\nabcdefgh\nz", 4, Mode::Insert);
        e.cursor.pos = Pos { line: 0, col: 1 };
        e.settle();
        press(&mut e, KeyCode::Down);
        assert_eq!(at(&e), (1, 1));
        press(&mut e, KeyCode::Down);
        assert_eq!(at(&e), (1, 5));
        press(&mut e, KeyCode::Down);
        assert_eq!(at(&e), (1, 8));
        press(&mut e, KeyCode::Down);
        assert_eq!(at(&e), (2, 1));
        press(&mut e, KeyCode::Up);
        press(&mut e, KeyCode::Up);
        assert_eq!(at(&e), (1, 5));
    }

    #[test]
    fn opens_scrolled_to_the_top_when_the_body_fits() {
        let e = editor("a\nb\nc\n", 20, Mode::Insert);
        assert_eq!(e.rows().next(), Some((0, 0..1)));
        assert_eq!(e.cursor_cell(), Some((3, 0)));
    }

    #[test]
    fn escape_moves_left_but_not_past_line_start() {
        let mut e = editor("ab\n", 20, Mode::Insert);
        press(&mut e, KeyCode::Esc);
        assert_eq!((e.mode(), at(&e)), (Mode::Normal, (1, 0)));
        cmd(&mut e, Command::Motion(motion::Motion::ScreenUp), None);
        cmd(&mut e, Command::Insert(At::LineEnd), None);
        press(&mut e, KeyCode::Esc);
        assert_eq!(at(&e), (0, 1));
    }

    #[test]
    fn delete_chars_stay_on_the_line() {
        let mut e = editor("abcd\nef", 20, Mode::Normal);
        e.cursor.pos = Pos { line: 0, col: 2 };
        assert!(cmd(&mut e, Command::DeleteChar { before: false }, Some(5)));
        assert_eq!((e.text().as_str(), at(&e)), ("ab\nef", (0, 1)));
        assert!(cmd(&mut e, Command::DeleteChar { before: true }, Some(5)));
        assert_eq!((e.text().as_str(), at(&e)), ("b\nef", (0, 0)));
        assert!(!cmd(&mut e, Command::DeleteChar { before: true }, None));
        cmd(&mut e, Command::DeleteChar { before: false }, None);
        assert!(!cmd(&mut e, Command::DeleteChar { before: false }, None));
        assert_eq!(e.text(), "\nef");
    }

    #[test]
    fn open_line_below_and_above() {
        let mut e = editor("ab\ncd", 20, Mode::Normal);
        e.cursor.pos = Pos { line: 0, col: 1 };
        assert!(cmd(&mut e, Command::Insert(At::Below), None));
        assert_eq!((e.text().as_str(), at(&e)), ("ab\n\ncd", (1, 0)));
        press(&mut e, KeyCode::Esc);
        e.cursor.pos = Pos { line: 2, col: 1 };
        assert!(cmd(&mut e, Command::Insert(At::Above), None));
        assert_eq!((e.text().as_str(), at(&e)), ("ab\n\n\ncd", (2, 0)));
        assert_eq!(e.mode(), Mode::Insert);
    }

    #[test]
    fn append_on_empty_line() {
        let mut e = editor("", 20, Mode::Normal);
        cmd(&mut e, Command::Insert(At::After), None);
        assert_eq!((e.mode(), at(&e)), (Mode::Insert, (0, 0)));
    }
}
