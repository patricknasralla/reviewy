//! Motions over read-only text.

use super::wrap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Motion {
    Left,
    Right,
    Down,
    Up,
    ScreenDown,
    #[allow(dead_code, reason = "used by task 08; remove then")]
    ScreenUp,
    WordForward,
    WordBackward,
    WordEnd,
    LineStart,
    LineEnd,
    FileStart,
    FileEnd,
    HalfPageDown,
    HalfPageUp,
}

pub trait TextView {
    fn line_count(&self) -> usize;
    fn line(&self, idx: usize) -> &str;
}

/// `col` is a char index into the line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Pos {
    pub line: usize,
    pub col: usize,
}

/// `want` is the display column vertical motions aim for: within the line, or within the
/// screen segment when wrapping. `usize::MAX` sticks to line ends (after `$`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Cursor {
    pub pos: Pos,
    pub want: usize,
}

pub struct MotionCtx {
    /// Viewport height in lines, for half-page motions.
    pub height: usize,
    /// Wrap width; `Some` makes screen motions move by segment.
    pub wrap: Option<u16>,
}

/// Applies `motion` with vim semantics. `count` is `None` when no count was typed.
#[allow(dead_code, reason = "used by task 08; remove then")]
pub fn apply(
    motion: Motion,
    count: Option<usize>,
    cur: Cursor,
    view: &dyn TextView,
    ctx: &MotionCtx,
) -> Cursor {
    if view.line_count() == 0 {
        return Cursor::default();
    }
    let last = view.line_count() - 1;
    let n = count.unwrap_or(1);
    let Pos { line, col } = clamp(view, cur.pos);
    let half = (ctx.height / 2).max(1);
    let pos = match motion {
        Motion::Left => Pos {
            line,
            col: col.saturating_sub(n),
        },
        Motion::Right => Pos {
            line,
            col: col.saturating_add(n),
        },
        Motion::LineStart => Pos { line, col: 0 },
        Motion::LineEnd => {
            let pos = clamp(
                view,
                Pos {
                    line,
                    col: usize::MAX,
                },
            );
            return Cursor {
                pos,
                want: usize::MAX,
            };
        }
        Motion::WordForward => repeat(n, Pos { line, col }, |p| word_forward(view, p)),
        Motion::WordBackward => repeat(n, Pos { line, col }, |p| word_backward(view, p)),
        Motion::WordEnd => repeat(n, Pos { line, col }, |p| word_end(view, p)),
        Motion::FileStart | Motion::FileEnd => {
            let default = if motion == Motion::FileStart { 0 } else { last };
            let line = count.map_or(default, |c| c.saturating_sub(1).min(last));
            Pos {
                line,
                col: first_non_blank(view.line(line)),
            }
        }
        Motion::Down => return vertical(view, line.saturating_add(n).min(last), cur.want),
        Motion::Up => return vertical(view, line.saturating_sub(n), cur.want),
        Motion::HalfPageDown => {
            return vertical(view, line.saturating_add(half).min(last), cur.want);
        }
        Motion::HalfPageUp => return vertical(view, line.saturating_sub(half), cur.want),
        Motion::ScreenDown | Motion::ScreenUp => {
            let down = motion == Motion::ScreenDown;
            return match ctx.wrap {
                Some(width) => screen(view, Pos { line, col }, n, down, width, cur.want),
                None if down => vertical(view, line.saturating_add(n).min(last), cur.want),
                None => vertical(view, line.saturating_sub(n), cur.want),
            };
        }
    };
    let pos = clamp(view, pos);
    let text = view.line(pos.line);
    let want = match ctx.wrap {
        Some(width) => wrap::to_screen(text, width, pos.col).1,
        None => wrap::display_col(text, pos.col),
    };
    Cursor { pos, want }
}

fn clamp(view: &dyn TextView, pos: Pos) -> Pos {
    let line = pos.line.min(view.line_count() - 1);
    let chars = view.line(line).chars().count();
    Pos {
        line,
        col: pos.col.min(chars.saturating_sub(1)),
    }
}

fn repeat(n: usize, pos: Pos, step: impl Fn(Pos) -> Pos) -> Pos {
    (0..n).fold(pos, |p, _| step(p))
}

fn first_non_blank(line: &str) -> usize {
    line.chars().position(|c| !c.is_whitespace()).unwrap_or(0)
}

fn vertical(view: &dyn TextView, line: usize, want: usize) -> Cursor {
    let col = wrap::char_at_display(view.line(line), want);
    Cursor {
        pos: Pos { line, col },
        want,
    }
}

/// Moves `n` screen segments, crossing lines.
fn screen(view: &dyn TextView, pos: Pos, n: usize, down: bool, width: u16, want: usize) -> Cursor {
    let height = |line| wrap::line_height(view.line(line), width);
    let (mut line, (mut seg, _)) = (
        pos.line,
        wrap::to_screen(view.line(pos.line), width, pos.col),
    );
    for _ in 0..n {
        if down && seg + 1 < height(line) {
            seg += 1;
        } else if down && line + 1 < view.line_count() {
            (line, seg) = (line + 1, 0);
        } else if !down && seg > 0 {
            seg -= 1;
        } else if !down && line > 0 {
            line -= 1;
            seg = height(line) - 1;
        } else {
            break;
        }
    }
    let col = wrap::from_screen(view.line(line), width, seg, want);
    Cursor {
        pos: Pos { line, col },
        want,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Class {
    Blank,
    Punct,
    Word,
    /// Virtual newline at the end of each line; an empty line is only this.
    Eol,
}

fn class(c: char) -> Class {
    if c.is_whitespace() {
        Class::Blank
    } else if c.is_alphanumeric() || c == '_' {
        Class::Word
    } else {
        Class::Punct
    }
}

/// Walks the text as one char stream with a virtual newline after each line.
struct Walker<'a> {
    view: &'a dyn TextView,
    line: usize,
    col: usize,
    chars: Vec<char>,
}

impl<'a> Walker<'a> {
    fn new(view: &'a dyn TextView, pos: Pos) -> Self {
        let chars = view.line(pos.line).chars().collect();
        Self {
            view,
            line: pos.line,
            col: pos.col,
            chars,
        }
    }

    fn class(&self) -> Class {
        self.chars.get(self.col).map_or(Class::Eol, |&c| class(c))
    }

    fn class_at(&self, col: usize) -> Class {
        class(self.chars[col])
    }

    fn at_empty_line(&self) -> bool {
        self.chars.is_empty()
    }

    fn load(&mut self, line: usize) {
        self.line = line;
        self.chars = self.view.line(line).chars().collect();
    }

    fn next(&mut self) -> bool {
        if self.col < self.chars.len() {
            self.col += 1;
        } else if self.line + 1 < self.view.line_count() {
            self.load(self.line + 1);
            self.col = 0;
        } else {
            return false;
        }
        true
    }

    fn prev(&mut self) -> bool {
        if self.col > 0 {
            self.col -= 1;
        } else if self.line > 0 {
            self.load(self.line - 1);
            self.col = self.chars.len();
        } else {
            return false;
        }
        true
    }

    /// Skips blanks and line ends; empty lines stop it when `empty_stops`.
    fn skip_blanks(&mut self, forward: bool, empty_stops: bool) {
        while matches!(self.class(), Class::Blank | Class::Eol)
            && !(empty_stops && self.at_empty_line())
        {
            let moved = if forward { self.next() } else { self.prev() };
            if !moved {
                break;
            }
        }
    }

    fn pos(&self) -> Pos {
        Pos {
            line: self.line,
            col: self.col,
        }
    }
}

fn word_forward(view: &dyn TextView, pos: Pos) -> Pos {
    let mut w = Walker::new(view, pos);
    let start = w.class();
    if start == Class::Eol {
        w.next();
    } else {
        while w.class() == start && w.next() {}
    }
    w.skip_blanks(true, true);
    w.pos()
}

fn word_end(view: &dyn TextView, pos: Pos) -> Pos {
    let mut w = Walker::new(view, pos);
    w.next();
    w.skip_blanks(true, false);
    if w.class() != Class::Eol {
        let c = w.class();
        while w.col + 1 < w.chars.len() && w.class_at(w.col + 1) == c {
            w.col += 1;
        }
    }
    w.pos()
}

fn word_backward(view: &dyn TextView, pos: Pos) -> Pos {
    let mut w = Walker::new(view, pos);
    w.prev();
    w.skip_blanks(false, true);
    if w.class() != Class::Eol {
        let c = w.class();
        while w.col > 0 && w.class_at(w.col - 1) == c {
            w.col -= 1;
        }
    }
    w.pos()
}

#[cfg(test)]
mod tests {
    use super::*;

    impl TextView for &[&str] {
        fn line_count(&self) -> usize {
            self.len()
        }
        fn line(&self, idx: usize) -> &str {
            self[idx]
        }
    }

    const TEXT: &[&str] = &["fn foo(a, b) {", "", "    let x = a.b;", "}"];
    const CTX: MotionCtx = MotionCtx {
        height: 10,
        wrap: None,
    };

    fn at(line: usize, col: usize) -> Cursor {
        Cursor {
            pos: Pos { line, col },
            want: col,
        }
    }

    /// Applies each motion in turn, returning every position reached.
    fn run(
        view: &[&str],
        ctx: &MotionCtx,
        start: Cursor,
        motions: &[Motion],
    ) -> Vec<(usize, usize)> {
        let mut cur = start;
        motions
            .iter()
            .map(|&m| {
                cur = apply(m, None, cur, &view, ctx);
                (cur.pos.line, cur.pos.col)
            })
            .collect()
    }

    #[test]
    fn word_motions() {
        use Motion::*;
        let w = run(TEXT, &CTX, at(0, 0), &[WordForward; 9]);
        assert_eq!(
            w,
            [
                (0, 3),
                (0, 6),
                (0, 7),
                (0, 8),
                (0, 10),
                (0, 11),
                (0, 13),
                (1, 0),
                (2, 4)
            ]
        );
        let b = run(TEXT, &CTX, at(2, 4), &[WordBackward; 3]);
        assert_eq!(b, [(1, 0), (0, 13), (0, 11)]);
        let e = run(TEXT, &CTX, at(0, 0), &[WordEnd, WordEnd, WordEnd]);
        assert_eq!(e, [(0, 1), (0, 5), (0, 6)]);
        // `e` doesn't stop at empty lines.
        assert_eq!(run(TEXT, &CTX, at(0, 13), &[WordEnd]), [(2, 6)]);
        // `w` at the last word stops on the last char.
        assert_eq!(run(TEXT, &CTX, at(3, 0), &[WordForward]), [(3, 0)]);
    }

    #[test]
    fn line_start_end() {
        let r = run(TEXT, &CTX, at(2, 6), &[Motion::LineEnd, Motion::LineStart]);
        assert_eq!(r, [(2, 15), (2, 0)]);
    }

    #[test]
    fn vertical_keeps_column() {
        use Motion::*;
        let r = run(TEXT, &CTX, at(0, 11), &[Down, Down, Up, Up]);
        assert_eq!(r, [(1, 0), (2, 11), (1, 0), (0, 11)]);
    }

    #[test]
    fn file_end_with_count() {
        let goto = |count| apply(Motion::FileEnd, count, at(0, 0), &TEXT, &CTX).pos;
        assert_eq!(goto(Some(3)), Pos { line: 2, col: 4 });
        assert_eq!(goto(Some(9)), Pos { line: 3, col: 0 });
        assert_eq!(goto(None), Pos { line: 3, col: 0 });
    }

    #[test]
    fn screen_down_within_wrapped_line() {
        let long = "x".repeat(25);
        let view: &[&str] = &[&long, "y"];
        let ctx = MotionCtx {
            height: 10,
            wrap: Some(10),
        };
        let r = run(view, &ctx, at(0, 3), &[Motion::ScreenDown; 3]);
        assert_eq!(r, [(0, 13), (0, 23), (1, 0)]);
    }
}
