//! Actions, vim input → action mapping, and `App::apply`.

use super::App;
use super::state::{Modal, Mode};
use crate::comments::model::Side;
use crate::vim::keys::{Command, Input};
use crate::vim::motion::Motion;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    ModalConfirm,
    ModalCancel,
    ExplorerMove(Motion, Option<usize>),
    ExplorerOpen,
    ExplorerClose,
    RowMove(Motion, Option<usize>),
    Switch(Side),
    Hunk { next: bool, count: usize },
    File { next: bool },
    ToExplorer,
    Comment,
    DeleteComment,
    Quit,
}

impl App {
    /// The top modal gets input first; otherwise it depends on the mode.
    pub fn map(&self, input: Input) -> Option<Action> {
        let Input { command, count } = input;
        if command == Command::Quit {
            return Some(Action::Quit);
        }
        if !self.modals.is_empty() {
            return match command {
                Command::Confirm => Some(Action::ModalConfirm),
                Command::Escape => Some(Action::ModalCancel),
                _ => None,
            };
        }
        match self.mode {
            Mode::Explorer => match command {
                Command::Motion(
                    m @ (Motion::Down | Motion::Up | Motion::FileStart | Motion::FileEnd),
                ) => Some(Action::ExplorerMove(m, count)),
                Command::Confirm => Some(Action::ExplorerOpen),
                Command::Escape => Some(Action::ExplorerClose),
                _ => None,
            },
            Mode::Row => match command {
                Command::Motion(Motion::Left) => Some(Action::Switch(Side::Old)),
                Command::Motion(Motion::Right) => Some(Action::Switch(Side::New)),
                Command::Motion(
                    m @ (Motion::Down
                    | Motion::Up
                    | Motion::FileStart
                    | Motion::FileEnd
                    | Motion::HalfPageDown
                    | Motion::HalfPageUp),
                ) => Some(Action::RowMove(m, count)),
                Command::NextHunk | Command::PrevHunk => Some(Action::Hunk {
                    next: command == Command::NextHunk,
                    count: count.unwrap_or(1),
                }),
                Command::NextFile => Some(Action::File { next: true }),
                Command::PrevFile => Some(Action::File { next: false }),
                Command::Confirm => Some(Action::Comment),
                Command::DeleteComment => Some(Action::DeleteComment),
                Command::Escape => Some(Action::ToExplorer),
                _ => None,
            },
        }
    }

    pub fn apply(&mut self, action: Action) {
        let height = self.view_height();
        match action {
            Action::ModalConfirm => match self.modals.pop() {
                Some(Modal::LargeFile { pending, .. }) => self.start_diff(pending),
                Some(Modal::Quit) => {
                    // task 10: save
                    self.should_quit = true;
                }
                None => {}
            },
            Action::ModalCancel => {
                self.modals.pop();
            }
            Action::ExplorerMove(motion, count) => {
                let last = self.files.len() - 1;
                self.explorer = match (motion, count) {
                    (Motion::Down, n) => (self.explorer + n.unwrap_or(1)).min(last),
                    (Motion::Up, n) => self.explorer.saturating_sub(n.unwrap_or(1)),
                    (_, Some(n)) => n.clamp(1, last + 1) - 1,
                    (Motion::FileStart, None) => 0,
                    _ => last,
                };
            }
            Action::ExplorerOpen => self.open(self.explorer),
            Action::ExplorerClose => {
                if self.current.is_some() {
                    self.mode = Mode::Row;
                }
            }
            Action::RowMove(motion, count) => {
                if let Some(view) = self.view_mut() {
                    view.motion(motion, count, height);
                }
            }
            Action::Switch(side) => {
                if let Some(view) = self.view_mut() {
                    view.switch(side);
                }
            }
            Action::Hunk { next, count } => {
                if self.view_mut().is_some_and(|v| !v.hunk(next, count)) {
                    let dir = if next { "next" } else { "previous" };
                    self.status = Some(format!("No {dir} hunk"));
                }
            }
            Action::File { next } => self.next_file(next),
            Action::ToExplorer => {
                self.explorer = self.current.unwrap_or(0);
                self.mode = Mode::Explorer;
            }
            Action::Comment | Action::DeleteComment => {
                self.status = Some("Comments: task 08/09".into());
            }
            Action::Quit => {
                if !matches!(self.modals.last(), Some(Modal::Quit)) {
                    self.modals.push(Modal::Quit);
                }
            }
        }
        self.refresh_layout();
    }

    /// Opens the next/previous viewable file in explorer order; no wraparound.
    fn next_file(&mut self, next: bool) {
        let Some(current) = self.current else { return };
        let viewable = |&i: &usize| self.files[i].viewable();
        let found = if next {
            (current + 1..self.files.len()).find(viewable)
        } else {
            (0..current).rev().find(viewable)
        };
        match found {
            Some(index) => self.open(index),
            None => {
                let dir = if next { "next" } else { "previous" };
                self.status = Some(format!("No {dir} file"));
            }
        }
    }
}
