//! Operators (`d`, `c`, `y`) and put over the editor's buffer.

use std::ops::{Range, RangeInclusive};

use super::{Editor, Mode};
use crate::vim::keys::{Op, Target};
use crate::vim::motion::{self, Motion, Pos, TextView};
use crate::vim::register::{Register, Registers};
use crate::vim::selection;

/// Text an operator acts on: chars (end exclusive) or whole lines.
enum Span {
    Chars(Range<Pos>),
    Lines(RangeInclusive<usize>),
}

impl Editor {
    /// Applies `op` to `target`; returns whether the text changed. Ends in Normal mode, or
    /// Insert for a change (even when there is nothing to change, e.g. `C` on an empty line).
    pub(super) fn operator(
        &mut self,
        op: Op,
        target: Target,
        count: Option<usize>,
        regs: &mut Registers,
    ) -> bool {
        let mut changed = false;
        match self.span(op, target, count) {
            Some(Span::Chars(range)) => {
                let start = range.start;
                let text = if op == Op::Yank {
                    self.buffer.slice(range)
                } else {
                    changed = true;
                    self.buffer.delete(range)
                };
                regs.unnamed = Register {
                    text,
                    linewise: false,
                };
                self.cursor.pos = start;
            }
            Some(Span::Lines(lines)) => {
                let (a, b) = (*lines.start(), *lines.end());
                let range = Pos { line: a, col: 0 }..self.line_end(b);
                let text = self.buffer.slice(range.clone());
                regs.unnamed = Register {
                    text,
                    linewise: true,
                };
                match op {
                    Op::Delete => {
                        self.buffer.delete_lines(lines);
                        let line = a.min(self.buffer.line_count() - 1);
                        self.cursor.pos = self.first_non_blank(line);
                    }
                    Op::Change => {
                        self.buffer.delete(range);
                        self.cursor.pos = Pos { line: a, col: 0 };
                    }
                    Op::Yank => self.cursor.pos.line = a,
                }
                changed = op != Op::Yank;
            }
            None => {}
        }
        self.mode = if op == Op::Change {
            Mode::Insert
        } else {
            Mode::Normal
        };
        self.settle();
        changed
    }

    /// `p`/`P`; in a visual mode, replaces the selection, keeping the register. Returns whether
    /// the text changed.
    pub(super) fn put(&mut self, before: bool, count: Option<usize>, regs: &Registers) -> bool {
        let reg = &regs.unnamed;
        if reg.text.is_empty() && !reg.linewise {
            return false;
        }
        let Pos { line, col } = self.cursor.pos;
        let n = count.unwrap_or(1);
        match self.mode {
            Mode::Visual { .. } | Mode::VisualLine { .. } => {
                let (range, linewise) = match self.span(Op::Delete, Target::Selection, None) {
                    Some(Span::Chars(range)) => (range, false),
                    Some(Span::Lines(lines)) => {
                        let start = Pos {
                            line: *lines.start(),
                            col: 0,
                        };
                        (start..self.line_end(*lines.end()), true)
                    }
                    None => return false,
                };
                let start = range.start;
                self.buffer.delete(range);
                let end = self.buffer.insert(start, &reg.text);
                self.cursor.pos = if linewise && reg.linewise {
                    self.first_non_blank(start.line)
                } else {
                    last_char(end)
                };
                self.mode = Mode::Normal;
            }
            _ if reg.linewise => {
                let text = vec![reg.text.as_str(); n].join("\n");
                let first = if before {
                    self.buffer
                        .insert(Pos { line, col: 0 }, &format!("{text}\n"));
                    line
                } else {
                    let end = self.line_end(line);
                    self.buffer.insert(end, &format!("\n{text}"));
                    line + 1
                };
                self.cursor.pos = self.first_non_blank(first);
            }
            _ => {
                let len = self.buffer.len(line);
                let col = if before { col } else { (col + 1).min(len) };
                let end = self.buffer.insert(Pos { line, col }, &reg.text.repeat(n));
                self.cursor.pos = last_char(end);
            }
        }
        self.settle();
        true
    }

    fn span(&self, op: Op, target: Target, count: Option<usize>) -> Option<Span> {
        let cur = self.cursor.pos;
        let last = self.buffer.line_count() - 1;
        match target {
            Target::Motion(m) => self.motion_span(op, m, count),
            Target::Line => lines(cur.line, (cur.line + count.unwrap_or(1) - 1).min(last)),
            Target::Selection | Target::SelectionLines => {
                let (anchor, linewise) = match self.mode {
                    Mode::Visual { anchor } => (anchor, target == Target::SelectionLines),
                    Mode::VisualLine { anchor } => (anchor, true),
                    _ => return None,
                };
                let (start, end) = selection::bounds(anchor, cur);
                if linewise {
                    lines(start.line, end.line)
                } else {
                    chars(start, self.after(end))
                }
            }
        }
    }

    fn motion_span(&self, op: Op, motion: Motion, count: Option<usize>) -> Option<Span> {
        let c = self.cursor.pos;
        let motion = match motion {
            // Operators act on logical lines, even when wrapped.
            Motion::ScreenDown => Motion::Down,
            Motion::ScreenUp => Motion::Up,
            // `cw` on a word changes to its end; on the word's last char, just that char.
            Motion::WordForward if op == Op::Change && !self.blank(c) => {
                if count.unwrap_or(1) == 1 && self.word_end(c) {
                    return chars(c, self.after(c));
                }
                Motion::WordEnd
            }
            m => m,
        };
        let t = motion::apply(motion, count, self.cursor, &self.buffer, &self.ctx()).pos;
        let forward = matches!(motion, Motion::Right | Motion::WordForward);
        match motion {
            Motion::Down
            | Motion::Up
            | Motion::FileStart
            | Motion::FileEnd
            | Motion::HalfPageDown
            | Motion::HalfPageUp => lines(c.line, t.line),
            Motion::WordEnd | Motion::LineEnd => chars(c, self.after(t)),
            _ if (t.line, t.col) < (c.line, c.col) => chars(t, c),
            // Couldn't move forward: up to the line end, never the line break.
            _ if t == c && forward => chars(c, self.line_end(c.line)),
            // `dw` keeps the line break and the next line's indent.
            Motion::WordForward
                if t.line > c.line
                    && t.col <= motion::first_non_blank(self.buffer.line(t.line)) =>
            {
                chars(c, self.line_end(t.line - 1))
            }
            // `w` stuck on the buffer's last word: through its end.
            Motion::WordForward if !self.word_start(t) => chars(c, self.line_end(t.line)),
            _ => chars(c, t),
        }
    }

    /// The position after the char at `p`, capped at the line end.
    fn after(&self, p: Pos) -> Pos {
        Pos {
            line: p.line,
            col: (p.col + 1).min(self.buffer.len(p.line)),
        }
    }

    fn first_non_blank(&self, line: usize) -> Pos {
        Pos {
            line,
            col: motion::first_non_blank(self.buffer.line(line)),
        }
    }

    fn char_at(&self, p: Pos) -> Option<char> {
        self.buffer.line(p.line).chars().nth(p.col)
    }

    fn blank(&self, p: Pos) -> bool {
        self.char_at(p).is_none_or(char::is_whitespace)
    }

    fn word_start(&self, p: Pos) -> bool {
        let prev = p
            .col
            .checked_sub(1)
            .and_then(|col| self.char_at(Pos { col, ..p }));
        match (prev, self.char_at(p)) {
            (_, None) => false,
            (_, Some(c)) if c.is_whitespace() => false,
            (Some(prev), Some(c)) => !motion::same_word(prev, c),
            (None, Some(_)) => true,
        }
    }

    fn word_end(&self, p: Pos) -> bool {
        let next = self.char_at(Pos {
            col: p.col + 1,
            ..p
        });
        match (self.char_at(p), next) {
            (Some(c), Some(next)) => !motion::same_word(c, next),
            _ => true,
        }
    }
}

fn chars(start: Pos, end: Pos) -> Option<Span> {
    (start != end).then_some(Span::Chars(start..end))
}

fn lines(a: usize, b: usize) -> Option<Span> {
    Some(Span::Lines(a.min(b)..=a.max(b)))
}

/// The last char of text inserted up to `end`.
fn last_char(end: Pos) -> Pos {
    Pos {
        col: end.col.saturating_sub(1),
        ..end
    }
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyCode, KeyEvent};

    use super::*;
    use crate::vim::keys::{Context, KeyParser};

    struct Run {
        e: Editor,
        regs: Registers,
    }

    /// Normal-mode editor `width` wide with the cursor at `(line, col)`.
    fn run(body: &str, width: u16, (line, col): (usize, usize)) -> Run {
        let mut e = Editor::new(1, String::new(), body, Mode::Normal);
        e.resize(width, 10);
        e.cursor.pos = Pos { line, col };
        e.settle();
        Run {
            e,
            regs: Registers::default(),
        }
    }

    impl Run {
        /// Feeds `keys` as typed; `\x1b` is Esc.
        fn keys(&mut self, keys: &str) -> &mut Self {
            let mut parser = KeyParser::default();
            for c in keys.chars() {
                let code = if c == '\x1b' {
                    KeyCode::Esc
                } else {
                    KeyCode::Char(c)
                };
                let ctx = match self.e.mode {
                    Mode::Insert => {
                        self.e.insert_key(KeyEvent::from(code));
                        continue;
                    }
                    Mode::Normal => Context::EditorNormal,
                    _ => Context::EditorVisual,
                };
                if let Some(input) = parser.handle(KeyEvent::from(code), ctx) {
                    self.e.input(input, &mut self.regs);
                }
            }
            self
        }

        fn is(&self, text: &str, (line, col): (usize, usize)) {
            assert_eq!(self.e.text(), text);
            assert_eq!(self.e.cursor.pos, Pos { line, col });
        }
    }

    #[test]
    fn dw_stops_at_line_end() {
        run("foo bar\nbaz", 20, (0, 4))
            .keys("dw")
            .is("foo \nbaz", (0, 3));
        // On the buffer's last word, `w` can't move past it: delete through its end.
        run("a bar", 20, (0, 2)).keys("dw").is("a ", (0, 1));
    }

    #[test]
    fn cw_is_ce() {
        run("foo bar", 20, (0, 0))
            .keys("cwx\x1b")
            .is("x bar", (0, 0));
        // On a word's last char, just that char.
        run("foo bar", 20, (0, 2))
            .keys("cwx\x1b")
            .is("fox bar", (0, 2));
    }

    #[test]
    fn de_is_inclusive() {
        run("foo bar", 20, (0, 0)).keys("de").is(" bar", (0, 0));
    }

    #[test]
    fn dj_deletes_logical_lines_when_wrapped() {
        run("abcdefgh\nij\nkl", 4, (0, 0))
            .keys("dj")
            .is("kl", (0, 0));
    }

    #[test]
    fn dd_on_last_and_only_line() {
        run("a\n  b\nc", 20, (2, 0)).keys("dd").is("a\n  b", (1, 2));
        run("abc", 20, (0, 1)).keys("dd").is("", (0, 0));
    }

    #[test]
    fn x_on_last_char_keeps_the_line_break() {
        run("foo\nbar", 20, (0, 2)).keys("x").is("fo\nbar", (0, 1));
    }

    #[test]
    fn put_linewise_and_charwise() {
        run("ab\ncd", 20, (0, 1))
            .keys("yy2p")
            .is("ab\nab\nab\ncd", (1, 0));
        run("ab\ncd", 20, (1, 0))
            .keys("yyP")
            .is("ab\ncd\ncd", (1, 0));
        run("ab", 20, (0, 0)).keys("yl3p").is("aaaab", (0, 3));
    }

    #[test]
    fn visual_line_delete_and_visual_put() {
        run("a\nb\nc", 20, (0, 0)).keys("VjD").is("c", (0, 0));
        let mut r = run("foo bar", 20, (0, 0));
        r.keys("ywwvep").is("foo foo ", (0, 7));
        assert_eq!(r.regs.unnamed.text, "foo ");
    }
}
