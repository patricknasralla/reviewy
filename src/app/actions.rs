//! Actions, vim input → action mapping, and `App::apply`.

use super::App;
use super::state::{Modal, Mode};
use crate::comments::model::Side;
use crate::vim::keys::{Command, Input};
use crate::vim::motion::{Motion, MotionCtx};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    ModalConfirm,
    ModalCancel,
    ExplorerMove(Motion, Option<usize>),
    ExplorerOpen,
    ExplorerClose,
    Move(Motion, Option<usize>),
    Switch(Side),
    Hunk { next: bool, count: usize },
    File { next: bool },
    ToExplorer,
    Visual { line: bool },
    ToNormal,
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
            Mode::Normal | Mode::Visual { .. } | Mode::VisualLine { .. } => match command {
                Command::Motion(m) => {
                    let m = match m {
                        Motion::Down => Motion::ScreenDown,
                        Motion::Up => Motion::ScreenUp,
                        m => m,
                    };
                    Some(Action::Move(m, count))
                }
                Command::ColumnLeft => Some(Action::Switch(Side::Old)),
                Command::ColumnRight => Some(Action::Switch(Side::New)),
                Command::NextHunk | Command::PrevHunk => Some(Action::Hunk {
                    next: command == Command::NextHunk,
                    count: count.unwrap_or(1),
                }),
                Command::NextFile => Some(Action::File { next: true }),
                Command::PrevFile => Some(Action::File { next: false }),
                Command::Visual => Some(Action::Visual { line: false }),
                Command::VisualLine => Some(Action::Visual { line: true }),
                Command::Confirm => Some(Action::Comment),
                Command::DeleteComment if self.mode == Mode::Normal => Some(Action::DeleteComment),
                Command::Escape if self.mode == Mode::Normal => Some(Action::ToExplorer),
                Command::Escape => Some(Action::ToNormal),
                _ => None,
            },
        }
    }

    pub fn apply(&mut self, action: Action) {
        let height = self.view_height();
        match action {
            Action::ModalConfirm => match self.modals.pop() {
                Some(Modal::LargeFile { pending, .. }) => self.start_diff(pending),
                Some(Modal::Quit) => self.should_quit = true,
                Some(Modal::DeleteComment(id)) => {
                    self.comments.remove(id);
                }
                Some(Modal::CloseComment) => self.close_editor(),
                // The editor takes its own keys (`App::handle_key`).
                // Discard: `comments` is still the empty startup set.
                Some(Modal::LoadExisting(_) | Modal::Editor(_)) | None => {}
            },
            Action::ModalCancel => {
                if let Some(Modal::LoadExisting(Ok(set))) = self.modals.pop() {
                    self.keep(set);
                }
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
                    self.mode = Mode::Normal;
                }
            }
            Action::Move(motion, count) => {
                let wrap = self.config.ui.wrap;
                if let Some(view) = self.view_mut() {
                    let ctx = MotionCtx {
                        height,
                        wrap: wrap.then(|| view.text_width()),
                    };
                    view.cursor_motion(motion, count, &ctx);
                }
            }
            Action::Switch(side) => {
                self.mode = Mode::Normal;
                if let Some(view) = self.view_mut() {
                    view.switch(side);
                }
            }
            Action::Hunk { next, count } => {
                self.mode = Mode::Normal;
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
            Action::Visual { line } => self.visual(line),
            Action::ToNormal => self.mode = Mode::Normal,
            Action::Comment => self.comment(),
            Action::DeleteComment => self.delete_comment(),
            Action::Quit => {
                if !matches!(self.modals.last(), Some(Modal::Quit)) {
                    self.modals.push(Modal::Quit);
                }
            }
        }
        self.refresh_layout();
    }

    /// `v`/`V` as in vim: the same kind again leaves visual, the other kind switches to it.
    fn visual(&mut self, line: bool) {
        let anchor = match (self.mode, self.view()) {
            (Mode::Visual { anchor } | Mode::VisualLine { anchor }, _) => anchor,
            (_, Some(view)) => view.cursor.pos,
            (_, None) => return,
        };
        self.mode = match (self.mode, line) {
            (Mode::Visual { .. }, false) | (Mode::VisualLine { .. }, true) => Mode::Normal,
            (_, true) => Mode::VisualLine { anchor },
            (_, false) => Mode::Visual { anchor },
        };
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
