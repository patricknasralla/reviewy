//! Key, chord and count parser producing vim commands.

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use super::motion::Motion;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    Motion(Motion),
    Confirm,
    Escape,
    Visual,
    VisualLine,
    DeleteComment,
    ColumnLeft,
    ColumnRight,
    NextHunk,
    PrevHunk,
    NextFile,
    PrevFile,
    Quit,
    /// Enter insert mode at `At`.
    Insert(At),
    /// `x` (`before: false`) or `X`.
    DeleteChar {
        before: bool,
    },
    /// `d` in a modal.
    Delete,
    Operator {
        op: Op,
        target: Target,
    },
    /// `p` (`before: false`) or `P`; in visual mode, replace the selection.
    Put {
        before: bool,
    },
    Undo,
    Redo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    Delete,
    Change,
    Yank,
}

/// What an operator acts on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Motion(Motion),
    /// `dd`, `cc`, `yy`.
    Line,
    /// The visual selection.
    Selection,
    /// The lines the visual selection covers.
    SelectionLines,
}

/// Where an insert-mode entry puts the cursor: `i a I A o O`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum At {
    Before,
    After,
    LineStart,
    LineEnd,
    Below,
    Above,
}

/// Which key table applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Context {
    Diff,
    EditorNormal,
    EditorVisual,
    /// A confirmation modal: no counts or chords.
    Modal,
}

/// `count` is `None` when none was typed, so `G` and `1G` differ.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Input {
    pub command: Command,
    pub count: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Key {
    /// Shift is ignored: `G`, `$` arrive with SHIFT set.
    Char(char),
    Ctrl(char),
    Code(KeyCode),
    /// Ctrl + a non-char key, e.g. Ctrl-Left.
    CtrlCode(KeyCode),
}

const MAX_COUNT: usize = 99_999;

/// Motions shared by the diff view and the editor. `j`/`k` move by screen row.
const MOTIONS: &[(Key, Command)] = &[
    (Key::Char('h'), Command::Motion(Motion::Left)),
    (Key::Char('j'), Command::Motion(Motion::ScreenDown)),
    (Key::Char('k'), Command::Motion(Motion::ScreenUp)),
    (Key::Char('l'), Command::Motion(Motion::Right)),
    (Key::Code(KeyCode::Left), Command::Motion(Motion::Left)),
    (
        Key::Code(KeyCode::Down),
        Command::Motion(Motion::ScreenDown),
    ),
    (Key::Code(KeyCode::Up), Command::Motion(Motion::ScreenUp)),
    (Key::Code(KeyCode::Right), Command::Motion(Motion::Right)),
    (Key::Char('w'), Command::Motion(Motion::WordForward)),
    (Key::Char('b'), Command::Motion(Motion::WordBackward)),
    (Key::Char('e'), Command::Motion(Motion::WordEnd)),
    (Key::Char('0'), Command::Motion(Motion::LineStart)),
    (Key::Char('$'), Command::Motion(Motion::LineEnd)),
    (Key::Char('G'), Command::Motion(Motion::FileEnd)),
    (Key::Ctrl('d'), Command::Motion(Motion::HalfPageDown)),
    (Key::Ctrl('u'), Command::Motion(Motion::HalfPageUp)),
];

const DIFF: &[(Key, Command)] = &[
    (Key::Ctrl('h'), Command::ColumnLeft),
    (Key::Ctrl('l'), Command::ColumnRight),
    (Key::CtrlCode(KeyCode::Left), Command::ColumnLeft),
    (Key::CtrlCode(KeyCode::Right), Command::ColumnRight),
    (Key::Char(']'), Command::NextHunk),
    (Key::Char('['), Command::PrevHunk),
    (Key::Char('v'), Command::Visual),
    (Key::Char('V'), Command::VisualLine),
    (Key::Char('D'), Command::DeleteComment),
    (Key::Code(KeyCode::Enter), Command::Confirm),
    (Key::Code(KeyCode::Esc), Command::Escape),
    (Key::Ctrl('q'), Command::Quit),
];

const EDITOR: &[(Key, Command)] = &[
    (Key::Char('i'), Command::Insert(At::Before)),
    (Key::Char('a'), Command::Insert(At::After)),
    (Key::Char('I'), Command::Insert(At::LineStart)),
    (Key::Char('A'), Command::Insert(At::LineEnd)),
    (Key::Char('o'), Command::Insert(At::Below)),
    (Key::Char('O'), Command::Insert(At::Above)),
    (Key::Char('x'), Command::DeleteChar { before: false }),
    (Key::Char('X'), Command::DeleteChar { before: true }),
    (Key::Char('D'), line_end(Op::Delete)),
    (Key::Char('C'), line_end(Op::Change)),
    (Key::Char('Y'), line_end(Op::Yank)),
    (Key::Char('p'), Command::Put { before: false }),
    (Key::Char('P'), Command::Put { before: true }),
    (Key::Char('v'), Command::Visual),
    (Key::Char('V'), Command::VisualLine),
    (Key::Char('u'), Command::Undo),
    (Key::Ctrl('r'), Command::Redo),
    (Key::Code(KeyCode::Esc), Command::Escape),
    (Key::Ctrl('q'), Command::Quit),
];

const VISUAL: &[(Key, Command)] = &[
    (Key::Char('v'), Command::Visual),
    (Key::Char('V'), Command::VisualLine),
    (Key::Char('d'), selection(Op::Delete)),
    (Key::Char('x'), selection(Op::Delete)),
    (Key::Char('c'), selection(Op::Change)),
    (Key::Char('y'), selection(Op::Yank)),
    (Key::Char('D'), selection_lines(Op::Delete)),
    (Key::Char('X'), selection_lines(Op::Delete)),
    (Key::Char('C'), selection_lines(Op::Change)),
    (Key::Char('Y'), selection_lines(Op::Yank)),
    (Key::Char('p'), Command::Put { before: false }),
    (Key::Code(KeyCode::Esc), Command::Escape),
    (Key::Ctrl('q'), Command::Quit),
];

/// Operator keys: wait for a motion, or the same key again for whole lines.
const OPS: &[(char, Op)] = &[('d', Op::Delete), ('c', Op::Change), ('y', Op::Yank)];

const fn line_end(op: Op) -> Command {
    Command::Operator {
        op,
        target: Target::Motion(Motion::LineEnd),
    }
}

const fn selection(op: Op) -> Command {
    Command::Operator {
        op,
        target: Target::Selection,
    }
}

const fn selection_lines(op: Op) -> Command {
    Command::Operator {
        op,
        target: Target::SelectionLines,
    }
}

const MODAL: &[(Key, Command)] = &[
    (Key::Code(KeyCode::Enter), Command::Confirm),
    (Key::Code(KeyCode::Esc), Command::Escape),
    (Key::Char('d'), Command::Delete),
    (Key::Ctrl('q'), Command::Quit),
];

/// Second key of `g` chords. `gj`/`gk` move by line.
const G_MOTIONS: &[(char, Command)] = &[
    ('g', Command::Motion(Motion::FileStart)),
    ('j', Command::Motion(Motion::Down)),
    ('k', Command::Motion(Motion::Up)),
];

const G_DIFF: &[(char, Command)] = &[('n', Command::NextFile), ('p', Command::PrevFile)];

type Table<K> = &'static [(K, Command)];

/// Single-key and `g` chord tables of a context, searched in order.
fn tables(ctx: Context) -> (&'static [Table<Key>], &'static [Table<char>]) {
    match ctx {
        Context::Diff => (&[MOTIONS, DIFF], &[G_MOTIONS, G_DIFF]),
        Context::EditorNormal => (&[MOTIONS, EDITOR], &[G_MOTIONS]),
        Context::EditorVisual => (&[MOTIONS, VISUAL], &[G_MOTIONS]),
        Context::Modal => (&[MODAL], &[]),
    }
}

fn find<K: PartialEq + Copy>(tables: &[&[(K, Command)]], key: K) -> Option<Command> {
    tables
        .iter()
        .flat_map(|t| t.iter())
        .find(|(k, _)| *k == key)
        .map(|&(_, c)| c)
}

/// Product of two optional counts; `None` when neither was typed.
fn mul(a: Option<usize>, b: Option<usize>) -> Option<usize> {
    (a.is_some() || b.is_some()).then(|| (a.unwrap_or(1) * b.unwrap_or(1)).min(MAX_COUNT))
}

#[derive(Debug, Default)]
pub struct KeyParser {
    count: Option<usize>,
    g: bool,
    /// Pending operator and the count typed before it.
    op: Option<(Op, Option<usize>)>,
}

impl KeyParser {
    /// `Quit` and `Escape` take effect even mid-chord.
    pub fn handle(&mut self, event: KeyEvent, ctx: Context) -> Option<Input> {
        if event.kind == KeyEventKind::Release {
            return None;
        }
        let key = match event.code {
            KeyCode::Char(c) if event.modifiers.contains(KeyModifiers::CONTROL) => Key::Ctrl(c),
            KeyCode::Char(c) => Key::Char(c),
            code if event.modifiers.contains(KeyModifiers::CONTROL) => Key::CtrlCode(code),
            code => Key::Code(code),
        };
        let (keys, g_keys) = tables(ctx);
        let command = find(keys, key);

        if let Some(c @ (Command::Quit | Command::Escape)) = command {
            *self = Self::default();
            return Some(Input {
                command: c,
                count: None,
            });
        }
        if ctx == Context::Modal {
            return self.emit(command);
        }
        if self.g {
            let second = match key {
                Key::Char(c) => find(g_keys, c),
                _ => None,
            };
            return self.emit(second);
        }
        match key {
            Key::Char(c @ '1'..='9') | Key::Char(c @ '0') if c != '0' || self.count.is_some() => {
                let digit = c.to_digit(10).unwrap() as usize;
                self.count = Some((self.count.unwrap_or(0) * 10 + digit).min(MAX_COUNT));
                None
            }
            Key::Char('g') => {
                self.g = true;
                None
            }
            Key::Char(c)
                if ctx == Context::EditorNormal
                    && let Some(op) = op_key(c) =>
            {
                match self.op {
                    None => {
                        self.op = Some((op, self.count.take()));
                        None
                    }
                    Some((pending, before)) if pending == op => {
                        let count = mul(before, self.count);
                        *self = Self::default();
                        Some(Input {
                            command: Command::Operator {
                                op,
                                target: Target::Line,
                            },
                            count,
                        })
                    }
                    Some(_) => self.emit(None),
                }
            }
            _ => self.emit(command),
        }
    }

    /// Pending counts, operator and chord prefix, e.g. `2d3g`.
    pub fn pending(&self) -> String {
        let num = |n: Option<usize>| n.map(|c| c.to_string()).unwrap_or_default();
        let op = self.op.map_or(String::new(), |(op, before)| {
            let key = OPS.iter().find(|&&(_, o)| o == op).unwrap().0;
            format!("{}{key}", num(before))
        });
        format!("{op}{}{}", num(self.count), if self.g { "g" } else { "" })
    }

    /// Emits `command` with the pending count and clears pending state; `None` just clears it.
    /// With an operator pending, a motion becomes its target and anything else cancels.
    fn emit(&mut self, command: Option<Command>) -> Option<Input> {
        let state = std::mem::take(self);
        let Some((op, before)) = state.op else {
            return command.map(|command| Input {
                command,
                count: state.count,
            });
        };
        match command {
            Some(Command::Motion(m)) => Some(Input {
                command: Command::Operator {
                    op,
                    target: Target::Motion(m),
                },
                count: mul(before, state.count),
            }),
            _ => None,
        }
    }
}

fn op_key(c: char) -> Option<Op> {
    OPS.iter().find(|&&(k, _)| k == c).map(|&(_, op)| op)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn feed(p: &mut KeyParser, keys: &str) -> Vec<Input> {
        keys.chars()
            .filter_map(|c| p.handle(key(KeyCode::Char(c)), Context::Diff))
            .collect()
    }

    fn input(command: Command, count: Option<usize>) -> Input {
        Input { command, count }
    }

    #[test]
    fn counts() {
        let mut p = KeyParser::default();
        assert_eq!(
            feed(&mut p, "5j"),
            [input(Command::Motion(Motion::ScreenDown), Some(5))]
        );
        assert_eq!(
            feed(&mut p, "0"),
            [input(Command::Motion(Motion::LineStart), None)]
        );
        assert_eq!(
            feed(&mut p, "10j"),
            [input(Command::Motion(Motion::ScreenDown), Some(10))]
        );
        assert_eq!(
            feed(&mut p, "G"),
            [input(Command::Motion(Motion::FileEnd), None)]
        );
        assert_eq!(
            p.handle(key(KeyCode::Down), Context::Diff),
            Some(input(Command::Motion(Motion::ScreenDown), None))
        );
    }

    #[test]
    fn chords() {
        let mut p = KeyParser::default();
        assert_eq!(
            feed(&mut p, "gg"),
            [input(Command::Motion(Motion::FileStart), None)]
        );
        assert_eq!(feed(&mut p, "3]"), [input(Command::NextHunk, Some(3))]);
        // Unknown second key cancels the chord and the count.
        assert_eq!(
            feed(&mut p, "5gxj"),
            [input(Command::Motion(Motion::ScreenDown), None)]
        );
    }

    #[test]
    fn tables_per_context() {
        let mut p = KeyParser::default();
        let mut feed_in = |keys: &str, ctx| -> Vec<Input> {
            keys.chars()
                .filter_map(|c| p.handle(key(KeyCode::Char(c)), ctx))
                .collect()
        };
        assert_eq!(
            feed_in("gn", Context::Diff),
            [input(Command::NextFile, None)]
        );
        assert_eq!(feed_in("gn", Context::EditorNormal), []);
        for ctx in [Context::Diff, Context::EditorNormal] {
            assert_eq!(
                feed_in("jgj", ctx),
                [
                    input(Command::Motion(Motion::ScreenDown), None),
                    input(Command::Motion(Motion::Down), None)
                ]
            );
        }
    }

    fn feed_in(p: &mut KeyParser, keys: &str, ctx: Context) -> Vec<Input> {
        keys.chars()
            .filter_map(|c| p.handle(key(KeyCode::Char(c)), ctx))
            .collect()
    }

    #[test]
    fn operators_and_counts() {
        use Context::{EditorNormal, EditorVisual};
        let mut p = KeyParser::default();
        let op = |op, target| Command::Operator { op, target };
        let del = |m| op(Op::Delete, Target::Motion(m));
        assert_eq!(
            feed_in(&mut p, "2d3w", EditorNormal),
            [input(del(Motion::WordForward), Some(6))]
        );
        assert_eq!(
            feed_in(&mut p, "dd", EditorNormal),
            [input(op(Op::Delete, Target::Line), None)]
        );
        // `0` after an operator is a motion, but continues a count typed after it.
        assert_eq!(
            feed_in(&mut p, "d0", EditorNormal),
            [input(del(Motion::LineStart), None)]
        );
        assert_eq!(
            feed_in(&mut p, "d10w", EditorNormal),
            [input(del(Motion::WordForward), Some(10))]
        );
        assert_eq!(
            feed_in(&mut p, "dgg", EditorNormal),
            [input(del(Motion::FileStart), None)]
        );
        assert_eq!(
            feed_in(&mut p, "d", EditorVisual),
            [input(op(Op::Delete, Target::Selection), None)]
        );
    }

    #[test]
    fn escape_cancels_operator() {
        let mut p = KeyParser::default();
        feed_in(&mut p, "2d3", Context::EditorNormal);
        assert_eq!(p.pending(), "2d3");
        let esc = p.handle(key(KeyCode::Esc), Context::EditorNormal);
        assert_eq!(esc, Some(input(Command::Escape, None)));
        assert_eq!(p.pending(), "");
        assert_eq!(
            feed_in(&mut p, "w", Context::EditorNormal),
            [input(Command::Motion(Motion::WordForward), None)]
        );
    }

    #[test]
    fn ctrl_h_and_ctrl_left_switch_column() {
        let mut p = KeyParser::default();
        for code in [KeyCode::Char('h'), KeyCode::Left] {
            let event = KeyEvent::new(code, KeyModifiers::CONTROL);
            assert_eq!(
                p.handle(event, Context::Diff),
                Some(input(Command::ColumnLeft, None))
            );
        }
    }

    #[test]
    fn escape_and_quit_mid_chord() {
        let mut p = KeyParser::default();
        feed(&mut p, "5g");
        assert_eq!(p.pending(), "5g");
        assert_eq!(
            p.handle(key(KeyCode::Esc), Context::Diff),
            Some(input(Command::Escape, None))
        );
        assert_eq!(p.pending(), "");

        feed(&mut p, "5g");
        let ctrl_q = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::CONTROL);
        assert_eq!(
            p.handle(ctrl_q, Context::Diff),
            Some(input(Command::Quit, None))
        );
        assert_eq!(p.pending(), "");
    }
}
