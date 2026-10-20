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
    (Key::Code(KeyCode::Esc), Command::Escape),
    (Key::Ctrl('q'), Command::Quit),
];

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

#[derive(Debug, Default)]
pub struct KeyParser {
    count: Option<usize>,
    g: bool,
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
            _ => self.emit(command),
        }
    }

    /// Pending count and chord prefix, e.g. `5g`.
    pub fn pending(&self) -> String {
        let count = self.count.map(|c| c.to_string()).unwrap_or_default();
        format!("{count}{}", if self.g { "g" } else { "" })
    }

    /// Emits `command` with the pending count and clears pending state; `None` just clears it.
    fn emit(&mut self, command: Option<Command>) -> Option<Input> {
        let count = std::mem::take(self).count;
        command.map(|command| Input { command, count })
    }
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
