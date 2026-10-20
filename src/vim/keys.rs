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

/// Single keys. `Quit` and `Escape` take effect even mid-chord.
const KEYS: &[(Key, Command)] = &[
    (Key::Char('h'), Command::Motion(Motion::Left)),
    (Key::Char('j'), Command::Motion(Motion::Down)),
    (Key::Char('k'), Command::Motion(Motion::Up)),
    (Key::Char('l'), Command::Motion(Motion::Right)),
    (Key::Code(KeyCode::Left), Command::Motion(Motion::Left)),
    (Key::Code(KeyCode::Down), Command::Motion(Motion::Down)),
    (Key::Code(KeyCode::Up), Command::Motion(Motion::Up)),
    (Key::Code(KeyCode::Right), Command::Motion(Motion::Right)),
    (Key::Char('w'), Command::Motion(Motion::WordForward)),
    (Key::Char('b'), Command::Motion(Motion::WordBackward)),
    (Key::Char('e'), Command::Motion(Motion::WordEnd)),
    (Key::Char('0'), Command::Motion(Motion::LineStart)),
    (Key::Char('$'), Command::Motion(Motion::LineEnd)),
    (Key::Char('G'), Command::Motion(Motion::FileEnd)),
    (Key::Ctrl('d'), Command::Motion(Motion::HalfPageDown)),
    (Key::Ctrl('u'), Command::Motion(Motion::HalfPageUp)),
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

/// Second key of `g` chords.
const G_KEYS: &[(char, Command)] = &[
    ('g', Command::Motion(Motion::FileStart)),
    ('n', Command::NextFile),
    ('p', Command::PrevFile),
];

#[derive(Debug, Default)]
pub struct KeyParser {
    count: Option<usize>,
    g: bool,
}

impl KeyParser {
    pub fn handle(&mut self, event: KeyEvent) -> Option<Input> {
        if event.kind == KeyEventKind::Release {
            return None;
        }
        let key = match event.code {
            KeyCode::Char(c) if event.modifiers.contains(KeyModifiers::CONTROL) => Key::Ctrl(c),
            KeyCode::Char(c) => Key::Char(c),
            code if event.modifiers.contains(KeyModifiers::CONTROL) => Key::CtrlCode(code),
            code => Key::Code(code),
        };
        let command = KEYS.iter().find(|(k, _)| *k == key).map(|&(_, c)| c);

        if let Some(c @ (Command::Quit | Command::Escape)) = command {
            *self = Self::default();
            return Some(Input {
                command: c,
                count: None,
            });
        }
        if self.g {
            let second = match key {
                Key::Char(c) => G_KEYS.iter().find(|(k, _)| *k == c).map(|&(_, c)| c),
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
            .filter_map(|c| p.handle(key(KeyCode::Char(c))))
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
            [input(Command::Motion(Motion::Down), Some(5))]
        );
        assert_eq!(
            feed(&mut p, "0"),
            [input(Command::Motion(Motion::LineStart), None)]
        );
        assert_eq!(
            feed(&mut p, "10j"),
            [input(Command::Motion(Motion::Down), Some(10))]
        );
        assert_eq!(
            feed(&mut p, "G"),
            [input(Command::Motion(Motion::FileEnd), None)]
        );
        assert_eq!(
            p.handle(key(KeyCode::Down)),
            Some(input(Command::Motion(Motion::Down), None))
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
            [input(Command::Motion(Motion::Down), None)]
        );
    }

    #[test]
    fn ctrl_h_and_ctrl_left_switch_column() {
        let mut p = KeyParser::default();
        for code in [KeyCode::Char('h'), KeyCode::Left] {
            let event = KeyEvent::new(code, KeyModifiers::CONTROL);
            assert_eq!(p.handle(event), Some(input(Command::ColumnLeft, None)));
        }
    }

    #[test]
    fn escape_and_quit_mid_chord() {
        let mut p = KeyParser::default();
        feed(&mut p, "5g");
        assert_eq!(p.pending(), "5g");
        assert_eq!(
            p.handle(key(KeyCode::Esc)),
            Some(input(Command::Escape, None))
        );
        assert_eq!(p.pending(), "");

        feed(&mut p, "5g");
        let ctrl_q = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::CONTROL);
        assert_eq!(p.handle(ctrl_q), Some(input(Command::Quit, None)));
        assert_eq!(p.pending(), "");
    }
}
